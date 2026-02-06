use crate::storage;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStatus {
    Idle,
    Checking,
    NoUpdate,
    UpdateAvailable,
    WaitingForIdle,
    Downloading,
    Verifying,
    SelfTest,
    ReadyToApply,
    AppliedOnNextLaunch,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateState {
    pub status: UpdateStatus,
    pub current_version: String,
    pub target_version: Option<String>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub failure_count: u32,
    pub last_error: Option<String>,
    pub download_path: Option<String>,
    pub sha256: Option<String>,
    pub pending: bool,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            current_version: "0.0.0".to_string(),
            target_version: None,
            last_checked_at: None,
            failure_count: 0,
            last_error: None,
            download_path: None,
            sha256: None,
            pending: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub version: String,
    pub downloaded_at: DateTime<Utc>,
    pub asset_name: String,
    pub file_path: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize, Clone)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, PartialEq)]
enum UpdatePlan {
    NoUpdate,
    WaitingForIdle { latest_version: String },
    ReadyToDownload { latest_version: String, asset: GithubAsset },
}

fn plan_update(
    current_version: &str,
    release: &GithubRelease,
    idle_ok: bool,
) -> Result<UpdatePlan, String> {
    let tag = release.tag_name.trim_start_matches('v');
    let latest_version = Version::parse(tag)
        .map_err(|err| format!("Invalid version tag: {err}"))?;

    let current = Version::parse(current_version).unwrap_or_else(|_| Version::new(0, 0, 0));
    if latest_version <= current {
        return Ok(UpdatePlan::NoUpdate);
    }

    if !idle_ok {
        return Ok(UpdatePlan::WaitingForIdle {
            latest_version: latest_version.to_string(),
        });
    }

    let asset = select_asset(&release.assets)
        .ok_or_else(|| "No suitable asset found".to_string())?;

    Ok(UpdatePlan::ReadyToDownload {
        latest_version: latest_version.to_string(),
        asset,
    })
}

pub struct AutoUpdateService {
    client: Client,
}

