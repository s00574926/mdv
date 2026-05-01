use anyhow::{Context, Result, bail};
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    app_menu, explorer, markdown,
    state::AppState,
    watcher,
    workspace_payload::{ExplorerRoot, WorkspacePayload},
    workspace_session,
};

pub const WORKSPACE_UPDATED_EVENT: &str = "workspace://updated";

enum OpenDisposition {
    AddTab,
    ReplaceActive,
}

pub fn new_document(app: &AppHandle, state: &State<'_, AppState>) -> Result<WorkspacePayload> {
    workspace_session::create_untitled_document(state)?;
    app_menu::refresh_menu(app)?;
    current_workspace(state)
}

pub fn select_document(
    app: &AppHandle,
    state: &State<'_, AppState>,
    index: usize,
) -> Result<WorkspacePayload> {
    workspace_session::set_active_document_index(state, index)?;
    sync_active_watchers(app, state)?;
    app_menu::refresh_menu(app)?;
    current_workspace(state)
}

pub fn close_document(
    app: &AppHandle,
    state: &State<'_, AppState>,
    index: usize,
) -> Result<WorkspacePayload> {
    workspace_session::close_document(state, index)?;
    sync_active_watchers(app, state)?;
    app_menu::refresh_menu(app)?;
    current_workspace(state)
}

pub fn open_markdown_path(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
) -> Result<WorkspacePayload> {
    open_markdown_with_directory(app, state, path, None, None, true, OpenDisposition::AddTab)
}

