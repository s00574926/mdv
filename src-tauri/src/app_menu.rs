use anyhow::{Context, Result};
use std::path::PathBuf;
use tauri::{
    App, AppHandle, Manager, Wry,
    menu::{Menu, MenuBuilder, MenuItem, Submenu, SubmenuBuilder},
};
use tauri_plugin_dialog::DialogExt;

use crate::{state::AppState, workspace};

const MENU_FILE: &str = "file";
const MENU_NEW: &str = "file.new";
const MENU_OPEN: &str = "file.open";
const MENU_OPEN_FOLDER: &str = "file.open_folder";
const MENU_OPEN_RECENT: &str = "file.open_recent";
const MENU_QUIT: &str = "file.quit";
const MENU_RECENT_PREFIX: &str = "file.recent.";
const MENU_RECENT_EMPTY: &str = "file.recent.empty";

pub fn install(app: &App) -> Result<()> {
    refresh_menu(app.handle())?;

    let handle = app.handle().clone();
    handle.on_menu_event(move |app, event| {
        let _ = handle_menu_event(app, event.id().as_ref());
    });

    Ok(())
}

pub fn refresh_menu(app: &AppHandle) -> Result<()> {
    let recent_paths = app
        .state::<AppState>()
        .recent_paths()
        .unwrap_or_else(|_| Vec::new());
    let menu = build_menu(app, &recent_paths)?;
    app.set_menu(menu)?;

    Ok(())
}

fn build_menu(app: &AppHandle, recent_paths: &[PathBuf]) -> Result<Menu<Wry>> {
    let recent_submenu = build_recent_submenu(app, recent_paths)?;
    let file_menu = SubmenuBuilder::with_id(app, MENU_FILE, "File")
        .text(MENU_NEW, "New")
        .separator()
        .text(MENU_OPEN, "Open…")
        .text(MENU_OPEN_FOLDER, "Open Folder…")
        .item(&recent_submenu)
        .separator()
        .text(MENU_QUIT, "Quit")
        .build()?;

    Ok(MenuBuilder::new(app).item(&file_menu).build()?)
}

fn build_recent_submenu(app: &AppHandle, recent_paths: &[PathBuf]) -> Result<Submenu<Wry>> {
    let submenu = Submenu::with_id(app, MENU_OPEN_RECENT, "Open Recent", true)?;

    if recent_paths.is_empty() {
        let placeholder = MenuItem::with_id(
            app,
            MENU_RECENT_EMPTY,
            "No Recent Files",
            false,
            None::<&str>,
        )?;
        submenu.append(&placeholder)?;
        return Ok(submenu);
    }

    for (index, path) in recent_paths.iter().enumerate() {
        let label = format!("{} {}", index + 1, path.display());
        let item = MenuItem::with_id(
            app,
            format!("{MENU_RECENT_PREFIX}{index}"),
            label,
            true,
            None::<&str>,
        )?;
        submenu.append(&item)?;
    }

    Ok(submenu)
}

fn handle_menu_event(app: &AppHandle, menu_id: &str) -> Result<()> {
    match menu_id {
        MENU_NEW => {
            workspace::new_document(app, &app.state::<AppState>())?;
            workspace::emit_workspace_update(app)?;
        }
        MENU_OPEN => {
            if let Some(path) = app
                .dialog()
                .file()
                .add_filter("Markdown", &["md"])
                .blocking_pick_file()
            {
                let path = path
                    .into_path()
                    .context("The selected file could not be converted into a local path")?;
                workspace::open_markdown_path(app, &app.state::<AppState>(), &path)?;
                workspace::emit_workspace_update(app)?;
            }
        }
        MENU_OPEN_FOLDER => {
            if let Some(path) = app.dialog().file().blocking_pick_folder() {
                let path = path
                    .into_path()
                    .context("The selected folder could not be converted into a local path")?;
                workspace::open_folder_path(app, &app.state::<AppState>(), &path)?;
                workspace::emit_workspace_update(app)?;
            }
        }
        MENU_QUIT => app.exit(0),
        id if id.starts_with(MENU_RECENT_PREFIX) => {
            let index = id[MENU_RECENT_PREFIX.len()..]
                .parse::<usize>()
                .context("Invalid recent file menu id")?;
            workspace::open_recent_index(app, &app.state::<AppState>(), index)?;
            workspace::emit_workspace_update(app)?;
        }
        _ => {}
    }

    Ok(())
}
