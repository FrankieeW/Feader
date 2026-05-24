//! AI provider client for selector suggestions.

use serde::Deserialize;

use crate::models::{env_reference_name, AiSettings, XPathSelectors};
use crate::xpath_adapter::is_valid_xpath;

const AI_HTML_CHAR_CAP: usize = 12_000;

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
pub fn parse_selectors_json(text: &str) -> Result<XPathSelectors, String> {
    let json = extract_json_object(text).ok_or("AI response did not contain JSON")?;
    let raw: SuggestedSelectors = serde_json::from_str(&json).map_err(|error| error.to_string())?;

    let items = keep_valid(raw.items).unwrap_or_default();
    let title = keep_valid(raw.title).unwrap_or_default();
    let url = keep_valid(raw.url).unwrap_or_default();
    if items.is_empty() || title.is_empty() || url.is_empty() {
        return Err("AI did not return usable selectors".to_string());
    }

    Ok(XPathSelectors {
        items,
        title,
        url,
        summary: keep_valid(raw.summary),
        published_at: keep_valid(raw.published_at),
        author: keep_valid(raw.author),
        content: keep_valid(raw.content),
        image: keep_valid(raw.image),
        next_page: keep_valid(raw.next_page),
    })
}

fn build_prompt(html: &str) -> String {
    format!(
        "You generate XPath selectors for scraping an article-listing web page.\n\
         Return ONLY a JSON object with string keys: items, title, url, summary, \
         publishedAt, author, content, image, nextPage. Each value is an XPath expression; \
         use \"\" when not applicable. `items` selects each repeating article element; the \
         other selectors are relative to an item except `nextPage` (document-level). \
         No prose, no code fences.\n\nHTML:\n{html}"
    )
}

/// Ask the configured provider to suggest selectors for a page's HTML.
pub async fn suggest_xpath_selectors(
    settings: &AiSettings,
    stored_api_key: &str,
    page_html: &str,
) -> Result<XPathSelectors, String> {
    let base_url = settings.base_url.trim();
    if base_url.is_empty() {
        return Err("AI base URL is not configured".to_string());
    }
    if settings.model.trim().is_empty() {
        return Err("AI model is not configured".to_string());
    }

    let key = resolve_api_key(stored_api_key)?;
    let html: String = page_html.chars().take(AI_HTML_CHAR_CAP).collect();
    let prompt = build_prompt(&html);

    let text = match settings.provider.as_str() {
        "anthropic" => call_anthropic(settings, &key, &prompt).await?,
        "openai" => call_openai(settings, &key, &prompt).await?,
        other => return Err(format!("Unknown AI provider '{other}'")),
    };
    parse_selectors_json(&text)
}

async fn call_anthropic(settings: &AiSettings, key: &str, prompt: &str) -> Result<String, String> {
    let endpoint = format!("{}/v1/messages", settings.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": &settings.model,
        "max_tokens": 1024,
        "messages": [{ "role": "user", "content": prompt }],
    });
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("x-api-key", key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("AI request failed with status {}", response.status()));
    }
    let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    value["content"][0]["text"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "Unexpected Anthropic response shape".to_string())
}

async fn call_openai(settings: &AiSettings, key: &str, prompt: &str) -> Result<String, String> {
    let endpoint = format!("{}/chat/completions", settings.base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": &settings.model,
        "messages": [{ "role": "user", "content": prompt }],
    });
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("authorization", format!("Bearer {key}"))
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("AI request failed with status {}", response.status()));
    }
    let value: serde_json::Value = response.json().await.map_err(|error| error.to_string())?;
    value["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "Unexpected OpenAI response shape".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selectors_from_model_text() {
        let text = "Sure:\n```json\n{\"items\":\"//article\",\"title\":\".//h2/a\",\"url\":\".//h2/a/@href\",\"summary\":\"\",\"content\":\".//section\",\"image\":\".//img/@src\",\"author\":null,\"publishedAt\":\".//time/@datetime\",\"nextPage\":\"\"}\n```";
        let selectors = parse_selectors_json(text).expect("parses");
        assert_eq!(selectors.items, "//article");
        assert_eq!(selectors.content.as_deref(), Some(".//section"));
        assert_eq!(selectors.summary, None);
    }

    #[test]
    fn rejects_when_required_selectors_missing() {
        let text = "{\"items\":\"\",\"title\":\".//h2/a\",\"url\":\".//h2/a/@href\"}";
        assert!(parse_selectors_json(text).is_err());
    }

    #[test]
    fn resolves_env_reference_key() {
        std::env::set_var("FEADER_TEST_KEY", "resolved-secret");
        assert_eq!(resolve_api_key("$FEADER_TEST_KEY").unwrap(), "resolved-secret");
        assert_eq!(resolve_api_key("${FEADER_TEST_KEY}").unwrap(), "resolved-secret");
        assert_eq!(resolve_api_key("literal-key").unwrap(), "literal-key");
        assert!(resolve_api_key("$FEADER_MISSING_VAR_XYZ").is_err());
        std::env::remove_var("FEADER_TEST_KEY");
    }
}
