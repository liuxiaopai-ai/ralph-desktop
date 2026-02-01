use super::*;
use crate::engine::{LoopEngine, LoopEvent, CODEX_GIT_REPO_CHECK_REQUIRED};
use std::path::PathBuf;
use std::time::Duration;
use tauri::Emitter;
use tokio::process::Command;

/// Start Ralph Loop for a project
#[tauri::command]
pub async fn start_loop(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut project_state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

    let (
        task_cli,
        mut task_prompt,
        task_max_iterations,
        task_auto_commit,
        task_completion_signal,
        task_auto_init_git,
    ) = {
        if let Some(session_id) = project_state.active_session_id {
            let session = project_state
                .sessions
                .iter_mut()
                .find(|s| s.id == session_id)
                .ok_or("Active session not found")?;
            let task = session
                .task
                .as_mut()
                .ok_or("No task configured for this session")?;
            ensure_autodecide_prompt(task);
            (
                task.cli,
                task.prompt.clone(),
                task.max_iterations,
                task.auto_commit,
                task.completion_signal.clone(),
                task.auto_init_git,
            )
        } else {
            let task = project_state
                .task
                .as_mut()
                .ok_or("No task configured for this project")?;
            ensure_autodecide_prompt(task);
            (
                task.cli,
                task.prompt.clone(),
                task.max_iterations,
                task.auto_commit,
                task.completion_signal.clone(),
                task.auto_init_git,
            )
        }
    };

    let config = storage::load_config().map_err(|e| e.to_string())?;
    let project_path = PathBuf::from(&project_state.path);

    // prompt updated in block above

    let mut is_repo = is_git_repo(&project_path).await?;
    if task_auto_init_git && !is_repo {
        init_git_repo(&project_path).await?;
        is_repo = true;
        if project_state.skip_git_repo_check {
            project_state.skip_git_repo_check = false;
        }
    } else if !task_auto_init_git && !is_repo && task_cli == CliType::Codex {
        project_state.skip_git_repo_check = true;
    }

    if task_cli == CliType::Codex && !project_state.skip_git_repo_check {
        if !is_repo {
            return Err(CODEX_GIT_REPO_CHECK_REQUIRED.to_string());
        }
    }

    let iteration_timeout = if config.iteration_timeout_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(config.iteration_timeout_ms))
    };
    let idle_timeout = if config.idle_timeout_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(config.idle_timeout_ms))
    };

    // Create loop engine
    let engine = LoopEngine::new(
        project_id.clone(),
        project_path,
        task_cli,
        task_prompt,
        task_max_iterations,
        task_auto_commit,
        task_completion_signal,
        iteration_timeout,
        idle_timeout,
        project_state.skip_git_repo_check,
        app_handle.clone(),
    );

    // Store engine handle
    let handle = Arc::new(LoopEngineHandle {
        pause_flag: engine.get_pause_flag(),
        stop_flag: engine.get_stop_flag(),
        resume_notify: engine.get_resume_notify(),
    });

    {
        let mut loops = state.running_loops.write().await;
        loops.insert(uuid, handle);
    }

    // Update project status
    // Update project status
    let new_exec = ExecutionState {
        started_at: Utc::now(),
        paused_at: None,
        completed_at: None,
        current_iteration: 0,
        last_output: String::new(),
        last_error: None,
        last_exit_code: None,
        elapsed_ms: None,
        summary: None,
    };

    if let Some(session_id) = project_state.active_session_id {
        let session = project_state
            .sessions
            .iter_mut()
            .find(|s| s.id == session_id)
            .ok_or("Active session not found")?;
        session.status = ProjectStatus::Running;
        session.execution = Some(new_exec);
        project_state.status = ProjectStatus::Running;
    } else {
        project_state.status = ProjectStatus::Running;
        project_state.execution = Some(new_exec);
    }
    project_state.updated_at = Utc::now();
    storage::save_project_state(&project_state).map_err(|e| e.to_string())?;

    // Spawn loop in background
    let state_clone = state.inner().clone();
    tokio::spawn(async move {
        let result = engine.start().await;

        // Update project state based on result
        // Update project state based on result
        if let Ok(mut project_state) = storage::load_project_state(&uuid) {
            // Define a closure-like logic to update execution state
            // But due to borrow checker, we just use a macro or inline logic
            let (status, iteration) = match result {
                Ok(LoopState::Completed { iteration }) => (ProjectStatus::Done, iteration),
                Ok(LoopState::MaxIterationsReached { iteration }) => {
                    (ProjectStatus::Partial, iteration)
                }
                Ok(LoopState::Failed { iteration }) => (ProjectStatus::Failed, iteration),
                Ok(LoopState::Idle) => (ProjectStatus::Cancelled, 0),
                _ => (ProjectStatus::Cancelled, 0),
            };

            // Helper to update execution state fields
            let update_exec = |exec: &mut ExecutionState| {
                let now = Utc::now();
                if status == ProjectStatus::Done || status == ProjectStatus::Partial {
                    exec.completed_at = Some(now);
                }
                exec.current_iteration = iteration;
                exec.elapsed_ms = Some((now - exec.started_at).num_milliseconds().max(0) as u64);
            };

            if let Some(session_id) = project_state.active_session_id {
                if let Some(session) = project_state
                    .sessions
                    .iter_mut()
                    .find(|s| s.id == session_id)
                {
                    session.status = status;
                    project_state.status = status;
                    if let Some(ref mut exec) = session.execution {
                        update_exec(exec);
                    }
                }
            } else {
                project_state.status = status;
                if let Some(ref mut exec) = project_state.execution {
                    update_exec(exec);
                }
            }
            project_state.updated_at = Utc::now();
            let _ = storage::save_project_state(&project_state);
        }

        // Remove from running loops
        let mut loops = state_clone.running_loops.write().await;
        loops.remove(&uuid);
    });

    Ok(())
}

