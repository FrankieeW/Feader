//! Namespaced plugin configuration commands.

use chrono::Utc;
use serde_json::{json, Value};

use crate::db::AppDatabase;
use crate::error::Result;
use crate::models::{ImportPluginConfigRequest, SetPluginConfigRequest};

fn plugin_config_key(plugin_id: &str) -> Result<String> {
    let plugin_id = plugin_id.trim();
    if plugin_id.is_empty() {
        return Err("Plugin id is required".into());
    }
    if plugin_id.contains(':') || plugin_id.contains('/') || plugin_id.contains('\\') {
        return Err("Plugin id contains unsupported characters".into());
    }
    Ok(format!("plugin_config:{plugin_id}"))
}

#[tauri::command]
pub fn get_plugin_config(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Value> {
    let key = plugin_config_key(&plugin_id)?;
    Ok(database
        .get_setting(&key)?
        .and_then(|json| serde_json::from_str::<Value>(&json).ok())
        .unwrap_or_else(|| json!({})))
}

#[tauri::command]
pub fn set_plugin_config(
    request: SetPluginConfigRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Value> {
    let key = plugin_config_key(&request.plugin_id)?;
    let values = match request.values {
        Value::Object(_) => request.values,
        _ => return Err("Plugin config must be a JSON object".into()),
    };
    database.set_setting(&key, &serde_json::to_string(&values)?)?;
    Ok(values)
}

#[tauri::command]
pub fn export_plugin_config(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<String> {
    let values = get_plugin_config(plugin_id.clone(), database)?;
    Ok(serde_json::to_string_pretty(&json!({
        "schemaVersion": "feader-plugin-config/v1",
        "pluginId": plugin_id,
        "exportedAt": Utc::now().to_rfc3339(),
        "values": values,
    }))?)
}

#[tauri::command]
pub fn import_plugin_config(
    request: ImportPluginConfigRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Value> {
    let parsed: Value = serde_json::from_str(&request.json)?;
    let values = if parsed
        .get("schemaVersion")
        .and_then(Value::as_str)
        .is_some_and(|version| version == "feader-plugin-config/v1")
    {
        let plugin_id = parsed
            .get("pluginId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if plugin_id != request.plugin_id {
            return Err("Imported config belongs to a different plugin".into());
        }
        parsed.get("values").cloned().unwrap_or_else(|| json!({}))
    } else {
        parsed
    };
    set_plugin_config(
        SetPluginConfigRequest {
            plugin_id: request.plugin_id,
            values,
        },
        database,
    )
}
