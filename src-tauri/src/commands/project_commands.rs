use super::*;
use crate::engine::ai_brainstorm::{run_ai_brainstorm, AiBrainstormResponse, ConversationMessage};
use crate::security;
use tokio::process::Command;
use std::path::PathBuf;

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



/// Check if project directory is a git repository
#[tauri::command]
pub async fn check_project_git_repo(project_id: String) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&state.path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
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
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(state.path)
        .output()
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
