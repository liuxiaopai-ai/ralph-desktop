use crate::storage::models::CliType;
use async_trait::async_trait;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::process::Command;


/// Windows flag to prevent console window from appearing
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Apply CREATE_NO_WINDOW flag to hide console window on Windows
#[cfg(target_os = "windows")]
pub fn hide_console_window(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// No-op on non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub fn hide_console_window(_cmd: &mut Command) {
    // No-op
}

pub mod claude;
pub mod codex;
pub mod opencode;

/// Parsed output line from CLI
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub content: String,
    pub line_type: LineType,
    pub is_assistant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineType {
    Text,
    Json,
    Error,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandOptions {
    pub skip_git_repo_check: bool,
}

/// CLI adapter trait for different CLI implementations
#[allow(dead_code)]
#[async_trait]
pub trait CliAdapter: Send + Sync {
    /// Adapter name
    fn name(&self) -> &str;

    /// CLI type
    fn cli_type(&self) -> CliType;

    /// Check if CLI is installed
    fn is_installed(&self) -> bool;

    /// Get CLI executable path
    fn get_path(&self) -> Option<String>;

    /// Get CLI version
    async fn version(&self) -> Option<String>;

    /// Build command for execution
    fn build_command(&self, prompt: &str, working_dir: &Path, options: CommandOptions) -> Command;

    /// Build readonly command for brainstorm
    fn build_readonly_command(
        &self,
        prompt: &str,
        working_dir: &Path,
        options: CommandOptions,
    ) -> Command;

    /// Detect completion signal in output
    fn detect_completion(&self, output: &str, signal: &str) -> bool;

    /// Parse a single output line
    fn parse_output_line(&self, line: &str) -> ParsedLine;
}

fn push_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.contains(&path) {
        paths.push(path);
    }
}

fn add_path_if_exists(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.exists() {
        push_path(paths, path);
    }
}

fn collect_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(env_path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&env_path));
    }

    #[cfg(target_os = "macos")]
    {
        add_path_if_exists(&mut paths, PathBuf::from("/opt/homebrew/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/opt/homebrew/sbin"));
        add_path_if_exists(&mut paths, PathBuf::from("/usr/local/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/usr/local/sbin"));
        add_path_if_exists(&mut paths, PathBuf::from("/usr/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/usr/sbin"));
        add_path_if_exists(&mut paths, PathBuf::from("/sbin"));
    }

    #[cfg(target_os = "linux")]
    {
        add_path_if_exists(&mut paths, PathBuf::from("/usr/local/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/usr/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/bin"));
        add_path_if_exists(&mut paths, PathBuf::from("/usr/sbin"));
        add_path_if_exists(&mut paths, PathBuf::from("/sbin"));
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = env::var("APPDATA") {
            add_path_if_exists(&mut paths, PathBuf::from(app_data).join("npm"));
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            add_path_if_exists(
                &mut paths,
                PathBuf::from(local_app_data)
                    .join("Programs")
                    .join("nodejs"),
            );
        }
        add_path_if_exists(&mut paths, PathBuf::from(r"C:\Program Files\nodejs"));
    }

    if let Ok(home) = env::var("HOME") {
        let home = PathBuf::from(home);
        add_path_if_exists(&mut paths, home.join(".local/bin"));
        add_path_if_exists(&mut paths, home.join(".local/share/pnpm"));
        add_path_if_exists(&mut paths, home.join("Library/pnpm"));
        add_path_if_exists(&mut paths, home.join(".npm-global/bin"));
        add_path_if_exists(&mut paths, home.join(".volta/bin"));
        add_path_if_exists(&mut paths, home.join(".asdf/shims"));
        add_path_if_exists(&mut paths, home.join(".bun/bin"));

        let nvm = home.join(".nvm/versions/node");
        if let Ok(entries) = fs::read_dir(nvm) {
            for entry in entries.flatten() {
                add_path_if_exists(&mut paths, entry.path().join("bin"));
            }
        }

        let asdf = home.join(".asdf/installs/nodejs");
        if let Ok(entries) = fs::read_dir(asdf) {
            for entry in entries.flatten() {
                add_path_if_exists(&mut paths, entry.path().join("bin"));
            }
        }

        let fnm_aliases = home.join(".fnm/aliases");
        if let Ok(entries) = fs::read_dir(fnm_aliases) {
            for entry in entries.flatten() {
                add_path_if_exists(&mut paths, entry.path().join("bin"));
            }
        }
    }

    // DEBUG LOGGING
    // println!("DEBUG: Collected search paths:");
    // for p in &paths {
    //     println!("  - {:?}", p);
    // }
    paths
}

