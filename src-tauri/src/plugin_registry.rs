//! Static plugin-pack registry for XPath and AI source setup.
//!
//! This is the first plugin layer: data-only rule packs. They are intentionally
//! not executable, so provider support can move out to FeaderHub before Feader
//! needs a full sandboxed plugin runtime.

use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::models::{
    RegistryIndex, RegistryPluginEntry, RemotePluginManifest, RemoteXPathRulePack,
    XPathRuleCandidate, XPathRulePack, XPathSelectors,
};

const STATIC_XPATH_API_VERSION: &str = "xpath-rule-pack/v1";
const OFFICIAL_REGISTRY: &str = "https://github.com/FrankieeW/FeaderHub";
const REGISTRY_RAW_BASE: &str = "https://raw.githubusercontent.com/FrankieeW/FeaderHub/main";
const STATIC_XPATH_KIND: &str = "static-xpath-rule-pack";
const REGISTRY_FETCH_TIMEOUT_SECONDS: u64 = 15;
const REGISTRY_INDEX_BYTE_CAP: usize = 128 * 1024;
const PLUGIN_FILE_BYTE_CAP: usize = 256 * 1024;

pub fn bundled_xpath_rule_packs() -> Vec<XPathRulePack> {
    vec![discuz_rule_pack(), maccms_rule_pack(), generic_rule_pack()]
}

#[cfg(test)]
pub fn matching_prompt_rules(document: &str) -> Vec<String> {
    matching_prompt_rules_in_packs(document, &bundled_xpath_rule_packs())
}

pub fn matching_prompt_rules_in_packs(document: &str, packs: &[XPathRulePack]) -> Vec<String> {
    packs
        .iter()
        .flat_map(|pack| pack.candidates.iter())
        .filter(|candidate| candidate_matches(document, candidate))
        .map(|candidate| candidate.prompt_rule.clone())
        .collect()
}

#[cfg(test)]
pub fn matching_selector_candidates(document: &str) -> Vec<XPathSelectors> {
    matching_selector_candidates_in_packs(document, &bundled_xpath_rule_packs())
}

pub fn matching_selector_candidates_in_packs(
    document: &str,
    packs: &[XPathRulePack],
) -> Vec<XPathSelectors> {
    let mut candidates = packs
        .iter()
        .flat_map(|pack| pack.candidates.iter())
        .filter(|candidate| candidate_matches(document, candidate))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.priority));
    candidates
        .into_iter()
        .map(|candidate| candidate.selectors.clone())
        .collect()
}

fn candidate_matches(document: &str, candidate: &XPathRuleCandidate) -> bool {
    if candidate.detect.is_empty() {
        return true;
    }
    let lower = document.to_ascii_lowercase();
    candidate
        .detect
        .iter()
        .any(|marker| lower.contains(&marker.to_ascii_lowercase()))
}

/// Fetch the registry index from the remote FeaderHub repository.
pub async fn fetch_registry_index() -> Result<RegistryIndex, String> {
    let url = format!("{REGISTRY_RAW_BASE}/registry/index.json");
    let body = fetch_text_limited(&url, REGISTRY_INDEX_BYTE_CAP, "registry index").await?;

    let index = serde_json::from_str::<RegistryIndex>(&body)
        .map_err(|error| format!("Failed to parse registry index: {error}"))?;
    if index.schema_version != "feader-registry/v1" {
        return Err(format!(
            "Unsupported registry schema '{}'",
            index.schema_version
        ));
    }
    Ok(index)
}

