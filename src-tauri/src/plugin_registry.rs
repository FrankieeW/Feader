//! Static plugin-pack registry for XPath and AI source setup.
//!
//! This is the first plugin layer: data-only rule packs. They are intentionally
//! not executable, so provider support can move out to FeaderHub before Feader
//! needs a full sandboxed plugin runtime.

use crate::models::{RemotePluginManifest, RemoteXPathRulePack, RegistryIndex, XPathRuleCandidate, XPathRulePack, XPathSelectors};

const STATIC_XPATH_API_VERSION: &str = "xpath-rule-pack/v1";
const OFFICIAL_REGISTRY: &str = "https://github.com/FrankieeW/FeaderHub";
const REGISTRY_RAW_BASE: &str = "https://raw.githubusercontent.com/FrankieeW/FeaderHub/main";

pub fn bundled_xpath_rule_packs() -> Vec<XPathRulePack> {
    vec![discuz_rule_pack(), maccms_rule_pack(), generic_rule_pack()]
}

pub fn matching_prompt_rules(document: &str) -> Vec<String> {
    bundled_xpath_rule_packs()
        .into_iter()
        .flat_map(|pack| pack.candidates)
        .filter(|candidate| candidate_matches(document, candidate))
        .map(|candidate| candidate.prompt_rule)
        .collect()
}

pub fn matching_selector_candidates(document: &str) -> Vec<XPathSelectors> {
    let mut candidates = bundled_xpath_rule_packs()
        .into_iter()
        .flat_map(|pack| pack.candidates)
        .filter(|candidate| candidate_matches(document, candidate))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.priority));
    candidates
        .into_iter()
        .map(|candidate| candidate.selectors)
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
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| format!("Failed to fetch registry index: {error}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "Registry index returned HTTP {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|error| format!("Failed to read registry index body: {error}"))?;

    serde_json::from_str::<RegistryIndex>(&body)
        .map_err(|error| format!("Failed to parse registry index: {error}"))
}

/// Fetch a single remote plugin manifest and its rule pack, returning a merged XPathRulePack.
pub async fn fetch_remote_plugin_pack(manifest_path: &str) -> Result<XPathRulePack, String> {
    let manifest_url = format!("{REGISTRY_RAW_BASE}/{manifest_path}");
    let client = reqwest::Client::new();

    // Fetch manifest
    let manifest_response = client
        .get(&manifest_url)
        .header("User-Agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| format!("Failed to fetch manifest {manifest_path}: {error}"))?;

    if !manifest_response.status().is_success() {
        return Err(format!(
            "Manifest {manifest_path} returned HTTP {}",
            manifest_response.status()
        ));
    }

    let manifest_body = manifest_response
        .text()
        .await
        .map_err(|error| format!("Failed to read manifest body: {error}"))?;

    let manifest: RemotePluginManifest = serde_json::from_str(&manifest_body)
        .map_err(|error| format!("Failed to parse manifest {manifest_path}: {error}"))?;

    // Fetch the rule pack (entry file, relative to manifest directory)
    let pack_dir = manifest_path
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let pack_url = format!("{REGISTRY_RAW_BASE}/{pack_dir}/{}", manifest.entry);

    let pack_response = client
        .get(&pack_url)
        .header("User-Agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| format!("Failed to fetch rule pack {}: {error}", manifest.entry))?;

    if !pack_response.status().is_success() {
        return Err(format!(
            "Rule pack {} returned HTTP {}",
            manifest.entry,
            pack_response.status()
        ));
    }

    let pack_body = pack_response
        .text()
        .await
        .map_err(|error| format!("Failed to read rule pack body: {error}"))?;

    let pack: RemoteXPathRulePack = serde_json::from_str(&pack_body)
        .map_err(|error| format!("Failed to parse rule pack {}: {error}", manifest.entry))?;

    Ok(XPathRulePack {
        id: pack.id,
        name: pack.name,
        version: pack.version,
        api_version: manifest.feader_api_version,
        registry: OFFICIAL_REGISTRY.to_string(),
        trust: "official".to_string(),
        description: pack.description.unwrap_or_default(),
        capabilities: vec![
            "xpath.selectorCandidates".to_string(),
            "ai.promptRules".to_string(),
        ],
        candidates: pack.candidates,
        parameters: pack.parameters,
    })
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
            "- Discuz/forum thread list: prefer items `//ul[@id='threadlisttableid']/li[contains(concat(' ', normalize-space(@class), ' '), ' kmlist ')]`, title `.//*[contains(concat(' ', normalize-space(@class), ' '), ' km_subject ')]`, url `.//a[contains(concat(' ', normalize-space(@class), ' '), ' kmtit ')]/@href`, author from the first `space-uid` link inside `.kminfo`, date from `.kmtime/*[@title]/@title`, next page from `.nxt/@href`.",
            XPathSelectors {
                items: "//ul[@id='threadlisttableid']/li[contains(concat(' ', normalize-space(@class), ' '), ' kmlist ')]".to_string(),
                title: ".//*[contains(concat(' ', normalize-space(@class), ' '), ' km_subject ')]".to_string(),
                url: ".//a[contains(concat(' ', normalize-space(@class), ' '), ' kmtit ')]/@href".to_string(),
                summary: Some(".//*[contains(concat(' ', normalize-space(@class), ' '), ' kminfo ')]".to_string()),
                published_at: Some(".//*[contains(concat(' ', normalize-space(@class), ' '), ' kmtime ')]/*[@title][1]/@title".to_string()),
                author: Some(".//div[contains(concat(' ', normalize-space(@class), ' '), ' kminfo ')]/a[starts-with(@href, 'space-uid')][1]".to_string()),
                content: None,
                image: Some(".//a[contains(concat(' ', normalize-space(@class), ' '), ' kmimg ')]//img/@src".to_string()),
                next_page: Some("//a[contains(concat(' ', normalize-space(@class), ' '), ' nxt ')]/@href".to_string()),
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
