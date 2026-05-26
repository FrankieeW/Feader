//! Feader Tauri command surface and application bootstrap.

mod ai;
pub mod cli;
mod commands;
mod db;
mod error;
mod feed_adapter;
mod json_adapter;
mod models;
mod plugin_registry;
mod refresh;
mod xpath_adapter;

use std::fs;

use db::AppDatabase;
use refresh::{RefreshScheduler, DEFAULT_REFRESH_INTERVAL};
use tauri::Manager;

pub(crate) use commands::sources::refresh_source_record;

/// Start the Feader Tauri application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(Box::<dyn std::error::Error>::from)?;
            fs::create_dir_all(&app_data_dir)?;
            let database = AppDatabase::open(&app_data_dir.join("feader.sqlite"))
                .map_err(Box::<dyn std::error::Error>::from)?;

            // Determine initial auto-refresh state from persisted settings.
            let auto_enabled = database
                .get_setting("auto_refresh_enabled")
                .ok()
                .flatten()
                .map(|v| v == "true")
                .unwrap_or(true);
            let global_interval = database
                .get_setting("global_refresh_interval")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(DEFAULT_REFRESH_INTERVAL);
            let scheduler = RefreshScheduler::new(auto_enabled, global_interval);

            app.manage(database);
            app.manage(scheduler);

            if auto_enabled {
                let handle = app.handle().clone();
                let sched = (*app.state::<RefreshScheduler>()).clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    sched.start(handle).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sources::list_sources,
            commands::wallet::create_wallet_login_challenge,
            commands::wallet::get_wallet_session,
            commands::wallet::verify_wallet_login,
            commands::wallet::disconnect_wallet_login,
            commands::ai_settings::get_ai_settings,
            commands::ai_settings::set_ai_settings,
            commands::credentials::get_plugin_credential,
            commands::credentials::set_plugin_credential,
            commands::credentials::check_plugin_credential,
            commands::marketplace::list_plugin_markets,
            commands::marketplace::add_plugin_market,
            commands::marketplace::install_plugin_from_market,
            commands::marketplace::install_plugin_from_url,
            commands::marketplace::uninstall_plugin,
            commands::marketplace::list_installed_plugin_packs,
            commands::marketplace::create_plugin_market_template,
            commands::plugin_config::get_plugin_config,
            commands::plugin_config::set_plugin_config,
            commands::plugin_config::export_plugin_config,
            commands::plugin_config::import_plugin_config,
            commands::rsshub::get_rsshub_settings,
            commands::rsshub::set_rsshub_global_instance,
            commands::rsshub::add_rsshub_instance,
            commands::rsshub::check_rsshub_instance,
            commands::marketplace::list_xpath_plugin_packs,
            commands::marketplace::fetch_registry_packs,
            commands::sources::suggest_xpath_source,
            commands::sources::add_source,
            commands::sources::add_rsshub_source,
            commands::sources::update_rsshub_source_instance,
            commands::sources::preview_xpath_source,
            commands::sources::preview_json_api_source,
            commands::sources::add_xpath_source,
            commands::sources::add_json_api_source,
            commands::sources::update_xpath_source,
            commands::sources::update_source_title,
            commands::sources::set_source_category,
            commands::sources::delete_source,
            commands::sources::refresh_source,
            commands::sources::refresh_all_sources,
            commands::articles::list_articles,
            commands::articles::mark_article_read,
            commands::articles::save_article,
            commands::articles::mark_articles_read,
            refresh::get_auto_refresh_config,
            refresh::set_global_refresh_interval,
            refresh::set_plugin_refresh_interval,
            refresh::set_source_refresh_interval,
            refresh::set_auto_refresh_enabled,
            commands::settings_layout::get_settings_layout,
            commands::settings_layout::set_settings_layout,
            commands::settings_layout::delete_settings_layout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
