//! Settings card layout persistence commands.

use crate::db::AppDatabase;
use crate::error::Result;
use crate::models::SettingsCardLayout;

#[tauri::command]
pub fn get_settings_layout(database: tauri::State<'_, AppDatabase>) -> Result<SettingsCardLayout> {
    Ok(database.get_settings_layout()?)
}

#[tauri::command]
pub fn set_settings_layout(
    layout: SettingsCardLayout,
    database: tauri::State<'_, AppDatabase>,
) -> Result<()> {
    Ok(database.set_settings_layout(&layout)?)
}

#[tauri::command]
pub fn delete_settings_layout(database: tauri::State<'_, AppDatabase>) -> Result<()> {
    Ok(database.delete_setting("settings_layout:v1")?)
}
