//! Background auto-refresh scheduler and its configuration commands.

use std::sync::Arc;

use tauri::{Emitter, Manager};
use tokio::sync::Mutex as TokioMutex;

use crate::commands::sources::refresh_source_record;
use crate::db::AppDatabase;
use crate::models::{AutoRefreshConfig, RefreshTickEvent, Source};

pub(crate) const DEFAULT_REFRESH_INTERVAL: i64 = 1800; // 30 minutes
const SCHEDULER_TICK_SECONDS: u64 = 30;

/// Managed state for the background auto-refresh loop.
#[derive(Clone)]
pub struct RefreshScheduler {
    enabled: Arc<TokioMutex<bool>>,
    global_interval: Arc<TokioMutex<i64>>,
    cancel_tx: Arc<TokioMutex<Option<tokio::sync::watch::Sender<()>>>>,
}

impl RefreshScheduler {
    pub(crate) fn new(enabled: bool, global_interval: i64) -> Self {
        Self {
            enabled: Arc::new(TokioMutex::new(enabled)),
            global_interval: Arc::new(TokioMutex::new(global_interval)),
            cancel_tx: Arc::new(TokioMutex::new(None)),
        }
    }

    async fn is_enabled(&self) -> bool {
        *self.enabled.lock().await
    }

    async fn set_enabled(&self, value: bool) {
        *self.enabled.lock().await = value;
    }

    async fn get_global_interval(&self) -> i64 {
        *self.global_interval.lock().await
    }

    async fn set_global_interval(&self, value: i64) {
        *self.global_interval.lock().await = value;
    }

    /// Start the background refresh loop. No-op if already running.
    pub(crate) async fn start(&self, app_handle: tauri::AppHandle) {
        if self.cancel_tx.lock().await.is_some() {
            return; // already running
        }
        let (tx, mut rx) = tokio::sync::watch::channel(());
        *self.cancel_tx.lock().await = Some(tx);

        let enabled = self.enabled.clone();
        let global_interval = self.global_interval.clone();

        tauri::async_runtime::spawn(async move {
            let mut tick =
                tokio::time::interval(std::time::Duration::from_secs(SCHEDULER_TICK_SECONDS));
            loop {
                tokio::select! {
                    _ = tick.tick() => {}
                    _ = rx.changed() => {
                        break;
                    }
                }

                if !*enabled.lock().await {
                    continue;
                }

                let database = app_handle.state::<AppDatabase>();
                let sources = match database.list_sources() {
                    Ok(s) => s,
                    Err(_) => continue,
                };

                let global = *global_interval.lock().await;
                let now = chrono::Utc::now();
                let due: Vec<&Source> = sources
                    .iter()
                    .filter(|s| s.enabled && is_source_due(s, now, global, database.inner()))
                    .collect();

                let total_due = due.len();
                for (i, source) in due.iter().enumerate() {
                    if !*enabled.lock().await {
                        break;
                    }

                    let _ = app_handle.emit(
                        "refresh-tick",
                        RefreshTickEvent {
                            refreshing: true,
                            current_source_id: Some(source.id),
                            current_source_title: Some(source.title.clone()),
                            next_refresh_at: None,
                            sources_checked: total_due,
                            sources_refreshed: i,
                        },
                    );

                    let _ = refresh_source_record(database.inner(), source).await;

                    let _ = app_handle.emit(
                        "refresh-tick",
                        RefreshTickEvent {
                            refreshing: true,
                            current_source_id: Some(source.id),
                            current_source_title: Some(source.title.clone()),
                            next_refresh_at: None,
                            sources_checked: total_due,
                            sources_refreshed: i + 1,
                        },
                    );
                }

                // Compute approximate next refresh time.
                let next_at = now + chrono::Duration::seconds(std::cmp::max(global, 60));
                let _ = app_handle.emit(
                    "refresh-tick",
                    RefreshTickEvent {
                        refreshing: false,
                        current_source_id: None,
                        current_source_title: None,
                        next_refresh_at: Some(next_at.to_rfc3339()),
                        sources_checked: total_due,
                        sources_refreshed: total_due,
                    },
                );
            }
        });
    }

    /// Stop the background loop.
    async fn stop(&self) {
        if let Some(tx) = self.cancel_tx.lock().await.take() {
            let _ = tx.send(());
        }
    }

    /// Restart the loop (stop + start) to pick up new settings.
    async fn restart(&self, app_handle: tauri::AppHandle) {
        self.stop().await;
        self.start(app_handle).await;
    }
}

