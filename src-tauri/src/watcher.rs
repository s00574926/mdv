use anyhow::{Context, Result};
use notify::{
    Event, RecommendedWatcher, RecursiveMode, Watcher,
    event::{CreateKind, EventKind, ModifyKind, RemoveKind, RenameMode},
    recommended_watcher,
};
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
    let paths_in_workspace = event
        .paths
        .iter()
        .filter(|candidate| path_is_within_root(candidate, watched_root))
        .collect::<Vec<_>>();

    if paths_in_workspace.is_empty() {
        return false;
    }

    if should_refresh_for_create_or_remove(&event.kind, &paths_in_workspace) {
        return true;
    }

    if matches!(event.kind, EventKind::Modify(ModifyKind::Name(_))) {
        return should_refresh_for_name_modify(&event.kind, &paths_in_workspace);
    }

    event.kind.is_modify()
        && paths_in_workspace
            .iter()
            .any(|candidate| path_may_affect_explorer(candidate))
}

fn should_refresh_for_create_or_remove(event_kind: &EventKind, paths: &[&PathBuf]) -> bool {
    match event_kind {
        EventKind::Create(CreateKind::File) | EventKind::Remove(RemoveKind::File) => {
            paths.iter().any(|candidate| path_is_markdown(candidate))
        }
        EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => true,
        kind if kind.is_create() || kind.is_remove() => paths
            .iter()
            .any(|candidate| path_may_affect_explorer(candidate)),
        _ => false,
    }
}

fn should_refresh_for_name_modify(event_kind: &EventKind, paths: &[&PathBuf]) -> bool {
    if paths.len() > 1 {
        return paths
            .iter()
            .any(|candidate| path_may_affect_explorer(candidate));
    }

    let Some(path) = paths.first() else {
        return false;
    };

    if path_may_affect_explorer(path) {
        return true;
    }

    matches!(
        event_kind,
        EventKind::Modify(ModifyKind::Name(
            RenameMode::Any | RenameMode::From | RenameMode::Both | RenameMode::Other
        ))
    ) || !path.exists()
}

fn path_is_within_root(candidate: &Path, watched_root: &Path) -> bool {
    let normalized_candidate = normalize_path_for_compare(candidate);
    let normalized_root = normalize_path_for_compare(watched_root);
    normalized_candidate.starts_with(&normalized_root) || same_path(candidate, watched_root)
}

fn path_is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("md"))
}

fn path_may_affect_explorer(path: &Path) -> bool {
    path.is_dir() || path_is_markdown(path) || (!path.exists() && path.extension().is_none())
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
        const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";
        const VERBATIM_PREFIX: &str = r"\\?\";

        let path = path.to_string_lossy();
        let lower_path = path.to_lowercase();

        if lower_path.starts_with(&VERBATIM_UNC_PREFIX.to_lowercase()) {
            return PathBuf::from(
                format!(r"\\{}", &path[VERBATIM_UNC_PREFIX.len()..]).to_lowercase(),
            );
        }

        if lower_path.starts_with(&VERBATIM_PREFIX.to_lowercase()) {
            return PathBuf::from(path[VERBATIM_PREFIX.len()..].to_lowercase());
        }

        PathBuf::from(lower_path)
    }

    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }
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
    fn ignores_workspace_explorer_for_multiple_non_markdown_file_edits() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        fs::create_dir_all(&root).expect("failed to create root");

        let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(root.join("notes.txt"))
            .add_path(root.join("draft.tmp"));

        assert!(!should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn ignores_workspace_explorer_for_extensionless_file_edits() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        let extensionless_file = root.join("README");
        fs::create_dir_all(&root).expect("failed to create root");
        fs::write(&extensionless_file, "ignore").expect("failed to create extensionless file");

        let event = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(extensionless_file);

        assert!(!should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn ignores_workspace_explorer_for_non_markdown_file_create_and_remove() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        fs::create_dir_all(&root).expect("failed to create root");

        let create_event =
            Event::new(EventKind::Create(CreateKind::File)).add_path(root.join("notes.txt"));
        assert!(!should_refresh_workspace_explorer(&create_event, &root));

        let remove_event =
            Event::new(EventKind::Remove(RemoveKind::File)).add_path(root.join("notes.txt"));
        assert!(!should_refresh_workspace_explorer(&remove_event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn ignores_workspace_explorer_for_non_markdown_file_renames() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        fs::create_dir_all(&root).expect("failed to create root");

        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(root.join("notes.txt"))
            .add_path(root.join("renamed.txt"));

        assert!(!should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn ignores_workspace_explorer_for_single_non_markdown_rename_to() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        let renamed_text = root.join("renamed.txt");
        fs::create_dir_all(&root).expect("failed to create root");
        fs::write(&renamed_text, "ignore").expect("failed to create text file");

        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
            .add_path(renamed_text);

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
    fn same_path_matches_windows_paths_case_insensitively() {
        assert!(same_path(
            Path::new(r"C:\Docs\Plan.md"),
            Path::new(r"c:\docs\plan.md")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn normalize_path_for_compare_strips_windows_verbatim_prefix() {
        assert_eq!(
            normalize_path_for_compare(Path::new(r"\\?\C:\docs\plan.md")),
            PathBuf::from(r"c:\docs\plan.md")
        );
        assert_eq!(
            normalize_path_for_compare(Path::new(r"\\?\UNC\server\share\plan.md")),
            PathBuf::from(r"\\server\share\plan.md")
        );
        assert_eq!(
            normalize_path_for_compare(Path::new(r"\\?\unc\server\share\plan.md")),
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
    fn directory_events_with_dots_refresh_workspace_explorer() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        fs::create_dir_all(&root).expect("failed to create root");

        let event =
            Event::new(EventKind::Remove(RemoveKind::Folder)).add_path(root.join("docs.v1"));
        assert!(should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn single_path_directory_rename_with_dots_refreshes_workspace_explorer() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        fs::create_dir_all(&root).expect("failed to create root");

        let event =
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::To)))
                .add_path(root.join("docs.v1"));
        assert!(should_refresh_workspace_explorer(&event, &root));

        cleanup_test_dir(&root);
    }

    #[test]
    fn paired_directory_rename_with_dots_refreshes_workspace_explorer() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("workspace");
        let renamed_directory = root.join("docs.v2");
        fs::create_dir_all(&renamed_directory).expect("failed to create renamed dir");

        let event = Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
            .add_path(root.join("docs.v1"))
            .add_path(renamed_directory);
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
