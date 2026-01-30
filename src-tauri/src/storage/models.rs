use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Global configuration stored in ~/.ralph-desktop/config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalConfig {
    pub version: String,
    pub default_cli: CliType,
    pub default_max_iterations: u32,
    pub max_concurrent_projects: u32,
    pub iteration_timeout_ms: u64,
    pub idle_timeout_ms: u64,
    pub theme: Theme,
    pub log_retention_days: u32,
    pub permissions_confirmed: bool,
    pub permissions_confirmed_at: Option<DateTime<Utc>>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            default_cli: CliType::Claude,
            default_max_iterations: 10,
            max_concurrent_projects: 3,
            iteration_timeout_ms: 600000, // 10 minutes
            idle_timeout_ms: 120000,      // 2 minutes
            theme: Theme::System,
            log_retention_days: 7,
            permissions_confirmed: false,
            permissions_confirmed_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CliType {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

/// Project index stored in ~/.ralph-desktop/projects.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub version: String,
    pub projects: Vec<ProjectMeta>,
}

impl Default for ProjectIndex {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            projects: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub status: ProjectStatus,
    pub created_at: DateTime<Utc>,
    pub last_opened_at: DateTime<Utc>,
}

/// Project state stored in ~/.ralph-desktop/projects/{id}/state.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    pub id: Uuid,
    pub name: String,
    pub path: String,
    pub status: ProjectStatus,
    pub brainstorm: Option<BrainstormState>,
    pub task: Option<TaskConfig>,
    pub execution: Option<ExecutionState>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectStatus {
    Brainstorming,
    Ready,
    Queued,
    Running,
    Pausing,
    Paused,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormState {
    pub answers: Vec<BrainstormAnswer>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainstormAnswer {
    pub question_id: String,
    pub question: String,
    pub answer: serde_json::Value, // String or Vec<String>
    pub answered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskConfig {
    pub prompt: String,
    pub design_doc_path: Option<String>,
    pub cli: CliType,
    pub max_iterations: u32,
    pub completion_signal: String,
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            design_doc_path: None,
            cli: CliType::Claude,
            max_iterations: 50,
            completion_signal: "<done>COMPLETE</done>".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionState {
    pub started_at: DateTime<Utc>,
    pub paused_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_iteration: u32,
    pub last_output: String,
    pub last_error: Option<String>,
    pub last_exit_code: Option<i32>,
}

/// CLI info returned by detect_installed_clis
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInfo {
    pub cli_type: CliType,
    pub name: String,
    pub version: Option<String>,
    pub path: String,
    pub available: bool,
}
