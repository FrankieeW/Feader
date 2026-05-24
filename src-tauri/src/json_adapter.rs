//! JSON API feed adapter for sources that consume REST/JSON endpoints.
//!
//! Currently Weibo-specific (m.weibo.cn). When a second JSON provider lands, extract
//! a provider trait that handles auth headers, error detection, time parsing, and
//! content rendering — keep this file focused on the engine (fetch → parse → extract).

use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde_json::Value;

use crate::models::{ParsedArticle, ParsedFeed, XPathSelectors};

const JSON_FETCH_TIMEOUT_SECONDS: u64 = 20;
const MAX_JSON_PAGES: usize = 5;

static HTML_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<[^>]+>").expect("compile html tag regex"));
static TEMPLATE_VAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{([^}]+)\}").expect("compile template var regex"));

/// Weibo API request headers.
fn weibo_headers(uid: &str) -> Vec<(&'static str, String)> {
    vec![
        ("MWeibo-Pwa", "1".to_string()),
        ("X-Requested-With", "XMLHttpRequest".to_string()),
        (
            "User-Agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) AppleWebKit/605.1.15 \
             (KHTML, like Gecko) Version/16.0 Mobile/15E148 Safari/604.1"
                .to_string(),
        ),
        ("Referer", format!("https://m.weibo.cn/u/{uid}")),
    ]
}

/// Fetch and parse a JSON API feed into a ParsedFeed.
pub async fn fetch_json_feed(
    url: &str,
    selectors: &XPathSelectors,
    cookie: Option<&str>,
) -> Result<ParsedFeed, String> {
    let max_items = selectors.max_items.unwrap_or(40);
    let mut items: Vec<Value> = Vec::new();
    let mut current_url = url.to_string();

    for _page in 0..MAX_JSON_PAGES {
        let body = fetch_json_with_cookie(&current_url, cookie).await?;
        let root: Value =
            serde_json::from_str(&body).map_err(|e| format!("Failed to parse JSON: {e}"))?;

        // Check Weibo error code
        if root.get("ok").and_then(|v| v.as_i64()) == Some(-100) {
            return Err("Weibo returned ok=-100: login cookies required or expired".to_string());
        }

        let page_items = extract_items(&root, &selectors.items)?;
        let count_before = items.len();
        items.extend(page_items);

        if items.len() >= max_items {
            break;
        }

        // Resolve next cursor for cursor-based pagination
        let next_cursor = selectors
            .next_page
            .as_ref()
            .and_then(|path| resolve_json_path(&root, path))
            .and_then(|v| match v {
                Value::Number(n) => Some(n.to_string()),
                Value::String(s) => Some(s.clone()),
                _ => None,
            });

        let done = next_cursor.as_deref() == Some("0")
            || next_cursor.is_none()
            || items.len() == count_before;
        if done {
            break;
        }

        // Build next page URL by appending since_id cursor
        let since_id = next_cursor.unwrap();
        current_url = if url.contains('?') {
            format!("{url}&since_id={since_id}")
        } else {
            format!("{url}?since_id={since_id}")
        };
    }

    items.truncate(max_items);

    let articles: Vec<ParsedArticle> = items
        .iter()
        .filter_map(|item| parse_mblog_item(item, selectors))
        .collect();

    Ok(ParsedFeed {
        title: None,
        articles,
    })
}

