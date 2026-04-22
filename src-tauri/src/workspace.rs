use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{app_menu, markdown, state::AppState, watcher};

pub const WORKSPACE_UPDATED_EVENT: &str = "workspace://updated";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacePayload {
    pub document: markdown::RenderedDocument,
    pub current_file_path: Option<String>,
    pub explorer: Option<ExplorerRoot>,
    pub recent_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerRoot {
    pub name: String,
    pub path: String,
    pub children: Vec<ExplorerNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerNode {
    pub name: String,
    pub path: String,
    pub kind: ExplorerNodeKind,
    pub children: Vec<ExplorerNode>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExplorerNodeKind {
    Directory,
    File,
}

pub fn load_initial_workspace(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<WorkspacePayload> {
    let has_existing_workspace = {
        let session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.current_path.is_some() || session.current_directory.is_some()
    };

    if has_existing_workspace {
        return current_workspace(state);
    }

    let sample_path = markdown::resolve_default_path()?;
    open_markdown_with_directory(app, state, &sample_path, None, false)
}

pub fn new_document(app: &AppHandle, state: &State<'_, AppState>) -> Result<WorkspacePayload> {
    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.current_path = None;
        session.current_directory = None;
        session.watcher = None;
    }

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

    let first_markdown = find_first_markdown_file(&canonical_directory)?;
    if let Some(first_markdown) = first_markdown {
        return open_markdown_with_directory(
            app,
            state,
            &first_markdown,
            Some(canonical_directory),
            true,
        );
    }

    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.current_path = None;
        session.current_directory = Some(canonical_directory);
        session.watcher = None;
    }

    current_workspace(state)
}

pub fn select_explorer_file(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
) -> Result<WorkspacePayload> {
    let current_directory = {
        let session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.current_directory.clone()
    };

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
    let snapshot = {
        let session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

        (
            session.current_path.clone(),
            session.current_directory.clone(),
            session.current_path.is_some() && session.watcher.is_some(),
            session.recent_paths.clone(),
        )
    };

    let document = match (&snapshot.0, &snapshot.1) {
        (Some(path), _) => match markdown::render_file(path, snapshot.2) {
            Ok(document) => document,
            Err(error) => markdown::render_error(path, &error, snapshot.2),
        },
        (None, Some(directory)) => markdown::folder_placeholder_document(directory),
        (None, None) => markdown::new_document(),
    };
    let explorer = snapshot
        .1
        .as_ref()
        .map(|directory| build_explorer_root(directory))
        .transpose()?;

    Ok(WorkspacePayload {
        document,
        current_file_path: snapshot.0.map(|path| path.display().to_string()),
        explorer,
        recent_paths: snapshot
            .3
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

    {
        let mut session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.current_path = Some(canonical_path.clone());
        session.current_directory = resolved_directory;
        session.watcher = Some(watcher);
    }

    if remember_recent {
        state.remember_recent_file(&canonical_path)?;
    }
    app_menu::refresh_menu(app)?;

    let explorer = {
        let session = state
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.current_directory.clone()
    };

    Ok(WorkspacePayload {
        document,
        current_file_path: Some(canonical_path.display().to_string()),
        explorer: explorer
            .as_ref()
            .map(|directory| build_explorer_root(directory))
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

fn build_explorer_root(path: &Path) -> Result<ExplorerRoot> {
    Ok(ExplorerRoot {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("Folder")
            .to_owned(),
        path: path.display().to_string(),
        children: build_explorer_nodes(path)?,
    })
}

fn build_explorer_nodes(path: &Path) -> Result<Vec<ExplorerNode>> {
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("Failed to read {}", path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("Failed to read {}", path.display()))?;

    entries.sort_by(|left, right| {
        let left_type = left.file_type().ok();
        let right_type = right.file_type().ok();
        let left_is_dir = left_type.is_some_and(|file_type| file_type.is_dir());
        let right_is_dir = right_type.is_some_and(|file_type| file_type.is_dir());

        match right_is_dir.cmp(&left_is_dir) {
            std::cmp::Ordering::Equal => left
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&right.file_name().to_string_lossy().to_lowercase()),
            ordering => ordering,
        }
    });

    let mut nodes = Vec::new();
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", path.display()))?;

        if file_type.is_dir() {
            nodes.push(ExplorerNode {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.display().to_string(),
                kind: ExplorerNodeKind::Directory,
                children: build_explorer_nodes(&path)?,
            });
            continue;
        }

        if file_type.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            nodes.push(ExplorerNode {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: path.display().to_string(),
                kind: ExplorerNodeKind::File,
                children: Vec::new(),
            });
        }
    }

    Ok(nodes)
}

fn find_first_markdown_file(path: &Path) -> Result<Option<PathBuf>> {
    for node in build_explorer_nodes(path)? {
        if let Some(first_markdown) = first_markdown_from_node(&node) {
            return Ok(Some(first_markdown));
        }
    }

    Ok(None)
}

fn first_markdown_from_node(node: &ExplorerNode) -> Option<PathBuf> {
    match node.kind {
        ExplorerNodeKind::File => Some(PathBuf::from(&node.path)),
        ExplorerNodeKind::Directory => node.children.iter().find_map(first_markdown_from_node),
    }
}