const AUTO_DECIDE_MARKER: &str = "[Ralph Auto-Decision Policy]";

fn ensure_autodecide_prompt(task: &mut TaskConfig) -> bool {
    if task.prompt.contains(AUTO_DECIDE_MARKER) {
        return false;
    }

    let policy = [
        AUTO_DECIDE_MARKER,
        "You MUST NOT ask the user any questions during execution.",
        "Assume the user is away and cannot respond.",
        "If multiple valid choices exist, prefer the more maintainable, clear, engineering-oriented option.",
        "If required information is missing, make reasonable assumptions and proceed without blocking.",
        "Never pause for clarification; log assumptions in the output when necessary.",
    ]
    .join("\n");

    task.prompt = format!("{policy}\n\n{}", task.prompt.trim());
    true
}

async fn init_git_repo(project_path: &PathBuf) -> Result<(), String> {
    let output = Command::new("git")
        .arg("init")
        .current_dir(project_path)
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

async fn is_git_repo(project_path: &PathBuf) -> Result<bool, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
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

/// Pause Ralph Loop
#[tauri::command]
pub async fn pause_loop(state: State<'_, AppState>, project_id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let loops = state.running_loops.read().await;
    if let Some(handle) = loops.get(&uuid) {
        handle
            .pause_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // Update project status
        let mut project_state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

        if let Some(session_id) = project_state.active_session_id {
            if let Some(session) = project_state
                .sessions
                .iter_mut()
                .find(|s| s.id == session_id)
            {
                session.status = ProjectStatus::Pausing;
                project_state.status = ProjectStatus::Pausing;
            }
        } else {
            project_state.status = ProjectStatus::Pausing;
        }

        project_state.updated_at = Utc::now();
        storage::save_project_state(&project_state).map_err(|e| e.to_string())?;

        Ok(())
    } else {
        Err("Loop not running for this project".to_string())
    }
}

/// Resume Ralph Loop
#[tauri::command]
pub async fn resume_loop(state: State<'_, AppState>, project_id: String) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let loops = state.running_loops.read().await;
    if let Some(handle) = loops.get(&uuid) {
        handle.resume_notify.notify_one();

        // Update project status
        // Update project status
        let mut project_state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

        if let Some(session_id) = project_state.active_session_id {
            if let Some(session) = project_state
                .sessions
                .iter_mut()
                .find(|s| s.id == session_id)
            {
                session.status = ProjectStatus::Running;
                project_state.status = ProjectStatus::Running;
                if let Some(ref mut exec) = session.execution {
                    exec.paused_at = None;
                }
            }
        } else {
            project_state.status = ProjectStatus::Running;
            if let Some(ref mut exec) = project_state.execution {
                exec.paused_at = None;
            }
        }
        project_state.updated_at = Utc::now();
        storage::save_project_state(&project_state).map_err(|e| e.to_string())?;

        Ok(())
    } else {
        Err("Loop not running for this project".to_string())
    }
}

/// Stop Ralph Loop
#[tauri::command]
pub async fn stop_loop(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;

    let mut found = false;
    {
        let loops = state.running_loops.read().await;
        if let Some(handle) = loops.get(&uuid) {
            handle
                .stop_flag
                .store(true, std::sync::atomic::Ordering::SeqCst);
            handle.resume_notify.notify_one(); // In case it's paused
            found = true;
        }
    }

    if let Ok(mut project_state) = storage::load_project_state(&uuid) {
        let now = Utc::now();
        if let Some(session_id) = project_state.active_session_id {
            if let Some(session) = project_state
                .sessions
                .iter_mut()
                .find(|s| s.id == session_id)
            {
                session.status = ProjectStatus::Cancelled;
                project_state.status = ProjectStatus::Cancelled;
                if let Some(ref mut exec) = session.execution {
                    exec.completed_at = Some(now);
                }
            }
        } else {
            project_state.status = ProjectStatus::Cancelled;
            if let Some(ref mut exec) = project_state.execution {
                exec.completed_at = Some(now);
            }
        }
        project_state.updated_at = Utc::now();
        let _ = storage::save_project_state(&project_state);
    }

    let _ = app_handle.emit(
        "loop-event",
        LoopEvent::Stopped {
            project_id: project_id.clone(),
        },
    );

    if found {
        Ok(())
    } else {
        Err("Loop not running for this project".to_string())
    }
}

/// Get loop status for a project
#[tauri::command]
pub async fn get_loop_status(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<bool, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let loops = state.running_loops.read().await;
    Ok(loops.contains_key(&uuid))
}
