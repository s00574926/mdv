use anyhow::Result;
use notify::RecommendedWatcher;
use std::path::PathBuf;
use tauri::State;

use crate::state::AppState;

#[derive(Clone)]
pub struct WorkspaceSnapshot {
    pub current_path: Option<PathBuf>,
    pub current_directory: Option<PathBuf>,
    pub watching: bool,
    pub recent_paths: Vec<PathBuf>,
}

pub fn clear(state: &State<'_, AppState>) -> Result<()> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
    session.current_path = None;
    session.current_directory = None;
    session.watcher = None;

    Ok(())
}

pub fn set_open_directory(state: &State<'_, AppState>, directory: PathBuf) -> Result<()> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
    session.current_path = None;
    session.current_directory = Some(directory);
    session.watcher = None;

    Ok(())
}

pub fn set_open_document(
    state: &State<'_, AppState>,
    path: PathBuf,
    directory: Option<PathBuf>,
    watcher: RecommendedWatcher,
) -> Result<()> {
    let mut session = state
        .session
        .lock()
        .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
    session.current_path = Some(path);
    session.current_directory = directory;
    session.watcher = Some(watcher);

    Ok(())
}

pub fn current_directory(state: &State<'_, AppState>) -> Result<Option<PathBuf>> {
    let session = state
        .session
        .lock()
        .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

    Ok(session.current_directory.clone())
}

pub fn snapshot(state: &State<'_, AppState>) -> Result<WorkspaceSnapshot> {
    let session = state
        .session
        .lock()
        .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

    Ok(WorkspaceSnapshot {
        current_path: session.current_path.clone(),
        current_directory: session.current_directory.clone(),
        watching: session.current_path.is_some() && session.watcher.is_some(),
        recent_paths: session.recent_paths.clone(),
    })
}
