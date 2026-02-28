use super::*;
use crate::adapters::hide_console_window;
use crate::engine::ai_brainstorm::{
    generate_project_title, run_ai_brainstorm, truncate_to_title, AiBrainstormResponse,
    ConversationMessage,
};
use crate::security;
use std::path::PathBuf;
use tokio::process::Command;

/// List all projects with synced status
#[tauri::command]
pub async fn list_projects() -> Result<Vec<ProjectMeta>, String> {
    let mut index = storage::load_project_index().map_err(|e| e.to_string())?;

    // Sync status from each project's state
    for meta in &mut index.projects {
        if let Ok(state) = storage::load_project_state(&meta.id) {
            meta.status = state.status;
        }
    }

    Ok(index.projects)
}

/// Create a new project
#[tauri::command]
pub async fn create_project(path: String, name: String) -> Result<ProjectState, String> {
    let now = Utc::now();
    let id = Uuid::new_v4();

    // Create project meta
    let meta = ProjectMeta {
        id,
        name: name.clone(),
        path: path.clone(),
        status: ProjectStatus::Brainstorming,
        created_at: now,
        last_opened_at: now,
    };

    // Add to index
    let mut index = storage::load_project_index().map_err(|e| e.to_string())?;
    index.projects.push(meta);
    storage::save_project_index(&index).map_err(|e| e.to_string())?;

    // Create project state
    let state = ProjectState {
        id,
        name,
        path,
        status: ProjectStatus::Brainstorming,
        skip_git_repo_check: false,
        brainstorm: Some(BrainstormState {
            answers: vec![],
            completed_at: None,
        }),
        task: None,
        execution: None,
        created_at: now,
        updated_at: now,
    };

    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    Ok(state)
}

/// Get a project by ID
#[tauri::command]
pub async fn get_project(id: String) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    storage::load_project_state(&uuid).map_err(|e| e.to_string())
}

