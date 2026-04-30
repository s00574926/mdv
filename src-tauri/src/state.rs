use anyhow::{Context, Result};
use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::TryFrom,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, Window};

use crate::{explorer, markdown, workspace_payload::ExplorerRoot};

const MAX_RECENT_FILES: usize = 10;
const RECENT_FILES_NAME: &str = "recent-files.json";
const WINDOW_STATE_NAME: &str = "window-state.json";

#[derive(Clone)]
pub struct OpenDocumentSession {
    pub path: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub untitled_number: Option<usize>,
    pub content: String,
}

pub struct AppSession {
    pub documents: Vec<OpenDocumentSession>,
    pub active_document_index: Option<usize>,
    pub current_document_watcher: Option<RecommendedWatcher>,
    pub explorer_watcher: Option<RecommendedWatcher>,
    pub watched_document_path: Option<PathBuf>,
    pub watched_explorer_root: Option<PathBuf>,
    pub last_sent_explorer_root: Option<PathBuf>,
    pub explorer_dirty: bool,
    pub(crate) rendered_document: Option<CachedRenderedDocument>,
    pub explorer_roots: HashMap<PathBuf, ExplorerRoot>,
    pub recent_paths: Vec<PathBuf>,
    pub next_untitled_number: usize,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            active_document_index: None,
            current_document_watcher: None,
            explorer_watcher: None,
            watched_document_path: None,
            watched_explorer_root: None,
            last_sent_explorer_root: None,
            explorer_dirty: false,
            rendered_document: None,
            explorer_roots: HashMap::new(),
            recent_paths: Vec::new(),
            next_untitled_number: 1,
        }
    }
}

pub struct AppState {
    pub session: Mutex<AppSession>,
    recent_store_path: PathBuf,
    saved_window_state: Option<SavedWindowState>,
    window_state_store_path: PathBuf,
}

impl AppState {
    pub fn load(app: &AppHandle) -> Result<Self> {
        let recent_store_path = store_path(app, RECENT_FILES_NAME)?;
        let window_state_store_path = store_path(app, WINDOW_STATE_NAME)?;
        let recent_paths = load_recent_paths(&recent_store_path);
        let saved_window_state = load_window_state(&window_state_store_path);

        Ok(Self {
            session: Mutex::new(AppSession {
                recent_paths,
                ..Default::default()
            }),
            recent_store_path,
            saved_window_state,
            window_state_store_path,
        })
    }

    pub(crate) fn restore_window_state<T: WindowStateTarget>(&self, window: &T) -> Result<()> {
        let Some(saved_window_state) = self.saved_window_state.as_ref() else {
            return Ok(());
        };

        apply_window_state(window, saved_window_state)
    }

    pub(crate) fn persist_window_state<T: WindowStateTarget>(&self, window: &T) -> Result<()> {
        let saved_window_state = capture_window_state(window)?;
        persist_saved_window_state(&self.window_state_store_path, &saved_window_state)
    }

    pub fn persist_recent_paths(&self) -> Result<()> {
        let recent_paths = {
            let session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
            session.recent_paths.clone()
        };

        persist_recent_paths(&self.recent_store_path, &recent_paths)
    }

    pub fn recent_paths(&self) -> Result<Vec<PathBuf>> {
        let session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

        Ok(session.recent_paths.clone())
    }

    pub fn remember_recent_file(&self, path: &Path) -> Result<()> {
        {
            let mut session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

            session
                .recent_paths
                .retain(|candidate| !same_recent_path(candidate, path));
            session.recent_paths.insert(0, path.to_path_buf());
            session.recent_paths.truncate(MAX_RECENT_FILES);
        }

        self.persist_recent_paths()
    }

    pub fn explorer_root(&self, directory: &Path) -> Result<ExplorerRoot> {
        {
            let session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

            if let Some(explorer_root) = session.explorer_roots.get(directory) {
                return Ok(explorer_root.clone());
            }
        }

        let explorer_root = explorer::build_root(directory)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

        Ok(cache_explorer_root(&mut session, directory, explorer_root))
    }

