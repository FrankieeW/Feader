//! AI provider client for selector suggestions.

use std::time::Duration;

use serde::Deserialize;

use crate::models::{
    env_reference_name, AiSettings, XPathRulePack, XPathSelectors, XPathSourceSuggestion,
};
use crate::plugin_registry;
use crate::xpath_adapter::is_valid_xpath;

const AI_HTML_CHAR_CAP: usize = 6_000;
const AI_OUTPUT_TOKEN_CAP: usize = 4096;
const AI_REQUEST_TIMEOUT_SECONDS: u64 = 45;
const AI_RESPONSE_SNIPPET_CAP: usize = 320;

/// Resolve a stored API key: `$NAME`/`${NAME}` from the environment, otherwise literal.
pub fn resolve_api_key(stored: &str) -> Result<String, String> {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        return Err("AI API key is not configured".to_string());
    }
    if let Some(name) = env_reference_name(trimmed) {
        return std::env::var(&name).map_err(|_| format!("Environment variable {name} is not set"));
    }
    Ok(trimmed.to_string())
}

#[derive(Deserialize)]
struct SuggestedSelectors {
    #[serde(rename = "sourceTitle")]
    source_title: Option<String>,
    items: Option<String>,
    title: Option<String>,
    url: Option<String>,
    summary: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    author: Option<String>,
    content: Option<String>,
    image: Option<String>,
    #[serde(rename = "nextPage")]
    next_page: Option<String>,
    #[serde(rename = "detailContent")]
    detail_content: Option<String>,
}

fn extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| text[start..=end].to_string())
}

fn keep_valid(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && is_valid_xpath(value))
}

/// Parse a model response (possibly wrapped in prose/code fences) into validated selectors.
pub fn parse_selectors_json(text: &str) -> Result<XPathSourceSuggestion, String> {
    let json = extract_json_object(text).ok_or_else(|| {
        if text.trim_start().starts_with('{') {
            return format!(
                "AI response JSON was incomplete. Response started with: {}",
                response_snippet(text)
            );
        }
        format!(
            "AI response did not contain JSON. Response started with: {}",
            response_snippet(text)
        )
    })?;
    let raw: SuggestedSelectors = serde_json::from_str(&json).map_err(|error| error.to_string())?;

    let items = keep_valid(raw.items).unwrap_or_default();
    let title = keep_valid(raw.title).unwrap_or_default();
    let url = keep_valid(raw.url).unwrap_or_default();
    if items.is_empty() || title.is_empty() || url.is_empty() {
        return Err("AI did not return usable selectors".to_string());
    }

    Ok(XPathSourceSuggestion {
        title: raw
            .source_title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        selectors: XPathSelectors {
            items,
            title,
            url,
            summary: keep_valid(raw.summary),
            published_at: keep_valid(raw.published_at),
            author: keep_valid(raw.author),
            content: keep_valid(raw.content),
            detail_content: keep_valid(raw.detail_content),
            image: keep_valid(raw.image),
            next_page: keep_valid(raw.next_page),
            max_items: None,
        },
    })
}

fn build_prompt(html: &str, rule_packs: &[XPathRulePack]) -> String {
    let page_rules = page_type_rules(html, rule_packs);
    format!(
        "Generate XPath selectors for an article-listing page from the normalized XHTML below.\n\
         You must return exactly one JSON object and nothing else.\n\
         Required string keys: sourceTitle, items, title, url, summary, publishedAt, author, content, image, nextPage.\n\
         sourceTitle is a concise human-readable source name inferred from the page, not an XPath.\n\
         Values must be XPath expressions or an empty string. `items` selects each repeating article node.\n\
         title/url/summary/publishedAt/author/content/image are relative to each item. nextPage is document-level.\n\
         Fill every selector field you can infer with reasonable confidence, especially summary, date, author, content, image, and nextPage.\n\
         Keep selectors short and robust. Prefer one stable semantic/tag/class selector over long union expressions.\n\
         Avoid absolute positional paths like /html/body/div[3]/div[2].\n\
         Use the page-specific rules below before generic rules. If the page is an anti-bot/challenge page, return empty selectors.\n\n\
         Page-specific rules:\n{page_rules}\n\n\
         The entire response must be a complete JSON object that starts with {{ and ends with }}.\n\
         Do not use Markdown. Do not explain. Do not wrap in code fences.\n\n\
         Example output:\n\
         {{\"sourceTitle\":\"Example Blog\",\"items\":\"//article\",\"title\":\".//h2/a\",\"url\":\".//h2/a/@href\",\"summary\":\".//p\",\"publishedAt\":\".//time/@datetime\",\"author\":\".//*[contains(@class,'author')]\",\"content\":\".\",\"image\":\".//img/@src\",\"nextPage\":\"//a[@rel='next']/@href\"}}\n\n\
         Normalized XHTML:\n{html}"
    )
}