/// Set whether to skip git repo check for a project
#[tauri::command]
pub async fn set_project_skip_git_repo_check(
    project_id: String,
    skip: bool,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    state.skip_git_repo_check = skip;
    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Update max iterations for a project's task
#[tauri::command]
pub async fn update_task_max_iterations(
    project_id: String,
    max_iterations: u32,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let task = state
        .task
        .as_mut()
        .ok_or("No task configured for this project")?;
    task.max_iterations = max_iterations;
    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Update auto-commit setting for a project's task
#[tauri::command]
pub async fn update_task_auto_commit(
    project_id: String,
    auto_commit: bool,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let task = state
        .task
        .as_mut()
        .ok_or("No task configured for this project")?;
    task.auto_commit = auto_commit;
    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Update auto-init git setting for a project's task
#[tauri::command]
pub async fn update_task_auto_init(
    project_id: String,
    auto_init_git: bool,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let task = state
        .task
        .as_mut()
        .ok_or("No task configured for this project")?;
    task.auto_init_git = auto_init_git;
    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Update prompt content for a project's task
#[tauri::command]
pub async fn update_task_prompt(
    project_id: String,
    prompt: String,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let task = state
        .task
        .as_mut()
        .ok_or("No task configured for this project")?;
    task.prompt = prompt;
    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;
    Ok(state)
}

/// Check if project directory is a git repository
#[tauri::command]
pub async fn check_project_git_repo(project_id: String) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(&state.path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree");
    hide_console_window(&mut cmd);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.trim() == "true")
}

/// Initialize git repository in project directory
#[tauri::command]
pub async fn init_project_git_repo(project_id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let mut cmd = Command::new("git");
    cmd.arg("init").current_dir(state.path);
    hide_console_window(&mut cmd);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git init failed: {}", stderr.trim()))
    }
}

/// Delete a project
#[tauri::command]
pub async fn delete_project(id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;

    // Remove from index
    let mut index = storage::load_project_index().map_err(|e| e.to_string())?;
    index.projects.retain(|p| p.id != uuid);
    storage::save_project_index(&index).map_err(|e| e.to_string())?;

    // Delete project data
    storage::delete_project_data(&uuid).map_err(|e| e.to_string())?;

    Ok(())
}

/// Detect installed CLIs
#[tauri::command]
pub async fn detect_installed_clis() -> Result<Vec<CliInfo>, String> {
    Ok(adapters::detect_installed_clis().await)
}

/// Get global config
#[tauri::command]
pub async fn get_config() -> Result<GlobalConfig, String> {
    storage::load_config().map_err(|e| e.to_string())
}

/// Save global config
#[tauri::command]
pub async fn save_config(config: GlobalConfig) -> Result<(), String> {
    storage::save_config(&config).map_err(|e| e.to_string())
}

/// Confirm permissions
#[tauri::command]
pub async fn confirm_permissions() -> Result<(), String> {
    let mut config = storage::load_config().map_err(|e| e.to_string())?;
    config.permissions_confirmed = true;
    config.permissions_confirmed_at = Some(Utc::now());
    storage::save_config(&config).map_err(|e| e.to_string())
}

/// Update project status
#[tauri::command]
pub async fn update_project_status(
    project_id: String,
    status: ProjectStatus,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

    state.status = status;
    state.updated_at = Utc::now();

    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    Ok(state)
}

/// AI-driven brainstorming - send a message and get AI response
#[tauri::command]
pub async fn ai_brainstorm_chat(
    project_id: String,
    conversation: Vec<ConversationMessage>,
) -> Result<AiBrainstormResponse, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let config = storage::load_config().map_err(|e| e.to_string())?;

    let working_dir = PathBuf::from(&state.path);
    run_ai_brainstorm(
        &working_dir,
        &conversation,
        config.default_cli,
        state.skip_git_repo_check,
    )
    .await
    .map_err(|e| security::sanitize_log(&e))
}

/// Complete AI brainstorming with the generated prompt
#[tauri::command]
pub async fn complete_ai_brainstorm(
    project_id: String,
    generated_prompt: String,
    cli: CliType,
    max_iterations: u32,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

    // Update brainstorm state
    if let Some(ref mut brainstorm) = state.brainstorm {
        brainstorm.completed_at = Some(Utc::now());
    }

    // Set task config with generated prompt
    state.task = Some(TaskConfig {
        prompt: generated_prompt,
        design_doc_path: None,
        cli,
        max_iterations,
        auto_commit: true,
        auto_init_git: true,
        completion_signal: "<done>COMPLETE</done>".to_string(),
    });

    state.status = ProjectStatus::Ready;
    state.updated_at = Utc::now();

    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    Ok(state)
}

/// Generate an AI title for a project from the first user message.
/// On success, persists the title to both ProjectState and ProjectIndex.
/// Falls back to a truncated version of `first_message` if AI call fails.
#[tauri::command]
pub async fn generate_project_title_cmd(
    project_id: String,
    first_message: String,
) -> Result<String, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let config = storage::load_config().map_err(|e| e.to_string())?;

    let working_dir = PathBuf::from(&state.path);

    // Attempt AI title generation; fall back to truncation on any error
    let title = match generate_project_title(
        &working_dir,
        &first_message,
        config.default_cli,
        state.skip_git_repo_check,
    )
    .await
    {
        Ok(t) => t,
        Err(_) => truncate_to_title(&first_message, 15),
    };

    // Persist: update state.name
    state.name = title.clone();
    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    // Persist: update project index entry
    let mut index = storage::load_project_index().map_err(|e| e.to_string())?;
    if let Some(meta) = index.projects.iter_mut().find(|p| p.id == uuid) {
        meta.name = title.clone();
    }
    storage::save_project_index(&index).map_err(|e| e.to_string())?;

    Ok(title)
}