pub fn open_markdown_dialog(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<WorkspacePayload> {
    let main_window = app
        .get_webview_window("main")
        .context("The main window is unavailable.")?;
    let Some(path) = app
        .dialog()
        .file()
        .set_parent(&main_window)
        .add_filter("Markdown", &["md"])
        .blocking_pick_file()
    else {
        return current_workspace(state);
    };

    let path = path
        .into_path()
        .context("The selected file could not be converted into a local path")?;
    open_markdown_path(app, state, &path)
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

    state.invalidate_rendered_document()?;
    state.invalidate_explorer_root(&canonical_directory)?;
    workspace_session::add_document(state, None, Some(canonical_directory.clone()))?;
    workspace_session::set_active_watchers(state, None, None, None, None)?;
    state.remember_explorer_root(
        &canonical_directory,
        explorer::placeholder_root(&canonical_directory),
    )?;
    app_menu::refresh_menu(app)?;
    let workspace = current_workspace(state)?;

    spawn_folder_open_refresh(app.clone(), canonical_directory);

    Ok(workspace)
}

pub fn open_folder_dialog(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<WorkspacePayload> {
    let main_window = app
        .get_webview_window("main")
        .context("The main window is unavailable.")?;
    let mut dialog = app.dialog().file().set_parent(&main_window);
    if let Ok(documents_directory) = app.path().document_dir() {
        dialog = dialog.set_directory(documents_directory);
    }
    let Some(path) = dialog.blocking_pick_folder() else {
        return current_workspace(state);
    };

    let path = path
        .into_path()
        .context("The selected folder could not be converted into a local path")?;
    open_folder_path(app, state, &path)
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

    open_markdown_with_directory(
        app,
        state,
        path,
        Some(current_directory),
        None,
        true,
        OpenDisposition::ReplaceActive,
    )
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

pub fn save_active_document(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<WorkspacePayload> {
    save_active_document_as(app, state)
}

pub fn save_active_document_as(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<WorkspacePayload> {
    if !workspace_session::active_document_is_untitled(state)? {
        bail!("Only untitled documents can be saved from this menu.");
    }

    let suggested_name = workspace_session::active_document_suggested_name(state)?
        .unwrap_or_else(|| String::from("Untitled.md"));
    let main_window = app
        .get_webview_window("main")
        .context("The main window is unavailable.")?;
    let Some(path) = app
        .dialog()
        .file()
        .set_parent(&main_window)
        .add_filter("Markdown", &["md"])
        .set_file_name(&suggested_name)
        .blocking_save_file()
    else {
        return current_workspace(state);
    };

    let path = normalize_save_path(
        path.into_path()
            .context("The selected file could not be converted into a local path")?,
    )?;
    save_active_document_to_path(app, state, &path)
}

pub fn save_active_document_to_path(
    app: &AppHandle,
    state: &State<'_, AppState>,
    path: &Path,
) -> Result<WorkspacePayload> {
    if !workspace_session::active_document_is_untitled(state)? {
        bail!("Only untitled documents can be saved from this menu.");
    }

    let path = normalize_save_path(path.to_path_buf())?;
    let contents = workspace_session::active_document_content(state)?;
    fs::write(&path, contents).with_context(|| format!("Failed to write {}", path.display()))?;

    open_markdown_with_directory(
        app,
        state,
        &path,
        None,
        None,
        true,
        OpenDisposition::ReplaceActive,
    )
}

pub fn reload_current_document<R: tauri::Runtime>(
    _app: &AppHandle<R>,
    state: &AppState,
) -> Result<WorkspacePayload> {
    reload_current_document_from_state(state)
}

fn reload_current_document_from_state(state: &AppState) -> Result<WorkspacePayload> {
    state.invalidate_rendered_document()?;
    current_workspace(state)
}

pub fn update_document_content(
    state: &State<'_, AppState>,
    index: usize,
    markdown: &str,
) -> Result<WorkspacePayload> {
    workspace_session::update_document_content(state, index, markdown.to_owned())?;
    current_workspace(state)
}

pub fn current_workspace(state: &AppState) -> Result<WorkspacePayload> {
    let snapshot = workspace_session::snapshot(state)?;
    let active_label = snapshot
        .active_document_index
        .and_then(|index| snapshot.document_tabs.get(index))
        .map(|document| document.label.clone());

    let (document, editor_text) = match snapshot.active_document.as_ref() {
        Some(active_document) => match (
            active_document.path.as_ref(),
            active_document.directory.as_ref(),
        ) {
            (Some(path), _) => (
                match state.rendered_document(path, snapshot.watching) {
                    Ok(document) => document,
                    Err(error) => markdown::render_error(path, &error, snapshot.watching),
                },
                None,
            ),
            (None, Some(directory)) => (markdown::folder_placeholder_document(directory), None),
            (None, None) => (
                markdown::untitled_document(
                    active_label.as_deref().unwrap_or("Untitled"),
                    &active_document.content,
                ),
                Some(active_document.content.clone()),
            ),
        },
        None => (markdown::new_document(), None),
    };
    let (explorer, explorer_updated) = state.resolve_explorer_payload(
        snapshot
            .active_document
            .as_ref()
            .and_then(|document| document.directory.as_deref()),
    )?;

    Ok(WorkspacePayload {
        document,
        editor_text,
        current_file_path: snapshot
            .active_document
            .as_ref()
            .and_then(|document| document.path.as_ref())
            .map(|path| path.display().to_string()),
        explorer,
        explorer_updated,
        recent_paths: snapshot
            .recent_paths
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
        document_tabs: workspace_session::build_document_tabs(
            &snapshot.document_tabs,
            snapshot.active_document_index,
        ),
        active_document_index: snapshot.active_document_index,
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
    prefetched_explorer_root: Option<ExplorerRoot>,
    remember_recent: bool,
    disposition: OpenDisposition,
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

    let rendered_document = markdown::render_file(&canonical_path, true)?;
    let explorer_directory = resolved_directory.clone();

    match disposition {
        OpenDisposition::AddTab => {
            workspace_session::add_document(
                state,
                Some(canonical_path.clone()),
                resolved_directory,
            )?;
        }
        OpenDisposition::ReplaceActive => {
            workspace_session::replace_active_document(
                state,
                Some(canonical_path.clone()),
                resolved_directory,
            )?;
        }
    }

    sync_active_watchers(app, state)?;
    state.remember_rendered_document(&canonical_path, rendered_document)?;
    if let (Some(directory), Some(explorer_root)) =
        (explorer_directory.as_deref(), prefetched_explorer_root)
    {
        state.remember_explorer_root(directory, explorer_root)?;
    }

    if remember_recent {
        state.remember_recent_file(&canonical_path)?;
    }
    app_menu::refresh_menu(app)?;

    current_workspace(state)
}

fn sync_active_watchers(app: &AppHandle, state: &State<'_, AppState>) -> Result<()> {
    let snapshot = workspace_session::snapshot(state)?;
    let Some(active_document) = snapshot.active_document else {
        state.invalidate_rendered_document()?;
        return workspace_session::set_active_watchers(state, None, None, None, None);
    };

    let previous_document_path = workspace_session::watched_document_path(state)?;
    let previous_explorer_root = workspace_session::watched_explorer_root(state)?;
    if previous_document_path.as_deref() != active_document.path.as_deref() {
        state.invalidate_rendered_document()?;
    }
    if let Some(directory) = active_document.directory.as_ref()
        && previous_explorer_root.as_deref() != Some(directory.as_path())
    {
        state.invalidate_explorer_root(directory)?;
    }

    let current_document_watcher = active_document
        .path
        .as_ref()
        .map(|path| watcher::watch_file(app.clone(), path.clone()))
        .transpose()?;
    let explorer_watcher = active_document
        .directory
        .as_ref()
        .map(|directory| watcher::watch_workspace_directory(app.clone(), directory.clone()))
        .transpose()?;

    workspace_session::set_active_watchers(
        state,
        current_document_watcher,
        explorer_watcher,
        active_document.path,
        active_document.directory,
    )
}

fn spawn_folder_open_refresh(app: AppHandle, directory: PathBuf) {
    thread::spawn(move || {
        let scanned_root = match explorer::scan_root(&directory) {
            Ok(scanned_root) => scanned_root,
            Err(error) => {
                eprintln!("failed to scan folder {}: {error:#}", directory.display());
                return;
            }
        };

        let state = app.state::<AppState>();
        let snapshot = match workspace_session::snapshot(&state) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                eprintln!(
                    "failed to snapshot workspace after opening {}: {error:#}",
                    directory.display()
                );
                return;
            }
        };
        let active_directory = snapshot
            .active_document
            .as_ref()
            .and_then(|document| document.directory.as_ref());

        if active_directory != Some(&directory) {
            let _ = state.remember_explorer_root(&directory, scanned_root.root);
            return;
        }

        if snapshot
            .active_document
            .as_ref()
            .is_some_and(|document| document.path.is_none())
        {
            if let Some(first_markdown) = scanned_root.first_markdown {
                match open_markdown_with_directory(
                    &app,
                    &state,
                    &first_markdown,
                    Some(directory.clone()),
                    Some(scanned_root.root),
                    true,
                    OpenDisposition::ReplaceActive,
                ) {
                    Ok(workspace) => {
                        let _ = app.emit(WORKSPACE_UPDATED_EVENT, workspace);
                    }
                    Err(error) => {
                        eprintln!(
                            "failed to open first markdown in {}: {error:#}",
                            directory.display()
                        );
                    }
                }
                return;
            }

            let explorer_watcher =
                match watcher::watch_workspace_directory(app.clone(), directory.clone()) {
                    Ok(watcher) => Some(watcher),
                    Err(error) => {
                        eprintln!(
                            "failed to watch workspace {}: {error:#}",
                            directory.display()
                        );
                        None
                    }
                };
            let _ = workspace_session::set_active_watchers(
                &state,
                None,
                explorer_watcher,
                None,
                Some(directory.clone()),
            );
        }

        let _ = state.invalidate_explorer_root(&directory);
        let _ = state.remember_explorer_root(&directory, scanned_root.root);
        let _ = emit_workspace_update(&app);
    });
}

fn normalize_save_path(mut path: PathBuf) -> Result<PathBuf> {
    if path.extension().is_none() {
        path.set_extension("md");
    }

    ensure_markdown_file(&path)?;
    Ok(path)
}

fn ensure_markdown_file(path: &Path) -> Result<()> {
    let extension = path.extension().and_then(|value| value.to_str());
    if !extension.is_some_and(|value| value.eq_ignore_ascii_case("md")) {
        bail!("Only .md files can be opened.");
    }

    if path.exists() && !path.is_file() {
        bail!("Only .md files can be opened.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_markdown_file, normalize_save_path};
    use crate::{markdown, state::AppState, test_support::filesystem_test_lock, trusted_preview};
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn accepts_markdown_paths_case_insensitively() {
        assert!(ensure_markdown_file(Path::new("notes.md")).is_ok());
        assert!(ensure_markdown_file(Path::new("notes.MD")).is_ok());
    }

    #[test]
    fn rejects_non_markdown_paths() {
        let error = ensure_markdown_file(Path::new("notes.txt")).expect_err("expected rejection");
        assert_eq!(error.to_string(), "Only .md files can be opened.");
    }

    #[test]
    fn rejects_markdown_named_directories() {
        let _filesystem_test_lock = filesystem_test_lock();
        let root = unique_test_path("markdown-dir");
        let path = root.join("archive.md");
        fs::create_dir_all(&path).expect("failed to create markdown-named directory");

        let error =
            ensure_markdown_file(&path).expect_err("expected markdown-named directory rejection");
        assert_eq!(error.to_string(), "Only .md files can be opened.");

        cleanup_test_dir(&root);
    }

    #[test]
    fn adds_markdown_extension_to_save_paths_without_one() {
        assert_eq!(
            normalize_save_path(PathBuf::from("notes"))
                .expect("expected markdown path normalization"),
            PathBuf::from("notes.md")
        );
    }

    #[test]
    fn reload_current_document_ignores_stale_render_cache() {
        let _filesystem_test_lock = filesystem_test_lock();
        let state = AppState::new_for_tests();
        let path = unique_test_path("guide.md");

        fs::write(&path, "# Fresh").expect("failed to write markdown file");
        {
            let mut session = state
                .session
                .lock()
                .expect("state lock should be available");
            session.documents.push(crate::state::OpenDocumentSession {
                path: Some(path.clone()),
                directory: None,
                untitled_number: None,
                content: String::new(),
            });
            session.active_document_index = Some(0);
        }
        state
            .remember_rendered_document(&path, rendered_document("<h1>Stale</h1>"))
            .expect("failed to seed rendered cache");

        let workspace = super::reload_current_document_from_state(&state).expect("reload failed");

        assert!(workspace.document.html.contains("Fresh"));
        assert!(!workspace.document.html.contains("Stale"));

        fs::remove_file(&path).expect("failed to remove markdown file");
        cleanup_test_dir(&path);
    }

    fn rendered_document(html: &str) -> markdown::RenderedDocument {
        markdown::RenderedDocument {
            title: String::from("guide"),
            html: html.to_string(),
            source_name: String::from("guide.md"),
            source_path: String::from("guide.md"),
            watching: true,
            trust_model: trusted_preview::TRUST_MODEL,
        }
    }

    fn unique_test_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let sequence = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir()
            .join("mdv-tests")
            .join(format!("{name}-{nonce}-{sequence}"));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create test directory");
        }
        path
    }

    fn cleanup_test_dir(path: &Path) {
        if let Some(parent) = path.parent()
            && parent.exists()
        {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