fn build_path_env() -> Option<OsString> {
    let paths = collect_search_paths();
    env::join_paths(paths).ok()
}

pub fn apply_extended_path(cmd: &mut Command) {
    if let Some(path_env) = build_path_env() {
        cmd.env("PATH", path_env);
    }
}

#[cfg(not(target_os = "windows"))]
fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            escaped.push_str("'\"'\"'");
        } else {
            escaped.push(ch);
        }
    }
    escaped.push('\'');
    escaped
}

#[cfg(not(target_os = "windows"))]
fn shell_join(exe: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(shell_escape(exe));
    for arg in args {
        parts.push(shell_escape(arg));
    }
    parts.join(" ")
}

pub fn command_for_cli(exe: &str, args: &[String], working_dir: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        // Special handling for batch files on Windows
        // Rust's Command doesn't automatically wrap batch files with cmd /c,
        // often resulting in "batch file arguments are invalid" error
        if exe.ends_with(".cmd") || exe.ends_with(".bat") {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C");
            // Pass exe directly - Rust's Command handles spaces automatically
            // Do NOT add extra quotes as Command::arg() already escapes properly
            cmd.arg(exe);
            cmd.args(args);
            cmd.current_dir(working_dir);
            hide_console_window(&mut cmd);
            cmd
        } else {
            let mut cmd = Command::new(exe);
            cmd.current_dir(working_dir);
            cmd.args(args);
            hide_console_window(&mut cmd);
            cmd
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = Command::new(shell);
        cmd.arg("-lc").arg(shell_join(exe, args));
        cmd.current_dir(working_dir);
        cmd
    }
}

#[cfg(target_os = "windows")]
fn load_shell_env() -> HashMap<String, String> {
    env::vars().collect()
}

#[cfg(not(target_os = "windows"))]
fn load_shell_env() -> HashMap<String, String> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = std::process::Command::new(&shell)
        .arg("-lic")
        .arg("env")
        .output();

    let mut vars = HashMap::new();
    let Ok(output) = output else {
        return vars;
    };
    if !output.status.success() {
        return vars;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some((key, value)) = line.split_once('=') {
            vars.insert(key.to_string(), value.to_string());
        }
    }
    vars
}

fn shell_env() -> &'static HashMap<String, String> {
    static SHELL_ENV: OnceLock<HashMap<String, String>> = OnceLock::new();
    SHELL_ENV.get_or_init(load_shell_env)
}

pub fn shell_env_has(key: &str) -> bool {
    shell_env().contains_key(key)
}

pub fn shell_env_value(key: &str) -> Option<String> {
    shell_env().get(key).cloned()
}

fn env_key_is_set(key: &str, shell_envs: &HashMap<String, String>) -> bool {
    shell_envs.get(key).is_some() || env::var(key).is_ok()
}

fn resolve_home(shell_envs: &HashMap<String, String>) -> Option<String> {
    shell_envs
        .get("HOME")
        .cloned()
        .or_else(|| env::var("HOME").ok())
        .or_else(|| env::var("USERPROFILE").ok())
        .or_else(|| {
            let drive = env::var("HOMEDRIVE").ok()?;
            let path = env::var("HOMEPATH").ok()?;
            Some(format!("{}{}", drive, path))
        })
        .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().to_string()))
}