async fn fetch_json_with_cookie(url: &str, cookie: Option<&str>) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(JSON_FETCH_TIMEOUT_SECONDS))
        .build()
        .map_err(|e| e.to_string())?;

    let mut req = client.get(url);

    // Apply Weibo headers if this is a Weibo URL
    if url.contains("m.weibo.cn") {
        let uid = url
            .split('&')
            .find(|p| p.starts_with("value="))
            .and_then(|p| p.strip_prefix("value="))
            .unwrap_or("0");
        for (key, val) in weibo_headers(uid) {
            req = req.header(key, &val);
        }
    }

    if let Some(c) = cookie {
        req = req.header("Cookie", c);
    }

    let resp = req.send().await.map_err(|e| format!("Fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| format!("Read body: {e}"))
}

/// Extract items from JSON using a path with optional filter.
/// Path format: "data.cards[?(@.card_type==9)]" or "data.cards"
fn extract_items(root: &Value, path: &str) -> Result<Vec<Value>, String> {
    let (base_path, filter) = if let Some(idx) = path.find("[?(@.") {
        let base = &path[..idx];
        let filter_part = &path[idx..];
        (base, Some(filter_part))
    } else {
        (path, None)
    };

    let arr = match resolve_json_path(root, base_path) {
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return Err(format!("Path '{}' did not resolve to an array", base_path)),
        None => return Err(format!("Path '{}' not found in response", base_path)),
    };

    match filter {
        Some(filter_str) => {
            let inner = filter_str
                .trim()
                .strip_prefix("[?(@.")
                .and_then(|s| s.strip_suffix(")]"))
                .ok_or_else(|| format!("Invalid filter syntax: {}", filter_str))?;

            let (key, val) = inner
                .split_once("==")
                .ok_or_else(|| format!("Invalid filter expression: {}", inner))?;

            let key = key.trim();
            let val_str = val.trim().trim_matches('"').trim_matches('\'');
            let val_num: Option<i64> = val_str.parse().ok();

            let filtered: Vec<Value> = arr
                .into_iter()
                .filter(|card| match resolve_json_path(card, key) {
                    Some(Value::Number(n)) => val_num.map_or(false, |vn| n.as_i64() == Some(vn)),
                    Some(Value::String(s)) => s == val_str,
                    _ => false,
                })
                .collect();
            Ok(filtered)
        }
        None => Ok(arr),
    }
}

/// Resolve a dot-separated JSON path with optional array indexing `[N]`.
/// Example: "user.screen_name", "pics[0].large.url"
///
/// Returns `None` for unsupported syntax (e.g. mid-path filters).
pub fn resolve_json_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for seg in path.split('.') {
        if seg.is_empty() {
            continue;
        }
        if let Some(bracket) = seg.find('[') {
            let name = &seg[..bracket];
            if !name.is_empty() {
                current = current.get(name)?;
            }
            let remaining = &seg[bracket..];
            if remaining.starts_with("[?") {
                // Mid-path filter syntax not supported — fail explicitly
                return None;
            }
            let inner = remaining.strip_prefix('[').and_then(|s| s.strip_suffix(']'))?;
            if let Ok(idx) = inner.parse::<usize>() {
                current = current.get(idx)?;
            }
        } else {
            current = current.get(seg)?;
        }
    }
    Some(current)
}

fn parse_mblog_item(item: &Value, selectors: &XPathSelectors) -> Option<ParsedArticle> {
    let author: String = selectors
        .author
        .as_ref()
        .and_then(|path| resolve_json_path(item, path))
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();

    let text = selectors
        .summary
        .as_ref()
        .and_then(|path| resolve_json_path(item, path))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let title = if author.is_empty() {
        strip_html_summary(text, 80)
    } else {
        format!("{}: {}", author, strip_html_summary(text, 60))
    };

    let url_str = build_article_url(item, &selectors.url)?;

    let raw_text = selectors
        .content
        .as_ref()
        .and_then(|path| resolve_json_path(item, path))
        .and_then(|v| v.as_str());

    let published_at = selectors
        .published_at
        .as_ref()
        .and_then(|path| resolve_json_path(item, path))
        .and_then(|v| v.as_str())
        .map(parse_weibo_time);

    let image_url = selectors
        .image
        .as_ref()
        .and_then(|path| resolve_json_path(item, path))
        .and_then(|v| v.as_str().map(String::from));

    let external_id = item
        .get("bid")
        .and_then(|v| v.as_str())
        .or_else(|| item.get("id").and_then(|v| v.as_str()))
        .map(String::from);

    let content_html = build_content_html(raw_text, item);

    Some(ParsedArticle {
        external_id,
        title,
        url: url_str,
        canonical_url: None,
        summary: Some(strip_html_summary(text, 200)),
        content_html,
        content_text: None,
        author: if author.is_empty() { None } else { Some(author) },
        published_at,
        image_url,
        tags_json: None,
    })
}

