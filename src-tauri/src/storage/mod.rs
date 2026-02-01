pub mod models;

use crate::adapters::resolve_cli_path;
use crate::storage::models::*;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Home directory not found")]
    HomeDirNotFound,
    #[error("Project not found: {0}")]
    ProjectNotFound(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;

/// Get the Ralph Desktop data directory (~/.ralph-desktop/)
pub fn get_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or(StorageError::HomeDirNotFound)?;
    let data_dir = home.join(".ralph-desktop");
    Ok(data_dir)
}

/// Ensure the data directory structure exists
pub fn ensure_data_dir() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(data_dir.join("projects"))?;
    Ok(data_dir)
}

/// Detect the first available CLI to use as default
fn detect_default_cli() -> CliType {
    if resolve_cli_path("claude").is_some() {
        return CliType::Claude;
    }
    if resolve_cli_path("codex").is_some() {
        return CliType::Codex;
    }
    if resolve_cli_path("opencode").is_some() {
        return CliType::OpenCode;
    }
    // Fallback to Claude (will show proper error if not installed)
    CliType::Claude
}

/// Load global config
pub fn load_config() -> Result<GlobalConfig> {
    let data_dir = get_data_dir()?;
    let config_path = data_dir.join("config.json");

    if !config_path.exists() {
        let mut config = GlobalConfig::default();
        // Auto-detect installed CLI for first-time users
        config.default_cli = detect_default_cli();
        save_config(&config)?;
        return Ok(config);
    }

    let content = fs::read_to_string(&config_path)?;
    let mut config: GlobalConfig = serde_json::from_str(&content)?;
    let mut updated = false;

    if config.iteration_timeout_ms == 600000 {
        config.iteration_timeout_ms = 0;
        updated = true;
    }
    if config.idle_timeout_ms == 120000 {
        config.idle_timeout_ms = 0;
        updated = true;
    }
    if config.language.trim().is_empty() {
        config.language = "system".to_string();
        updated = true;
    }

    if updated {
        save_config(&config)?;
    }

    Ok(config)
}

/// Save global config
pub fn save_config(config: &GlobalConfig) -> Result<()> {
    let data_dir = ensure_data_dir()?;
    let config_path = data_dir.join("config.json");
    let content = serde_json::to_string_pretty(config)?;
    fs::write(config_path, content)?;
    Ok(())
}

/// Load project index
pub fn load_project_index() -> Result<ProjectIndex> {
    let data_dir = get_data_dir()?;
    let index_path = data_dir.join("projects.json");

    if !index_path.exists() {
        let index = ProjectIndex::default();
        save_project_index(&index)?;
        return Ok(index);
    }

    let content = fs::read_to_string(&index_path)?;
    let index: ProjectIndex = serde_json::from_str(&content)?;
    Ok(index)
}

/// Save project index
pub fn save_project_index(index: &ProjectIndex) -> Result<()> {
    let data_dir = ensure_data_dir()?;
    let index_path = data_dir.join("projects.json");
    let content = serde_json::to_string_pretty(index)?;
    fs::write(index_path, content)?;
    Ok(())
}

/// Get project directory
pub fn get_project_dir(project_id: &uuid::Uuid) -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("projects").join(project_id.to_string()))
}

/// Ensure project directory exists
pub fn ensure_project_dir(project_id: &uuid::Uuid) -> Result<PathBuf> {
    let project_dir = get_project_dir(project_id)?;
    fs::create_dir_all(&project_dir)?;
    fs::create_dir_all(project_dir.join("logs"))?;
    Ok(project_dir)
}

/// Load project state
pub fn load_project_state(project_id: &uuid::Uuid) -> Result<ProjectState> {
    let project_dir = get_project_dir(project_id)?;
    let state_path = project_dir.join("state.json");

    if !state_path.exists() {
        return Err(StorageError::ProjectNotFound(project_id.to_string()));
    }

    let content = fs::read_to_string(&state_path)?;
    let state: ProjectState = serde_json::from_str(&content)?;
    Ok(state)
}

/// Save project state
pub fn save_project_state(state: &ProjectState) -> Result<()> {
    let project_dir = ensure_project_dir(&state.id)?;
    let state_path = project_dir.join("state.json");
    let content = serde_json::to_string_pretty(state)?;
    fs::write(state_path, content)?;
    Ok(())
}

/// Delete project data
pub fn delete_project_data(project_id: &uuid::Uuid) -> Result<()> {
    let project_dir = get_project_dir(project_id)?;
    if project_dir.exists() {
        fs::remove_dir_all(project_dir)?;
    }
    Ok(())
}
