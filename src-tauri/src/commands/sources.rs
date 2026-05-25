//! Feed source commands: create, preview, update, delete, and refresh.

use crate::commands::rsshub::{build_rsshub_url, normalize_rsshub_route, resolve_rsshub_instance};
use crate::db::AppDatabase;
use crate::models::{
    AddRssHubSourceRequest, AddSourceRequest, AddXPathSourceRequest, Article, ArticleFilter,
    PreviewXPathSourceRequest, RssHubSourceConfig, Source, SourceRefreshResult,
    UpdateRssHubSourceInstanceRequest, UpdateSourceTitleRequest, UpdateXPathSourceRequest,
    XPathPreview, XPathSelectors, XPathSourceSuggestion, SOURCE_KIND_JSON_API, SOURCE_KIND_RSS,
    SOURCE_KIND_RSSHUB, SOURCE_KIND_XPATH,
};
use crate::{ai, feed_adapter, json_adapter, plugin_registry, xpath_adapter};

/// Return all configured sources.
#[tauri::command]
pub fn list_sources(database: tauri::State<'_, AppDatabase>) -> Result<Vec<Source>, String> {
    database.list_sources()
}

/// Suggest XPath selectors for a page using the configured AI provider.
#[tauri::command]
pub async fn suggest_xpath_source(
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
pub async fn add_source(
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

/// Add an RSSHub route source after validating that the selected instance returns a feed.
#[tauri::command]
pub async fn add_rsshub_source(
    request: AddRssHubSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let route = normalize_rsshub_route(&request.route)?;
    let instance = resolve_rsshub_instance(&database, request.instance_id.as_deref())?;
    let feed_url = build_rsshub_url(&instance, &route)?;
    let feed = feed_adapter::fetch_feed(&feed_url).await?;
    let config = RssHubSourceConfig {
        route: route.clone(),
        instance_id: request
            .instance_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.to_string()),
    };
    let source = database.add_rsshub_source(
        &route,
        request
            .title
            .as_deref()
            .or(feed.title.as_deref())
            .filter(|title| !title.trim().is_empty()),
        &config,
    )?;
    database.upsert_articles(source.id, feed.title.as_deref(), &feed.articles)?;
    database.get_source(source.id)
}

/// Update the RSSHub instance override for one source.
#[tauri::command]
pub fn update_rsshub_source_instance(
    request: UpdateRssHubSourceInstanceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let source = database.get_source(request.source_id)?;
    if source.kind != SOURCE_KIND_RSSHUB {
        return Err("RSSHub instance can only be changed for RSSHub sources".to_string());
    }
    let mut config = parse_rsshub_source_config(&source)?;
    if let Some(instance_id) = request
        .instance_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        resolve_rsshub_instance(&database, Some(instance_id))?;
        config.instance_id = Some(instance_id.to_string());
    } else {
        config.instance_id = None;
    }
    database.update_rsshub_source_config(source.id, &config)
}

/// Preview extracted articles for a declarative XPath source.
#[tauri::command]
pub async fn preview_xpath_source(
    request: PreviewXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathPreview, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("XPath source URL is required".to_string());
    }
    let selectors = apply_plugin_cookie(&database, request.selectors);
    xpath_adapter::preview_xpath_source(url, &selectors).await
}

/// Preview extracted articles for a JSON API feed source.
#[tauri::command]
pub async fn preview_json_api_source(
    request: PreviewXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathPreview, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("JSON feed URL is required".to_string());
    }
    let cookie = resolve_json_cookie(&database, &request.selectors);
    let feed = json_adapter::fetch_json_feed(url, &request.selectors, cookie.as_deref()).await?;
    Ok(XPathPreview {
        articles: feed.articles,
        diagnostics: Vec::new(),
        next_page_url: None,
    })
}

/// Add an XPath source after validating that selectors can extract articles.
#[tauri::command]
pub async fn add_xpath_source(
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

    let selectors = apply_plugin_cookie(&database, request.selectors.clone());
    let feed = xpath_adapter::fetch_xpath_source(url, &selectors).await?;
    if feed.articles.is_empty() {
        return Err("XPath selectors did not extract any articles".to_string());
    }

    let source = database.add_xpath_source(url, title, &request.selectors)?;
    database.upsert_articles(source.id, Some(title), &feed.articles)?;
    database.get_source(source.id)
}

/// Add a JSON API feed source after validating it can extract articles.
#[tauri::command]
pub async fn add_json_api_source(
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

    let cookie = resolve_json_cookie(&database, &request.selectors);
    let feed = json_adapter::fetch_json_feed(url, &request.selectors, cookie.as_deref()).await?;
    if feed.articles.is_empty() {
        return Err("JSON selectors did not extract any articles".to_string());
    }

    let source = database.add_json_api_source(url, title, &request.selectors)?;
    database.upsert_articles(source.id, Some(title), &feed.articles)?;
    database.get_source(source.id)
}

