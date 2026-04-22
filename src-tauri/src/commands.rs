use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::{state::AppState, workspace_payload::WorkspacePayload};

#[tauri::command]
pub fn load_initial_workspace(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::load_initial_workspace(&app, &state).map_err(format_error)
}

#[tauri::command]
pub fn new_document(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkspacePayload, String> {
    crate::workspace::new_document(&app, &state).map_err(format_error)
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
pub fn open_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<WorkspacePayload, String> {
    let path = PathBuf::from(path);
    crate::workspace::open_folder_path(&app, &state, &path).map_err(format_error)
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

fn format_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