/// Fetch a single remote plugin manifest and its rule pack, returning a merged XPathRulePack.
pub async fn fetch_remote_plugin_pack(
    entry: &RegistryPluginEntry,
) -> Result<XPathRulePack, String> {
    if entry.kind != STATIC_XPATH_KIND {
        return Err(format!(
            "Plugin {} has unsupported kind '{}'",
            entry.id, entry.kind
        ));
    }
    let expected_sha = entry
        .sha256
        .as_deref()
        .map(str::trim)
        .filter(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
        .ok_or_else(|| format!("Plugin {} has no valid sha256", entry.id))?;

    let manifest_path = validate_registry_path(&entry.manifest, "manifest")?;
    let manifest_url = format!("{REGISTRY_RAW_BASE}/{manifest_path}");

    let manifest_body =
        fetch_text_limited(&manifest_url, PLUGIN_FILE_BYTE_CAP, "plugin manifest").await?;

    let manifest: RemotePluginManifest = serde_json::from_str(&manifest_body)
        .map_err(|error| format!("Failed to parse manifest {manifest_path}: {error}"))?;
    validate_manifest(entry, &manifest)?;

    let pack_dir = manifest_path
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let entry_file = validate_registry_path(&manifest.entry, "rule pack entry")?;
    let pack_url = format!("{REGISTRY_RAW_BASE}/{pack_dir}/{entry_file}");

    let pack_body = fetch_text_limited(&pack_url, PLUGIN_FILE_BYTE_CAP, "rule pack").await?;
    verify_sha256(&pack_body, expected_sha, &entry.id)?;

    let pack: RemoteXPathRulePack = serde_json::from_str(&pack_body)
        .map_err(|error| format!("Failed to parse rule pack {}: {error}", manifest.entry))?;
    validate_rule_pack(entry, &manifest, &pack)?;

    Ok(XPathRulePack {
        id: pack.id,
        name: pack.name,
        version: pack.version,
        api_version: manifest.feader_api_version,
        registry: OFFICIAL_REGISTRY.to_string(),
        trust: "official".to_string(),
        description: pack
            .description
            .or(manifest.description)
            .unwrap_or_default(),
        capabilities: vec!["xpath.selectorCandidates".to_string()],
        candidates: pack.candidates,
        parameters: pack.parameters,
    })
}

async fn fetch_text_limited(url: &str, cap: usize, label: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REGISTRY_FETCH_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .header("User-Agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| format!("Failed to fetch {label}: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("{label} returned HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("Failed to read {label} body: {error}"))?;
    if bytes.len() > cap {
        return Err(format!("{label} exceeded {} bytes", cap));
    }
    String::from_utf8(bytes.to_vec()).map_err(|error| format!("{label} was not UTF-8: {error}"))
}

fn validate_registry_path<'a>(path: &'a str, label: &str) -> Result<&'a str, String> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains("://")
        || trimmed.split('/').any(|part| part == "..")
    {
        return Err(format!("Invalid {label} path '{path}'"));
    }
    Ok(trimmed)
}

fn validate_manifest(
    entry: &RegistryPluginEntry,
    manifest: &RemotePluginManifest,
) -> Result<(), String> {
    if manifest.id != entry.id {
        return Err(format!(
            "Manifest id '{}' does not match registry id '{}'",
            manifest.id, entry.id
        ));
    }
    if manifest.version != entry.version {
        return Err(format!(
            "Manifest version '{}' does not match registry version '{}'",
            manifest.version, entry.version
        ));
    }
    if manifest.name != entry.name {
        return Err(format!(
            "Manifest name '{}' does not match registry name '{}'",
            manifest.name, entry.name
        ));
    }
    if manifest.kind != STATIC_XPATH_KIND {
        return Err(format!(
            "Manifest kind '{}' is not supported",
            manifest.kind
        ));
    }
    if manifest.feader_api_version != STATIC_XPATH_API_VERSION {
        return Err(format!(
            "Manifest API version '{}' is not supported",
            manifest.feader_api_version
        ));
    }
    Ok(())
}

fn validate_rule_pack(
    entry: &RegistryPluginEntry,
    manifest: &RemotePluginManifest,
    pack: &RemoteXPathRulePack,
) -> Result<(), String> {
    if pack.schema_version != STATIC_XPATH_API_VERSION {
        return Err(format!(
            "Rule pack schema '{}' is not supported",
            pack.schema_version
        ));
    }
    if pack.id != manifest.id || pack.id != entry.id {
        return Err(format!(
            "Rule pack id '{}' does not match manifest",
            pack.id
        ));
    }
    if pack.version != manifest.version {
        return Err(format!(
            "Rule pack version '{}' does not match manifest version '{}'",
            pack.version, manifest.version
        ));
    }
    Ok(())
}

fn verify_sha256(body: &str, expected: &str, plugin_id: &str) -> Result<(), String> {
    let digest = Sha256::digest(body.as_bytes());
    let actual = hex::encode(digest);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "Plugin {plugin_id} checksum mismatch: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn rule_pack(
    id: &str,
    name: &str,
    version: &str,
    description: &str,
    candidates: Vec<XPathRuleCandidate>,
) -> XPathRulePack {
    XPathRulePack {
        id: id.to_string(),
        name: name.to_string(),
        version: version.to_string(),
        api_version: STATIC_XPATH_API_VERSION.to_string(),
        registry: OFFICIAL_REGISTRY.to_string(),
        trust: "bundled-official".to_string(),
        description: description.to_string(),
        capabilities: vec![
            "xpath.selectorCandidates".to_string(),
            "ai.promptRules".to_string(),
        ],
        candidates,
        parameters: None,
    }
}

fn candidate(
    id: &str,
    page_type: &str,
    priority: usize,
    detect: &[&str],
    prompt_rule: &str,
    selectors: XPathSelectors,
) -> XPathRuleCandidate {
    XPathRuleCandidate {
        id: id.to_string(),
        page_type: page_type.to_string(),
        priority,
        detect: detect.iter().map(|value| value.to_string()).collect(),
        prompt_rule: prompt_rule.to_string(),
        selectors,
    }
}

fn discuz_rule_pack() -> XPathRulePack {
    rule_pack(
        "official.discuz.xpath",
        "Discuz XPath Rules",
        "0.1.0",
        "Static XPath and AI prompt rules for Discuz-style forum thread lists.",
        vec![candidate(
            "discuz-thread-list",
            "forum-thread-list",
            90,
            &["threadlisttableid", "km_subject", "discuz"],
            "- Discuz/forum thread list: prefer items `//*[@id='threadlisttableid']/li[contains(@class, 'kmlist')]`, title `.//*[contains(@class, 'km_subject')]`, url `.//a[contains(@class, 'kmtit')]/@href`, author from the first `space-uid` link inside `.kminfo`, date from `.kmtime/*[@title]/@title`, next page from `.nxt/@href`.",
            XPathSelectors {
                items: "//*[@id='threadlisttableid']/li[contains(@class, 'kmlist')]".to_string(),
                title: ".//*[contains(@class, 'km_subject')]".to_string(),
                url: ".//a[contains(@class, 'kmtit')]/@href".to_string(),
                summary: Some(".//*[contains(@class, 'kminfo')]".to_string()),
                published_at: Some(".//*[contains(@class, 'kmtime')]/*[@title][1]/@title".to_string()),
                author: Some(".//div[contains(@class, 'kminfo')]/a[starts-with(@href, 'space-uid')][1]".to_string()),
                content: None,
                image: Some(".//a[contains(@class, 'kmimg')]//img/@src".to_string()),
                next_page: Some("//a[contains(@class, 'nxt')]/@href".to_string()),
            },
        )],
    )
}

fn maccms_rule_pack() -> XPathRulePack {
    rule_pack(
        "official.maccms.xpath",
        "MacCMS XPath Rules",
        "0.1.0",
        "Static XPath and AI prompt rules for MacCMS video list and detail pages.",
        vec![
            candidate(
                "maccms-video-list",
                "video-list",
                80,
                &["vodlist_item", "maccms", "vod/detail"],
                "- MacCMS/video listing: prefer `//li[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_item ')]` with title under `.vodlist_title`, url/image from `.vodlist_thumb` href/data-original, summary from `.vodlist_sub`.",
                XPathSelectors {
                    items: "//li[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_item ')]".to_string(),
                    title: ".//*[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_title ')]//a[1]".to_string(),
                    url: ".//a[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_thumb ')]/@href".to_string(),
                    summary: Some(".//*[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_sub ')]".to_string()),
                    published_at: None,
                    author: None,
                    content: None,
                    image: Some(".//a[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_thumb ')]/@data-original".to_string()),
                    next_page: Some("//a[contains(concat(' ', normalize-space(@class), ' '), ' page-link ') and contains(., '下一')]/@href".to_string()),
                },
            ),
            candidate(
                "maccms-video-detail",
                "video-detail",
                85,
                &["detail_list_box", "btn_primary", "vodlist_thumb"],
                "- MacCMS/video detail page: a valid single-item source may use the first `.detail_list_box .content_box`, title from `h2.title`, url from `.btn_primary/@href`, image from `.vodlist_thumb/@data-original`, summary/content from `.desc`.",
                XPathSelectors {
                    items: "//div[contains(concat(' ', normalize-space(@class), ' '), ' detail_list_box ')]//div[contains(concat(' ', normalize-space(@class), ' '), ' content_box ')][1]".to_string(),
                    title: ".//h2[contains(concat(' ', normalize-space(@class), ' '), ' title ')]".to_string(),
                    url: ".//a[contains(concat(' ', normalize-space(@class), ' '), ' btn_primary ')]/@href".to_string(),
                    summary: Some(".//li[contains(concat(' ', normalize-space(@class), ' '), ' desc ')]".to_string()),
                    published_at: Some(".//li[contains(concat(' ', normalize-space(@class), ' '), ' data ')]/em[1]".to_string()),
                    author: Some(".//li[contains(concat(' ', normalize-space(@class), ' '), ' data ')][span[contains(., '主演')]]/a[1]".to_string()),
                    content: Some(".//li[contains(concat(' ', normalize-space(@class), ' '), ' desc ')]".to_string()),
                    image: Some(".//a[contains(concat(' ', normalize-space(@class), ' '), ' vodlist_thumb ')]/@data-original".to_string()),
                    next_page: None,
                },
            ),
        ],
    )
}

fn generic_rule_pack() -> XPathRulePack {
    rule_pack(
        "official.generic-html.xpath",
        "Generic HTML XPath Rules",
        "0.1.0",
        "Fallback static XPath and AI prompt rules for generic article listings.",
        vec![candidate(
            "generic-article-list",
            "article-list",
            10,
            &[],
            "- Generic listing: first identify the smallest repeated node that contains one stable title link. Avoid navigation, footer, sidebar, ad, and ranking widgets unless the whole page is a ranking source.",
            XPathSelectors {
                items: "//article".to_string(),
                title: ".//h1 | .//h2 | .//h3 | .//a[normalize-space()][1]".to_string(),
                url: ".//a[@href][1]/@href".to_string(),
                summary: Some(".//p[normalize-space()][1]".to_string()),
                published_at: Some(".//time/@datetime | .//time".to_string()),
                author: Some(".//*[contains(@class, 'author')][1]".to_string()),
                content: Some(".".to_string()),
                image: Some(".//img[@src][1]/@src".to_string()),
                next_page: Some("//a[@rel='next']/@href | //a[contains(@class, 'next')]/@href".to_string()),
            },
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_bundled_static_rule_packs() {
        let packs = bundled_xpath_rule_packs();
        assert!(packs.iter().any(|pack| pack.id == "official.discuz.xpath"));
        assert!(packs
            .iter()
            .all(|pack| pack.api_version == STATIC_XPATH_API_VERSION));
    }

    #[test]
    fn matches_prompt_rules_by_page_markers() {
        let rules = matching_prompt_rules("<ul id=\"threadlisttableid\"></ul>");
        assert!(rules.iter().any(|rule| rule.contains("Discuz/forum")));
        assert!(rules.iter().any(|rule| rule.contains("Generic listing")));
    }

    #[test]
    fn matches_selector_candidates_by_page_markers() {
        let candidates = matching_selector_candidates("<div class=\"detail_list_box\"></div>");
        assert!(candidates
            .iter()
            .any(|selectors| selectors.items.contains("detail_list_box")));
        assert!(candidates
            .iter()
            .any(|selectors| selectors.items == "//article"));
    }
}
