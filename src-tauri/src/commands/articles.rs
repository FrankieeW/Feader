//! Article read/save/list commands.

use crate::db::AppDatabase;
use crate::models::{Article, ArticleFilter};

/// Return articles matching optional filters.
#[tauri::command]
pub fn list_articles(
    filter: Option<ArticleFilter>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<Article>, String> {
    database.list_articles(filter.unwrap_or_default())
}

/// Set the read state for an article.
#[tauri::command]
pub fn mark_article_read(
    article_id: i64,
    read: bool,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.mark_article_read(article_id, read)
}

/// Set the saved state for an article.
#[tauri::command]
pub fn save_article(
    article_id: i64,
    saved: bool,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.save_article(article_id, saved)
}

/// Set read state on every article, optionally scoped to one source.
#[tauri::command]
pub fn mark_articles_read(
    source_id: Option<i64>,
    read: bool,
    database: tauri::State<'_, AppDatabase>,
) -> Result<usize, String> {
    database.mark_articles_read(source_id, read)
}