fn build_article_url(item: &Value, template: &str) -> Option<String> {
    if template.starts_with("http") {
        let mut url = template.to_string();
        let mut success = true;
        for cap in TEMPLATE_VAR_RE.captures_iter(template) {
            let placeholder = &cap[1];
            let value = resolve_json_path(item, placeholder).and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            });
            match value {
                Some(v) => url = url.replace(&format!("{{{}}}", placeholder), &v),
                None => success = false,
            }
        }
        if success {
            Some(url)
        } else {
            None
        }
    } else {
        let bid = resolve_json_path(item, template).and_then(|v| v.as_str().map(String::from))?;
        let uid = resolve_json_path(item, "user.id").and_then(|v| match v {
            Value::Number(n) => Some(n.to_string()),
            Value::String(s) => Some(s.clone()),
            _ => None,
        })?;
        Some(format!("https://weibo.com/{uid}/{bid}"))
    }
}

fn parse_weibo_time(s: &str) -> String {
    // Format: "Mon May 25 00:19:17 +0800 2026"
    // Reorder so timezone comes after year: chrono DateTime supports %z
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 6 {
        let reordered = format!(
            "{} {} {} {} {} {}",
            parts[0], parts[1], parts[2], parts[3], parts[5], parts[4]
        );
        if let Ok(dt) = chrono::DateTime::parse_from_str(&reordered, "%a %b %d %H:%M:%S %Y %z") {
            return dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        }
    }
    s.to_string()
}

fn strip_html_summary(html: &str, max_len: usize) -> String {
    let plain = HTML_TAG_RE.replace_all(html, "").to_string();
    let trimmed: String = plain
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .take(max_len)
        .collect();
    if plain.chars().count() > max_len {
        format!("{}...", trimmed.trim())
    } else {
        trimmed.trim().to_string()
    }
}

fn build_content_html(text: Option<&str>, item: &Value) -> Option<String> {
    let text = text?;
    if text.is_empty() {
        return None;
    }
    let mut html = text.to_string();

    // Append <img> tags for pictures
    if let Some(pics) = item.get("pics").and_then(|v| v.as_array()) {
        for pic in pics {
            if let Some(url) = pic
                .get("large")
                .and_then(|l| l.get("url"))
                .and_then(|u| u.as_str())
            {
                html.push_str(&format!(
                    "<br><img src=\"{}\" style=\"max-width:100%;margin-top:0.5rem\">",
                    url
                ));
            }
        }
    }

    // Render retweet quote block
    if let Some(retweet) = item.get("retweeted_status") {
        let rt_user = retweet
            .get("user")
            .and_then(|u| u.get("screen_name"))
            .and_then(|s| s.as_str())
            .unwrap_or("");
        let rt_text = retweet.get("text").and_then(|t| t.as_str()).unwrap_or("");
        html.push_str(&format!(
            "<blockquote style=\"margin:0.5rem 0;padding:0.5rem;border-left:3px solid #ccc;color:#666\">\
             <strong>@{} 转发:</strong><br>{}</blockquote>",
            rt_user, rt_text
        ));
    }

    Some(html)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_simple_path() {
        let v = json!({"user": {"screen_name": "人民日报"}});
        let result = resolve_json_path(&v, "user.screen_name");
        assert_eq!(result.and_then(|v| v.as_str()), Some("人民日报"));
    }

    #[test]
    fn resolves_array_index() {
        let v = json!({"pics": [{"large": {"url": "https://img.jpg"}}]});
        let result = resolve_json_path(&v, "pics[0].large.url");
        assert_eq!(result.and_then(|v| v.as_str()), Some("https://img.jpg"));
    }

    #[test]
    fn rejects_mid_path_filter() {
        let v = json!({"data": {"cards": [{"type": "text"}]}});
        let result = resolve_json_path(&v, "data.cards[?(@.type=='text')].content");
        assert!(result.is_none());
    }

    #[test]
    fn parses_weibo_time() {
        let result = parse_weibo_time("Mon May 25 00:19:17 +0800 2026");
        assert!(result.contains("2026-05-25T00:19:17"));
    }

    #[test]
    fn extracts_items_with_filter() {
        let v = json!({
            "data": {
                "cards": [
                    {"card_type": 9, "mblog": {"bid": "abc"}},
                    {"card_type": 156, "commend": true},
                    {"card_type": 9, "mblog": {"bid": "def"}}
                ]
            }
        });
        let items = extract_items(&v, "data.cards[?(@.card_type==9)]").unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn extracts_items_without_filter() {
        let v = json!({"data": {"cards": [{"a": 1}, {"a": 2}]}});
        let items = extract_items(&v, "data.cards").unwrap();
        assert_eq!(items.len(), 2);
    }
}