pub fn apply_shell_env(cmd: &mut Command) {
    const ALLOWED_KEYS: &[&str] = &[
        "HOME",
        "USER",
        "SHELL",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
        "CLAUDE_CONFIG_DIR",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_API_URL",
        "OPENAI_API_KEY",
        "OPENAI_BASE_URL",
        "OPENAI_API_BASE",
        "OPENAI_API_URL",
        "CODEX_HOME",
        "NVM_DIR",
        "VOLTA_HOME",
        "ASDF_DIR",
        "ASDF_DATA_DIR",
        "FNM_DIR",
        "PNPM_HOME",
        "BUN_INSTALL",
        "NODE_OPTIONS",
        "OPENCODE_CONFIG",
        "OPENCODE_CONFIG_DIR",
        "OPENCODE_CONFIG_CONTENT",
        "OPENCODE_GIT_BASH_PATH",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
    ];

    let envs = shell_env();
    if let Some(path) = envs.get("PATH") {
        let mut combined: Vec<PathBuf> = env::split_paths(&OsString::from(path)).collect();
        if let Some(extra) = build_path_env() {
            for p in env::split_paths(&extra) {
                if !combined.contains(&p) {
                    combined.push(p);
                }
            }
        }
        if let Ok(joined) = env::join_paths(combined) {
            cmd.env("PATH", joined);
        }
    } else if let Some(extra) = build_path_env() {
        cmd.env("PATH", extra);
    }

    let home = resolve_home(envs);
    if let Some(home_dir) = home.clone() {
        if !env_key_is_set("HOME", envs) {
            cmd.env("HOME", &home_dir);
        }

        if !env_key_is_set("XDG_CONFIG_HOME", envs) {
            cmd.env("XDG_CONFIG_HOME", format!("{}/.config", home_dir));
        }
        if !env_key_is_set("XDG_DATA_HOME", envs) {
            cmd.env("XDG_DATA_HOME", format!("{}/.local/share", home_dir));
        }
        if !env_key_is_set("XDG_STATE_HOME", envs) {
            cmd.env("XDG_STATE_HOME", format!("{}/.local/state", home_dir));
        }
        if !env_key_is_set("CODEX_HOME", envs) {
            cmd.env("CODEX_HOME", format!("{}/.codex", home_dir));
        }
    }

    for key in ALLOWED_KEYS {
        if let Some(value) = envs.get(*key) {
            cmd.env(key, value);
        }
    }

    for (key, value) in envs {
        if key.starts_with("OPENCODE_")
            || key.starts_with("CODEX_")
            || key.starts_with("CLAUDE_")
            || key.starts_with("ANTHROPIC_")
        {
            cmd.env(key, value);
        }
    }
}

pub fn resolve_cli_path(binary: &str) -> Option<String> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(path) = shell_env().get("PATH") {
        if let Ok(found) = which::which_in(binary, Some(path), &cwd) {
            return Some(found.to_string_lossy().to_string());
        }
    }

    let path_env = build_path_env()?;

    // println!(
    //     "DEBUG: Resolving CLI path for '{}' using PATH: {:?}",
    //     binary, path_env
    // );

    match which::which_in(binary, Some(&path_env), cwd) {
        Ok(p) => {
            // println!("DEBUG: Found '{}' at {:?}", binary, p);
            Option::from(p.to_string_lossy().to_string())
        }
        Err(_e) => {
            // println!("DEBUG: Failed to find '{}': {}", binary, e);
            None
        }
    }
}

/// Get all available CLI adapters
pub fn get_adapters() -> Vec<Box<dyn CliAdapter>> {
    vec![
        Box::new(claude::ClaudeCodeAdapter::new()),
        Box::new(codex::CodexAdapter::new()),
        Box::new(opencode::OpenCodeAdapter::new()),
    ]
}

/// Detect all installed CLIs
pub async fn detect_installed_clis() -> Vec<crate::storage::models::CliInfo> {
    let adapters = get_adapters();
    let mut results = Vec::new();

    for adapter in adapters {
        let available = adapter.is_installed();
        let path = adapter.get_path().unwrap_or_default();
        let version = if available {
            adapter.version().await
        } else {
            None
        };

        results.push(crate::storage::models::CliInfo {
            cli_type: adapter.cli_type(),
            name: adapter.name().to_string(),
            version,
            path,
            available,
        });
    }

    results
}

/// Get adapter for a specific CLI type
pub fn get_adapter(cli_type: CliType) -> Box<dyn CliAdapter> {
    match cli_type {
        CliType::Claude => Box::new(claude::ClaudeCodeAdapter::new()),
        CliType::Codex => Box::new(codex::CodexAdapter::new()),
        CliType::OpenCode => Box::new(opencode::OpenCodeAdapter::new()),
    }
}
