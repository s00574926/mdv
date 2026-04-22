mod app_menu;
mod commands;
mod markdown;
mod state;
mod watcher;
mod workspace;

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
            commands::load_initial_workspace,
            commands::new_document,
            commands::open_markdown,
            commands::open_folder,
            commands::select_explorer_file,
            commands::reload_current_document
        ])
        .run(tauri::generate_context!())
        .expect("error while running mdv");
}
