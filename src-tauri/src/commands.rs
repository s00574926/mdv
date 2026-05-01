use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::{
    powerpoint_clipboard::MermaidClipboardDiagram, state::AppState,
    workspace_payload::WorkspacePayload,
};

#[tauri::command]
pub fn new_document(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::new_document(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn current_workspace(state: State<'_, AppState>) -> Result<WorkspacePayload, String> {
    crate::workspace::current_workspace(&state).map_err(format_error)
}

#[tauri::command]
pub fn select_document(
    app: AppHandle,
    state: State<'_, AppState>,
    index: usize,
) -> Result<WorkspacePayload, String> {
    crate::workspace::select_document(&app, &state, index).map_err(format_error)
}

#[tauri::command]
pub fn close_document(
    app: AppHandle,
    state: State<'_, AppState>,
    index: usize,
) -> Result<WorkspacePayload, String> {
    crate::workspace::close_document(&app, &state, index).map_err(format_error)
}

#[tauri::command]
pub fn open_markdown(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspacePayload, String> {
    let path = PathBuf::from(path);
    crate::workspace::open_markdown_path(&app, &state, &path).map_err(format_error)
}

#[tauri::command]
pub fn open_markdown_dialog(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::open_markdown_dialog(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspacePayload, String> {
    let path = PathBuf::from(path);
    crate::workspace::open_folder_path(&app, &state, &path).map_err(format_error)
}

#[tauri::command]
pub fn open_dropped_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspacePayload, String> {
    let path = PathBuf::from(path);
    crate::workspace::open_dropped_path(&app, &state, &path).map_err(format_error)
}

#[tauri::command]
pub fn open_folder_dialog(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::open_folder_dialog(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn open_recent_index(
    app: AppHandle,
    state: State<'_, AppState>,
    index: usize,
) -> Result<WorkspacePayload, String> {
    crate::workspace::open_recent_index(&app, &state, index).map_err(format_error)
}

#[tauri::command]
pub fn select_explorer_file(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspacePayload, String> {
    let path = PathBuf::from(path);
    crate::workspace::select_explorer_file(&app, &state, &path).map_err(format_error)
}

#[tauri::command]
pub fn reload_current_document(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::reload_current_document(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn save_active_document(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::save_active_document(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn save_active_document_as(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::save_active_document_as(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn save_active_document_to_path(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspacePayload, String> {
    let path = PathBuf::from(path);
    crate::workspace::save_active_document_to_path(&app, &state, &path).map_err(format_error)
}

#[tauri::command]
pub fn update_document_content(
    state: State<'_, AppState>,
    index: usize,
    markdown: String,
) -> Result<WorkspacePayload, String> {
    crate::workspace::update_document_content(&state, index, &markdown).map_err(format_error)
}

#[tauri::command]
pub fn copy_mermaid_diagram_as_powerpoint(diagram: MermaidClipboardDiagram) -> Result<(), String> {
    crate::powerpoint_clipboard::copy_mermaid_diagram_as_powerpoint(&diagram).map_err(format_error)
}

#[tauri::command]
pub fn persist_window_state(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| String::from("The main window is unavailable."))?;
    state.persist_window_state(&window).map_err(format_error)
}

#[tauri::command]
pub fn exit_app(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        state.persist_window_state(&window).map_err(format_error)?;
    }
    app.exit(0);
    Ok(())
}

fn format_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
