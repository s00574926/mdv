use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    app_menu, explorer, markdown, state::AppState, watcher, workspace_payload::WorkspacePayload,
    workspace_session,
};

pub const WORKSPACE_UPDATED_EVENT: &str = "workspace://updated";

pub fn new_document(app: &AppHandle, state: &State<'_, AppState>) -> Result<WorkspacePayload> {
    workspace_session::clear(state)?;
    app_menu::refresh_menu(app)?;
    current_workspace(state)
}

pub fn open_markdown_path(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
) -> Result<WorkspacePayload> {
    open_markdown_with_directory(app, state, path, None, true)
}

pub fn open_folder_path(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
) -> Result<WorkspacePayload> {
    let canonical_directory = path
        .canonicalize()
        .with_context(|| format!("Failed to resolve {}", path.display()))?;

    if !canonical_directory.is_dir() {
        bail!("{} is not a folder.", canonical_directory.display());
    }

    let first_markdown = explorer::find_first_markdown_file(&canonical_directory)?;
    if let Some(first_markdown) = first_markdown {
        return open_markdown_with_directory(
            app,
            state,
            &first_markdown,
            Some(canonical_directory),
            true,
        );
    }

    workspace_session::set_open_directory(state, canonical_directory)?;
    current_workspace(state)
}

pub fn select_explorer_file(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
) -> Result<WorkspacePayload> {
    let current_directory = workspace_session::current_directory(state)?;
    let Some(current_directory) = current_directory else {
        bail!("Open a folder before selecting files from the explorer.");
    };

    open_markdown_with_directory(app, state, path, Some(current_directory), true)
}

pub fn open_recent_index(
    app: &AppHandle,
    state: &State<'_, AppState>,
    index: usize,
) -> Result<WorkspacePayload> {
    let recent_paths = state.recent_paths()?;
    let Some(path) = recent_paths.get(index) else {
        bail!("That recent file entry no longer exists.");
    };

    open_markdown_path(app, state, path)
}

pub fn reload_current_document(
    _app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<WorkspacePayload> {
    current_workspace(state)
}

pub fn current_workspace(state: &State<'_, AppState>) -> Result<WorkspacePayload> {
    let snapshot = workspace_session::snapshot(state)?;

    let document = match (&snapshot.current_path, &snapshot.current_directory) {
        (Some(path), _) => match markdown::render_file(path, snapshot.watching) {
            Ok(document) => document,
            Err(error) => markdown::render_error(path, &error, snapshot.watching),
        },
        (None, Some(directory)) => markdown::folder_placeholder_document(directory),
        (None, None) => markdown::new_document(),
    };
    let explorer = snapshot
        .current_directory
        .as_ref()
        .map(|directory| explorer::build_root(directory))
        .transpose()?;

    Ok(WorkspacePayload {
        document,
        current_file_path: snapshot.current_path.map(|path| path.display().to_string()),
        explorer,
        recent_paths: snapshot
            .recent_paths
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
    })
}

pub fn emit_workspace_update(app: &AppHandle) -> Result<()> {
    let payload = current_workspace(&app.state::<AppState>())?;
    app.emit(WORKSPACE_UPDATED_EVENT, payload)?;
    Ok(())
}

fn open_markdown_with_directory(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
    directory: Option<PathBuf>,
    remember_recent: bool,
) -> Result<WorkspacePayload> {
    ensure_markdown_file(path)?;

    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("Failed to resolve {}", path.display()))?;
    let resolved_directory = match directory {
        Some(directory) => {
            let directory = directory
                .canonicalize()
                .with_context(|| format!("Failed to resolve {}", directory.display()))?;
            if !canonical_path.starts_with(&directory) {
                bail!(
                    "{} is not inside {}.",
                    canonical_path.display(),
                    directory.display()
                );
            }
            Some(directory)
        }
        None => None,
    };

    let document = markdown::render_file(&canonical_path, true)?;
    let watcher = watcher::watch_file(app.clone(), canonical_path.clone())?;

    workspace_session::set_open_document(
        state,
        canonical_path.clone(),
        resolved_directory,
        watcher,
    )?;

    if remember_recent {
        state.remember_recent_file(&canonical_path)?;
    }
    app_menu::refresh_menu(app)?;

    let explorer_directory = workspace_session::current_directory(state)?;

    Ok(WorkspacePayload {
        document,
        current_file_path: Some(canonical_path.display().to_string()),
        explorer: explorer_directory
            .as_ref()
            .map(|directory| explorer::build_root(directory))
            .transpose()?,
        recent_paths: state
            .recent_paths()?
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
    })
}

fn ensure_markdown_file(path: &Path) -> Result<()> {
    let extension = path.extension().and_then(|value| value.to_str());
    if extension.is_some_and(|value| value.eq_ignore_ascii_case("md")) {
        return Ok(());
    }

    bail!("Only .md files can be opened.");
}
