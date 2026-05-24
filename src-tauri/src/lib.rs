//! Feader Tauri command surface and application bootstrap.

mod ai;
pub mod cli;
mod db;
mod feed_adapter;
mod json_adapter;
mod models;
mod plugin_registry;
mod xpath_adapter;

use std::fs;

use db::AppDatabase;
use hex::FromHex;
use models::{
    AddPluginMarketRequest, AddRssHubInstanceRequest, AddRssHubSourceRequest, AddSourceRequest,
    AddXPathSourceRequest, AiSettings, AiSettingsInput, Article, ArticleFilter, AutoRefreshConfig,
    CreateWalletLoginChallengeRequest, CredentialCheck, InstallPluginFromMarketRequest,
    InstallPluginFromUrlRequest, MarketplacePluginPack, PluginCredential, PluginMarket,
    PluginMarketTemplate, PreviewXPathSourceRequest, RefreshTickEvent, RegistryIndex,
    RssHubInstance, RssHubInstanceCheck, RssHubSettings, RssHubSourceConfig, Source,
    SourceRefreshResult, UpdateRssHubSourceInstanceRequest, UpdateSourceTitleRequest,
    UpdateXPathSourceRequest, VerifyWalletLoginRequest, WalletLoginChallenge, WalletSession,
    XPathPreview, XPathRulePack, XPathSelectors, XPathSourceSuggestion, SOURCE_KIND_JSON_API,
    SOURCE_KIND_RSS, SOURCE_KIND_RSSHUB, SOURCE_KIND_XPATH,
};
use siwe::{eip55, generate_nonce, Message, VerificationOpts};
use std::sync::Arc;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex as TokioMutex;

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