    pub fn remember_explorer_root(
        &self,
        directory: &Path,
        explorer_root: ExplorerRoot,
    ) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        cache_explorer_root(&mut session, directory, explorer_root);
        Ok(())
    }

    pub fn invalidate_explorer_root(&self, directory: &Path) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.explorer_roots.remove(directory);
        session.explorer_dirty = true;
        Ok(())
    }

    pub fn resolve_explorer_payload(
        &self,
        directory: Option<&Path>,
    ) -> Result<(Option<ExplorerRoot>, bool)> {
        let Some(directory) = directory else {
            let mut session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
            let explorer_updated = session.last_sent_explorer_root.take().is_some();
            session.explorer_dirty = false;
            return Ok((None, explorer_updated));
        };

        {
            let session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
            if session.last_sent_explorer_root.as_deref() == Some(directory)
                && !session.explorer_dirty
            {
                return Ok((None, false));
            }
        }

        let explorer_root = self.explorer_root(directory)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.last_sent_explorer_root = Some(directory.to_path_buf());
        session.explorer_dirty = false;
        Ok((Some(explorer_root), true))
    }

    pub fn rendered_document(
        &self,
        path: &Path,
        watching: bool,
    ) -> Result<markdown::RenderedDocument> {
        {
            let session = self
                .session
                .lock()
                .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;

            if let Some(rendered_document) = rendered_document_from_cache(&session, path, watching)
            {
                return Ok(rendered_document);
            }
        }

        let rendered_document = markdown::render_file(path, watching)?;
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        cache_rendered_document(&mut session, path, rendered_document.clone());
        Ok(rendered_document)
    }

    pub fn remember_rendered_document(
        &self,
        path: &Path,
        rendered_document: markdown::RenderedDocument,
    ) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        cache_rendered_document(&mut session, path, rendered_document);
        Ok(())
    }

    pub fn invalidate_rendered_document(&self) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| anyhow::anyhow!("The preview state is unavailable."))?;
        session.rendered_document = None;
        Ok(())
    }
}

fn cache_explorer_root(
    session: &mut AppSession,
    directory: &Path,
    explorer_root: ExplorerRoot,
) -> ExplorerRoot {
    session
        .explorer_roots
        .insert(directory.to_path_buf(), explorer_root.clone());
    explorer_root
}

#[derive(Clone)]
pub(crate) struct CachedRenderedDocument {
    path: PathBuf,
    document: markdown::RenderedDocument,
}

fn cache_rendered_document(
    session: &mut AppSession,
    path: &Path,
    rendered_document: markdown::RenderedDocument,
) {
    session.rendered_document = Some(CachedRenderedDocument {
        path: path.to_path_buf(),
        document: rendered_document,
    });
}

fn rendered_document_from_cache(
    session: &AppSession,
    path: &Path,
    watching: bool,
) -> Option<markdown::RenderedDocument> {
    let cached = session.rendered_document.as_ref()?;
    if cached.path != path {
        return None;
    }

    let mut rendered_document = cached.document.clone();
    rendered_document.watching = watching;
    Some(rendered_document)
}