fn page_type_rules(html: &str, rule_packs: &[XPathRulePack]) -> String {
    let lower = html.to_ascii_lowercase();
    let mut rules = Vec::new();

    if lower.contains("just a moment")
        || lower.contains("cf_chl")
        || lower.contains("challenge-platform")
    {
        rules.push("- Detected anti-bot/challenge markup. Do not infer content selectors from this page; return empty XPath strings.".to_string());
    }
    rules.extend(plugin_registry::matching_prompt_rules_in_packs(
        html, rule_packs,
    ));
    if lower.contains("book") || lower.contains("novel") || lower.contains("/c/") {
        rules.push("- Novel/category pages: prefer repeated book cards/list rows, title/url from the book detail link, author from nearby author text/link, summary from the description paragraph, image from cover `img/@src`, and nextPage from rel/next or pagination next link.".to_string());
    }

    rules.join("\n")
}

/// Ask the configured provider to suggest selectors for a page's HTML.
pub async fn suggest_xpath_selectors(
    settings: &AiSettings,
    stored_api_key: &str,
    page_html: &str,
    rule_packs: &[XPathRulePack],
) -> Result<XPathSourceSuggestion, String> {
    let base_url = settings.base_url.trim();
    if base_url.is_empty() {
        return Err("AI base URL is not configured".to_string());
    }
    if settings.model.trim().is_empty() {
        return Err("AI model is not configured".to_string());
    }

    let key = resolve_api_key(stored_api_key)?;
    let html: String = page_html.chars().take(AI_HTML_CHAR_CAP).collect();
    let prompt = build_prompt(&html, rule_packs);

    let text = match settings.provider.as_str() {
        "anthropic" => call_anthropic(settings, &key, &prompt).await?,
        "openai" => call_openai(settings, &key, &prompt).await?,
        other => return Err(format!("Unknown AI provider '{other}'")),
    };
    parse_selectors_json(&text)
}

async fn call_anthropic(settings: &AiSettings, key: &str, prompt: &str) -> Result<String, String> {
    let endpoint = configured_endpoint(&settings.base_url, "anthropic");
    let body = serde_json::json!({
        "model": &settings.model,
        "max_tokens": AI_OUTPUT_TOKEN_CAP,
        "system": "Return only valid JSON for XPath selectors. No prose, no markdown.",
        "messages": [{ "role": "user", "content": prompt }],
    });
    let response = ai_http_client()?
        .post(endpoint)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "AI request failed with status {}",
            response.status()
        ));
    }
    let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    let text = extract_model_text(&value).ok_or_else(|| {
        format!(
            "Unexpected Anthropic response shape. Response started with: {}",
            response_snippet(&value.to_string())
        )
    })?;
    Ok(text)
}

async fn call_openai(settings: &AiSettings, key: &str, prompt: &str) -> Result<String, String> {
    let endpoint = configured_endpoint(&settings.base_url, "openai");
    let json_body = serde_json::json!({
        "model": &settings.model,
        "max_tokens": AI_OUTPUT_TOKEN_CAP,
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": "Return only valid JSON for XPath selectors. No prose, no markdown." },
            { "role": "user", "content": prompt }
        ],
    });
    match send_openai_request(&endpoint, key, &json_body).await {
        Ok(text) => Ok(text),
        Err(error) if error.contains("status 400") => {
            let plain_body = serde_json::json!({
                "model": &settings.model,
                "max_tokens": AI_OUTPUT_TOKEN_CAP,
                "messages": [
                    { "role": "system", "content": "Return only valid JSON for XPath selectors. No prose, no markdown." },
                    { "role": "user", "content": prompt }
                ],
            });
            send_openai_request(&endpoint, key, &plain_body).await
        }
        Err(error) => Err(error),
    }
}

async fn send_openai_request(
    endpoint: &str,
    key: &str,
    body: &serde_json::Value,
) -> Result<String, String> {
    let response = ai_http_client()?
        .post(endpoint.to_string())
        .header("authorization", format!("Bearer {key}"))
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "AI request failed with status {}",
            response.status()
        ));
    }
    let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    extract_model_text(&value).ok_or_else(|| {
        format!(
            "Unexpected OpenAI response shape. Response started with: {}",
            response_snippet(&value.to_string())
        )
    })
}

fn ai_http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(AI_REQUEST_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| error.to_string())
}

fn configured_endpoint(value: &str, provider: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    if is_complete_endpoint(trimmed) {
        return trimmed.to_string();
    }
    match provider {
        "anthropic" => format!("{trimmed}/v1/messages"),
        "openai" => format!("{trimmed}/chat/completions"),
        _ => trimmed.to_string(),
    }
}

