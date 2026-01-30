use crate::adapters;
use crate::engine::LoopState;
use crate::storage;
use crate::storage::models::*;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod loop_commands;
pub mod project_commands;
pub mod recovery_commands;

/// Application state shared across commands
#[derive(Clone)]
pub struct AppState {
    pub running_loops: Arc<RwLock<HashMap<Uuid, Arc<LoopEngineHandle>>>>,
}

pub struct LoopEngineHandle {
    pub pause_flag: Arc<std::sync::atomic::AtomicBool>,
    pub stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pub resume_notify: Arc<tokio::sync::Notify>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running_loops: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// Re-export commands
pub use loop_commands::*;
pub use project_commands::*;
pub use recovery_commands::*;