fn is_source_due(
    source: &Source,
    now: chrono::DateTime<chrono::Utc>,
    global: i64,
    database: &AppDatabase,
) -> bool {
    let effective = effective_refresh_interval(source, global, database);
    match &source.last_fetched_at {
        Some(ts) => {
            let last = chrono::DateTime::parse_from_rfc3339(ts)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::DateTime::UNIX_EPOCH);
            now.signed_duration_since(last).num_seconds() >= effective
        }
        None => true, // never fetched — refresh immediately
    }
}

fn effective_refresh_interval(source: &Source, global: i64, database: &AppDatabase) -> i64 {
    // 1. Source-level override
    if let Some(secs) = source.refresh_interval_seconds {
        if secs > 0 {
            return secs;
        }
    }
    // 2. Plugin-level override
    if let Some(plugin_id) = extract_plugin_id(source) {
        if let Ok(Some(secs)) = database.get_plugin_refresh_interval(&plugin_id) {
            if secs > 0 {
                return secs;
            }
        }
    }
    // 3. Global setting
    if global > 0 {
        return global;
    }
    // 4. Hardcoded fallback
    DEFAULT_REFRESH_INTERVAL
}

fn extract_plugin_id(source: &Source) -> Option<String> {
    let config = source.config_json.as_deref()?;
    let parsed: serde_json::Value = serde_json::from_str(config).ok()?;
    parsed
        .get("plugin")?
        .get("id")?
        .as_str()
        .map(|s| s.to_string())
}

#[tauri::command]
pub async fn get_auto_refresh_config(
    database: tauri::State<'_, AppDatabase>,
    scheduler: tauri::State<'_, RefreshScheduler>,
) -> Result<AutoRefreshConfig, String> {
    let enabled = scheduler.is_enabled().await;
    let global_interval_seconds = scheduler.get_global_interval().await;
    let plugin_overrides = database.list_plugin_refresh_overrides()?;
    Ok(AutoRefreshConfig {
        enabled,
        global_interval_seconds,
        plugin_overrides,
        next_refresh_at: None,
    })
}

#[tauri::command]
pub async fn set_global_refresh_interval(
    seconds: i64,
    database: tauri::State<'_, AppDatabase>,
    scheduler: tauri::State<'_, RefreshScheduler>,
    app_handle: tauri::AppHandle,
) -> Result<AutoRefreshConfig, String> {
    if seconds < 60 {
        return Err("Refresh interval must be at least 60 seconds".to_string());
    }
    database.set_setting("global_refresh_interval", &seconds.to_string())?;
    scheduler.set_global_interval(seconds).await;
    scheduler.restart(app_handle).await;
    get_auto_refresh_config(database, scheduler).await
}

#[tauri::command]
pub async fn set_plugin_refresh_interval(
    plugin_id: String,
    seconds: i64,
    database: tauri::State<'_, AppDatabase>,
    scheduler: tauri::State<'_, RefreshScheduler>,
    app_handle: tauri::AppHandle,
) -> Result<AutoRefreshConfig, String> {
    if seconds < 60 {
        return Err("Refresh interval must be at least 60 seconds".to_string());
    }
    database.set_plugin_refresh_interval(&plugin_id, seconds)?;
    scheduler.restart(app_handle).await;
    get_auto_refresh_config(database, scheduler).await
}

#[tauri::command]
pub async fn set_source_refresh_interval(
    source_id: i64,
    seconds: Option<i64>,
    database: tauri::State<'_, AppDatabase>,
    scheduler: tauri::State<'_, RefreshScheduler>,
    app_handle: tauri::AppHandle,
) -> Result<AutoRefreshConfig, String> {
    if let Some(secs) = seconds {
        if secs < 60 {
            return Err("Refresh interval must be at least 60 seconds".to_string());
        }
    }
    database.set_source_refresh_interval(source_id, seconds)?;
    scheduler.restart(app_handle).await;
    get_auto_refresh_config(database, scheduler).await
}

#[tauri::command]
pub async fn set_auto_refresh_enabled(
    enabled: bool,
    database: tauri::State<'_, AppDatabase>,
    scheduler: tauri::State<'_, RefreshScheduler>,
    app_handle: tauri::AppHandle,
) -> Result<AutoRefreshConfig, String> {
    database.set_setting(
        "auto_refresh_enabled",
        if enabled { "true" } else { "false" },
    )?;
    scheduler.set_enabled(enabled).await;
    if enabled {
        scheduler.start(app_handle.clone()).await;
    } else {
        scheduler.stop().await;
    }
    get_auto_refresh_config(database, scheduler).await
}
