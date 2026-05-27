//! Shared data shapes exposed through Tauri commands.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

/// Canonical source kinds persisted in the database.
pub const SOURCE_KIND_RSS: &str = "rss";
pub const SOURCE_KIND_RSSHUB: &str = "rsshub";
pub const SOURCE_KIND_XPATH: &str = "xpath";
pub const SOURCE_KIND_JSON_API: &str = "json-api";

/// Plugin manifests use a longer name to distinguish pack intent from storage kind.
pub const PLUGIN_KIND_XPATH: &str = "static-xpath-rule-pack";
pub const PLUGIN_KIND_JSON_API_FEED: &str = "json-api-feed";
pub const PLUGIN_KIND_APP_UI_THEME: &str = "app-ui-theme";
pub const PLUGIN_KIND_SOURCE_LIST_VIEW: &str = "source-list-view";
pub const PLUGIN_KIND_DETAIL_VIEW: &str = "detail-view";
pub const PLUGIN_KIND_RUNTIME_SOURCE: &str = "runtime-source-plugin";

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
    pub refresh_interval_seconds: Option<i64>,
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

/// A known or user-added RSSHub instance.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RssHubInstance {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub maintainer: String,
    pub location: Option<String>,
    pub official: bool,
    pub builtin: bool,
}

/// RSSHub instance preferences exposed to the renderer.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RssHubSettings {
    pub global_instance_id: String,
    pub instances: Vec<RssHubInstance>,
    #[serde(default)]
    pub order: Vec<String>,
}

/// Source-level RSSHub route configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RssHubSourceConfig {
    pub route: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    #[serde(default = "default_allow_fallback")]
    pub allow_fallback: bool,
}

fn default_allow_fallback() -> bool {
    true
}

/// Request body for adding an RSSHub route source.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRssHubSourceRequest {
    pub route: String,
    pub title: Option<String>,
    pub instance_id: Option<String>,
}

/// Request body for adding a custom RSSHub instance.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRssHubInstanceRequest {
    pub name: String,
    pub base_url: String,
}

/// Request body for changing a source-level RSSHub instance override.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRssHubSourceInstanceRequest {
    pub source_id: i64,
    pub instance_id: Option<String>,
}

/// Result of probing an RSSHub instance.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RssHubInstanceCheck {
    pub ok: bool,
    pub message: String,
    pub checked_url: String,
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

/// Plugin credential metadata returned to the renderer (cookie never echoed).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCredential {
    pub plugin_id: String,
    pub cookie_set: bool,
    pub cookie_reference: Option<String>,
    pub updated_at: Option<String>,
    pub last_checked_at: Option<String>,
    pub last_check_ok: Option<bool>,
    pub last_check_message: Option<String>,
}

/// Result of probing a plugin credential's validity.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCheck {
    pub ok: bool,
    pub message: String,
    pub checked_at: String,
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
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathSelectors {
    #[serde(deserialize_with = "deserialize_selector_string")]
    pub items: String,
    #[serde(deserialize_with = "deserialize_selector_string")]
    pub title: String,
    #[serde(deserialize_with = "deserialize_selector_string")]
    pub url: String,
    #[serde(default, deserialize_with = "deserialize_optional_selector_string")]
    pub summary: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_selector_string")]
    pub published_at: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_selector_string")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cookie: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_selector_string")]
    pub content: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_selector_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub detail_content: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content_cleanup: Vec<ContentCleanupRule>,
    #[serde(default, deserialize_with = "deserialize_optional_selector_string")]
    pub image: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_selector_string")]
    pub next_page: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_fields: Vec<XPathCustomField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<XPathSourcePluginInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reader: Option<ReaderConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum SelectorStringValue {
    String(String),
    Object { xpath: String },
}

impl SelectorStringValue {
    fn into_xpath(self) -> String {
        match self {
            SelectorStringValue::String(value) => value,
            SelectorStringValue::Object { xpath } => xpath,
        }
    }
}