/// Read a plugin credential (cookie masked).
#[tauri::command]
fn get_plugin_credential(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<PluginCredential, String> {
    database.get_plugin_credential(&plugin_id)
}

/// Save (or clear, when blank) a plugin-level cookie.
#[tauri::command]
fn set_plugin_credential(
    plugin_id: String,
    cookie: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<PluginCredential, String> {
    database.set_plugin_credential(&plugin_id, &cookie)?;
    database.get_plugin_credential(&plugin_id)
}

/// Probe whether the stored cookie is still valid for a plugin.
#[tauri::command]
async fn check_plugin_credential(
    plugin_id: String,
    check_url: String,
    logged_in_xpath: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<CredentialCheck, String> {
    let cookie = database.raw_plugin_cookie(&plugin_id)?;
    if cookie.is_none() {
        return Err("尚未设置该插件的 cookie".to_string());
    }
    let (ok, message) = xpath_adapter::check_login_state(
        check_url.trim(),
        cookie.as_deref(),
        logged_in_xpath.trim(),
    )
    .await?;
    database.record_plugin_credential_check(&plugin_id, ok, &message)?;
    let checked_at = database
        .get_plugin_credential(&plugin_id)?
        .last_checked_at
        .unwrap_or_default();
    Ok(CredentialCheck {
        ok,
        message,
        checked_at,
    })
}

/// Return bundled static XPath plugin packs.
#[tauri::command]
fn list_xpath_plugin_packs() -> Vec<XPathRulePack> {
    plugin_registry::bundled_xpath_rule_packs()
}

const REGISTRY_CACHE_TTL_SECONDS: i64 = 86_400; // 24 hours
const RSSHUB_SETTINGS_KEY: &str = "rsshub_settings";
const RSSHUB_DEFAULT_INSTANCE_ID: &str = "rsshub-rssforever";
const PLUGIN_MARKETS_KEY: &str = "plugin_markets";

/// Fetch the plugin registry from the remote FeaderHub repository.
/// Results are cached locally with a 24-hour TTL.
#[tauri::command]
async fn fetch_registry_packs(
    force_refresh: Option<bool>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<MarketplacePluginPack>, String> {
    load_registry_packs(&database, force_refresh.unwrap_or(false)).await
}

async fn load_registry_packs(
    database: &AppDatabase,
    force_refresh: bool,
) -> Result<Vec<MarketplacePluginPack>, String> {
    let markets = load_plugin_markets(database)?;
    let installed_ids = installed_plugin_ids(database)?;
    let mut all_packs = Vec::new();
    let mut seen_sources = std::collections::HashSet::new();
    let mut remote_ids = std::collections::HashSet::new();

    for market in markets {
        let index = match load_market_index(database, &market, force_refresh).await {
            Ok(index) => index,
            Err(error) => {
                eprintln!("Failed to fetch market {}: {error}", market.id);
                continue;
            }
        };
        for entry in index.plugins {
            let source_key = marketplace_source_key(&market.id, &entry);
            if !seen_sources.insert(source_key) {
                continue;
            }
            remote_ids.insert(entry.id.clone());

            let cache_key = market_plugin_cache_key(&market.id, &entry);
            let pack_json = if force_refresh {
                None
            } else {
                database
                    .get_cache(&cache_key, REGISTRY_CACHE_TTL_SECONDS)?
                    .and_then(|cached| serde_json::from_str::<XPathRulePack>(&cached).ok())
            };

            let pack = match pack_json {
                Some(pack) => pack,
                None => {
                    match plugin_registry::fetch_remote_plugin_pack_from_market(&market, &entry)
                        .await
                    {
                        Ok(pack) => {
                            if let Ok(json) = serde_json::to_string(&pack) {
                                let _ = database.set_cache(&cache_key, &json);
                            }
                            pack
                        }
                        Err(error) => {
                            eprintln!("Failed to fetch plugin {}: {error}", entry.id);
                            continue;
                        }
                    }
                }
            };
            all_packs.push(MarketplacePluginPack {
                installed: installed_ids.contains(&pack.id),
                source_market_id: Some(market.id.clone()),
                source_market_name: Some(market.name.clone()),
                source_market_repository: Some(market.repository.clone()),
                pack,
            });
        }
    }

    for pack in database.list_installed_plugin_packs()? {
        if !remote_ids.contains(&pack.id) {
            all_packs.push(MarketplacePluginPack {
                installed: true,
                source_market_id: None,
                source_market_name: None,
                source_market_repository: None,
                pack,
            });
        }
    }

    Ok(all_packs)
}

async fn load_market_index(
    database: &AppDatabase,
    market: &PluginMarket,
    force_refresh: bool,
) -> Result<RegistryIndex, String> {
    if force_refresh {
        return fetch_and_cache_market_index(database, market).await;
    }
    let cache_key = format!("registry_index_{}", market.id);
    match database.get_cache(&cache_key, REGISTRY_CACHE_TTL_SECONDS)? {
        Some(cached) => match serde_json::from_str::<RegistryIndex>(&cached) {
            Ok(index)
                if registry_cache_is_usable(&index)
                    && registry_cache_includes_required_official_templates(market, &index) =>
            {
                Ok(index)
            }
            _ => fetch_and_cache_market_index(database, market).await,
        },
        None => fetch_and_cache_market_index(database, market).await,
    }
}

async fn fetch_and_cache_market_index(
    database: &AppDatabase,
    market: &PluginMarket,
) -> Result<RegistryIndex, String> {
    let index = plugin_registry::fetch_registry_index_from_market(market).await?;
    let json = serde_json::to_string(&index).map_err(|error| error.to_string())?;
    database.set_cache(&format!("registry_index_{}", market.id), &json)?;
    Ok(index)
}

fn marketplace_source_key(market_id: &str, entry: &models::RegistryPluginEntry) -> String {
    format!("{}:{}:{}", market_id, entry.id, entry.version)
}

fn market_plugin_cache_key(market_id: &str, entry: &models::RegistryPluginEntry) -> String {
    format!(
        "plugin_pack_{}_{}_{}_{}",
        market_id,
        entry.id,
        entry.version,
        entry.sha256.as_deref().unwrap_or("nosha")
    )
}

fn installed_plugin_ids(
    database: &AppDatabase,
) -> Result<std::collections::HashSet<String>, String> {
    Ok(database
        .list_installed_plugin_packs()?
        .into_iter()
        .map(|pack| pack.id)
        .collect())
}

fn load_plugin_markets(database: &AppDatabase) -> Result<Vec<PluginMarket>, String> {
    let mut markets = database
        .get_setting(PLUGIN_MARKETS_KEY)?
        .and_then(|json| serde_json::from_str::<Vec<PluginMarket>>(&json).ok())
        .unwrap_or_default();
    let official = plugin_registry::official_plugin_market();
    if !markets.iter().any(|market| market.id == official.id) {
        markets.insert(0, official);
    }
    Ok(markets)
}

fn save_plugin_markets(database: &AppDatabase, markets: &[PluginMarket]) -> Result<(), String> {
    let json = serde_json::to_string(markets).map_err(|error| error.to_string())?;
    database.set_setting(PLUGIN_MARKETS_KEY, &json)
}

#[tauri::command]
fn list_plugin_markets(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<PluginMarket>, String> {
    load_plugin_markets(&database)
}

#[tauri::command]
async fn add_plugin_market(
    request: AddPluginMarketRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<PluginMarket>, String> {
    let market = plugin_market_from_github(&request)?;
    plugin_registry::fetch_registry_index_from_market(&market).await?;
    let mut markets = load_plugin_markets(&database)?;
    if markets.iter().any(|item| item.id == market.id) {
        return Err("Plugin market already exists".to_string());
    }
    markets.push(market);
    save_plugin_markets(&database, &markets)?;
    load_plugin_markets(&database)
}

#[tauri::command]
async fn install_plugin_from_market(
    request: InstallPluginFromMarketRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathRulePack, String> {
    let markets = load_plugin_markets(&database)?;
    let market = markets
        .into_iter()
        .find(|market| market.id == request.market_id)
        .ok_or_else(|| "Plugin market not found".to_string())?;
    let index = load_market_index(&database, &market, false).await?;
    let entry = index
        .plugins
        .into_iter()
        .find(|entry| entry.id == request.plugin_id)
        .ok_or_else(|| "Plugin not found in market".to_string())?;
    let cache_key = market_plugin_cache_key(&market.id, &entry);
    let pack = match database.get_cache(&cache_key, REGISTRY_CACHE_TTL_SECONDS)? {
        Some(cached) => serde_json::from_str::<XPathRulePack>(&cached).ok(),
        None => None,
    };
    let pack = match pack {
        Some(pack) => pack,
        None => {
            let pack =
                plugin_registry::fetch_remote_plugin_pack_from_market(&market, &entry).await?;
            if let Ok(json) = serde_json::to_string(&pack) {
                let _ = database.set_cache(&cache_key, &json);
            }
            pack
        }
    };
    database.install_plugin_pack(&pack)?;
    Ok(pack)
}

#[tauri::command]
async fn install_plugin_from_url(
    request: InstallPluginFromUrlRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathRulePack, String> {
    let pack = plugin_registry::fetch_plugin_pack_from_url(&request.url).await?;
    database.install_plugin_pack(&pack)?;
    Ok(pack)
}

#[tauri::command]
fn uninstall_plugin(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.uninstall_plugin_pack(&plugin_id)
}

#[tauri::command]
fn list_installed_plugin_packs(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<XPathRulePack>, String> {
    let mut packs = plugin_registry::bundled_xpath_rule_packs();
    packs.extend(database.list_installed_plugin_packs()?);
    Ok(packs)
}

#[tauri::command]
fn create_plugin_market_template(
    app_handle: tauri::AppHandle,
) -> Result<PluginMarketTemplate, String> {
    let root = app_handle
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("plugin-market-template");
    fs::create_dir_all(root.join("registry")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("plugins/example")).map_err(|error| error.to_string())?;
    let index = r#"{
  "schemaVersion": "feader-registry/v1",
  "updatedAt": "2026-05-24T00:00:00Z",
  "plugins": [
    {
      "id": "example.generic.xpath",
      "name": "Example Generic XPath",
      "version": "0.1.0",
      "kind": "static-xpath-rule-pack",
      "manifest": "plugins/example/plugin.json",
      "sha256": "replace-with-sha256-of-xpath-rule-pack-json"
    }
  ]
}
"#;
    let manifest = r#"{
  "id": "example.generic.xpath",
  "name": "Example Generic XPath",
  "version": "0.1.0",
  "kind": "static-xpath-rule-pack",
  "feaderApiVersion": "xpath-rule-pack/v1",
  "description": "Starter plugin pack for a GitHub-hosted Feader marketplace.",
  "entry": "xpath-rule-pack.json",
  "authors": [{ "name": "Your Name" }]
}
"#;
    let pack = r#"{
  "schemaVersion": "xpath-rule-pack/v1",
  "id": "example.generic.xpath",
  "name": "Example Generic XPath",
  "version": "0.1.0",
  "description": "Replace these selectors with a real site rule.",
  "candidates": [
    {
      "id": "generic-article-list",
      "pageType": "article-list",
      "priority": 10,
      "detect": [],
      "promptRule": "Use the smallest repeated article item with one stable title link.",
      "selectors": {
        "items": "//article",
        "title": ".//h2/a",
        "url": ".//h2/a/@href",
        "summary": ".//p[1]",
        "publishedAt": ".//time/@datetime",
        "author": "",
        "content": ".",
        "image": ".//img/@src",
        "nextPage": ""
      }
    }
  ]
}
"#;
    let files = [
        ("registry/index.json", index),
        ("plugins/example/plugin.json", manifest),
        ("plugins/example/xpath-rule-pack.json", pack),
    ];
    for (path, content) in files {
        fs::write(root.join(path), content).map_err(|error| error.to_string())?;
    }
    Ok(PluginMarketTemplate {
        path: root.to_string_lossy().to_string(),
        files: vec![
            "registry/index.json".to_string(),
            "plugins/example/plugin.json".to_string(),
            "plugins/example/xpath-rule-pack.json".to_string(),
        ],
    })
}

fn plugin_market_from_github(request: &AddPluginMarketRequest) -> Result<PluginMarket, String> {
    let branch = request.branch.as_deref().unwrap_or("main").trim();
    let branch = if branch.is_empty() { "main" } else { branch };
    let repo = request.repository.trim().trim_end_matches('/');
    let (owner, name) = parse_github_repo(repo)?;
    let id = format!(
        "github-{}-{}",
        owner.to_ascii_lowercase(),
        name.to_ascii_lowercase()
    );
    let repository = format!("https://github.com/{owner}/{name}");
    let market_name = request
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&name);
    Ok(PluginMarket {
        id,
        name: market_name.to_string(),
        repository,
        raw_base_url: format!("https://raw.githubusercontent.com/{owner}/{name}/{branch}"),
        branch: branch.to_string(),
        builtin: false,
    })
}

fn parse_github_repo(value: &str) -> Result<(String, String), String> {
    let without_git = value.trim_end_matches(".git");
    if let Some(rest) = without_git.strip_prefix("https://github.com/") {
        return parse_owner_repo(rest);
    }
    if let Some(rest) = without_git.strip_prefix("http://github.com/") {
        return parse_owner_repo(rest);
    }
    if let Some(rest) = without_git.strip_prefix("github.com/") {
        return parse_owner_repo(rest);
    }
    parse_owner_repo(without_git)
}

fn parse_owner_repo(value: &str) -> Result<(String, String), String> {
    let mut parts = value.split('/').filter(|part| !part.trim().is_empty());
    let owner = parts
        .next()
        .ok_or_else(|| "GitHub owner is required".to_string())?;
    let repo = parts
        .next()
        .ok_or_else(|| "GitHub repo is required".to_string())?;
    if owner.contains("..") || repo.contains("..") || parts.next().is_some() {
        return Err("Use a GitHub repository in owner/repo form".to_string());
    }
    Ok((owner.to_string(), repo.to_string()))
}

fn registry_cache_is_usable(index: &RegistryIndex) -> bool {
    index.schema_version == "feader-registry/v1"
        && index.plugins.iter().all(|entry| {
            entry.sha256.as_deref().map(str::trim).is_some_and(|value| {
                value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
            })
        })
}

fn registry_cache_includes_required_official_templates(
    market: &PluginMarket,
    index: &RegistryIndex,
) -> bool {
    if market.id != "official-feaderhub" {
        return true;
    }
    ["app-ui-theme", "source-list-view", "detail-view"]
        .iter()
        .all(|kind| index.plugins.iter().any(|entry| entry.kind == *kind))
}

fn builtin_rsshub_instances() -> Vec<RssHubInstance> {
    vec![
        RssHubInstance {
            id: "rsshub-app".to_string(),
            name: "RSSHub Official".to_string(),
            base_url: "https://rsshub.app".to_string(),
            maintainer: "DIYgod".to_string(),
            location: Some("US".to_string()),
            official: true,
            builtin: true,
        },
        RssHubInstance {
            id: RSSHUB_DEFAULT_INSTANCE_ID.to_string(),
            name: "RSSForever".to_string(),
            base_url: "https://rsshub.rssforever.com".to_string(),
            maintainer: "Stille".to_string(),
            location: Some("AE".to_string()),
            official: false,
            builtin: true,
        },
        RssHubInstance {
            id: "hub-slarker".to_string(),
            name: "Slarker".to_string(),
            base_url: "https://hub.slarker.me".to_string(),
            maintainer: "Slarker".to_string(),
            location: Some("US".to_string()),
            official: false,
            builtin: true,
        },
        RssHubInstance {
            id: "rsshub-pseudoyu".to_string(),
            name: "pseudoyu".to_string(),
            base_url: "https://rsshub.pseudoyu.com".to_string(),
            maintainer: "pseudoyu".to_string(),
            location: Some("FR".to_string()),
            official: false,
            builtin: true,
        },
        RssHubInstance {
            id: "rsshub-rss-tips".to_string(),
            name: "AboutRSS".to_string(),
            base_url: "https://rsshub.rss.tips".to_string(),
            maintainer: "AboutRSS".to_string(),
            location: Some("US".to_string()),
            official: false,
            builtin: true,
        },
        RssHubInstance {
            id: "rsshub-ktachibana".to_string(),
            name: "KTachibanaM".to_string(),
            base_url: "https://rsshub.ktachibana.party".to_string(),
            maintainer: "KTachibanaM".to_string(),
            location: Some("US".to_string()),
            official: false,
            builtin: true,
        },
        RssHubInstance {
            id: "rss-owo".to_string(),
            name: "rss.owo.nz".to_string(),
            base_url: "https://rss.owo.nz".to_string(),
            maintainer: "Vincent Yang".to_string(),
            location: Some("DE".to_string()),
            official: false,
            builtin: true,
        },
        RssHubInstance {
            id: "rsshub-wudifeixue".to_string(),
            name: "wudifeixue".to_string(),
            base_url: "https://rss.wudifeixue.com".to_string(),
            maintainer: "wudifeixue".to_string(),
            location: Some("CA".to_string()),
            official: false,
            builtin: true,
        },
    ]
}

fn load_rsshub_settings(database: &AppDatabase) -> Result<RssHubSettings, String> {
    let mut settings = database
        .get_setting(RSSHUB_SETTINGS_KEY)?
        .and_then(|json| serde_json::from_str::<RssHubSettings>(&json).ok())
        .unwrap_or_else(|| RssHubSettings {
            global_instance_id: RSSHUB_DEFAULT_INSTANCE_ID.to_string(),
            instances: Vec::new(),
        });

    let mut instances = builtin_rsshub_instances();
    for instance in settings
        .instances
        .drain(..)
        .filter(|instance| !instance.builtin)
    {
        if !instances.iter().any(|item| item.id == instance.id) {
            instances.push(instance);
        }
    }
    if !instances
        .iter()
        .any(|instance| instance.id == settings.global_instance_id)
    {
        settings.global_instance_id = RSSHUB_DEFAULT_INSTANCE_ID.to_string();
    }
    settings.instances = instances;
    Ok(settings)
}

fn save_rsshub_settings(database: &AppDatabase, settings: &RssHubSettings) -> Result<(), String> {
    let json = serde_json::to_string(settings).map_err(|error| error.to_string())?;
    database.set_setting(RSSHUB_SETTINGS_KEY, &json)
}

fn normalize_rsshub_base_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("RSSHub instance URL is required".to_string());
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err("RSSHub instance URL must start with http:// or https://".to_string());
    }
    Ok(trimmed.to_string())
}

fn normalize_rsshub_route(route: &str) -> Result<String, String> {
    let trimmed = route.trim();
    if trimmed.is_empty() {
        return Err("RSSHub route is required".to_string());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let parsed = url::Url::parse(trimmed).map_err(|error| error.to_string())?;
        let mut route = parsed.path().to_string();
        if let Some(query) = parsed.query() {
            route.push('?');
            route.push_str(query);
        }
        return normalize_rsshub_route(&route);
    }
    Ok(format!("/{}", trimmed.trim_start_matches('/')))
}

fn rsshub_instance_id_from_base(base_url: &str) -> String {
    base_url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn build_rsshub_url(instance: &RssHubInstance, route: &str) -> Result<String, String> {
    Ok(format!(
        "{}{}",
        normalize_rsshub_base_url(&instance.base_url)?,
        normalize_rsshub_route(route)?
    ))
}

fn resolve_rsshub_instance(
    database: &AppDatabase,
    instance_id: Option<&str>,
) -> Result<RssHubInstance, String> {
    let settings = load_rsshub_settings(database)?;
    let selected_id = instance_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&settings.global_instance_id);
    settings
        .instances
        .into_iter()
        .find(|instance| instance.id == selected_id)
        .ok_or_else(|| format!("RSSHub instance '{selected_id}' is not configured"))
}

/// Return configured RSSHub instances and the global default.
#[tauri::command]
fn get_rsshub_settings(database: tauri::State<'_, AppDatabase>) -> Result<RssHubSettings, String> {
    load_rsshub_settings(&database)
}

/// Set the global RSSHub instance used by sources without their own override.
#[tauri::command]
fn set_rsshub_global_instance(
    instance_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<RssHubSettings, String> {
    let mut settings = load_rsshub_settings(&database)?;
    if !settings
        .instances
        .iter()
        .any(|instance| instance.id == instance_id)
    {
        return Err("RSSHub instance is not configured".to_string());
    }
    settings.global_instance_id = instance_id;
    save_rsshub_settings(&database, &settings)?;
    load_rsshub_settings(&database)
}

/// Add a custom RSSHub instance to the selectable list.
#[tauri::command]
fn add_rsshub_instance(
    request: AddRssHubInstanceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<RssHubSettings, String> {
    let base_url = normalize_rsshub_base_url(&request.base_url)?;
    let id = rsshub_instance_id_from_base(&base_url);
    let mut settings = load_rsshub_settings(&database)?;
    if settings.instances.iter().any(|instance| instance.id == id) {
        return Err("RSSHub instance already exists".to_string());
    }
    let custom_name = request.name.trim();
    let name = if custom_name.is_empty() {
        base_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string()
    } else {
        custom_name.to_string()
    };
    settings.instances.push(RssHubInstance {
        id,
        name,
        base_url,
        maintainer: "Custom".to_string(),
        location: None,
        official: false,
        builtin: false,
    });
    save_rsshub_settings(&database, &settings)?;
    load_rsshub_settings(&database)
}

/// Probe an RSSHub instance health endpoint.
#[tauri::command]
async fn check_rsshub_instance(base_url: String) -> Result<RssHubInstanceCheck, String> {
    let base_url = normalize_rsshub_base_url(&base_url)?;
    let checked_url = format!("{base_url}/healthz");
    let response = reqwest::Client::new()
        .get(&checked_url)
        .header("user-agent", "Feader/0.1")
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    Ok(RssHubInstanceCheck {
        ok: status.is_success(),
        message: if status.is_success() {
            format!("RSSHub health check passed with status {status}")
        } else {
            format!("RSSHub health check returned status {status}")
        },
        checked_url,
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

/// Add an RSSHub route source after validating that the selected instance returns a feed.
#[tauri::command]
async fn add_rsshub_source(
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
fn update_rsshub_source_instance(
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
async fn preview_xpath_source(
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
async fn preview_json_api_source(
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
async fn update_xpath_source(
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

/// Fill `selectors.cookie` with the plugin-level cookie when the source has no override.
/// Resolve the cookie for a JSON API feed: source override first, then plugin credential fallback.
fn resolve_json_cookie(database: &AppDatabase, selectors: &XPathSelectors) -> Option<String> {
    let source_cookie = selectors.cookie.as_deref().filter(|c| !c.trim().is_empty());
    if source_cookie.is_some() {
        return source_cookie.map(|c| c.to_string());
    }
    let plugin_id = selectors.plugin.as_ref().map(|p| p.id.as_str());
    plugin_id.and_then(|id| database.raw_plugin_cookie(id).ok().flatten())
}

fn apply_plugin_cookie(database: &AppDatabase, mut selectors: XPathSelectors) -> XPathSelectors {
    let plugin_id = selectors.plugin.as_ref().map(|plugin| plugin.id.clone());
    let plugin_cookie = plugin_id
        .as_deref()
        .and_then(|id| database.raw_plugin_cookie(id).ok().flatten());
    selectors.cookie =
        xpath_adapter::resolve_cookie(selectors.cookie.as_deref(), plugin_cookie.as_deref());
    selectors
}

// ── RefreshScheduler ────────────────────────────────────────────────

const DEFAULT_REFRESH_INTERVAL: i64 = 1800; // 30 minutes
const SCHEDULER_TICK_SECONDS: u64 = 30;

/// Managed state for the background auto-refresh loop.
#[derive(Clone)]
pub struct RefreshScheduler {
    enabled: Arc<TokioMutex<bool>>,
    global_interval: Arc<TokioMutex<i64>>,
    cancel_tx: Arc<TokioMutex<Option<tokio::sync::watch::Sender<()>>>>,
}

impl RefreshScheduler {
    fn new(enabled: bool, global_interval: i64) -> Self {
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
    async fn start(&self, app_handle: tauri::AppHandle) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsshub_route_normalization_accepts_paths_and_full_urls() {
        assert_eq!(
            normalize_rsshub_route("github/trending/daily/rust").unwrap(),
            "/github/trending/daily/rust"
        );
        assert_eq!(
            normalize_rsshub_route("https://rsshub.app/bilibili/user/video/123?limit=10").unwrap(),
            "/bilibili/user/video/123?limit=10"
        );
    }

    #[test]
    fn rsshub_settings_merge_builtin_and_custom_instances() {
        let database = AppDatabase::in_memory().expect("database opens");
        let mut settings = load_rsshub_settings(&database).expect("settings load");
        settings.instances.push(RssHubInstance {
            id: "custom-example".to_string(),
            name: "Custom Example".to_string(),
            base_url: "https://rsshub.example.com".to_string(),
            maintainer: "Custom".to_string(),
            location: None,
            official: false,
            builtin: false,
        });
        settings.global_instance_id = "custom-example".to_string();
        save_rsshub_settings(&database, &settings).expect("settings save");

        let reloaded = load_rsshub_settings(&database).expect("settings reload");
        assert_eq!(reloaded.global_instance_id, "custom-example");
        assert!(reloaded
            .instances
            .iter()
            .any(|instance| instance.id == RSSHUB_DEFAULT_INSTANCE_ID));
        assert!(reloaded
            .instances
            .iter()
            .any(|instance| instance.id == "custom-example" && !instance.builtin));
    }

    #[test]
    fn plugin_market_from_github_normalizes_repo_urls() {
        let market = plugin_market_from_github(&AddPluginMarketRequest {
            repository: "https://github.com/example/feader-market.git".to_string(),
            name: Some("Example Market".to_string()),
            branch: Some("stable".to_string()),
        })
        .expect("market parses");

        assert_eq!(market.id, "github-example-feader-market");
        assert_eq!(
            market.repository,
            "https://github.com/example/feader-market"
        );
        assert_eq!(
            market.raw_base_url,
            "https://raw.githubusercontent.com/example/feader-market/stable"
        );
        assert_eq!(market.name, "Example Market");
    }

    #[test]
    fn official_feaderhub_market_uses_official_trust() {
        let official = plugin_registry::official_plugin_market();
        assert_eq!(plugin_registry::market_trust(&official), "official");

        let community = plugin_market_from_github(&AddPluginMarketRequest {
            repository: "https://github.com/example/feader-market".to_string(),
            name: None,
            branch: None,
        })
        .expect("market parses");
        assert_eq!(plugin_registry::market_trust(&community), "community");
    }

    #[test]
    fn stale_official_registry_cache_without_view_templates_is_rejected() {
        let official = plugin_registry::official_plugin_market();
        let stale_index = RegistryIndex {
            schema_version: "feader-registry/v1".to_string(),
            updated_at: "2026-05-24T00:00:00Z".to_string(),
            plugins: vec![models::RegistryPluginEntry {
                id: "official.naixi-forum.xpath".to_string(),
                name: "Naixi Forum XPath Rules".to_string(),
                version: "0.1.0".to_string(),
                kind: models::PLUGIN_KIND_XPATH.to_string(),
                manifest: "plugins/official.naixi-forum.xpath/manifest.json".to_string(),
                sha256: Some("0".repeat(64)),
            }],
        };
        assert!(!registry_cache_includes_required_official_templates(
            &official,
            &stale_index
        ));
    }

    #[test]
    fn marketplace_source_key_distinguishes_same_plugin_from_different_markets() {
        let entry = models::RegistryPluginEntry {
            id: "official.cyberpunk-ui.view".to_string(),
            name: "Cyberpunk UI Theme".to_string(),
            version: "0.1.0".to_string(),
            kind: models::PLUGIN_KIND_APP_UI_THEME.to_string(),
            manifest: "plugins/official.cyberpunk-ui.view/manifest.json".to_string(),
            sha256: Some("0".repeat(64)),
        };

        assert_ne!(
            marketplace_source_key("official-feaderhub", &entry),
            marketplace_source_key("github-frankiee-feaderhub-fork", &entry)
        );
    }
}

// ── Auto-refresh Tauri commands ────────────────────────────────────

#[tauri::command]
async fn get_auto_refresh_config(
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
async fn set_global_refresh_interval(
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
async fn set_plugin_refresh_interval(
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
async fn set_source_refresh_interval(
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
async fn set_auto_refresh_enabled(
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
            list_sources,
            create_wallet_login_challenge,
            get_wallet_session,
            verify_wallet_login,
            disconnect_wallet_login,
            get_ai_settings,
            set_ai_settings,
            get_plugin_credential,
            set_plugin_credential,
            check_plugin_credential,
            list_plugin_markets,
            add_plugin_market,
            install_plugin_from_market,
            install_plugin_from_url,
            uninstall_plugin,
            list_installed_plugin_packs,
            create_plugin_market_template,
            get_rsshub_settings,
            set_rsshub_global_instance,
            add_rsshub_instance,
            check_rsshub_instance,
            list_xpath_plugin_packs,
            fetch_registry_packs,
            suggest_xpath_source,
            add_source,
            add_rsshub_source,
            update_rsshub_source_instance,
            preview_xpath_source,
            preview_json_api_source,
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
            mark_articles_read,
            get_auto_refresh_config,
            set_global_refresh_interval,
            set_plugin_refresh_interval,
            set_source_refresh_interval,
            set_auto_refresh_enabled
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
