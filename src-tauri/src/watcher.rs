use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher, recommended_watcher};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager};

use crate::{state::AppState, workspace};

const REFRESH_COALESCE_DELAY: Duration = Duration::from_millis(100);

pub fn watch_file(app: AppHandle, path: PathBuf) -> Result<RecommendedWatcher> {
    let watch_root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let watched_path = path.clone();
    let refresh_pending = Arc::new(AtomicBool::new(false));

    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };

        if !should_refresh_current_document(&event, &watched_path) {
            return;
        }

        if !begin_refresh_window(refresh_pending.as_ref()) {
            return;
        }

        let app = app.clone();
        let refresh_pending = Arc::clone(&refresh_pending);
        thread::spawn(move || {
            thread::sleep(REFRESH_COALESCE_DELAY);
            let _ = app.state::<AppState>().invalidate_rendered_document();
            let _ = workspace::emit_workspace_update(&app);
            end_refresh_window(refresh_pending.as_ref());
        });
    })?;

    watcher
        .watch(&watch_root, RecursiveMode::NonRecursive)
        .with_context(|| format!("Failed to watch {}", watch_root.display()))?;

    Ok(watcher)
}

pub fn watch_workspace_directory(app: AppHandle, path: PathBuf) -> Result<RecommendedWatcher> {
    let watched_root = path.clone();
    let refresh_pending = Arc::new(AtomicBool::new(false));

    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        let Ok(event) = result else {
            return;
        };

        if !should_refresh_workspace_explorer(&event, &watched_root) {
            return;
        }

        if !begin_refresh_window(refresh_pending.as_ref()) {
            return;
        }

        let app = app.clone();
        let watched_root = watched_root.clone();
        let refresh_pending = Arc::clone(&refresh_pending);
        thread::spawn(move || {
            thread::sleep(REFRESH_COALESCE_DELAY);
            let _ = app
                .state::<AppState>()
                .invalidate_explorer_root(&watched_root);
            let _ = workspace::emit_workspace_update(&app);
            end_refresh_window(refresh_pending.as_ref());
        });
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
    let normalized_candidate = normalize_path_for_compare(candidate);
    let normalized_root = normalize_path_for_compare(watched_root);
    normalized_candidate.starts_with(&normalized_root) || same_path(candidate, watched_root)
}

fn path_may_affect_explorer(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        || path.extension().is_none()
}

fn same_path(candidate: &Path, target: &Path) -> bool {
    let normalized_candidate = normalize_path_for_compare(candidate);
    let normalized_target = normalize_path_for_compare(target);

    if normalized_candidate == normalized_target {
        return true;
    }

    match (candidate.canonicalize(), target.canonicalize()) {
        (Ok(left), Ok(right)) => {
            normalize_path_for_compare(&left) == normalize_path_for_compare(&right)
        }
        _ => false,
    }
}

fn normalize_path_for_compare(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let path = path.to_string_lossy();

        if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{path}"));
        }

        if let Some(path) = path.strip_prefix(r"\\?\") {
            return PathBuf::from(path);
        }
    }

    path.to_path_buf()
}

fn begin_refresh_window(refresh_pending: &AtomicBool) -> bool {
    !refresh_pending.swap(true, Ordering::AcqRel)
}

fn end_refresh_window(refresh_pending: &AtomicBool) {
    refresh_pending.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::{
        begin_refresh_window, end_refresh_window, normalize_path_for_compare, same_path,
        should_refresh_current_document, should_refresh_workspace_explorer,
    };
    use crate::test_support::filesystem_test_lock;
    use notify::{
        Event,
        event::{CreateKind, DataChange, EventKind, ModifyKind, RemoveKind, RenameMode},
    };
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicBool, AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn refreshes_current_document_for_matching_write_events() {
        let _filesystem_test_lock = filesystem_test_lock();
        let path = unique_test_path("current.md");
        fs::create_dir_all(path.parent().expect("missing parent")).expect("failed to create dir");
        fs::write(&path, "# hello").expect("failed to seed file");

        let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(path.clone());

        assert!(should_refresh_current_document(&event, &path));

        fs::remove_file(&path).expect("failed to remove file");
        cleanup_test_dir(&path);
    }

    #[test]
    fn ignores_current_document_events_for_other_paths() {
        let _filesystem_test_lock = filesystem_test_lock();
        let watched_path = unique_test_path("watched.md");
        let other_path = unique_test_path("other.md");
        fs::create_dir_all(watched_path.parent().expect("missing parent"))
            .expect("failed to create dir");
        fs::write(&watched_path, "# watched").expect("failed to seed watched file");
        fs::write(&other_path, "# other").expect("failed to seed other file");

        let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(other_path.clone());

        assert!(!should_refresh_current_document(&event, &watched_path));

        fs::remove_file(&watched_path).expect("failed to remove watched file");
        fs::remove_file(&other_path).expect("failed to remove other file");
        cleanup_test_dir(&watched_path);
    }

    #[test]
    fn refreshes_workspace_explorer_for_markdown_create_and_directory_rename() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        let markdown = root.join("note.md");
        let renamed_dir = root.join("renamed");

        fs::create_dir_all(&root).expect("failed to create root");
        let create_event = Event::new(EventKind::Create(CreateKind::File)).add_path(markdown);
        assert!(should_refresh_workspace_explorer(&create_event, &root));

        let rename_event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(root.join("before"))
            .add_path(renamed_dir);
        assert!(should_refresh_workspace_explorer(&rename_event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn ignores_workspace_explorer_for_non_markdown_file_edits() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        let text_file = root.join("notes.txt");
        fs::create_dir_all(&root).expect("failed to create root");

        let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(text_file);

        assert!(!should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn same_path_matches_canonicalized_aliases() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("canonical");
        let nested = root.join("nested");
        let file = nested.join("doc.md");
        fs::create_dir_all(&nested).expect("failed to create nested dir");
        fs::write(&file, "# canonical").expect("failed to seed file");

        let alias = nested.join(".").join("doc.md");
        assert!(same_path(&alias, &file));

        fs::remove_file(&file).expect("failed to remove file");
        cleanup_test_dir(&root);
    }

    #[cfg(windows)]
    #[test]
    fn normalize_path_for_compare_strips_windows_verbatim_prefix() {
        assert_eq!(
            normalize_path_for_compare(Path::new(r"\\?\C:\docs\plan.md")),
            PathBuf::from(r"C:\docs\plan.md")
        );
        assert_eq!(
            normalize_path_for_compare(Path::new(r"\\?\UNC\server\share\plan.md")),
            PathBuf::from(r"\\server\share\plan.md")
        );
    }

    #[test]
    fn remove_events_refresh_explorer_for_markdown_files() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        fs::create_dir_all(&root).expect("failed to create root");

        let event = Event::new(EventKind::Remove(RemoveKind::File)).add_path(root.join("gone.md"));
        assert!(should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn refresh_window_allows_only_one_pending_refresh() {
        let refresh_pending = AtomicBool::new(false);

        assert!(begin_refresh_window(&refresh_pending));
        assert!(!begin_refresh_window(&refresh_pending));

        end_refresh_window(&refresh_pending);

        assert!(begin_refresh_window(&refresh_pending));
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let sequence = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        env::temp_dir()
            .join("mdv-tests")
            .join(format!("{nonce}-{sequence}-{name}"))
    }

    fn cleanup_test_dir(path: &Path) {
        let root = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .expect("path should have parent")
                .to_path_buf()
        };

        let _ = fs::remove_dir_all(root);
    }
}