fn deserialize_selector_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(SelectorStringValue::deserialize(deserializer)?.into_xpath())
}

fn deserialize_optional_selector_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<SelectorStringValue>::deserialize(deserializer)?
        .map(SelectorStringValue::into_xpath))
}

/// A regex replacement applied to extracted article body HTML.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCleanupRule {
    pub pattern: String,
    #[serde(default)]
    pub replacement: String,
}

/// Plugin-authored customization of the article reading view.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_selectors: Vec<String>,
    #[serde(default)]
    pub resolve_relative_urls: bool,
    #[serde(default)]
    pub rewrite_links: bool,
    #[serde(default)]
    pub show_custom_fields: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<ReaderLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub css: Option<String>,
}

/// Recommended reader presentation defaults from a plugin.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderLayout {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typography: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immersive: Option<bool>,
}

/// A non-standard metadata field extracted from either a list item or detail page.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathCustomField {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub xpath: String,
    #[serde(default)]
    pub scope: XPathCustomFieldScope,
}

/// Where a custom XPath field is evaluated.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum XPathCustomFieldScope {
    #[default]
    Item,
    Detail,
}

/// Plugin metadata copied into a source config when it is created from Hub.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathSourcePluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub registry: String,
    pub trust: String,
    pub candidate_id: String,
    pub page_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<PluginAuthor>,
}

/// AI-suggested XPath source draft.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathSourceSuggestion {
    pub title: Option<String>,
    pub selectors: XPathSelectors,
}

/// A static plugin pack that contributes XPath and AI-assist rules.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathRulePack {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    #[serde(default)]
    pub kind: String,
    pub registry: String,
    pub trust: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    pub capabilities: Vec<String>,
    pub candidates: Vec<XPathRuleCandidate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<PluginAuthor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<PluginParameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<PluginAuth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<HashMap<String, String>>,
}

/// Common marketplace/install envelope for all Feader plugin kinds.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPack {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_version: String,
    pub kind: String,
    pub registry: String,
    pub trust: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<PluginAuthor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PluginPermissions>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xpath: Option<XPathRulePack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<RemoteViewPlugin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeSourcePlugin>,
}

/// A plugin pack as shown in the marketplace, including local install state.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketplacePluginPack {
    #[serde(flatten)]
    pub pack: PluginPack,
    pub installed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_market_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_market_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_market_repository: Option<String>,
}

/// A configured plugin marketplace repository.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMarket {
    pub id: String,
    pub name: String,
    pub repository: String,
    pub raw_base_url: String,
    pub branch: String,
    pub builtin: bool,
}

/// Request body for adding a GitHub-hosted plugin marketplace.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPluginMarketRequest {
    pub repository: String,
    pub name: Option<String>,
    pub branch: Option<String>,
}

/// Request body for installing a plugin from a configured marketplace.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginFromMarketRequest {
    pub market_id: String,
    pub plugin_id: String,
}

/// Request body for installing a plugin directly from a URL.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPluginFromUrlRequest {
    pub url: String,
}

/// Filesystem result of creating a starter marketplace template.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginMarketTemplate {
    pub path: String,
    pub files: Vec<String>,
}

/// Public author metadata displayed in the plugin Hub.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuthor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm_address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_id: Option<String>,
}

/// Permission metadata declared by a plugin manifest.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginPermissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub credentials: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<String>,
}

/// Login probe declared by a plugin for credential validity checks.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuth {
    #[serde(default)]
    pub check_url: String,
    #[serde(default)]
    pub logged_in_xpath: String,
}

/// A selectable option for a plugin parameter dropdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginParamOption {
    pub value: String,
    pub label: String,
}

/// A user-editable input control shown in the Add Source dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginParam {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<PluginParamOption>>,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

/// Optional parameter block for source creation dialogs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url_template: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<PluginSection>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<PluginParam>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<PluginDefaults>,
}

/// A navigable section tree node for forum/site plugins.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSection {
    pub id: String,
    pub path: Vec<String>,
    pub url: String,
}

