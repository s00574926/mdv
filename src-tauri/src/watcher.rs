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

        if !event.kind.is_create() && !event.kind.is_modify() && !event.kind.is_remove() {
            return;
        }

        let touches_target = event
            .paths
            .iter()
            .any(|candidate| same_path(candidate, &watched_path));
        if !touches_target {
            return;
        }

        let _ = workspace::emit_workspace_update(&app);
    })?;

    watcher
        .watch(&watch_root, RecursiveMode::NonRecursive)
        .with_context(|| format!("Failed to watch {}", watch_root.display()))?;

    Ok(watcher)
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