/// Get logs for a project (latest session)
#[tauri::command]
pub async fn get_project_logs(project_id: String) -> Result<Vec<String>, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let manager = crate::engine::logs::LogManager::new(uuid);
    manager.get_latest_session_log()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::{OsStr, OsString};
    use tempfile::tempdir;

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let prev = env::var_os(key);
            env::set_var(key, value);
            Self { key, prev }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.prev.take() {
                env::set_var(self.key, prev);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[tokio::test]
    async fn update_task_prompt_persists_prompt() {
        let _env_lock = crate::test_support::lock_env();
        let home_dir = tempdir().unwrap();
        let _home_guard = EnvVarGuard::set("HOME", home_dir.path());

        let now = Utc::now();
        let id = Uuid::new_v4();
        let project_dir = tempdir().unwrap();

        let initial_prompt = "old prompt".to_string();
        let state = ProjectState {
            id,
            name: "Test".to_string(),
            path: project_dir.path().to_string_lossy().to_string(),
            status: ProjectStatus::Ready,
            skip_git_repo_check: false,
            brainstorm: None,
            task: Some(TaskConfig {
                prompt: initial_prompt,
                design_doc_path: None,
                cli: CliType::Codex,
                max_iterations: 3,
                auto_commit: false,
                auto_init_git: false,
                completion_signal: "<done>COMPLETE</done>".to_string(),
            }),
            execution: None,
            created_at: now,
            updated_at: now,
        };
        storage::save_project_state(&state).unwrap();

        let updated_prompt = "new prompt content".to_string();
        let updated = update_task_prompt(id.to_string(), updated_prompt.clone())
            .await
            .expect("update prompt");
        assert_eq!(
            updated.task.as_ref().map(|t| t.prompt.as_str()),
            Some(updated_prompt.as_str())
        );

        let loaded = storage::load_project_state(&id).expect("load updated state");
        assert_eq!(
            loaded.task.as_ref().map(|t| t.prompt.as_str()),
            Some(updated_prompt.as_str())
        );

        let _ = storage::delete_project_data(&id);
    }

    // --- Unit tests for generate_project_title_cmd fallback ---

    #[tokio::test]
    async fn generate_project_title_cmd_fallback_when_cli_missing() {
        let _env_lock = crate::test_support::lock_env();
        // Point HOME to a temp dir so storage is isolated
        let home_dir = tempdir().unwrap();
        let _home_guard = EnvVarGuard::set("HOME", home_dir.path());

        // Point PATH to an empty dir so no CLI exists → AI call fails → fallback
        let empty_bin = home_dir.path().join("emptybin");
        std::fs::create_dir_all(&empty_bin).unwrap();
        let orig_path = std::env::var_os("PATH").unwrap_or_default();
        let new_path = format!("{};{}", empty_bin.display(), orig_path.to_string_lossy());
        let _path_guard = EnvVarGuard::set("PATH", new_path);

        let now = Utc::now();
        let id = Uuid::new_v4();
        let project_dir = tempdir().unwrap();

        let state = ProjectState {
            id,
            name: "Old Name".to_string(),
            path: project_dir.path().to_string_lossy().to_string(),
            status: ProjectStatus::Brainstorming,
            skip_git_repo_check: true,
            brainstorm: None,
            task: None,
            execution: None,
            created_at: now,
            updated_at: now,
        };
        storage::save_project_state(&state).unwrap();
        // Also create a project index entry so persist-to-index works
        let meta = crate::storage::models::ProjectIndex {
            version: "1.0.0".to_string(),
            projects: vec![crate::storage::models::ProjectMeta {
                id,
                name: "Old Name".to_string(),
                path: project_dir.path().to_string_lossy().to_string(),
                status: ProjectStatus::Brainstorming,
                created_at: now,
                last_opened_at: now,
            }],
        };
        storage::save_project_index(&meta).unwrap();

        let long_msg = "帮我写一个贪吃蛇游戏，加上难度选择".to_string();
        let title = generate_project_title_cmd(id.to_string(), long_msg)
            .await
            .expect("should not error even when AI fails");

        // Fallback title must be ≤ 16 chars (15 + ellipsis) and not the original path
        assert!(title.chars().count() <= 16, "fallback title too long: {}", title);
        assert!(
            !title.contains('/') && !title.contains('\\'),
            "fallback title must not be a path: {}",
            title
        );

        let _ = storage::delete_project_data(&id);
    }

    #[test]
    fn generate_title_only_fires_on_first_message() {
        // This is a logic invariant: titleGenerated flag prevents re-triggering.
        // Represented here as a pure Rust invariant test — the JS flag is tested
        // in e2e/unit tests on the Svelte side.
        // We verify truncate_to_title is idempotent on already-short strings.
        use crate::engine::ai_brainstorm::truncate_to_title;
        let short = "贪吃蛇";
        assert_eq!(truncate_to_title(short, 15), short.to_string());
        assert_eq!(
            truncate_to_title(short, 15),
            truncate_to_title(short, 15),
            "truncate_to_title must be deterministic"
        );
    }
}
