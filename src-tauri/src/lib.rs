//! Feader Tauri command surface and application bootstrap.

mod db;
mod feed_adapter;
mod models;

use std::fs;

use db::AppDatabase;
use models::{AddSourceRequest, Article, ArticleFilter, Source};
use tauri::Manager;

/// Return all configured sources.
#[tauri::command]
fn list_sources(database: tauri::State<'_, AppDatabase>) -> Result<Vec<Source>, String> {
    database.list_sources()
}

/// Add a feed source and return the persisted record.
#[tauri::command]
fn add_source(
    request: AddSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("Feed URL is required".to_string());
    }

    database.add_source(url, request.title.as_deref())
}

/// Fetch a source and merge its latest articles into the local database.
#[tauri::command]
async fn refresh_source(
    source_id: i64,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<Article>, String> {
    let source = database.get_source(source_id)?;
    if source.kind != "rss" {
        return Err(format!(
            "Source kind '{}' is not refreshable yet",
            source.kind
        ));
    }

    let feed = feed_adapter::fetch_feed(&source.url).await?;
    database.upsert_articles(source.id, feed.title.as_deref(), &feed.articles)?;
    database.list_articles(ArticleFilter {
        source_id: Some(source.id),
        unread_only: None,
        saved_only: None,
    })
}

/// Return articles matching optional filters.
#[tauri::command]
fn list_articles(
    filter: Option<ArticleFilter>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<Article>, String> {
    database.list_articles(filter.unwrap_or_default())
}

/// Set the read state for an article.
#[tauri::command]
fn mark_article_read(
    article_id: i64,
    read: bool,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.mark_article_read(article_id, read)
}

/// Set the saved state for an article.
#[tauri::command]
fn save_article(
    article_id: i64,
    saved: bool,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.save_article(article_id, saved)
}

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
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sources,
            add_source,
            refresh_source,
            list_articles,
            mark_article_read,
            save_article
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
