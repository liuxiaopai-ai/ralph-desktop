mod adapters;
mod auto_update;
mod commands;
mod engine;
mod security;
mod storage;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ensure data directory exists
    let _ = storage::ensure_data_dir();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            // Project commands
            commands::list_projects,
            commands::create_project,
            commands::get_project,
            commands::set_project_skip_git_repo_check,
            commands::update_task_max_iterations,
            commands::update_task_auto_commit,
            commands::update_task_auto_init,
            commands::update_task_prompt,
            commands::init_project_git_repo,
            commands::check_project_git_repo,
            commands::delete_project,
            commands::detect_installed_clis,
            commands::get_config,
            commands::save_config,
            commands::confirm_permissions,
            commands::update_project_status,
            commands::ai_brainstorm_chat,
            commands::complete_ai_brainstorm,
            // Loop commands
            commands::start_loop,
            commands::pause_loop,
            commands::resume_loop,
            commands::stop_loop,
            commands::get_loop_status,
            // Recovery commands
            commands::check_interrupted_tasks,
            commands::cancel_interrupted_task,
            commands::cleanup_logs,
            commands::get_project_logs,
            // Update commands
            commands::get_update_state,
            commands::check_for_updates,
            commands::load_update_state_cmd,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = auto_update::apply_pending_update().await;
                let mut loaded = auto_update::load_update_state().unwrap_or_default();
                loaded.current_version = env!("CARGO_PKG_VERSION").to_string();
                let state = app_handle.state::<AppState>();
                let mut update_state = state.update_state.write().await;
                *update_state = loaded;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
