mod app_menu;
mod commands;
mod explorer;
mod markdown;
mod state;
mod trusted_preview;
mod watcher;
mod workspace;
mod workspace_payload;
mod workspace_session;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = state::AppState::load(app.handle())?;
            app.manage(state);
            app_menu::install(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::new_document,
            commands::open_markdown,
            commands::open_folder,
            commands::select_explorer_file,
            commands::reload_current_document
        ])
        .run(tauri::generate_context!())
        .expect("error while running mdv");
}
