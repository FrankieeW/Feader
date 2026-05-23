//! Feader Tauri command surface and application bootstrap.

mod db;
mod feed_adapter;
mod models;

use std::fs;

use db::AppDatabase;
use models::{
    AddSourceRequest, Article, ArticleFilter, Source, SourceRefreshResult, UpdateSourceTitleRequest,
};
use tauri::Manager;

/// Return all configured sources.
#[tauri::command]
fn list_sources(database: tauri::State<'_, AppDatabase>) -> Result<Vec<Source>, String> {
    database.list_sources()
}

/// Add a feed source after validating that it can be parsed.
#[tauri::command]
async fn add_source(
    request: AddSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("Feed URL is required".to_string());
    }

    let feed = feed_adapter::fetch_feed(url).await?;
    let source = database.add_source(
        url,
        request
            .title
            .as_deref()
            .or(feed.title.as_deref())
            .filter(|title| !title.trim().is_empty()),
    )?;
    database.upsert_articles(source.id, feed.title.as_deref(), &feed.articles)?;
    database.get_source(source.id)
}

/// Rename a source.
#[tauri::command]
fn update_source_title(
    request: UpdateSourceTitleRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    database.update_source_title(request.source_id, &request.title)
}

/// Delete a source and all of its articles.
#[tauri::command]
fn delete_source(source_id: i64, database: tauri::State<'_, AppDatabase>) -> Result<(), String> {
    database.delete_source(source_id)
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

    let feed = match feed_adapter::fetch_feed(&source.url).await {
        Ok(feed) => feed,
        Err(error) => {
            database.record_source_error(source.id, &error)?;
            return Err(error);
        }
    };
    database.upsert_articles(source.id, feed.title.as_deref(), &feed.articles)?;
    database.list_articles(ArticleFilter {
        source_id: Some(source.id),
        unread_only: None,
        saved_only: None,
    })
}

/// Refresh all enabled sources and return per-source status.
#[tauri::command]
async fn refresh_all_sources(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<SourceRefreshResult>, String> {
    let sources = database.list_sources()?;
    let mut results = Vec::with_capacity(sources.len());

    for source in sources.into_iter().filter(|source| source.enabled) {
        if source.kind != "rss" {
            let error = format!("Source kind '{}' is not refreshable yet", source.kind);
            results.push(SourceRefreshResult {
                source_id: source.id,
                ok: false,
                article_count: 0,
                error: Some(error),
            });
            continue;
        }

        match feed_adapter::fetch_feed(&source.url).await {
            Ok(feed) => {
                let article_count = feed.articles.len();
                database.upsert_articles(source.id, feed.title.as_deref(), &feed.articles)?;
                results.push(SourceRefreshResult {
                    source_id: source.id,
                    ok: true,
                    article_count,
                    error: None,
                });
            }
            Err(error) => {
                database.record_source_error(source.id, &error)?;
                results.push(SourceRefreshResult {
                    source_id: source.id,
                    ok: false,
                    article_count: 0,
                    error: Some(error),
                });
            }
        }
    }

    Ok(results)
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

/// Set read state on every article, optionally scoped to one source.
#[tauri::command]
fn mark_articles_read(
    source_id: Option<i64>,
    read: bool,
    database: tauri::State<'_, AppDatabase>,
) -> Result<usize, String> {
    database.mark_articles_read(source_id, read)
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
            update_source_title,
            delete_source,
            refresh_source,
            refresh_all_sources,
            list_articles,
            mark_article_read,
            save_article,
            mark_articles_read
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
