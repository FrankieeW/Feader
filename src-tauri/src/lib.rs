//! Feader Tauri command surface and application bootstrap.

mod ai;
pub mod cli;
mod db;
mod feed_adapter;
mod models;
mod plugin_registry;
mod json_adapter;
mod xpath_adapter;

use std::fs;

use db::AppDatabase;
use hex::FromHex;
use models::{
    AddSourceRequest, AddXPathSourceRequest, AiSettings, AiSettingsInput, Article, ArticleFilter,
    CreateWalletLoginChallengeRequest, PreviewXPathSourceRequest, RegistryIndex, Source,
    SourceRefreshResult, UpdateSourceTitleRequest, UpdateXPathSourceRequest,
    VerifyWalletLoginRequest, WalletLoginChallenge, WalletSession, XPathPreview, XPathRulePack,
    XPathSelectors, XPathSourceSuggestion,
};
use siwe::{eip55, generate_nonce, Message, VerificationOpts};
use tauri::Manager;

/// Return all configured sources.
#[tauri::command]
fn list_sources(database: tauri::State<'_, AppDatabase>) -> Result<Vec<Source>, String> {
    database.list_sources()
}

/// Create a single-use SIWE challenge for local wallet login.
#[tauri::command]
fn create_wallet_login_challenge(
    request: CreateWalletLoginChallengeRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<WalletLoginChallenge, String> {
    database.create_wallet_login_challenge(&request.domain, &request.uri, &generate_nonce())
}

/// Return the current verified wallet session.
#[tauri::command]
fn get_wallet_session(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Option<WalletSession>, String> {
    database.current_wallet_session()
}

/// Verify a signed SIWE login message and persist the local wallet session.
#[tauri::command]
async fn verify_wallet_login(
    request: VerifyWalletLoginRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<WalletSession, String> {
    let message: Message = request
        .message
        .parse()
        .map_err(|error| format!("Invalid SIWE message: {error}"))?;
    let signature = decode_signature(&request.signature)?;
    let verification_opts = VerificationOpts {
        domain: Some(message.domain.clone()),
        nonce: Some(message.nonce.clone()),
        ..Default::default()
    };

    message
        .verify(&signature, &verification_opts)
        .await
        .map_err(|error| format!("Wallet signature verification failed: {error}"))?;

    database.consume_wallet_login_challenge(
        &message.nonce,
        &message.domain.to_string(),
        &message.uri.to_string(),
    )?;

    let address = eip55(&message.address);
    database.save_wallet_session(
        &address,
        message.chain_id,
        &request.message,
        &request.signature,
    )
}

/// Revoke the current local wallet session.
#[tauri::command]
fn disconnect_wallet_login(database: tauri::State<'_, AppDatabase>) -> Result<(), String> {
    database.disconnect_wallet_session()
}

/// Return AI settings (API key masked).
#[tauri::command]
fn get_ai_settings(database: tauri::State<'_, AppDatabase>) -> Result<AiSettings, String> {
    database.get_ai_settings()
}

/// Save AI settings (blank api_key keeps the existing key).
#[tauri::command]
fn set_ai_settings(
    input: AiSettingsInput,
    database: tauri::State<'_, AppDatabase>,
) -> Result<AiSettings, String> {
    database.set_ai_settings(&input)
}

/// Return bundled static XPath plugin packs.
#[tauri::command]
fn list_xpath_plugin_packs() -> Vec<XPathRulePack> {
    Vec::new()
}

const REGISTRY_CACHE_TTL_SECONDS: i64 = 86_400; // 24 hours

/// Fetch the plugin registry from the remote FeaderHub repository.
/// Results are cached locally with a 24-hour TTL.
#[tauri::command]
async fn fetch_registry_packs(
    force_refresh: Option<bool>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<XPathRulePack>, String> {
    load_registry_packs(&database, force_refresh.unwrap_or(false)).await
}

async fn load_registry_packs(
    database: &AppDatabase,
    force_refresh: bool,
) -> Result<Vec<XPathRulePack>, String> {
    let index = if force_refresh {
        fetch_and_cache_registry_index(database).await?
    } else {
        match database.get_cache("registry_index", REGISTRY_CACHE_TTL_SECONDS)? {
            Some(cached) => match serde_json::from_str::<RegistryIndex>(&cached) {
                Ok(index) if registry_cache_is_usable(&index) => index,
                _ => fetch_and_cache_registry_index(database).await?,
            },
            None => fetch_and_cache_registry_index(database).await?,
        }
    };

    let mut all_packs = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for entry in index.plugins {
        if !seen_ids.insert(entry.id.clone()) {
            continue;
        }

        let cache_key = format!(
            "plugin_pack_{}_{}_{}",
            entry.id,
            entry.version,
            entry.sha256.as_deref().unwrap_or("nosha")
        );
        let pack_json = if force_refresh {
            None
        } else {
            match database.get_cache(&cache_key, REGISTRY_CACHE_TTL_SECONDS)? {
                Some(cached) => serde_json::from_str::<XPathRulePack>(&cached).ok(),
                None => None,
            }
        };

        if let Some(pack) = pack_json {
            all_packs.push(pack);
        } else {
            match plugin_registry::fetch_remote_plugin_pack(&entry).await {
                Ok(pack) => {
                    if let Ok(json) = serde_json::to_string(&pack) {
                        let _ = database.set_cache(&cache_key, &json);
                    }
                    all_packs.push(pack);
                }
                Err(error) => {
                    eprintln!("Failed to fetch plugin {}: {error}", entry.id);
                }
            }
        }
    }

    Ok(all_packs)
}

async fn fetch_and_cache_registry_index(database: &AppDatabase) -> Result<RegistryIndex, String> {
    let index = plugin_registry::fetch_registry_index().await?;
    let json = serde_json::to_string(&index).map_err(|error| error.to_string())?;
    database.set_cache("registry_index", &json)?;
    Ok(index)
}

fn registry_cache_is_usable(index: &RegistryIndex) -> bool {
    index.schema_version == "feader-registry/v1"
        && index.plugins.iter().all(|entry| {
            entry.sha256.as_deref().map(str::trim).is_some_and(|value| {
                value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
            })
        })
}

/// Suggest XPath selectors for a page using the configured AI provider.
#[tauri::command]
async fn suggest_xpath_source(
    url: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathSourceSuggestion, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("XPath source URL is required".to_string());
    }

    let settings = database.get_ai_settings()?;
    if !settings.enabled || !settings.api_key_set {
        return Err("AI is not configured".to_string());
    }
    let raw_key = database.raw_ai_api_key()?;
    let html = xpath_adapter::fetch_normalized(url).await?;
    if xpath_adapter::looks_like_interstitial_document(&html) {
        return Err(
            "Fetched page is an anti-bot or browser-check page, not the static page content."
                .to_string(),
        );
    }
    let rule_packs = plugin_registry::bundled_xpath_rule_packs();
    let mut suggestion =
        ai::suggest_xpath_selectors(&settings, &raw_key, &html, &rule_packs).await?;
    suggestion.selectors = xpath_adapter::select_best_xpath_selectors_for_preview_with_packs(
        url,
        &html,
        &suggestion.selectors,
        &rule_packs,
    );
    Ok(suggestion)
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
async fn preview_xpath_source(request: PreviewXPathSourceRequest) -> Result<XPathPreview, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("XPath source URL is required".to_string());
    }

    xpath_adapter::preview_xpath_source(url, &request.selectors).await
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

/// Add a JSON API feed source after validating it can extract articles.
#[tauri::command]
async fn add_json_api_source(
    request: AddXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let url = request.url.trim();
    let title = request.title.trim();
    if url.is_empty() {
        return Err("JSON feed URL is required".to_string());
    }
    if title.is_empty() {
        return Err("Source title is required".to_string());
    }

    let feed = json_adapter::fetch_json_feed(url, &request.selectors, None).await?;
    if feed.articles.is_empty() {
        return Err("JSON selectors did not extract any articles".to_string());
    }

    let source = database.add_json_api_source(url, title, &request.selectors)?;
    database.upsert_articles(source.id, Some(title), &feed.articles)?;
    database.get_source(source.id)
}

/// Update an XPath source after validating the new selectors against the same static page.
#[tauri::command]
async fn update_xpath_source(
    request: UpdateXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let source = database.get_source(request.source_id)?;
    if source.kind != "xpath" {
        return Err("Only XPath sources can update selector config".to_string());
    }

    let feed = xpath_adapter::fetch_xpath_source(&source.url, &request.selectors).await?;
    if feed.articles.is_empty() {
        return Err("XPath selectors did not extract any articles".to_string());
    }

    let source = database.update_xpath_source_config(source.id, &request.selectors)?;
    database.upsert_articles(source.id, Some(&source.title), &feed.articles)?;
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

/// Set or clear a source's category folder.
#[tauri::command]
fn set_source_category(
    source_id: i64,
    category: Option<String>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    database.set_source_category(source_id, category.as_deref())
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
        "json-api" => {
            let selectors = parse_xpath_selectors(source)?;
            // TODO: fall back to plugin_credentials table when per-source cookie is empty
            let cookie = selectors.cookie.clone().filter(|c| !c.trim().is_empty());
            json_adapter::fetch_json_feed(&source.url, &selectors, cookie.as_deref()).await
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

fn decode_signature(signature: &str) -> Result<Vec<u8>, String> {
    let signature = signature
        .trim()
        .strip_prefix("0x")
        .unwrap_or(signature.trim());
    let bytes = Vec::from_hex(signature).map_err(|error| error.to_string())?;
    if bytes.len() != 65 {
        return Err("Wallet signature must be 65 bytes".to_string());
    }
    Ok(bytes)
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
            create_wallet_login_challenge,
            get_wallet_session,
            verify_wallet_login,
            disconnect_wallet_login,
            get_ai_settings,
            set_ai_settings,
            list_xpath_plugin_packs,
            fetch_registry_packs,
            suggest_xpath_source,
            add_source,
            preview_xpath_source,
            add_xpath_source,
            add_json_api_source,
            update_xpath_source,
            update_source_title,
            set_source_category,
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
