use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

use crate::workspace;

pub fn watch_file(app: AppHandle, path: PathBuf) -> Result<RecommendedWatcher> {
    let watch_root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let watched_path = path.clone();

    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };

        if !should_refresh_current_document(&event, &watched_path) {
            return;
        }

        let _ = workspace::emit_workspace_update(&app);
    })?;

    watcher
        .watch(&watch_root, RecursiveMode::NonRecursive)
        .with_context(|| format!("Failed to watch {}", watch_root.display()))?;

    Ok(watcher)
}

pub fn watch_workspace_directory(app: AppHandle, path: PathBuf) -> Result<RecommendedWatcher> {
    let watched_root = path.clone();

    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };

        if !should_refresh_workspace_explorer(&event, &watched_root) {
            return;
        }

        let _ = workspace::emit_workspace_update(&app);
    })?;

    watcher
        .watch(&path, RecursiveMode::Recursive)
        .with_context(|| format!("Failed to watch {}", path.display()))?;

    Ok(watcher)
}

fn should_refresh_current_document(event: &Event, watched_path: &Path) -> bool {
    if !event.kind.is_create() && !event.kind.is_modify() && !event.kind.is_remove() {
        return false;
    }

    event
        .paths
        .iter()
        .any(|candidate| same_path(candidate, watched_path))
}

fn should_refresh_workspace_explorer(event: &Event, watched_root: &Path) -> bool {
    let affects_workspace = event.paths.iter().any(|candidate| {
        path_is_within_root(candidate, watched_root) && path_may_affect_explorer(candidate)
    });

    if !affects_workspace {
        return false;
    }

    if event.kind.is_create() || event.kind.is_remove() {
        return true;
    }

    event.kind.is_modify()
        && (event.paths.len() > 1
            || event
                .paths
                .iter()
                .any(|candidate| candidate.extension().is_none()))
}

fn path_is_within_root(candidate: &Path, watched_root: &Path) -> bool {
    candidate.starts_with(watched_root) || same_path(candidate, watched_root)
}

fn path_may_affect_explorer(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        || path.extension().is_none()
}

fn same_path(candidate: &Path, target: &Path) -> bool {
    if candidate == target {
        return true;
    }

    match (candidate.canonicalize(), target.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