fn is_complete_endpoint(value: &str) -> bool {
    value.ends_with("/chat/completions") || value.ends_with("/v1/messages")
}

fn response_snippet(text: &str) -> String {
    let snippet: String = text.trim().chars().take(AI_RESPONSE_SNIPPET_CAP).collect();
    if snippet.is_empty() {
        "<empty>".to_string()
    } else {
        snippet.replace(['\n', '\r', '\t'], " ")
    }
}

fn extract_model_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value["choices"][0]["message"]["content"].as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = value["choices"][0]["text"].as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = value["content"].as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = value["completion"].as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = value["output_text"].as_str() {
        return Some(text.to_string());
    }
    let blocks = value["content"].as_array()?;
    let text = blocks
        .iter()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selectors_from_model_text() {
        let text = "Sure:\n```json\n{\"sourceTitle\":\"Example Blog\",\"items\":\"//article\",\"title\":\".//h2/a\",\"url\":\".//h2/a/@href\",\"summary\":\"\",\"content\":\".//section\",\"image\":\".//img/@src\",\"author\":null,\"publishedAt\":\".//time/@datetime\",\"nextPage\":\"\"}\n```";
        let suggestion = parse_selectors_json(text).expect("parses");
        assert_eq!(suggestion.title.as_deref(), Some("Example Blog"));
        assert_eq!(suggestion.selectors.items, "//article");
        assert_eq!(suggestion.selectors.content.as_deref(), Some(".//section"));
        assert_eq!(suggestion.selectors.summary, None);
    }

    #[test]
    fn rejects_when_required_selectors_missing() {
        let text = "{\"items\":\"\",\"title\":\".//h2/a\",\"url\":\".//h2/a/@href\"}";
        assert!(parse_selectors_json(text).is_err());
    }

    #[test]
    fn reports_non_json_model_response_snippet() {
        let error = parse_selectors_json("I cannot inspect this page.").unwrap_err();
        assert!(error.contains("I cannot inspect this page."));
    }

    #[test]
    fn reports_incomplete_json_response() {
        let error = parse_selectors_json("{\"items\":\"//article\"").unwrap_err();
        assert!(error.contains("JSON was incomplete"));
    }

    #[test]
    fn extracts_text_from_common_provider_shapes() {
        let anthropic = serde_json::json!({
            "content": [{ "type": "text", "text": "{\"items\":\"//article\"}" }]
        });
        let openai = serde_json::json!({
            "choices": [{ "message": { "content": "{\"items\":\"//article\"}" } }]
        });
        let completion = serde_json::json!({
            "completion": "{\"items\":\"//article\"}"
        });

        assert_eq!(
            extract_model_text(&anthropic).as_deref(),
            Some("{\"items\":\"//article\"}")
        );
        assert_eq!(
            extract_model_text(&openai).as_deref(),
            Some("{\"items\":\"//article\"}")
        );
        assert_eq!(
            extract_model_text(&completion).as_deref(),
            Some("{\"items\":\"//article\"}")
        );
    }

    #[test]
    fn prompt_includes_page_specific_rules() {
        let prompt = build_prompt(
            r#"<html><body><ul id="threadlisttableid"><li class="kmlist"><span class="km_subject">A</span></li></ul></body></html>"#,
            &plugin_registry::bundled_xpath_rule_packs(),
        );
        assert!(prompt.contains("Discuz/forum thread list"));
        assert!(prompt.contains("threadlisttableid"));
    }

    #[test]
    fn expands_provider_base_urls_to_default_endpoints() {
        assert_eq!(
            configured_endpoint("https://api.example.com/anthropic/", "anthropic"),
            "https://api.example.com/anthropic/v1/messages"
        );
        assert_eq!(
            configured_endpoint("https://api.example.com/v1", "openai"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn preserves_complete_configured_endpoints() {
        assert_eq!(
            configured_endpoint("https://api.example.com/custom/v1/messages/", "anthropic"),
            "https://api.example.com/custom/v1/messages"
        );
        assert_eq!(
            configured_endpoint("https://api.example.com/v1/chat/completions", "openai"),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn resolves_env_reference_key() {
        std::env::set_var("FEADER_TEST_KEY", "resolved-secret");
        assert_eq!(
            resolve_api_key("$FEADER_TEST_KEY").unwrap(),
            "resolved-secret"
        );
        assert_eq!(
            resolve_api_key("${FEADER_TEST_KEY}").unwrap(),
            "resolved-secret"
        );
        assert_eq!(resolve_api_key("literal-key").unwrap(), "literal-key");
        assert!(resolve_api_key("$FEADER_MISSING_VAR_XYZ").is_err());
        std::env::remove_var("FEADER_TEST_KEY");
    }
}
