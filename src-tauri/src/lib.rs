mod adapters;
mod commands;
mod engine;
mod security;
mod storage;

use commands::AppState;

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
            commands::init_project_git_repo,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