impl AutoUpdateService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn check_and_download(
        &self,
        current_version: &str,
        idle_ok: bool,
    ) -> storage::Result<UpdateState> {
        let mut state = load_update_state().unwrap_or_default();
        state.current_version = current_version.to_string();
        state.status = UpdateStatus::Checking;
        state.last_checked_at = Some(Utc::now());
        state.last_error = None;
        save_update_state(&state)?;

        if let Ok(Some(pending)) = load_pending_update() {
            state.status = UpdateStatus::ReadyToApply;
            state.target_version = Some(pending.version.clone());
            state.download_path = Some(pending.file_path.clone());
            state.sha256 = Some(pending.sha256.clone());
            state.pending = true;
            save_update_state(&state)?;
            return Ok(state);
        }

        let release = match self.fetch_latest_release().await {
            Ok(r) => r,
            Err(err) => {
                state.status = UpdateStatus::Failed;
                state.failure_count += 1;
                state.last_error = Some(err);
                save_update_state(&state)?;
                return Ok(state);
            }
        };

        let plan = match plan_update(current_version, &release, idle_ok) {
            Ok(p) => p,
            Err(err) => {
                state.status = UpdateStatus::Failed;
                state.failure_count += 1;
                state.last_error = Some(err);
                save_update_state(&state)?;
                return Ok(state);
            }
        };

        match plan {
            UpdatePlan::NoUpdate => {
                state.status = UpdateStatus::NoUpdate;
                state.target_version = None;
                state.pending = false;
                save_update_state(&state)?;
                return Ok(state);
            }
            UpdatePlan::WaitingForIdle { latest_version } => {
                state.status = UpdateStatus::WaitingForIdle;
                state.target_version = Some(latest_version);
                save_update_state(&state)?;
                return Ok(state);
            }
            UpdatePlan::ReadyToDownload { latest_version, asset } => {
                state.status = UpdateStatus::UpdateAvailable;
                state.target_version = Some(latest_version.clone());
                save_update_state(&state)?;

                let updates_dir = ensure_updates_dir()?;
                let download_path = updates_dir.join(&asset.name);

                state.status = UpdateStatus::Downloading;
                state.download_path = Some(download_path.to_string_lossy().to_string());
                save_update_state(&state)?;

                if let Err(err) = self.download_asset(&asset.browser_download_url, &download_path).await {
                    state.status = UpdateStatus::Failed;
                    state.failure_count += 1;
                    state.last_error = Some(err);
                    save_update_state(&state)?;
                    return Ok(state);
                }

                state.status = UpdateStatus::Verifying;
                save_update_state(&state)?;

                let sha256 = match compute_sha256(&download_path) {
                    Ok(hash) => hash,
                    Err(err) => {
                        state.status = UpdateStatus::Failed;
                        state.failure_count += 1;
                        state.last_error = Some(err);
                        save_update_state(&state)?;
                        return Ok(state);
                    }
                };

                state.sha256 = Some(sha256.clone());
                state.status = UpdateStatus::SelfTest;
                save_update_state(&state)?;

                if let Err(err) = self.run_self_test().await {
                    state.status = UpdateStatus::Failed;
                    state.failure_count += 1;
                    state.last_error = Some(err);
                    save_update_state(&state)?;
                    return Ok(state);
                }

                state.status = UpdateStatus::ReadyToApply;
                state.pending = true;
                save_update_state(&state)?;

                let pending = PendingUpdate {
                    version: latest_version,
                    downloaded_at: Utc::now(),
                    asset_name: asset.name,
                    file_path: download_path.to_string_lossy().to_string(),
                    sha256,
                };
                save_pending_update(&pending)?;

                return Ok(state);
            }
        }
    }

    async fn fetch_latest_release(&self) -> Result<GithubRelease, String> {
        let url = "https://api.github.com/repos/liuxiaopai-ai/ralph-desktop/releases/latest";
        let resp = self
            .client
            .get(url)
            .header("User-Agent", "ralph-desktop")
            .send()
            .await
            .map_err(|e| format!("Request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("GitHub API error: {}", resp.status()));
        }

        resp.json::<GithubRelease>()
            .await
            .map_err(|e| format!("Parse release failed: {e}"))
    }

    async fn download_asset(&self, url: &str, dest: &Path) -> Result<(), String> {
        let resp = self
            .client
            .get(url)
            .header("User-Agent", "ralph-desktop")
            .send()
            .await
            .map_err(|e| format!("Download failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!("Download error: {}", resp.status()));
        }

        let mut stream = resp.bytes_stream();
        let mut file = fs::File::create(dest).map_err(|e| format!("Create file failed: {e}"))?;
        while let Some(chunk) = stream.next().await {
            let data = chunk.map_err(|e| format!("Stream error: {e}"))?;
            std::io::copy(&mut data.as_ref(), &mut file)
                .map_err(|e| format!("Write failed: {e}"))?;
        }
        Ok(())
    }

    async fn run_self_test(&self) -> Result<(), String> {
        // Minimal self-test placeholder (can be expanded later)
        Ok(())
    }
}

fn select_asset(assets: &[GithubAsset]) -> Option<GithubAsset> {
    #[cfg(target_os = "macos")]
    let ext = ".dmg";
    #[cfg(target_os = "windows")]
    let ext = ".exe";
    #[cfg(target_os = "linux")]
    let ext = ".AppImage";

    assets
        .iter()
        .find(|asset| asset.name.ends_with(ext))
        .cloned()
}

fn compute_sha256(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("Read failed: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(hex::encode(hasher.finalize()))
}

fn ensure_updates_dir() -> storage::Result<PathBuf> {
    let data_dir = storage::ensure_data_dir()?;
    let updates_dir = data_dir.join("updates");
    fs::create_dir_all(&updates_dir)?;
    Ok(updates_dir)
}

fn update_state_path() -> storage::Result<PathBuf> {
    let updates_dir = ensure_updates_dir()?;
    Ok(updates_dir.join("state.json"))
}

fn pending_update_path() -> storage::Result<PathBuf> {
    let updates_dir = ensure_updates_dir()?;
    Ok(updates_dir.join("pending.json"))
}

pub fn load_update_state() -> storage::Result<UpdateState> {
    let path = update_state_path()?;
    if !path.exists() {
        return Ok(UpdateState::default());
    }
    let content = fs::read_to_string(path)?;
    let state = serde_json::from_str(&content)?;
    Ok(state)
}

pub fn save_update_state(state: &UpdateState) -> storage::Result<()> {
    let path = update_state_path()?;
    let content = serde_json::to_string_pretty(state)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn save_pending_update(pending: &PendingUpdate) -> storage::Result<()> {
    let path = pending_update_path()?;
    let content = serde_json::to_string_pretty(pending)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn load_pending_update() -> storage::Result<Option<PendingUpdate>> {
    let path = pending_update_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    let pending = serde_json::from_str(&content)?;
    Ok(Some(pending))
}

pub fn clear_pending_update() -> storage::Result<()> {
    let path = pending_update_path()?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub async fn apply_pending_update() -> storage::Result<()> {
    if let Some(pending) = load_pending_update()? {
        let file_path = PathBuf::from(&pending.file_path);

        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg(&file_path)
                .spawn();
        }

        #[cfg(target_os = "windows")]
        {
            let mut cmd = std::process::Command::new(&file_path);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.spawn();
        }

        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("chmod")
                .arg("+x")
                .arg(&file_path)
                .spawn();
            let _ = std::process::Command::new(&file_path).spawn();
        }

        let mut state = load_update_state().unwrap_or_default();
        state.status = UpdateStatus::AppliedOnNextLaunch;
        state.pending = false;
        state.download_path = Some(pending.file_path);
        state.sha256 = Some(pending.sha256);
        save_update_state(&state)?;
        clear_pending_update()?;
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn with_temp_home<T>(f: impl FnOnce() -> T) -> T {
        let _env_lock = crate::test_support::lock_env();
        let dir = tempdir().expect("tempdir");
        let original_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", dir.path());
        let result = f();
        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
        result
    }

    fn asset_name_for_os() -> &'static str {
        #[cfg(target_os = "macos")]
        { ".dmg" }
        #[cfg(target_os = "windows")]
        { ".exe" }
        #[cfg(target_os = "linux")]
        { ".AppImage" }
    }

    #[test]
    fn plan_update_no_update_when_latest_not_newer() {
        let release = GithubRelease {
            tag_name: "v1.0.0".to_string(),
            assets: vec![],
        };
        let plan = plan_update("1.0.0", &release, true).expect("plan");
        assert_eq!(plan, UpdatePlan::NoUpdate);
    }

    #[test]
    fn plan_update_waiting_for_idle() {
        let release = GithubRelease {
            tag_name: "v1.2.0".to_string(),
            assets: vec![GithubAsset {
                name: format!("ralph-1.2.0{}", asset_name_for_os()),
                browser_download_url: "https://example.invalid/asset".to_string(),
            }],
        };
        let plan = plan_update("1.0.0", &release, false).expect("plan");
        assert_eq!(
            plan,
            UpdatePlan::WaitingForIdle {
                latest_version: "1.2.0".to_string()
            }
        );
    }

    #[test]
    fn plan_update_ready_to_download() {
        let release = GithubRelease {
            tag_name: "v2.0.0".to_string(),
            assets: vec![GithubAsset {
                name: format!("ralph-2.0.0{}", asset_name_for_os()),
                browser_download_url: "https://example.invalid/asset".to_string(),
            }],
        };
        let plan = plan_update("1.0.0", &release, true).expect("plan");
        match plan {
            UpdatePlan::ReadyToDownload { latest_version, asset } => {
                assert_eq!(latest_version, "2.0.0");
                assert!(asset.name.ends_with(asset_name_for_os()));
            }
            other => panic!("unexpected plan: {other:?}"),
        }
    }

    #[test]
    fn plan_update_requires_asset_when_idle_ok() {
        let release = GithubRelease {
            tag_name: "v2.0.0".to_string(),
            assets: vec![],
        };
        let err = plan_update("1.0.0", &release, true).unwrap_err();
        assert!(err.contains("No suitable asset"));
    }

    #[test]
    fn pending_roundtrip() {
        with_temp_home(|| {
            let pending = PendingUpdate {
                version: "1.2.3".to_string(),
                downloaded_at: Utc::now(),
                asset_name: format!("ralph-1.2.3{}", asset_name_for_os()),
                file_path: "/tmp/ralph-update".to_string(),
                sha256: "abc123".to_string(),
            };
            save_pending_update(&pending).expect("save pending");
            let loaded = load_pending_update().expect("load pending").expect("pending");
            assert_eq!(loaded.version, pending.version);
            assert_eq!(loaded.asset_name, pending.asset_name);
            clear_pending_update().expect("clear pending");
            let loaded = load_pending_update().expect("load pending");
            assert!(loaded.is_none());
        });
    }

    #[test]
    fn update_state_roundtrip() {
        with_temp_home(|| {
            let mut state = UpdateState::default();
            state.current_version = "9.9.9".to_string();
            state.status = UpdateStatus::Checking;
            state.last_checked_at = Some(Utc::now());
            save_update_state(&state).expect("save");
            let loaded = load_update_state().expect("load");
            assert_eq!(loaded.current_version, state.current_version);
            assert!(matches!(loaded.status, UpdateStatus::Checking));
        });
    }
}
