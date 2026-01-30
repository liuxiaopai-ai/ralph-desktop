use super::*;
use crate::engine::ai_brainstorm::{run_ai_brainstorm, AiBrainstormResponse, ConversationMessage};
use crate::engine::brainstorm::generate_design_doc;
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

/// Get brainstorm questions
#[tauri::command]
pub async fn get_brainstorm_questions() -> Result<Vec<QuestionTemplate>, String> {
    Ok(get_question_flow())
}

/// Save brainstorm answer and update project state
#[tauri::command]
pub async fn save_brainstorm_answer(
    project_id: String,
    question_id: String,
    question: String,
    answer: serde_json::Value,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

    let brainstorm_answer = BrainstormAnswer {
        question_id,
        question,
        answer,
        answered_at: Utc::now(),
    };

    if let Some(ref mut brainstorm) = state.brainstorm {
        // Remove existing answer for this question if any
        brainstorm.answers.retain(|a| a.question_id != brainstorm_answer.question_id);
        brainstorm.answers.push(brainstorm_answer);
    }

    state.updated_at = Utc::now();
    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    Ok(state)
}

/// Complete brainstorm and generate prompt
#[tauri::command]
pub async fn complete_brainstorm(
    project_id: String,
    cli: CliType,
    max_iterations: u32,
) -> Result<ProjectState, String> {
    let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
    let mut state = storage::load_project_state(&uuid).map_err(|e| e.to_string())?;

    // Convert answers to HashMap
    let mut answers_map: HashMap<String, serde_json::Value> = HashMap::new();
    if let Some(ref brainstorm) = state.brainstorm {
        for answer in &brainstorm.answers {
            answers_map.insert(answer.question_id.clone(), answer.answer.clone());
        }
    }

    // Generate prompt
    let prompt = generate_prompt(&answers_map);

    // Generate design document
    let project_path = PathBuf::from(&state.path);
    let design_doc_path = generate_design_doc(
        &state.name,
        &project_path,
        &answers_map,
        &prompt,
    ).ok();

    // Update state
    if let Some(ref mut brainstorm) = state.brainstorm {
        brainstorm.completed_at = Some(Utc::now());
    }

    state.task = Some(TaskConfig {
        prompt,
        design_doc_path,
        cli,
        max_iterations,
        completion_signal: "<done>COMPLETE</done>".to_string(),
    });

    state.status = ProjectStatus::Ready;
    state.updated_at = Utc::now();

    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    Ok(state)
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

    let working_dir = PathBuf::from(&state.path);
    run_ai_brainstorm(&working_dir, &conversation).await
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
        completion_signal: "<done>COMPLETE</done>".to_string(),
    });

    state.status = ProjectStatus::Ready;
    state.updated_at = Utc::now();

    storage::save_project_state(&state).map_err(|e| e.to_string())?;

    Ok(state)
}
