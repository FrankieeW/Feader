//! Feader Tauri command surface and application bootstrap.

mod db;
mod feed_adapter;
mod models;
mod xpath_adapter;

use std::fs;

use db::AppDatabase;
use models::{
    AddSourceRequest, AddXPathSourceRequest, Article, ArticleFilter, ParsedArticle,
    PreviewXPathSourceRequest, Source, SourceRefreshResult, UpdateSourceTitleRequest,
    XPathSelectors,
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

/// Preview extracted articles for a declarative XPath source.
#[tauri::command]
async fn preview_xpath_source(
    request: PreviewXPathSourceRequest,
) -> Result<Vec<ParsedArticle>, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("XPath source URL is required".to_string());
    }

    let feed = xpath_adapter::fetch_xpath_source(url, &request.selectors).await?;
    Ok(feed.articles.into_iter().take(5).collect())
}

/// Add an XPath source after validating that selectors can extract articles.
#[tauri::command]
async fn add_xpath_source(
    request: AddXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let url = request.url.trim();
    let title = request.title.trim();
    if url.is_empty() {
        return Err("XPath source URL is required".to_string());
    }
    if title.is_empty() {
        return Err("XPath source title is required".to_string());
    }

    let feed = xpath_adapter::fetch_xpath_source(url, &request.selectors).await?;
    if feed.articles.is_empty() {
        return Err("XPath selectors did not extract any articles".to_string());
    }

    let source = database.add_xpath_source(url, title, &request.selectors)?;
    database.upsert_articles(source.id, Some(title), &feed.articles)?;
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
    refresh_source_record(&database, &source).await?;
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
        match refresh_source_record(&database, &source).await {
            Ok(article_count) => {
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

async fn refresh_source_record(database: &AppDatabase, source: &Source) -> Result<usize, String> {
    let feed = match source.kind.as_str() {
        "rss" => feed_adapter::fetch_feed(&source.url).await,
        "xpath" => {
            let selectors = parse_xpath_selectors(source)?;
            xpath_adapter::fetch_xpath_source(&source.url, &selectors).await
        }
        kind => Err(format!("Source kind '{kind}' is not refreshable yet")),
    };

    match feed {
        Ok(feed) => {
            let article_count = feed.articles.len();
            let source_title = (source.kind == "rss")
                .then_some(feed.title.as_deref())
                .flatten();
            database.upsert_articles(source.id, source_title, &feed.articles)?;
            Ok(article_count)
        }
        Err(error) => {
            database.record_source_error(source.id, &error)?;
            Err(error)
        }
    }
}

fn parse_xpath_selectors(source: &Source) -> Result<XPathSelectors, String> {
    let config = source
        .config_json
        .as_deref()
        .ok_or_else(|| format!("XPath source '{}' has no selector config", source.title))?;
    serde_json::from_str(config).map_err(|error| error.to_string())
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
            preview_xpath_source,
            add_xpath_source,
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
