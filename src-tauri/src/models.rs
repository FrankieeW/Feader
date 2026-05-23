//! Shared data shapes exposed through Tauri commands.

use serde::{Deserialize, Serialize};

/// A readable source that can produce articles.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub url: String,
    pub config_json: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub article_count: i64,
    pub unread_count: i64,
}

/// A normalized article emitted by RSS, XPath, or script adapters.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub id: i64,
    pub source_id: i64,
    pub source_title: String,
    pub external_id: Option<String>,
    pub title: String,
    pub url: String,
    pub canonical_url: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub image_url: Option<String>,
    pub tags_json: Option<String>,
    pub read: bool,
    pub saved: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Request body for adding a feed source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSourceRequest {
    pub url: String,
    pub title: Option<String>,
}

/// Request body for renaming a source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSourceTitleRequest {
    pub source_id: i64,
    pub title: String,
}

/// Result for one source refresh attempt.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRefreshResult {
    pub source_id: i64,
    pub ok: bool,
    pub article_count: usize,
    pub error: Option<String>,
}

/// Optional article list filters.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArticleFilter {
    pub source_id: Option<i64>,
    pub unread_only: Option<bool>,
    pub saved_only: Option<bool>,
}

/// An article parsed from an upstream adapter before database persistence.
#[derive(Debug, Clone)]
pub struct ParsedArticle {
    pub external_id: Option<String>,
    pub title: String,
    pub url: String,
    pub canonical_url: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub image_url: Option<String>,
    pub tags_json: Option<String>,
}

/// A parsed feed document ready to merge into the database.
#[derive(Debug, Clone)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub articles: Vec<ParsedArticle>,
}
