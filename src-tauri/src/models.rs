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
    pub category: Option<String>,
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

/// Request body for adding an RSS or Atom source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSourceRequest {
    pub url: String,
    pub title: Option<String>,
}

/// Request body for creating a SIWE wallet login challenge.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWalletLoginChallengeRequest {
    pub domain: String,
    pub uri: String,
}

/// Single-use SIWE challenge returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletLoginChallenge {
    pub nonce: String,
    pub domain: String,
    pub uri: String,
    pub statement: String,
    pub issued_at: String,
    pub expires_at: String,
}

/// Request body for verifying a signed SIWE login message.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyWalletLoginRequest {
    pub message: String,
    pub signature: String,
}

/// Locally verified wallet account session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletSession {
    pub address: String,
    pub chain_id: u64,
    pub signed_in_at: String,
    pub expires_at: Option<String>,
}

/// AI provider configuration exposed to the renderer (never carries a literal secret).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    pub api_key_set: bool,
    pub api_key_reference: Option<String>,
    pub updated_at: String,
}

/// AI settings input from the renderer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsInput {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    pub api_key: Option<String>,
}

/// Return the variable name if `value` is an env reference like `$NAME` or `${NAME}`.
pub fn env_reference_name(value: &str) -> Option<String> {
    let rest = value.trim().strip_prefix('$')?;
    let name = match rest.strip_prefix('{') {
        Some(inner) => inner.strip_suffix('}')?,
        None => rest,
    };
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic());
    if first_ok && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) {
        Some(name.to_string())
    } else {
        None
    }
}

/// True when `value` is an env reference (`$NAME` / `${NAME}`).
pub fn is_env_reference(value: &str) -> bool {
    env_reference_name(value).is_some()
}

/// XPath selectors for a static HTML/XML source.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathSelectors {
    pub items: String,
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
    pub published_at: Option<String>,
    pub author: Option<String>,
    pub content: Option<String>,
    pub image: Option<String>,
    pub next_page: Option<String>,
}

/// AI-suggested XPath source draft.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathSourceSuggestion {
    pub title: Option<String>,
    pub selectors: XPathSelectors,
}

/// Preview diagnostics for a single XPath selector field.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathFieldDiagnostic {
    pub field: String,
    pub label: String,
    pub required: bool,
    pub expression: Option<String>,
    pub status: String,
    pub message: String,
    pub sample: Option<String>,
}

/// Preview result for a declarative XPath source.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathPreview {
    pub articles: Vec<ParsedArticle>,
    pub diagnostics: Vec<XPathFieldDiagnostic>,
    pub next_page_url: Option<String>,
}

/// Request body for previewing an XPath source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewXPathSourceRequest {
    pub url: String,
    pub selectors: XPathSelectors,
}

/// Request body for adding an XPath source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddXPathSourceRequest {
    pub url: String,
    pub title: String,
    pub selectors: XPathSelectors,
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
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
