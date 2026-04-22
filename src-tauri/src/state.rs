use anyhow::{Context, Result};
use notify::RecommendedWatcher;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{AppHandle, Manager};

const MAX_RECENT_FILES: usize = 10;
const RECENT_FILES_NAME: &str = "recent-files.json";

#[derive(Default)]
pub struct AppSession {
    pub current_path: Option<PathBuf>,
    pub current_directory: Option<PathBuf>,
    pub current_document_watcher: Option<RecommendedWatcher>,
    pub explorer_watcher: Option<RecommendedWatcher>,
    pub recent_paths: Vec<PathBuf>,
}

pub struct AppState {
    pub session: Mutex<AppSession>,
    recent_store_path: PathBuf,
}

impl AppState {
    pub fn load(app: &AppHandle) -> Result<Self> {
        let recent_store_path = recent_store_path(app)?;
        let recent_paths = load_recent_paths(&recent_store_path);

        Ok(Self {
            session: Mutex::new(AppSession {
                recent_paths,
                ..Default::default()
            }),
            recent_store_path,
        })
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

            session.recent_paths.retain(|candidate| candidate != path);
            session.recent_paths.insert(0, path.to_path_buf());
            session.recent_paths.truncate(MAX_RECENT_FILES);
        }

        self.persist_recent_paths()
    }
}

#[derive(Serialize, Deserialize)]
struct RecentFilesStore {
    recent_paths: Vec<String>,
}

fn recent_store_path(app: &AppHandle) -> Result<PathBuf> {
    let app_data_dir = app
        .path()
        .app_local_data_dir()
        .context("Failed to resolve the app local data directory")?;
    fs::create_dir_all(&app_data_dir)
        .with_context(|| format!("Failed to create {}", app_data_dir.display()))?;

    Ok(app_data_dir.join(RECENT_FILES_NAME))
}

fn load_recent_paths(path: &Path) -> Vec<PathBuf> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let Ok(store) = serde_json::from_str::<RecentFilesStore>(&contents) else {
        return Vec::new();
    };

    store
        .recent_paths
        .into_iter()
        .map(PathBuf::from)
        .filter(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("md"))
        })
        .take(MAX_RECENT_FILES)
        .collect()
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
