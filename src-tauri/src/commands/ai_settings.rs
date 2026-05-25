//! AI provider settings commands.

use crate::db::AppDatabase;
use crate::models::{AiSettings, AiSettingsInput};

/// Return AI settings (API key masked).
#[tauri::command]
pub fn get_ai_settings(database: tauri::State<'_, AppDatabase>) -> Result<AiSettings, String> {
    database.get_ai_settings()
}

/// Save AI settings (blank api_key keeps the existing key).
#[tauri::command]
pub fn set_ai_settings(
    input: AiSettingsInput,
    database: tauri::State<'_, AppDatabase>,
) -> Result<AiSettings, String> {
    database.set_ai_settings(&input)
}