/// Default values for the source parameter dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDefaults {
    pub max_items: Option<usize>,
    pub max_pages: Option<usize>,
}

/// Registry index file from FeaderHub.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryIndex {
    pub schema_version: String,
    pub updated_at: String,
    pub plugins: Vec<RegistryPluginEntry>,
}

/// One plugin listed in the registry index.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryPluginEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub manifest: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Remote plugin manifest (minimal subset needed to locate the rule pack).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemotePluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub kind: String,
    pub feader_api_version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    pub entry: String,
    #[serde(default)]
    pub authors: Vec<PluginAuthor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PluginPermissions>,
}

/// Remote runtime-source payload for plugins that generate feeds via a local helper runtime.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourcePlugin {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: RuntimeSourceRuntime,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_templates: Vec<RuntimeRouteTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_card: Option<PluginSettingsCard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_page: Option<PluginSettingsPage>,
}

/// Runtime requirements for an advanced source plugin.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSourceRuntime {
    pub engine: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

/// A route family exposed by a runtime source plugin.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRouteTemplate {
    pub id: String,
    pub label: String,
    pub route_template: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_credentials: Vec<String>,
}

/// A host-rendered, plugin-declared card inserted into Settings.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettingsCard {
    pub id: String,
    pub title: String,
    pub renderer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    #[serde(default = "default_settings_card_width")]
    pub default_width: String,
    #[serde(default)]
    pub allowed_widths: Vec<String>,
}

fn default_settings_card_width() -> String {
    "two-column".to_string()
}

/// Host-rendered settings page declared by an advanced plugin.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettingsPage {
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<PluginSettingsSection>,
}

/// Group of settings fields displayed together.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettingsSection {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<PluginSettingsField>,
}

/// One host-rendered plugin setting field.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettingsField {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<PluginParamOption>>,
}

/// Request body for saving namespaced plugin config.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPluginConfigRequest {
    pub plugin_id: String,
    pub values: serde_json::Value,
}

/// Request body for importing namespaced plugin config.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPluginConfigRequest {
    pub plugin_id: String,
    pub json: String,
}

/// Remote xpath-rule-pack payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteXPathRulePack {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub candidates: Vec<XPathRuleCandidate>,
    #[serde(default)]
    pub parameters: Option<PluginParameters>,
    #[serde(default)]
    pub auth: Option<PluginAuth>,
}

/// Remote view-plugin template payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteViewPlugin {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub slot: String,
    pub description: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub tokens: HashMap<String, String>,
}

/// One page-family rule contributed by a static XPath plugin pack.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XPathRuleCandidate {
    pub id: String,
    pub page_type: String,
    pub priority: usize,
    pub detect: Vec<String>,
    #[serde(default)]
    pub prompt_rule: String,
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

/// Request body for updating an existing XPath source selector config.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateXPathSourceRequest {
    pub source_id: i64,
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

/// Auto-refresh configuration returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRefreshConfig {
    pub enabled: bool,
    pub global_interval_seconds: i64,
    pub plugin_overrides: Vec<PluginRefreshOverride>,
    pub next_refresh_at: Option<String>,
}

/// Per-plugin refresh interval override.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginRefreshOverride {
    pub plugin_id: String,
    pub plugin_name: String,
    pub refresh_interval_seconds: i64,
}

/// Emitted to the frontend on each scheduler tick.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTickEvent {
    pub refreshing: bool,
    pub current_source_id: Option<i64>,
    pub current_source_title: Option<String>,
    pub next_refresh_at: Option<String>,
    pub sources_checked: usize,
    pub sources_refreshed: usize,
}

/// A persisted settings card layout entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCardEntry {
    pub id: String,
    pub width: String,
    pub order: i32,
    pub visible: bool,
}

/// The full settings layout configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCardLayout {
    pub version: i32,
    pub cards: Vec<SettingsCardEntry>,
}
