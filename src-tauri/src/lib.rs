mod app_menu;
mod commands;
mod explorer;
mod markdown;
mod powerpoint_clipboard;
mod state;
#[cfg(test)]
mod test_support;
mod trusted_preview;
mod watcher;
mod workspace;
mod workspace_payload;
mod workspace_session;

use anyhow::Context;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }

            if matches!(event, tauri::WindowEvent::CloseRequested { .. })
                && let Err(error) = window
                    .state::<state::AppState>()
                    .persist_window_state(window)
            {
                eprintln!("failed to persist window state: {error:#}");
            }
        })
        .setup(|app| {
            let state = state::AppState::load(app.handle())?;
            app.manage(state);
            let main_window = app
                .handle()
                .get_webview_window("main")
                .context("failed to resolve the main window")?;
            app.state::<state::AppState>()
                .restore_window_state(&main_window)?;
            app_menu::install(app)?;
            main_window.show()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::new_document,
            commands::current_workspace,
            commands::select_document,
            commands::close_document,
            commands::open_markdown,
            commands::open_markdown_dialog,
            commands::open_folder,
            commands::open_dropped_path,
            commands::open_folder_dialog,
            commands::open_recent_index,
            commands::select_explorer_file,
            commands::reload_current_document,
            commands::save_active_document,
            commands::save_active_document_as,
            commands::save_active_document_to_path,
            commands::update_document_content,
            commands::copy_mermaid_diagram_as_powerpoint,
            commands::persist_window_state,
            commands::exit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running mdv");
}
