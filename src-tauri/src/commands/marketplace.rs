//! Plugin marketplace and registry commands.

use std::collections::{HashMap, HashSet};
use std::fs;

use tauri::Manager;

use crate::db::AppDatabase;
use crate::models::{
    AddPluginMarketRequest, InstallPluginFromMarketRequest, InstallPluginFromUrlRequest,
    MarketplacePluginPack, PluginMarket, PluginMarketTemplate, RegistryIndex, RegistryPluginEntry,
    XPathRulePack, PLUGIN_KIND_APP_UI_THEME,
};
use crate::plugin_registry;

const REGISTRY_CACHE_TTL_SECONDS: i64 = 86_400; // 24 hours
const PLUGIN_MARKETS_KEY: &str = "plugin_markets";

/// Return bundled static XPath plugin packs.
#[tauri::command]
pub fn list_xpath_plugin_packs() -> Vec<XPathRulePack> {
    plugin_registry::bundled_xpath_rule_packs()
}

/// Fetch the plugin registry from the remote FeaderHub repository.
/// Results are cached locally with a 24-hour TTL.
#[tauri::command]
pub async fn fetch_registry_packs(
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
    let installed_versions = installed_plugin_versions(database)?;
    let mut all_packs = Vec::new();
    let mut seen_sources = HashSet::new();
    let mut remote_ids = HashSet::new();

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
                    .filter(|pack| cached_plugin_pack_is_usable(pack))
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
                installed: installed_versions.contains_key(&pack.id),
                installed_version: installed_versions.get(&pack.id).cloned(),
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
                installed_version: Some(pack.version.clone()),
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

fn marketplace_source_key(market_id: &str, entry: &RegistryPluginEntry) -> String {
    format!("{}:{}:{}", market_id, entry.id, entry.version)
}

fn market_plugin_cache_key(market_id: &str, entry: &RegistryPluginEntry) -> String {
    format!(
        "plugin_pack_{}_{}_{}_{}",
        market_id,
        entry.id,
        entry.version,
        entry.sha256.as_deref().unwrap_or("nosha")
    )
}

fn cached_plugin_pack_is_usable(pack: &XPathRulePack) -> bool {
    pack.kind != PLUGIN_KIND_APP_UI_THEME || pack.tokens.is_some()
}

fn installed_plugin_versions(database: &AppDatabase) -> Result<HashMap<String, String>, String> {
    Ok(database
        .list_installed_plugin_packs()?
        .into_iter()
        .map(|pack| (pack.id, pack.version))
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
pub fn list_plugin_markets(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<PluginMarket>, String> {
    load_plugin_markets(&database)
}

#[tauri::command]
pub async fn add_plugin_market(
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
pub async fn install_plugin_from_market(
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
        Some(cached) => serde_json::from_str::<XPathRulePack>(&cached)
            .ok()
            .filter(|pack| cached_plugin_pack_is_usable(pack)),
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
pub async fn install_plugin_from_url(
    request: InstallPluginFromUrlRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathRulePack, String> {
    let pack = plugin_registry::fetch_plugin_pack_from_url(&request.url).await?;
    database.install_plugin_pack(&pack)?;
    Ok(pack)
}

#[tauri::command]
pub fn uninstall_plugin(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<(), String> {
    database.uninstall_plugin_pack(&plugin_id)
}

#[tauri::command]
pub fn list_installed_plugin_packs(
    database: tauri::State<'_, AppDatabase>,
) -> Result<Vec<XPathRulePack>, String> {
    let mut packs = plugin_registry::bundled_xpath_rule_packs();
    packs.extend(database.list_installed_plugin_packs()?);
    Ok(packs)
}

#[tauri::command]
pub fn create_plugin_market_template(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;

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
            plugins: vec![RegistryPluginEntry {
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
        let entry = RegistryPluginEntry {
            id: "official.cyberpunk-ui.view".to_string(),
            name: "Cyberpunk UI Theme".to_string(),
            version: "0.1.0".to_string(),
            kind: PLUGIN_KIND_APP_UI_THEME.to_string(),
            manifest: "plugins/official.cyberpunk-ui.view/manifest.json".to_string(),
            sha256: Some("0".repeat(64)),
        };

        assert_ne!(
            marketplace_source_key("official-feaderhub", &entry),
            marketplace_source_key("github-frankiee-feaderhub-fork", &entry)
        );
    }
}