#[derive(Serialize, Deserialize)]
struct RecentFilesStore {
    recent_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedWindowState {
    monitor_name: Option<String>,
    #[serde(default)]
    monitor_index: Option<usize>,
    absolute_position: StoredPosition,
    monitor_offset: StoredPosition,
    outer_size: StoredSize,
    maximized: bool,
}

impl SavedWindowState {
    fn is_valid(&self) -> bool {
        self.outer_size.width > 0 && self.outer_size.height > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct StoredPosition {
    x: i32,
    y: i32,
}

impl From<PhysicalPosition<i32>> for StoredPosition {
    fn from(value: PhysicalPosition<i32>) -> Self {
        Self {
            x: value.x,
            y: value.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct StoredSize {
    width: u32,
    height: u32,
}

impl From<PhysicalSize<u32>> for StoredSize {
    fn from(value: PhysicalSize<u32>) -> Self {
        Self {
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MonitorSnapshot {
    name: Option<String>,
    position: StoredPosition,
    size: StoredSize,
}

impl From<&tauri::Monitor> for MonitorSnapshot {
    fn from(value: &tauri::Monitor) -> Self {
        Self {
            name: value.name().cloned(),
            position: (*value.position()).into(),
            size: (*value.size()).into(),
        }
    }
}

pub(crate) trait WindowStateTarget {
    fn outer_position(&self) -> tauri::Result<PhysicalPosition<i32>>;
    fn outer_size(&self) -> tauri::Result<PhysicalSize<u32>>;
    fn current_monitor(&self) -> tauri::Result<Option<tauri::Monitor>>;
    fn available_monitors(&self) -> tauri::Result<Vec<tauri::Monitor>>;
    fn is_maximized(&self) -> tauri::Result<bool>;
    fn set_size(&self, size: PhysicalSize<u32>) -> tauri::Result<()>;
    fn set_position(&self, position: PhysicalPosition<i32>) -> tauri::Result<()>;
    fn maximize(&self) -> tauri::Result<()>;
}

impl WindowStateTarget for Window {
    fn outer_position(&self) -> tauri::Result<PhysicalPosition<i32>> {
        Window::outer_position(self)
    }

    fn outer_size(&self) -> tauri::Result<PhysicalSize<u32>> {
        Window::outer_size(self)
    }

    fn current_monitor(&self) -> tauri::Result<Option<tauri::Monitor>> {
        Window::current_monitor(self)
    }

    fn available_monitors(&self) -> tauri::Result<Vec<tauri::Monitor>> {
        Window::available_monitors(self)
    }

    fn is_maximized(&self) -> tauri::Result<bool> {
        Window::is_maximized(self)
    }

    fn set_size(&self, size: PhysicalSize<u32>) -> tauri::Result<()> {
        Window::set_size(self, size)
    }

    fn set_position(&self, position: PhysicalPosition<i32>) -> tauri::Result<()> {
        Window::set_position(self, position)
    }

    fn maximize(&self) -> tauri::Result<()> {
        Window::maximize(self)
    }
}

impl WindowStateTarget for WebviewWindow {
    fn outer_position(&self) -> tauri::Result<PhysicalPosition<i32>> {
        WebviewWindow::outer_position(self)
    }

    fn outer_size(&self) -> tauri::Result<PhysicalSize<u32>> {
        WebviewWindow::outer_size(self)
    }

    fn current_monitor(&self) -> tauri::Result<Option<tauri::Monitor>> {
        WebviewWindow::current_monitor(self)
    }

    fn available_monitors(&self) -> tauri::Result<Vec<tauri::Monitor>> {
        WebviewWindow::available_monitors(self)
    }

    fn is_maximized(&self) -> tauri::Result<bool> {
        WebviewWindow::is_maximized(self)
    }

    fn set_size(&self, size: PhysicalSize<u32>) -> tauri::Result<()> {
        WebviewWindow::set_size(self, size)
    }

    fn set_position(&self, position: PhysicalPosition<i32>) -> tauri::Result<()> {
        WebviewWindow::set_position(self, position)
    }

    fn maximize(&self) -> tauri::Result<()> {
        WebviewWindow::maximize(self)
    }
}

fn store_path(app: &AppHandle, file_name: &str) -> Result<PathBuf> {
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .context("Failed to resolve the app local data directory")?;
    fs::create_dir_all(&app_data_dir)
        .with_context(|| format!("Failed to create {}", app_data_dir.display()))?;

    Ok(app_data_dir.join(file_name))
}

fn load_recent_paths(path: &Path) -> Vec<PathBuf> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let Ok(store) = serde_json::from_str::<RecentFilesStore>(&contents) else {
        return Vec::new();
    };

    let mut recent_paths: Vec<PathBuf> = Vec::new();
    for path in store.recent_paths.into_iter().map(PathBuf::from) {
        if !path.is_absolute() {
            continue;
        }

        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        {
            continue;
        }

        if recent_paths
            .iter()
            .any(|candidate| same_recent_path(candidate, &path))
        {
            continue;
        }

        recent_paths.push(path);
        if recent_paths.len() >= MAX_RECENT_FILES {
            break;
        }
    }

    recent_paths
}

fn persist_recent_paths(path: &Path, recent_paths: &[PathBuf]) -> Result<()> {
    let store = RecentFilesStore {
        recent_paths: recent_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    };
    let contents = serde_json::to_string_pretty(&store)?;

    fs::write(path, contents).with_context(|| format!("Failed to write {}", path.display()))
}

fn same_recent_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        recent_path_key(left) == recent_path_key(right)
    }

    #[cfg(not(windows))]
    {
        left == right
    }
}

#[cfg(windows)]
fn recent_path_key(path: &Path) -> String {
    const VERBATIM_UNC_PREFIX: &str = r"\\?\UNC\";
    const VERBATIM_PREFIX: &str = r"\\?\";

    let path = path.to_string_lossy();
    let lower_path = path.to_lowercase();

    if lower_path.starts_with(&VERBATIM_UNC_PREFIX.to_lowercase()) {
        return format!(r"\\{}", &path[VERBATIM_UNC_PREFIX.len()..]).to_lowercase();
    }

    if lower_path.starts_with(&VERBATIM_PREFIX.to_lowercase()) {
        return path[VERBATIM_PREFIX.len()..].to_lowercase();
    }

    lower_path
}

fn load_window_state(path: &Path) -> Option<SavedWindowState> {
    let Ok(contents) = fs::read_to_string(path) else {
        return None;
    };

    let Ok(store) = serde_json::from_str::<SavedWindowState>(&contents) else {
        return None;
    };

    store.is_valid().then_some(store)
}

fn capture_window_state<T: WindowStateTarget>(window: &T) -> Result<SavedWindowState> {
    let absolute_position: StoredPosition = window.outer_position()?.into();
    let outer_size: StoredSize = window.outer_size()?.into();
    let available_monitors = window.available_monitors()?;
    let current_monitor = window.current_monitor()?;

    let (monitor_name, monitor_index, monitor_offset) = if let Some(monitor) = current_monitor {
        let monitor_position: StoredPosition = (*monitor.position()).into();
        (
            monitor.name().cloned(),
            available_monitors.iter().position(|candidate| {
                MonitorSnapshot::from(candidate) == MonitorSnapshot::from(&monitor)
            }),
            StoredPosition {
                x: absolute_position.x - monitor_position.x,
                y: absolute_position.y - monitor_position.y,
            },
        )
    } else {
        (None, None, absolute_position)
    };

    Ok(SavedWindowState {
        monitor_name,
        monitor_index,
        absolute_position,
        monitor_offset,
        outer_size,
        maximized: window.is_maximized()?,
    })
}

fn apply_window_state<T: WindowStateTarget>(
    window: &T,
    saved_window_state: &SavedWindowState,
) -> Result<()> {
    let monitors = window
        .available_monitors()?
        .iter()
        .map(MonitorSnapshot::from)
        .collect::<Vec<_>>();

    window.set_size(PhysicalSize::new(
        saved_window_state.outer_size.width,
        saved_window_state.outer_size.height,
    ))?;

    if let Some(position) = resolve_window_position(&monitors, saved_window_state) {
        window.set_position(PhysicalPosition::new(position.x, position.y))?;
    }

    if saved_window_state.maximized {
        window.maximize()?;
    }

    Ok(())
}

fn persist_saved_window_state(path: &Path, saved_window_state: &SavedWindowState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(saved_window_state)?;
    fs::write(path, contents).with_context(|| format!("Failed to write {}", path.display()))
}

fn resolve_window_position(
    monitors: &[MonitorSnapshot],
    saved_window_state: &SavedWindowState,
) -> Option<StoredPosition> {
    if let Some(saved_monitor) = saved_window_state
        .monitor_index
        .and_then(|monitor_index| monitors.get(monitor_index))
        .filter(|monitor| {
            saved_window_state.monitor_name.is_none()
                || monitor.name.as_deref() == saved_window_state.monitor_name.as_deref()
        })
    {
        return Some(clamp_position_to_monitor(
            StoredPosition {
                x: saved_monitor.position.x + saved_window_state.monitor_offset.x,
                y: saved_monitor.position.y + saved_window_state.monitor_offset.y,
            },
            saved_window_state.outer_size,
            saved_monitor,
        ));
    }

    if let Some(saved_monitor_name) = saved_window_state.monitor_name.as_deref()
        && let Some(saved_monitor) = monitors
            .iter()
            .find(|monitor| monitor.name.as_deref() == Some(saved_monitor_name))
    {
        return Some(clamp_position_to_monitor(
            StoredPosition {
                x: saved_monitor.position.x + saved_window_state.monitor_offset.x,
                y: saved_monitor.position.y + saved_window_state.monitor_offset.y,
            },
            saved_window_state.outer_size,
            saved_monitor,
        ));
    }

    if let Some(position) = monitors
        .iter()
        .find(|monitor| {
            rects_intersect(
                saved_window_state.absolute_position,
                saved_window_state.outer_size,
                monitor.position,
                monitor.size,
            )
        })
        .map(|monitor| {
            clamp_position_to_monitor(
                saved_window_state.absolute_position,
                saved_window_state.outer_size,
                monitor,
            )
        })
    {
        return Some(position);
    }

    if let Some(saved_monitor) = saved_window_state
        .monitor_index
        .and_then(|monitor_index| monitors.get(monitor_index))
    {
        return Some(clamp_position_to_monitor(
            StoredPosition {
                x: saved_monitor.position.x + saved_window_state.monitor_offset.x,
                y: saved_monitor.position.y + saved_window_state.monitor_offset.y,
            },
            saved_window_state.outer_size,
            saved_monitor,
        ));
    }

    None
}

fn clamp_position_to_monitor(
    position: StoredPosition,
    size: StoredSize,
    monitor: &MonitorSnapshot,
) -> StoredPosition {
    let monitor_width = i32::try_from(monitor.size.width).unwrap_or(i32::MAX);
    let monitor_height = i32::try_from(monitor.size.height).unwrap_or(i32::MAX);
    let window_width = i32::try_from(size.width).unwrap_or(i32::MAX);
    let window_height = i32::try_from(size.height).unwrap_or(i32::MAX);
    let min_x = monitor.position.x;
    let min_y = monitor.position.y;
    let max_x = min_x.saturating_add((monitor_width - window_width).max(0));
    let max_y = min_y.saturating_add((monitor_height - window_height).max(0));

    StoredPosition {
        x: position.x.clamp(min_x, max_x),
        y: position.y.clamp(min_y, max_y),
    }
}

fn rects_intersect(
    first_position: StoredPosition,
    first_size: StoredSize,
    second_position: StoredPosition,
    second_size: StoredSize,
) -> bool {
    let first_right = first_position
        .x
        .saturating_add(i32::try_from(first_size.width).unwrap_or(i32::MAX));
    let first_bottom = first_position
        .y
        .saturating_add(i32::try_from(first_size.height).unwrap_or(i32::MAX));
    let second_right = second_position
        .x
        .saturating_add(i32::try_from(second_size.width).unwrap_or(i32::MAX));
    let second_bottom = second_position
        .y
        .saturating_add(i32::try_from(second_size.height).unwrap_or(i32::MAX));

    first_position.x < second_right
        && first_right > second_position.x
        && first_position.y < second_bottom
        && first_bottom > second_position.y
}

#[cfg(test)]
mod tests {
    use super::{
        AppSession, MonitorSnapshot, SavedWindowState, StoredPosition, StoredSize,
        cache_explorer_root, cache_rendered_document, load_recent_paths, load_window_state,
        persist_saved_window_state, rendered_document_from_cache, resolve_window_position,
    };
    use crate::test_support::filesystem_test_lock;
    use crate::{
        markdown, trusted_preview,
        workspace_payload::{ExplorerNode, ExplorerNodeKind, ExplorerRoot},
    };
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn restore_prefers_saved_monitor_when_it_is_still_available() {
        let monitors = vec![
            monitor("Primary", 0, 0, 1920, 1080),
            monitor("Secondary", 1920, 0, 2560, 1440),
        ];
        let saved_window_state = saved_window_state(
            Some("Secondary"),
            Some(1),
            StoredPosition { x: 24, y: 36 },
            StoredPosition { x: 1800, y: 120 },
            StoredSize {
                width: 1200,
                height: 800,
            },
        );

        let position =
            resolve_window_position(&monitors, &saved_window_state).expect("expected position");

        assert_eq!(position, StoredPosition { x: 1944, y: 36 });
    }

    #[test]
    fn restore_falls_back_to_visible_absolute_position_when_monitor_name_is_missing() {
        let monitors = vec![monitor("Primary", 0, 0, 1920, 1080)];
        let saved_window_state = saved_window_state(
            Some("Secondary"),
            Some(1),
            StoredPosition { x: 80, y: 40 },
            StoredPosition { x: 640, y: 320 },
            StoredSize {
                width: 1000,
                height: 700,
            },
        );

        let position =
            resolve_window_position(&monitors, &saved_window_state).expect("expected position");

        assert_eq!(position, StoredPosition { x: 640, y: 320 });
    }

    #[test]
    fn restore_returns_none_when_saved_window_is_offscreen() {
        let monitors = vec![monitor("Primary", 0, 0, 1920, 1080)];
        let saved_window_state = saved_window_state(
            Some("Secondary"),
            Some(1),
            StoredPosition { x: 20, y: 20 },
            StoredPosition { x: 4000, y: 3000 },
            StoredSize {
                width: 900,
                height: 700,
            },
        );

        assert!(resolve_window_position(&monitors, &saved_window_state).is_none());
    }

    #[test]
    fn restore_clamps_position_inside_resized_monitor() {
        let monitors = vec![monitor("Primary", 0, 0, 1280, 720)];
        let saved_window_state = saved_window_state(
            Some("Primary"),
            Some(0),
            StoredPosition { x: 1100, y: 500 },
            StoredPosition { x: 1100, y: 500 },
            StoredSize {
                width: 800,
                height: 400,
            },
        );

        let position =
            resolve_window_position(&monitors, &saved_window_state).expect("expected position");

        assert_eq!(position, StoredPosition { x: 480, y: 320 });
    }

    #[test]
    fn load_window_state_skips_invalid_window_sizes() {
        let _filesystem_test_lock = filesystem_test_lock();
        let path = unique_test_path("invalid-window-state.json");

        fs::write(
            &path,
            r#"{
  "monitor_name": "Primary",
  "absolute_position": { "x": 0, "y": 0 },
  "monitor_offset": { "x": 0, "y": 0 },
  "outer_size": { "width": 0, "height": 720 },
  "maximized": false
}"#,
        )
        .expect("failed to write invalid window state");

        assert!(load_window_state(&path).is_none());

        fs::remove_file(&path).expect("failed to remove invalid window state");
        cleanup_test_dir(&path);
    }

    #[test]
    fn load_window_state_accepts_legacy_payloads_without_monitor_index() {
        let _filesystem_test_lock = filesystem_test_lock();
        let path = unique_test_path("legacy-window-state.json");

        fs::write(
            &path,
            r#"{
  "monitor_name": "Primary",
  "absolute_position": { "x": 32, "y": 48 },
  "monitor_offset": { "x": 32, "y": 48 },
  "outer_size": { "width": 1440, "height": 960 },
  "maximized": false
}"#,
        )
        .expect("failed to write legacy window state");

        let reloaded = load_window_state(&path).expect("expected saved window state");

        assert_eq!(reloaded.monitor_name.as_deref(), Some("Primary"));
        assert_eq!(reloaded.monitor_index, None);

        fs::remove_file(&path).expect("failed to remove legacy window state");
        cleanup_test_dir(&path);
    }

    #[test]
    fn load_recent_paths_dedupes_persisted_entries() {
        let _filesystem_test_lock = filesystem_test_lock();
        let path = unique_test_path("recent-files.json");
        let recent_dir = path
            .parent()
            .expect("recent store path should have a parent");
        let absolute_plan = recent_dir.join("plan.md");
        let absolute_notes = recent_dir.join("notes.md");
        let contents = serde_json::json!({
            "recent_paths": [
                absolute_plan.display().to_string(),
                absolute_plan.display().to_string(),
                absolute_notes.display().to_string()
            ]
        });

        fs::write(&path, contents.to_string()).expect("failed to write duplicate recent files");

        let recent_paths = load_recent_paths(&path);

        assert_eq!(recent_paths, vec![absolute_plan, absolute_notes]);

        fs::remove_file(&path).expect("failed to remove recent files");
        cleanup_test_dir(&path);
    }

    #[test]
    fn load_recent_paths_ignores_relative_persisted_entries() {
        let _filesystem_test_lock = filesystem_test_lock();
        let path = unique_test_path("recent-files.json");
        let recent_dir = path
            .parent()
            .expect("recent store path should have a parent");
        let absolute_plan = recent_dir.join("plan.md");
        let absolute_notes = recent_dir.join("notes.md");
        let contents = serde_json::json!({
            "recent_paths": [
                absolute_plan.display().to_string(),
                "docs/relative.md",
                absolute_notes.display().to_string()
            ]
        });

        fs::write(&path, contents.to_string()).expect("failed to write relative recent files");

        let recent_paths = load_recent_paths(&path);

        assert_eq!(recent_paths, vec![absolute_plan, absolute_notes]);

        fs::remove_file(&path).expect("failed to remove recent files");
        cleanup_test_dir(&path);
    }

    #[test]
    fn persist_and_load_window_state_round_trip() {
        let _filesystem_test_lock = filesystem_test_lock();
        let path = unique_test_path("window-state.json");
        let saved_window_state = saved_window_state(
            Some("Primary"),
            Some(0),
            StoredPosition { x: 32, y: 48 },
            StoredPosition { x: 32, y: 48 },
            StoredSize {
                width: 1440,
                height: 960,
            },
        );

        persist_saved_window_state(&path, &saved_window_state)
            .expect("failed to persist saved window state");

        let reloaded = load_window_state(&path).expect("expected saved window state");

        assert_eq!(reloaded.monitor_name.as_deref(), Some("Primary"));
        assert_eq!(reloaded.monitor_index, Some(0));
        assert_eq!(reloaded.absolute_position, StoredPosition { x: 32, y: 48 });
        assert_eq!(reloaded.outer_size.width, 1440);
        assert_eq!(reloaded.outer_size.height, 960);

        fs::remove_file(&path).expect("failed to remove saved window state");
        cleanup_test_dir(&path);
    }

    #[test]
    fn restore_uses_saved_monitor_index_when_names_are_unavailable() {
        let monitors = vec![
            monitor("Display 1", 0, 0, 1920, 1080),
            monitor("Display 2", 1920, 0, 2560, 1440),
        ];
        let saved_window_state = saved_window_state(
            None,
            Some(1),
            StoredPosition { x: 100, y: 80 },
            StoredPosition { x: 2020, y: 80 },
            StoredSize {
                width: 1200,
                height: 800,
            },
        );

        let position =
            resolve_window_position(&monitors, &saved_window_state).expect("expected position");

        assert_eq!(position, StoredPosition { x: 2020, y: 80 });
    }

    #[test]
    fn restore_prefers_visible_absolute_position_over_mismatched_monitor_index() {
        let monitors = vec![
            monitor("Primary", 0, 0, 1920, 1080),
            monitor("Replacement", 1920, 0, 2560, 1440),
        ];
        let saved_window_state = saved_window_state(
            Some("Secondary"),
            Some(1),
            StoredPosition { x: 40, y: 60 },
            StoredPosition { x: 120, y: 80 },
            StoredSize {
                width: 1000,
                height: 700,
            },
        );

        let position =
            resolve_window_position(&monitors, &saved_window_state).expect("expected position");

        assert_eq!(position, StoredPosition { x: 120, y: 80 });
    }

    #[test]
    fn cache_explorer_root_reuses_existing_entry_until_invalidated() {
        let directory = PathBuf::from(r"C:\docs");
        let mut session = AppSession::default();

        cache_explorer_root(&mut session, &directory, explorer_root("guide.md"));
        let cached = session
            .explorer_roots
            .get(&directory)
            .expect("expected cached explorer root");
        assert_eq!(cached.children[0].name, "guide.md");

        session.explorer_roots.remove(&directory);
        cache_explorer_root(&mut session, &directory, explorer_root("notes.md"));
        let refreshed = session
            .explorer_roots
            .get(&directory)
            .expect("expected refreshed explorer root");
        assert_eq!(refreshed.children[0].name, "notes.md");
    }

    #[test]
    fn rendered_document_cache_matches_only_the_active_path() {
        let path = PathBuf::from(r"C:\docs\guide.md");
        let mut session = AppSession::default();

        cache_rendered_document(&mut session, &path, rendered_document("# Guide", false));

        assert!(
            rendered_document_from_cache(&session, &path, true)
                .expect("expected cached document")
                .watching
        );
        assert!(
            rendered_document_from_cache(&session, Path::new(r"C:\docs\other.md"), true).is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    fn recent_path_matching_is_case_insensitive_on_windows() {
        assert!(super::same_recent_path(
            Path::new(r"C:\Docs\Plan.md"),
            Path::new(r"c:\docs\plan.md")
        ));
        assert!(super::same_recent_path(
            Path::new(r"\\?\UNC\server\share\Plan.md"),
            Path::new(r"\\server\share\plan.md")
        ));
    }

    #[test]
    fn explorer_payload_is_omitted_when_the_same_tree_is_already_sent() {
        let directory = PathBuf::from(r"C:\docs");
        let session = AppSession {
            last_sent_explorer_root: Some(directory.clone()),
            explorer_dirty: false,
            ..Default::default()
        };

        assert_eq!(
            session.last_sent_explorer_root.as_deref(),
            Some(directory.as_path())
        );
        assert!(!session.explorer_dirty);
    }

    fn saved_window_state(
        monitor_name: Option<&str>,
        monitor_index: Option<usize>,
        monitor_offset: StoredPosition,
        absolute_position: StoredPosition,
        outer_size: StoredSize,
    ) -> SavedWindowState {
        SavedWindowState {
            monitor_name: monitor_name.map(str::to_string),
            monitor_index,
            absolute_position,
            monitor_offset,
            outer_size,
            maximized: false,
        }
    }

    fn monitor(name: &str, x: i32, y: i32, width: u32, height: u32) -> MonitorSnapshot {
        MonitorSnapshot {
            name: Some(name.to_string()),
            position: StoredPosition { x, y },
            size: StoredSize { width, height },
        }
    }

    fn explorer_root(file_name: &str) -> ExplorerRoot {
        ExplorerRoot {
            name: String::from("docs"),
            path: String::from(r"C:\docs"),
            children: vec![ExplorerNode {
                name: file_name.to_string(),
                path: format!(r"C:\docs\{file_name}"),
                kind: ExplorerNodeKind::File,
                children: Vec::new(),
            }],
        }
    }

    fn rendered_document(html: &str, watching: bool) -> markdown::RenderedDocument {
        markdown::RenderedDocument {
            title: String::from("guide"),
            html: html.to_string(),
            source_name: String::from("guide.md"),
            source_path: String::from(r"C:\docs\guide.md"),
            watching,
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
