use anyhow::Result;
use tauri::{App, AppHandle};

pub fn install(_app: &App) -> Result<()> {
    Ok(())
}

pub fn refresh_menu(_app: &AppHandle) -> Result<()> {
    Ok(())
}