/// Update an XPath source after validating the new selectors against the same static page.
#[tauri::command]
pub async fn update_xpath_source(
    request: UpdateXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    let source = database.get_source(request.source_id)?;
    if source.kind != SOURCE_KIND_XPATH && source.kind != SOURCE_KIND_JSON_API {
        return Err("Selectors can only be updated for XPath and JSON API sources".to_string());
    }

    let is_json = source.kind == SOURCE_KIND_JSON_API;
    let feed = if is_json {
        let cookie = resolve_json_cookie(&database, &request.selectors);
        json_adapter::fetch_json_feed(&source.url, &request.selectors, cookie.as_deref()).await
    } else {
        let selectors = apply_plugin_cookie(&database, request.selectors.clone());
        xpath_adapter::fetch_xpath_source(&source.url, &selectors).await
    }?;
    if feed.articles.is_empty() {
        return Err("Selectors did not extract any articles".to_string());
    }

    let source = database.update_xpath_source_config(source.id, &request.selectors)?;
    database.upsert_articles(source.id, Some(&source.title), &feed.articles)?;
    database.get_source(source.id)
}

/// Rename a source.
#[tauri::command]
pub fn update_source_title(
    request: UpdateSourceTitleRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    database.update_source_title(request.source_id, &request.title)
}

/// Set or clear a source's category folder.
#[tauri::command]
pub fn set_source_category(
    source_id: i64,
    category: Option<String>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    database.set_source_category(source_id, category.as_deref())
}

/// Delete a source and all of its articles.
#[tauri::command]
pub fn delete_source(
    source_id: i64,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.delete_source(source_id)
}

/// Fetch a source and merge its latest articles into the local database.
#[tauri::command]
pub async fn refresh_source(
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
pub async fn refresh_all_sources(
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

/// Refresh one source by kind and merge the parsed articles into the database.
pub(crate) async fn refresh_source_record(
    database: &AppDatabase,
    source: &Source,
) -> Result<usize, String> {
    let feed = match source.kind.as_str() {
        SOURCE_KIND_RSS => feed_adapter::fetch_feed(&source.url).await,
        SOURCE_KIND_RSSHUB => {
            let config = parse_rsshub_source_config(source)?;
            let instance = resolve_rsshub_instance(database, config.instance_id.as_deref())?;
            let url = build_rsshub_url(&instance, &config.route)?;
            feed_adapter::fetch_feed(&url).await
        }
        SOURCE_KIND_XPATH => {
            let selectors = apply_plugin_cookie(database, parse_xpath_selectors(source)?);
            xpath_adapter::fetch_xpath_source(&source.url, &selectors).await
        }
        SOURCE_KIND_JSON_API => {
            let selectors = parse_xpath_selectors(source)?;
            let cookie = resolve_json_cookie(database, &selectors);
            json_adapter::fetch_json_feed(&source.url, &selectors, cookie.as_deref()).await
        }
        kind => Err(format!("Source kind '{kind}' is not refreshable yet")),
    };

    match feed {
        Ok(feed) => {
            let article_count = feed.articles.len();
            let source_title = (source.kind == SOURCE_KIND_RSS
                || source.kind == SOURCE_KIND_RSSHUB)
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

fn parse_rsshub_source_config(source: &Source) -> Result<RssHubSourceConfig, String> {
    let config = source
        .config_json
        .as_deref()
        .ok_or_else(|| format!("RSSHub source '{}' has no route config", source.title))?;
    serde_json::from_str(config).map_err(|error| error.to_string())
}

fn parse_xpath_selectors(source: &Source) -> Result<XPathSelectors, String> {
    let config = source
        .config_json
        .as_deref()
        .ok_or_else(|| format!("XPath source '{}' has no selector config", source.title))?;
    serde_json::from_str(config).map_err(|error| error.to_string())
}

/// Resolve the cookie for a JSON API feed: source override first, then plugin credential fallback.
fn resolve_json_cookie(database: &AppDatabase, selectors: &XPathSelectors) -> Option<String> {
    let source_cookie = selectors.cookie.as_deref().filter(|c| !c.trim().is_empty());
    if source_cookie.is_some() {
        return source_cookie.map(|c| c.to_string());
    }
    let plugin_id = selectors.plugin.as_ref().map(|p| p.id.as_str());
    plugin_id.and_then(|id| database.raw_plugin_cookie(id).ok().flatten())
}

/// Fill `selectors.cookie` with the plugin-level cookie when the source has no override.
fn apply_plugin_cookie(database: &AppDatabase, mut selectors: XPathSelectors) -> XPathSelectors {
    let plugin_id = selectors.plugin.as_ref().map(|plugin| plugin.id.clone());
    let plugin_cookie = plugin_id
        .as_deref()
        .and_then(|id| database.raw_plugin_cookie(id).ok().flatten());
    selectors.cookie =
        xpath_adapter::resolve_cookie(selectors.cookie.as_deref(), plugin_cookie.as_deref());
    selectors
}
