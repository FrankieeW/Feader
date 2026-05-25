//! Plugin credential (cookie) commands.

use crate::db::AppDatabase;
use crate::models::{CredentialCheck, PluginCredential};
use crate::xpath_adapter;

/// Read a plugin credential (cookie masked).
#[tauri::command]
pub fn get_plugin_credential(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<PluginCredential, String> {
    database.get_plugin_credential(&plugin_id)
}

/// Save (or clear, when blank) a plugin-level cookie.
#[tauri::command]
pub fn set_plugin_credential(
    plugin_id: String,
    cookie: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<PluginCredential, String> {
    database.set_plugin_credential(&plugin_id, &cookie)?;
    database.get_plugin_credential(&plugin_id)
}

/// Probe whether the stored cookie is still valid for a plugin.
#[tauri::command]
pub async fn check_plugin_credential(
    plugin_id: String,
    check_url: String,
    logged_in_xpath: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<CredentialCheck, String> {
    let cookie = database.raw_plugin_cookie(&plugin_id)?;
    if cookie.is_none() {
        return Err("尚未设置该插件的 cookie".to_string());
    }
    let (ok, message) = xpath_adapter::check_login_state(
        check_url.trim(),
        cookie.as_deref(),
        logged_in_xpath.trim(),
    )
    .await?;
    database.record_plugin_credential_check(&plugin_id, ok, &message)?;
    let checked_at = database
        .get_plugin_credential(&plugin_id)?
        .last_checked_at
        .unwrap_or_default();
    Ok(CredentialCheck {
        ok,
        message,
        checked_at,
    })
}
