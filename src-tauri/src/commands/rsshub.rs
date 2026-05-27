//! RSSHub instance management commands and route/url helpers.

use std::time::Duration;

use crate::db::AppDatabase;
use crate::models::{
    AddRssHubInstanceRequest, RssHubInstance, RssHubInstanceCheck, RssHubSettings,
    RssHubSourceConfig,
};
use crate::error::{FeaderError, Result};

const RSSHUB_SETTINGS_KEY: &str = "rsshub_settings";
const RSSHUB_DEFAULT_INSTANCE_ID: &str = "rsshub-rssforever";

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

fn load_rsshub_settings(database: &AppDatabase) -> Result<RssHubSettings> {
    let mut settings = database
        .get_setting(RSSHUB_SETTINGS_KEY)?
        .and_then(|json| serde_json::from_str::<RssHubSettings>(&json).ok())
        .unwrap_or_else(|| RssHubSettings {
            global_instance_id: RSSHUB_DEFAULT_INSTANCE_ID.to_string(),
            instances: Vec::new(),
            order: Vec::new(),
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
    if !settings.order.is_empty() {
        let order = settings.order.clone();
        settings.instances.sort_by_key(|instance| {
            order
                .iter()
                .position(|id| id == &instance.id)
                // instances not in the saved order sort to the end, keeping their relative order
                .unwrap_or(usize::MAX)
        });
    }
    Ok(settings)
}

fn save_rsshub_settings(database: &AppDatabase, settings: &RssHubSettings) -> Result<()> {
    let json = serde_json::to_string(settings)?;
    Ok(database.set_setting(RSSHUB_SETTINGS_KEY, &json)?)
}

fn normalize_rsshub_base_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("RSSHub instance URL is required".into());
    }
    if !(trimmed.starts_with("https://") || trimmed.starts_with("http://")) {
        return Err("RSSHub instance URL must start with http:// or https://".into());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn normalize_rsshub_route(route: &str) -> Result<String> {
    let trimmed = route.trim();
    if trimmed.is_empty() {
        return Err("RSSHub route is required".into());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let parsed = url::Url::parse(trimmed).map_err(|e| FeaderError::Message(e.to_string()))?;
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

pub(crate) fn build_rsshub_url(instance: &RssHubInstance, route: &str) -> Result<String> {
    Ok(format!(
        "{}{}",
        normalize_rsshub_base_url(&instance.base_url)?,
        normalize_rsshub_route(route)?
    ))
}

pub(crate) fn resolve_rsshub_instance(
    database: &AppDatabase,
    instance_id: Option<&str>,
) -> Result<RssHubInstance> {
    let settings = load_rsshub_settings(database)?;
    let selected_id = instance_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&settings.global_instance_id);
    settings
        .instances
        .into_iter()
        .find(|instance| instance.id == selected_id)
        .ok_or_else(|| format!("RSSHub instance '{selected_id}' is not configured").into())
}

/// Compose a single error message summarising every instance that failed during fallback.
pub(crate) fn format_fallback_error(failures: &[(String, String)]) -> String {
    let mut message = String::from("All RSSHub instances failed:");
    for (name, error) in failures {
        message.push_str(&format!("\n- {name}: {error}"));
    }
    message
}

/// Build the ordered list of instances to try for a source: primary first,
/// then the rest of the configured instances in priority order when fallback is allowed.
pub(crate) fn resolve_rsshub_candidates(
    database: &AppDatabase,
    config: &RssHubSourceConfig,
) -> Result<Vec<RssHubInstance>> {
    let mut settings = load_rsshub_settings(database)?;
    let primary_id = config
        .instance_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&settings.global_instance_id)
        .to_string();

    let primary_idx = settings
        .instances
        .iter()
        .position(|instance| instance.id == primary_id)
        .ok_or_else(|| {
            FeaderError::Message(format!("RSSHub instance '{primary_id}' is not configured"))
        })?;
    // `remove` (not `swap_remove`) keeps the remaining instances in priority order.
    let primary = settings.instances.remove(primary_idx);

    if !config.allow_fallback {
        return Ok(vec![primary]);
    }

    let mut candidates = vec![primary];
    candidates.extend(settings.instances);
    Ok(candidates)
}

/// Return configured RSSHub instances and the global default.
#[tauri::command]
pub fn get_rsshub_settings(
    database: tauri::State<'_, AppDatabase>,
) -> Result<RssHubSettings> {
    load_rsshub_settings(&database)
}

/// Set the global RSSHub instance used by sources without their own override.
#[tauri::command]
pub fn set_rsshub_global_instance(
    instance_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<RssHubSettings> {
    let mut settings = load_rsshub_settings(&database)?;
    if !settings
        .instances
        .iter()
        .any(|instance| instance.id == instance_id)
    {
        return Err("RSSHub instance is not configured".into());
    }
    settings.global_instance_id = instance_id;
    save_rsshub_settings(&database, &settings)?;
    load_rsshub_settings(&database)
}

/// Add a custom RSSHub instance to the selectable list.
#[tauri::command]
pub fn add_rsshub_instance(
    request: AddRssHubInstanceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<RssHubSettings> {
    let base_url = normalize_rsshub_base_url(&request.base_url)?;
    let id = rsshub_instance_id_from_base(&base_url);
    let mut settings = load_rsshub_settings(&database)?;
    if settings.instances.iter().any(|instance| instance.id == id) {
        return Err("RSSHub instance already exists".into());
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

/// Set the user-preferred priority order of RSSHub instances (by id).
#[tauri::command]
pub fn set_rsshub_instance_order(
    order: Vec<String>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<RssHubSettings> {
    let mut settings = load_rsshub_settings(&database)?;
    let known: std::collections::HashSet<&str> = settings
        .instances
        .iter()
        .map(|instance| instance.id.as_str())
        .collect();
    let mut order = order;
    order.retain(|id| known.contains(id.as_str()));
    settings.order = order;
    save_rsshub_settings(&database, &settings)?;
    load_rsshub_settings(&database)
}

/// Probe an RSSHub instance health endpoint.
#[tauri::command]
pub async fn check_rsshub_instance(base_url: String) -> Result<RssHubInstanceCheck> {
    let base_url = normalize_rsshub_base_url(&base_url)?;
    let checked_url = format!("{base_url}/healthz");
    let response = reqwest::Client::new()
        .get(&checked_url)
        .header("user-agent", "Feader/0.1")
        .timeout(Duration::from_secs(8))
        .send()
        .await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsshub_source_config_defaults_allow_fallback_true() {
        let config: crate::models::RssHubSourceConfig =
            serde_json::from_str(r#"{"route":"/github/trending/daily/rust"}"#).unwrap();
        assert!(config.allow_fallback);
        assert!(config.instance_id.is_none());
    }

    #[test]
    fn rsshub_instance_order_is_applied() {
        let database = AppDatabase::in_memory().expect("database opens");
        let default = load_rsshub_settings(&database).expect("load");
        // Move the second builtin to the front.
        let second_id = default.instances[1].id.clone();
        let mut order: Vec<String> = vec![second_id.clone()];
        order.extend(
            default
                .instances
                .iter()
                .map(|i| i.id.clone())
                .filter(|id| id != &second_id),
        );

        let mut settings = load_rsshub_settings(&database).expect("load");
        settings.order = order;
        save_rsshub_settings(&database, &settings).expect("save");

        let reloaded = load_rsshub_settings(&database).expect("reload");
        assert_eq!(reloaded.instances[0].id, second_id);
    }

    #[test]
    fn fallback_error_lists_each_instance() {
        let failures = vec![
            ("RSSHub Official".to_string(), "status 429".to_string()),
            ("RSSForever".to_string(), "timed out".to_string()),
        ];
        let message = format_fallback_error(&failures);
        assert!(message.contains("RSSHub Official: status 429"));
        assert!(message.contains("RSSForever: timed out"));
        assert!(message.starts_with("All RSSHub instances failed"));
    }

    fn rsshub_config(instance_id: Option<&str>, allow_fallback: bool) -> RssHubSourceConfig {
        RssHubSourceConfig {
            route: "/github/trending/daily/rust".to_string(),
            instance_id: instance_id.map(|s| s.to_string()),
            allow_fallback,
        }
    }

    #[test]
    fn candidates_without_fallback_is_primary_only() {
        let database = AppDatabase::in_memory().expect("db");
        let config = rsshub_config(Some("rsshub-app"), false);
        let candidates = resolve_rsshub_candidates(&database, &config).expect("candidates");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "rsshub-app");
    }

    #[test]
    fn candidates_with_fallback_lists_primary_first_then_rest() {
        let database = AppDatabase::in_memory().expect("db");
        let config = rsshub_config(Some("rsshub-app"), true);
        let candidates = resolve_rsshub_candidates(&database, &config).expect("candidates");
        assert!(candidates.len() > 1);
        assert_eq!(candidates[0].id, "rsshub-app");
        assert!(candidates.iter().filter(|c| c.id == "rsshub-app").count() == 1);
    }

    #[test]
    fn candidates_use_global_default_when_no_override() {
        let database = AppDatabase::in_memory().expect("db");
        let config = rsshub_config(None, true);
        let settings = load_rsshub_settings(&database).expect("settings");
        let candidates = resolve_rsshub_candidates(&database, &config).expect("candidates");
        assert_eq!(candidates[0].id, settings.global_instance_id);
    }

    #[test]
    fn candidates_error_on_unknown_primary() {
        let database = AppDatabase::in_memory().expect("db");
        let config = rsshub_config(Some("does-not-exist"), true);
        assert!(resolve_rsshub_candidates(&database, &config).is_err());
    }

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
}
