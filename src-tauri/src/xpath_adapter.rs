//! Declarative XPath source adapter for static HTML/XML pages.

use std::time::Duration;

use sxd_document::dom::{ChildOfElement, Element};
use sxd_document::parser;
use sxd_xpath::{nodeset::Node, Context, Factory, Value};
use url::Url;

use crate::models::{
    ParsedArticle, ParsedFeed, XPathFieldDiagnostic, XPathPreview, XPathRulePack, XPathSelectors,
};
use crate::plugin_registry;

const MAX_XPATH_PAGES: usize = 5;
const XPATH_FETCH_TIMEOUT_SECONDS: u64 = 20;
const BODY_SNIPPET_CAP: usize = 120;

fn normalize_html(raw: &str) -> String {
    use html5ever::tendril::TendrilSink;

    let dom =
        html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default()).one(raw);
    let handle: markup5ever_rcdom::SerializableHandle = dom.document.clone().into();

    let mut buffer = Vec::new();
    if xml5ever::serialize::serialize(
        &mut buffer,
        &handle,
        xml5ever::serialize::SerializeOpts::default(),
    )
    .is_err()
    {
        return raw.to_string();
    }

    let xml = String::from_utf8(buffer).unwrap_or_else(|_| raw.to_string());
    let without_namespaces = xml
        .replace(" xmlns=\"http://www.w3.org/1999/xhtml\"", "")
        .replace(" xmlns=\"http://www.w3.org/2000/svg\"", "")
        .replace(" xmlns=\"http://www.w3.org/1998/Math/MathML\"", "");
    sanitize_xml_attribute_values(&escape_invalid_xml_ampersands(&strip_leading_doctype(
        &without_namespaces,
    )))
}

fn strip_leading_doctype(value: &str) -> String {
    let trimmed = value.trim_start();
    if !trimmed
        .get(..9)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<!doctype"))
    {
        return value.to_string();
    }
    let Some(end) = trimmed.find('>') else {
        return value.to_string();
    };
    trimmed[end + 1..].trim_start().to_string()
}

fn escape_invalid_xml_ampersands(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < value.len() {
        if bytes[index] == b'&' && !starts_valid_xml_entity(&value[index + 1..]) {
            escaped.push_str("&amp;");
            index += 1;
            continue;
        }
        let ch = value[index..]
            .chars()
            .next()
            .expect("index is on a UTF-8 boundary");
        escaped.push(ch);
        index += ch.len_utf8();
    }
    escaped
}

fn starts_valid_xml_entity(value: &str) -> bool {
    value.starts_with("amp;")
        || value.starts_with("lt;")
        || value.starts_with("gt;")
        || value.starts_with("quot;")
        || value.starts_with("apos;")
        || starts_numeric_xml_entity(value)
}

fn starts_numeric_xml_entity(value: &str) -> bool {
    if let Some(rest) = value
        .strip_prefix("#x")
        .or_else(|| value.strip_prefix("#X"))
    {
        let Some(end) = rest.find(';') else {
            return false;
        };
        return end > 0 && rest[..end].chars().all(|ch| ch.is_ascii_hexdigit());
    }
    if let Some(rest) = value.strip_prefix('#') {
        let Some(end) = rest.find(';') else {
            return false;
        };
        return end > 0 && rest[..end].chars().all(|ch| ch.is_ascii_digit());
    }
    false
}

fn sanitize_xml_attribute_values(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut in_tag = false;
    let mut quote: Option<char> = None;

    for ch in value.chars() {
        match (in_tag, quote, ch) {
            (false, _, '<') => {
                in_tag = true;
                sanitized.push(ch);
            }
            (true, None, '>') => {
                in_tag = false;
                sanitized.push(ch);
            }
            (true, None, '"' | '\'') => {
                quote = Some(ch);
                sanitized.push(ch);
            }
            (true, Some(active_quote), current) if current == active_quote => {
                quote = None;
                sanitized.push(ch);
            }
            (true, Some(_), '<') => sanitized.push_str("&lt;"),
            _ => sanitized.push(ch),
        }
    }

    sanitized
}

/// Fetch a static page and extract articles with XPath selectors.
pub async fn fetch_xpath_source(
    url: &str,
    selectors: &XPathSelectors,
) -> Result<ParsedFeed, String> {
    let mut visited = std::collections::HashSet::new();
    let mut current = url.to_string();
    let mut articles = Vec::new();
    let mut first_page = true;

    for _ in 0..MAX_XPATH_PAGES {
        if !visited.insert(current.clone()) {
            break;
        }

        // The first page is the source's primary content: its failure fails the
        // refresh. Later pages are best-effort — a failure there keeps the
        // articles already gathered instead of discarding the whole refresh.
        let body = match fetch_page(&current).await {
            Ok(body) => body,
            Err(error) if first_page => return Err(error),
            Err(_) => break,
        };
        let normalized = normalize_html_document(&body)?;
        let feed = match parse_xpath_source(&current, &normalized, selectors) {
            Ok(feed) => feed,
            Err(error) if first_page => return Err(error),
            Err(_) => break,
        };
        articles.extend(feed.articles);
        first_page = false;

        match next_page_url(&current, &normalized, selectors) {
            Ok(Some(next)) if !visited.contains(&next) => current = next,
            _ => break,
        }
    }

    Ok(ParsedFeed {
        title: None,
        articles,
    })
}

/// Fetch a static page and return extracted article samples plus selector diagnostics.
pub async fn preview_xpath_source(
    url: &str,
    selectors: &XPathSelectors,
) -> Result<XPathPreview, String> {
    let body = fetch_page(url).await?;
    preview_xpath_document(url, &normalize_html_document(&body)?, selectors)
}

/// Fetch a URL and return its normalized (real-world-tolerant) XHTML.
pub async fn fetch_normalized(url: &str) -> Result<String, String> {
    let body = fetch_page(url).await?;
    normalize_html_document(&body)
}

/// True when `expression` compiles as a valid XPath.
pub fn is_valid_xpath(expression: &str) -> bool {
    Factory::new().build(expression).ok().flatten().is_some()
}

async fn fetch_page(url: &str) -> Result<String, String> {
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(XPATH_FETCH_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| error.to_string())?
        .get(url)
        .header("user-agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("XPath source request failed with status {status}"));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_default();
    let body = response.text().await.map_err(|error| error.to_string())?;
    reject_non_html_body(&body, &content_type)?;
    Ok(body)
}

fn reject_non_html_body(body: &str, content_type: &str) -> Result<(), String> {
    let trimmed = body.trim_start();
    let content_type = content_type.to_ascii_lowercase();
    if content_type.contains("json")
        || trimmed.starts_with('{')
        || trimmed.starts_with('[')
        || trimmed.starts_with("for (;;);")
        || trimmed.starts_with(")]}'")
    {
        return Err(format!(
            "XPath sources require an HTML/XML page, but this URL returned JSON-like content: {}",
            body_snippet(trimmed)
        ));
    }
    Ok(())
}

fn body_snippet(body: &str) -> String {
    body.chars()
        .take(BODY_SNIPPET_CAP)
        .collect::<String>()
        .replace(['\n', '\r', '\t'], " ")
}

fn normalize_html_document(raw: &str) -> Result<String, String> {
    reject_non_html_body(raw, "")?;
    if looks_like_interstitial_document(raw) {
        return Err(
            "XPath sources require the static HTML page, but this URL returned an anti-bot or browser-check page."
                .to_string(),
        );
    }
    let normalized = normalize_html(raw);
    let trimmed = normalized.trim_start();
    if looks_like_interstitial_document(trimmed) {
        return Err(
            "XPath sources require the static HTML page, but this URL returned an anti-bot or browser-check page."
                .to_string(),
        );
    }
    if !(trimmed.starts_with('<') || trimmed.starts_with("<?xml")) {
        return Err(format!(
            "XPath sources require an HTML/XML page, but normalization produced non-XML content: {}",
            body_snippet(trimmed)
        ));
    }
    Ok(normalized)
}

pub fn looks_like_interstitial_document(document: &str) -> bool {
    let lower = document.to_ascii_lowercase();
    let has_challenge_marker = lower.contains("cf_chl") || lower.contains("challenge-platform");
    let browser_check_title = lower.contains("<title>just a moment")
        || lower.contains("<title>attention required")
        || lower.contains("enable javascript and cookies to continue");
    let cloudflare_interstitial = lower.contains("just a moment") && has_challenge_marker;

    browser_check_title || cloudflare_interstitial
}

/// Extract articles from a static HTML/XML document string.
pub fn parse_xpath_source(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
) -> Result<ParsedFeed, String> {
    validate_selectors(selectors)?;
    let package = parser::parse(document).map_err(|error| {
        format!("XPath adapter currently expects well-formed static HTML/XML: {error}")
    })?;
    let document = package.as_document();
    let context = Context::new();
    let item_xpath = compile_xpath(&selectors.items)?;
    let items = match item_xpath
        .evaluate(&context, document.root())
        .map_err(|error| error.to_string())?
    {
        Value::Nodeset(nodeset) => nodeset.document_order(),
        _ => return Err("XPath items selector must return nodes".to_string()),
    };

    let mut articles = Vec::new();
    for item in items {
        let Some(title) = evaluate_required_string(item, &selectors.title)? else {
            continue;
        };
        let Some(raw_url) = evaluate_required_string(item, &selectors.url)? else {
            continue;
        };
        let url = absolutize_url(base_url, &raw_url)?;
        let content_html = evaluate_content_html(item, selectors.content.as_deref())?;
        let content_text = if content_html.is_some() {
            None
        } else {
            evaluate_optional_string(item, selectors.content.as_deref())?
        };

        articles.push(ParsedArticle {
            external_id: Some(url.clone()),
            title,
            url,
            canonical_url: None,
            summary: evaluate_optional_string(item, selectors.summary.as_deref())?,
            content_html,
            content_text,
            author: evaluate_optional_string(item, selectors.author.as_deref())?,
            published_at: evaluate_optional_string(item, selectors.published_at.as_deref())?,
            image_url: evaluate_optional_string(item, selectors.image.as_deref())?
                .map(|value| absolutize_url(base_url, &value))
                .transpose()?,
            tags_json: None,
        });
    }

    Ok(ParsedFeed {
        title: None,
        articles,
    })
}

/// Preview a static HTML/XML document string with field-level selector diagnostics.
pub fn preview_xpath_document(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
) -> Result<XPathPreview, String> {
    let package = parser::parse(document).map_err(|error| {
        format!("XPath adapter currently expects well-formed static HTML/XML: {error}")
    })?;
    let document = package.as_document();
    let context = Context::new();
    let mut diagnostics = Vec::new();

    let items_expression = selectors.items.trim();
    if items_expression.is_empty() {
        diagnostics.push(field_diagnostic(
            "items",
            "Items",
            true,
            None,
            "missing",
            "Required selector is empty.",
            None,
        ));
        return Ok(XPathPreview {
            articles: Vec::new(),
            diagnostics,
            next_page_url: None,
        });
    }

    let item_xpath = match compile_xpath(items_expression) {
        Ok(xpath) => xpath,
        Err(error) => {
            diagnostics.push(field_diagnostic(
                "items",
                "Items",
                true,
                Some(items_expression),
                "invalid",
                &error,
                None,
            ));
            return Ok(XPathPreview {
                articles: Vec::new(),
                diagnostics,
                next_page_url: None,
            });
        }
    };

    let items = match item_xpath
        .evaluate(&context, document.root())
        .map_err(|error| error.to_string())?
    {
        Value::Nodeset(nodeset) => nodeset.document_order(),
        _ => {
            diagnostics.push(field_diagnostic(
                "items",
                "Items",
                true,
                Some(items_expression),
                "invalid",
                "Items selector must return nodes.",
                None,
            ));
            return Ok(XPathPreview {
                articles: Vec::new(),
                diagnostics,
                next_page_url: None,
            });
        }
    };

    let item_count = items.len().to_string();
    diagnostics.push(field_diagnostic(
        "items",
        "Items",
        true,
        Some(items_expression),
        if items.is_empty() { "empty" } else { "ok" },
        if items.is_empty() {
            "No item nodes matched."
        } else {
            "Item nodes matched."
        },
        Some(item_count.as_str()),
    ));

    for field in selector_fields(selectors) {
        diagnostics.push(diagnose_selector_field(field, &items));
    }
    diagnostics.push(diagnose_root_selector_field(
        SelectorField {
            field: "nextPage",
            label: "Next page",
            required: false,
            expression: selectors.next_page.as_deref(),
        },
        Node::Root(document.root()),
    ));

    let next_page_url =
        preview_optional_string(Node::Root(document.root()), selectors.next_page.as_deref())
            .map(|value| absolutize_url(base_url, &value))
            .transpose()?;

    let mut articles = Vec::new();
    let required_fields_ok = diagnostics
        .iter()
        .all(|diagnostic| !diagnostic.required || diagnostic.status == "ok");
    if !required_fields_ok {
        return Ok(XPathPreview {
            articles,
            diagnostics,
            next_page_url,
        });
    }

    for item in items.into_iter().take(5) {
        let Some(title) = evaluate_required_string(item, &selectors.title)? else {
            continue;
        };
        let Some(raw_url) = evaluate_required_string(item, &selectors.url)? else {
            continue;
        };
        let url = absolutize_url(base_url, &raw_url)?;

        articles.push(ParsedArticle {
            external_id: Some(url.clone()),
            title,
            url,
            canonical_url: None,
            summary: preview_optional_string(item, selectors.summary.as_deref()),
            content_html: None,
            content_text: preview_optional_string(item, selectors.content.as_deref()),
            author: preview_optional_string(item, selectors.author.as_deref()),
            published_at: preview_optional_string(item, selectors.published_at.as_deref()),
            image_url: preview_optional_string(item, selectors.image.as_deref())
                .map(|value| absolutize_url(base_url, &value))
                .transpose()?,
            tags_json: None,
        });
    }

    Ok(XPathPreview {
        articles,
        diagnostics,
        next_page_url,
    })
}

/// Try to turn a model's selector draft into a selector set that actually previews.
///
/// AI models often infer a useful `items` selector but return title/URL selectors that
/// are absolute to the document instead of relative to each item. The adapter extracts
/// fields from each item node, so validate against the same normalized document and
/// repair required fields with conservative relative candidates.
#[cfg(test)]
pub fn select_best_xpath_selectors_for_preview(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
) -> XPathSelectors {
    select_best_xpath_selectors_for_preview_with_packs(
        base_url,
        document,
        selectors,
        &plugin_registry::bundled_xpath_rule_packs(),
    )
}

pub fn select_best_xpath_selectors_for_preview_with_packs(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
    rule_packs: &[XPathRulePack],
) -> XPathSelectors {
    let mut candidates = vec![repair_required_selectors_for_preview(
        base_url, document, selectors,
    )];
    candidates.extend(
        known_selector_candidates(document, rule_packs)
            .into_iter()
            .map(|candidate| repair_required_selectors_for_preview(base_url, document, &candidate)),
    );

    candidates
        .into_iter()
        .enumerate()
        .max_by_key(|(index, candidate)| {
            preview_selector_score(base_url, document, candidate) + known_candidate_bonus(*index)
        })
        .map(|(_, candidate)| candidate)
        .unwrap_or_else(|| selectors.clone())
}

fn known_candidate_bonus(index: usize) -> usize {
    usize::from(index > 0) * 25
}

fn repair_required_selectors_for_preview(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
) -> XPathSelectors {
    if preview_xpath_document(base_url, document, selectors)
        .map(|preview| !preview.articles.is_empty())
        .unwrap_or(false)
    {
        return selectors.clone();
    }

    let mut improved = selectors.clone();
    if let Some(title) = first_working_required_selector(
        base_url,
        document,
        &improved,
        "title",
        &[
            improved.title.as_str(),
            ".//a[normalize-space()][1]",
            ".//*[contains(@class,'title')][1]",
            ".//*[contains(@class,'name')][1]",
            ".//a[@title][1]/@title",
            ".//img[@alt][1]/@alt",
        ],
    ) {
        improved.title = title;
    }
    if let Some(url) = first_working_required_selector(
        base_url,
        document,
        &improved,
        "url",
        &[
            improved.url.as_str(),
            ".//a[@href][1]/@href",
            ".//a[normalize-space()][1]/@href",
            ".//*[contains(@class,'title')]//a[@href][1]/@href",
            ".//*[contains(@class,'name')]//a[@href][1]/@href",
        ],
    ) {
        improved.url = url;
    }

    improved
}

fn known_selector_candidates(document: &str, rule_packs: &[XPathRulePack]) -> Vec<XPathSelectors> {
    plugin_registry::matching_selector_candidates_in_packs(document, rule_packs)
}

fn preview_selector_score(base_url: &str, document: &str, selectors: &XPathSelectors) -> usize {
    let Ok(preview) = preview_xpath_document(base_url, document, selectors) else {
        return 0;
    };
    let article_score = preview.articles.len().min(5) * 100;
    let diagnostic_score = preview
        .diagnostics
        .iter()
        .map(
            |diagnostic| match (diagnostic.required, diagnostic.status.as_str()) {
                (true, "ok") => 40,
                (true, _) => 0,
                (false, "ok") => 8,
                _ => 0,
            },
        )
        .sum::<usize>();
    let next_page_score = usize::from(preview.next_page_url.is_some()) * 10;
    article_score + diagnostic_score + next_page_score
}

fn first_working_required_selector(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
    field: &str,
    candidates: &[&str],
) -> Option<String> {
    candidates
        .iter()
        .map(|candidate| candidate.trim())
        .filter(|candidate| !candidate.is_empty())
        .find_map(|candidate| {
            let mut draft = selectors.clone();
            match field {
                "title" => draft.title = candidate.to_string(),
                "url" => draft.url = candidate.to_string(),
                _ => return None,
            }
            let preview = preview_xpath_document(base_url, document, &draft).ok()?;
            preview
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.field == field && diagnostic.status == "ok")
                .then(|| candidate.to_string())
        })
}

fn next_page_url(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
) -> Result<Option<String>, String> {
    let package = parser::parse(document).map_err(|error| {
        format!("XPath adapter currently expects well-formed static HTML/XML: {error}")
    })?;
    let document = package.as_document();
    let raw = preview_optional_string(Node::Root(document.root()), selectors.next_page.as_deref());
    raw.map(|value| absolutize_url(base_url, &value))
        .transpose()
}

fn validate_selectors(selectors: &XPathSelectors) -> Result<(), String> {
    if selectors.items.trim().is_empty() {
        return Err("XPath items selector is required".to_string());
    }
    if selectors.title.trim().is_empty() {
        return Err("XPath title selector is required".to_string());
    }
    if selectors.url.trim().is_empty() {
        return Err("XPath URL selector is required".to_string());
    }
    Ok(())
}

struct SelectorField<'a> {
    field: &'static str,
    label: &'static str,
    required: bool,
    expression: Option<&'a str>,
}

fn selector_fields(selectors: &XPathSelectors) -> [SelectorField<'_>; 7] {
    [
        SelectorField {
            field: "title",
            label: "Title",
            required: true,
            expression: Some(selectors.title.as_str()),
        },
        SelectorField {
            field: "url",
            label: "URL",
            required: true,
            expression: Some(selectors.url.as_str()),
        },
        SelectorField {
            field: "summary",
            label: "Summary",
            required: false,
            expression: selectors.summary.as_deref(),
        },
        SelectorField {
            field: "publishedAt",
            label: "Date",
            required: false,
            expression: selectors.published_at.as_deref(),
        },
        SelectorField {
            field: "author",
            label: "Author",
            required: false,
            expression: selectors.author.as_deref(),
        },
        SelectorField {
            field: "content",
            label: "Content",
            required: false,
            expression: selectors.content.as_deref(),
        },
        SelectorField {
            field: "image",
            label: "Image",
            required: false,
            expression: selectors.image.as_deref(),
        },
    ]
}

fn diagnose_selector_field(field: SelectorField<'_>, items: &[Node<'_>]) -> XPathFieldDiagnostic {
    let expression = field
        .expression
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(expression) = expression else {
        return field_diagnostic(
            field.field,
            field.label,
            field.required,
            None,
            if field.required { "missing" } else { "unset" },
            if field.required {
                "Required selector is empty."
            } else {
                "Optional selector is not configured."
            },
            None,
        );
    };

    if let Err(error) = compile_xpath(expression) {
        return field_diagnostic(
            field.field,
            field.label,
            field.required,
            Some(expression),
            "invalid",
            &error,
            None,
        );
    }

    let values = items
        .iter()
        .filter_map(|item| {
            evaluate_optional_string(*item, Some(expression))
                .ok()
                .flatten()
        })
        .collect::<Vec<_>>();
    let status = if values.is_empty() { "empty" } else { "ok" };
    let message = if values.is_empty() {
        "No values found in preview items."
    } else {
        "Values found in preview items."
    };

    field_diagnostic(
        field.field,
        field.label,
        field.required,
        Some(expression),
        status,
        message,
        values.first().map(String::as_str),
    )
}

fn diagnose_root_selector_field(field: SelectorField<'_>, root: Node<'_>) -> XPathFieldDiagnostic {
    let expression = field
        .expression
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(expression) = expression else {
        return field_diagnostic(
            field.field,
            field.label,
            field.required,
            None,
            "unset",
            "Optional selector is not configured.",
            None,
        );
    };

    match evaluate_optional_string(root, Some(expression)) {
        Ok(Some(value)) => field_diagnostic(
            field.field,
            field.label,
            field.required,
            Some(expression),
            "ok",
            "Value found in document.",
            Some(value.as_str()),
        ),
        Ok(None) => field_diagnostic(
            field.field,
            field.label,
            field.required,
            Some(expression),
            "empty",
            "No value found in document.",
            None,
        ),
        Err(error) => field_diagnostic(
            field.field,
            field.label,
            field.required,
            Some(expression),
            "invalid",
            &error,
            None,
        ),
    }
}

fn field_diagnostic(
    field: &str,
    label: &str,
    required: bool,
    expression: Option<&str>,
    status: &str,
    message: &str,
    sample: Option<&str>,
) -> XPathFieldDiagnostic {
    XPathFieldDiagnostic {
        field: field.to_string(),
        label: label.to_string(),
        required,
        expression: expression.map(str::to_string),
        status: status.to_string(),
        message: message.to_string(),
        sample: sample.map(str::to_string),
    }
}

fn evaluate_required_string(node: Node<'_>, expression: &str) -> Result<Option<String>, String> {
    evaluate_optional_string(node, Some(expression))
}

fn preview_optional_string(node: Node<'_>, expression: Option<&str>) -> Option<String> {
    evaluate_optional_string(node, expression).ok().flatten()
}

fn evaluate_optional_string(
    node: Node<'_>,
    expression: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(expression) = expression.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let xpath = compile_xpath(expression)?;
    let value = xpath
        .evaluate(&Context::new(), node)
        .map_err(|error| error.to_string())?;
    let text = match value {
        Value::Nodeset(nodeset) => nodeset
            .document_order()
            .into_iter()
            .next()
            .map(|node| node.string_value())
            .unwrap_or_default(),
        other => other.string(),
    };
    let text = text.trim().to_string();
    Ok((!text.is_empty()).then_some(text))
}

fn node_inner_html(element: Element<'_>) -> String {
    let mut out = String::new();
    for child in element.children() {
        serialize_child(child, &mut out);
    }
    out
}

fn serialize_child(child: ChildOfElement<'_>, out: &mut String) {
    match child {
        ChildOfElement::Element(element) => {
            let name = element.name().local_part();
            out.push('<');
            out.push_str(name);
            for attribute in element.attributes() {
                out.push(' ');
                out.push_str(attribute.name().local_part());
                out.push_str("=\"");
                out.push_str(&escape_html(attribute.value(), true));
                out.push('"');
            }
            out.push('>');
            if is_void_element(name) {
                return;
            }
            for grandchild in element.children() {
                serialize_child(grandchild, out);
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        ChildOfElement::Text(text) => out.push_str(&escape_html(text.text(), false)),
        _ => {}
    }
}

fn is_void_element(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

fn escape_html(value: &str, in_attribute: bool) -> String {
    let mut escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if in_attribute {
        escaped = escaped.replace('"', "&quot;");
    }
    escaped
}

fn evaluate_content_html(
    item: Node<'_>,
    expression: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(expression) = expression.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let xpath = compile_xpath(expression)?;
    let value = xpath
        .evaluate(&Context::new(), item)
        .map_err(|error| error.to_string())?;
    if let Value::Nodeset(nodeset) = value {
        if let Some(Node::Element(element)) = nodeset.document_order().into_iter().next() {
            let html = node_inner_html(element);
            return Ok((!html.trim().is_empty()).then_some(html));
        }
    }
    Ok(None)
}

fn compile_xpath(expression: &str) -> Result<sxd_xpath::XPath, String> {
    Factory::new()
        .build(expression)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Invalid XPath expression: {expression}"))
}

fn absolutize_url(base_url: &str, value: &str) -> Result<String, String> {
    let base = Url::parse(base_url).map_err(|error| error.to_string())?;
    base.join(value)
        .map(|url| url.to_string())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selectors() -> XPathSelectors {
        XPathSelectors {
            items: "//article".to_string(),
            title: ".//h2/a/text()".to_string(),
            url: ".//h2/a/@href".to_string(),
            summary: Some(".//p/text()".to_string()),
            published_at: Some(".//time/@datetime".to_string()),
            author: Some(".//*[contains(@class, 'author')]/text()".to_string()),
            content: Some(".//section/text()".to_string()),
            image: Some(".//img/@src".to_string()),
            next_page: None,
        }
    }

    #[test]
    fn normalizes_malformed_html_for_extraction() {
        let messy = r#"<article><h2><a href="/one">First</a></h2><p>Summary one<br>more</article>"#;
        let feed = parse_xpath_source(
            "https://example.com/blog/",
            &normalize_html(messy),
            &selectors(),
        )
        .expect("xpath extracts from normalized html");

        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "First");
        assert_eq!(feed.articles[0].url, "https://example.com/one");
    }

    #[test]
    fn strips_doctype_before_xml_xpath_parse() {
        let html = r#"<!doctype html><html><body><article><h2><a href="/one">First</a></h2></article></body></html>"#;
        let normalized = normalize_html_document(html).expect("normalizes");
        assert!(!normalized
            .trim_start()
            .to_ascii_lowercase()
            .starts_with("<!doctype"));

        let feed = parse_xpath_source("https://example.com/blog/", &normalized, &selectors())
            .expect("xpath extracts after stripping doctype");
        assert_eq!(feed.articles.len(), 1);
    }

    #[test]
    fn escapes_bare_ampersands_for_xml_xpath_parse() {
        let html = r#"<html><body><article><h2><a href="/one?genre=a&secure=b">First & second</a></h2></article></body></html>"#;
        let normalized = normalize_html_document(html).expect("normalizes");
        assert!(normalized.contains("genre=a&amp;secure=b"));

        let feed = parse_xpath_source("https://example.com/blog/", &normalized, &selectors())
            .expect("xpath extracts after escaping ampersands");
        assert_eq!(
            feed.articles[0].url,
            "https://example.com/one?genre=a&secure=b"
        );
    }

    #[test]
    fn escapes_markup_inside_attribute_values_for_xml_xpath_parse() {
        let html = r#"<html><body><input type="hidden" value="<p>Hidden HTML</p>"><article><h2><a href="/one">First</a></h2></article></body></html>"#;
        let normalized = normalize_html_document(html).expect("normalizes");
        assert!(normalized.contains("value=\"&lt;p>Hidden HTML&lt;/p>\""));

        let feed = parse_xpath_source("https://example.com/blog/", &normalized, &selectors())
            .expect("xpath extracts after escaping attribute markup");
        assert_eq!(feed.articles.len(), 1);
    }

    #[test]
    fn content_selector_captures_inner_html() {
        let mut selectors = selectors();
        selectors.content = Some(".//section".to_string());

        let feed = parse_xpath_source(
            "https://example.com/blog/",
            r#"
            <html><body>
              <article>
                <h2><a href="/one">First</a></h2>
                <section><strong>Bold</strong> and <em>italic</em></section>
              </article>
            </body></html>
            "#,
            &selectors,
        )
        .expect("xpath extracts");

        let html = feed.articles[0].content_html.as_deref().unwrap_or_default();
        assert!(
            html.contains("<strong>"),
            "expected inner tags, got: {html}"
        );
        assert!(html.contains("Bold"));
    }

    #[test]
    fn content_inner_html_self_closes_void_elements() {
        let mut selectors = selectors();
        selectors.content = Some(".//section".to_string());

        let feed = parse_xpath_source(
            "https://example.com/blog/",
            r#"
            <html><body>
              <article>
                <h2><a href="/one">First</a></h2>
                <section>Pic<img src="/a.png"/>end</section>
              </article>
            </body></html>
            "#,
            &selectors,
        )
        .expect("xpath extracts");

        let html = feed.articles[0].content_html.as_deref().unwrap_or_default();
        assert!(html.contains("<img src=\"/a.png\">"), "got: {html}");
        assert!(
            !html.contains("</img>"),
            "void element must not close, got: {html}"
        );
    }

    #[test]
    fn resolves_absolute_next_page_url() {
        let mut selectors = selectors();
        selectors.next_page = Some("//a[@rel='next']/@href".to_string());

        let next = next_page_url(
            "https://example.com/blog/",
            r#"<html><body><a rel="next" href="/page/2">Next</a></body></html>"#,
            &selectors,
        )
        .expect("next page resolves");

        assert_eq!(next.as_deref(), Some("https://example.com/page/2"));
    }

    #[test]
    fn extracts_articles_from_static_markup() {
        let feed = parse_xpath_source(
            "https://example.com/blog/",
            r#"
            <html>
              <body>
                <article>
                  <h2><a href="/one">First</a></h2>
                  <p>Summary one</p>
                  <time datetime="2024-01-01T00:00:00Z"></time>
                  <span class="author">Ada</span>
                  <section>Body one</section>
                  <img src="/one.png" />
                </article>
                <article>
                  <h2><a href="https://example.com/two">Second</a></h2>
                  <p>Summary two</p>
                </article>
              </body>
            </html>
            "#,
            &selectors(),
        )
        .expect("xpath extracts");

        assert_eq!(feed.articles.len(), 2);
        assert_eq!(feed.articles[0].title, "First");
        assert_eq!(feed.articles[0].url, "https://example.com/one");
        assert_eq!(
            feed.articles[0].image_url.as_deref(),
            Some("https://example.com/one.png")
        );
        assert_eq!(feed.articles[1].url, "https://example.com/two");
    }

    #[test]
    fn skips_items_without_title_or_url() {
        let feed = parse_xpath_source(
            "https://example.com/blog/",
            r#"
            <html>
              <body>
                <article><h2>Missing URL</h2></article>
                <article><h2><a href="/valid">Valid</a></h2></article>
              </body>
            </html>
            "#,
            &selectors(),
        )
        .expect("xpath extracts");

        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "Valid");
    }

    #[test]
    fn previews_selector_diagnostics_and_next_page() {
        let mut selectors = selectors();
        selectors.next_page = Some("//a[@rel='next']/@href".to_string());

        let preview = preview_xpath_document(
            "https://example.com/blog/",
            r#"
            <html>
              <body>
                <article>
                  <h2><a href="/one">First</a></h2>
                  <p>Summary one</p>
                </article>
                <a rel="next" href="/page/2">Next</a>
              </body>
            </html>
            "#,
            &selectors,
        )
        .expect("xpath preview extracts diagnostics");

        assert_eq!(preview.articles.len(), 1);
        assert_eq!(
            preview.next_page_url.as_deref(),
            Some("https://example.com/page/2")
        );
        assert!(preview
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == "items"
                && diagnostic.sample.as_deref() == Some("1")));
        assert!(preview
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == "nextPage" && diagnostic.status == "ok"));
    }

    #[test]
    fn previews_invalid_required_selector_without_extracting_articles() {
        let mut selectors = selectors();
        selectors.title = ".//h2/[".to_string();

        let preview = preview_xpath_document(
            "https://example.com/blog/",
            r#"
            <html>
              <body>
                <article><h2><a href="/one">First</a></h2></article>
              </body>
            </html>
            "#,
            &selectors,
        )
        .expect("xpath preview returns diagnostics");

        assert!(preview.articles.is_empty());
        assert!(preview
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == "title" && diagnostic.status == "invalid"));
    }

    #[test]
    fn improves_ai_draft_required_selectors_against_preview() {
        let document = normalize_html_document(
            r#"
            <html><body>
              <ul>
                <li class="card"><a href="/one"><img alt="First title" src="/one.jpg"></a></li>
                <li class="card"><a href="/two"><img alt="Second title" src="/two.jpg"></a></li>
              </ul>
            </body></html>
            "#,
        )
        .expect("normalizes");
        let draft = XPathSelectors {
            items: "//li[contains(@class,'card')]".to_string(),
            title: ".//h3".to_string(),
            url: ".//h3/a/@href".to_string(),
            summary: None,
            published_at: None,
            author: None,
            content: None,
            image: Some(".//img/@src".to_string()),
            next_page: None,
        };

        let broken = preview_xpath_document("https://example.com/list", &document, &draft)
            .expect("previews broken draft");
        assert!(broken.articles.is_empty());

        let improved =
            select_best_xpath_selectors_for_preview("https://example.com/list", &document, &draft);
        let preview = preview_xpath_document("https://example.com/list", &document, &improved)
            .expect("previews improved selectors");

        assert_eq!(improved.title, ".//img[@alt][1]/@alt");
        assert_eq!(improved.url, ".//a[@href][1]/@href");
        assert_eq!(preview.articles.len(), 2);
        assert_eq!(preview.articles[0].title, "First title");
        assert_eq!(preview.articles[0].url, "https://example.com/one");
    }

    #[test]
    fn selects_discuz_thread_candidate_over_bad_ai_draft() {
        let document = normalize_html_document(
            r#"
            <html><body>
              <ul id="threadlisttableid">
                <li class="kmlist common">
                  <a class="kmtit" href="thread-1-1-1.html"><span class="km_subject">Forum title</span></a>
                  <div class="kminfo"><a href="space-uid-1.html">Ada</a><span class="kmtime">发表于 <span title="2026-5-24">today</span></span></div>
                </li>
              </ul>
              <a class="nxt" href="forum.php?page=2">下一页</a>
            </body></html>
            "#,
        )
        .expect("normalizes");
        let draft = XPathSelectors {
            items: "//li".to_string(),
            title: ".//h2".to_string(),
            url: ".//h2/a/@href".to_string(),
            summary: None,
            published_at: None,
            author: None,
            content: None,
            image: None,
            next_page: None,
        };

        let selected =
            select_best_xpath_selectors_for_preview("https://forum.example/", &document, &draft);
        let preview = preview_xpath_document("https://forum.example/", &document, &selected)
            .expect("previews selected forum selectors");

        assert_eq!(preview.articles.len(), 1);
        assert_eq!(preview.articles[0].title, "Forum title");
        assert_eq!(
            preview.articles[0].url,
            "https://forum.example/thread-1-1-1.html"
        );
        assert_eq!(preview.articles[0].author.as_deref(), Some("Ada"));
        assert_eq!(
            preview.next_page_url.as_deref(),
            Some("https://forum.example/forum.php?page=2")
        );
    }

    #[test]
    fn selects_maccms_detail_candidate_for_single_detail_page() {
        let document = normalize_html_document(
            r#"
            <html><body>
              <div class="detail_list_box">
                <div class="content_box">
                  <a class="vodlist_thumb" href="/index.php/vod/play/id/81060/sid/1/nid/1.html" data-original="https://img.example/poster.png"></a>
                  <h2 class="title">Video title</h2>
                  <ul>
                    <li class="data"><span>状态：</span><span class="data_style">更新至第07集</span> / <em>05-19</em></li>
                    <li class="data"><span>主演：</span><a href="/actor/a">Actor A</a></li>
                    <li class="desc"><span>简介：</span>Video summary</li>
                  </ul>
                  <a class="btn btn_primary" href="/index.php/vod/play/id/81060/sid/1/nid/1.html">立即播放</a>
                </div>
              </div>
            </body></html>
            "#,
        )
        .expect("normalizes");
        let draft = XPathSelectors {
            items: "//li".to_string(),
            title: ".//h2".to_string(),
            url: ".//h2/a/@href".to_string(),
            summary: None,
            published_at: None,
            author: None,
            content: None,
            image: None,
            next_page: None,
        };

        let selected = select_best_xpath_selectors_for_preview(
            "https://video.example/detail",
            &document,
            &draft,
        );
        let preview = preview_xpath_document("https://video.example/detail", &document, &selected)
            .expect("previews selected video selectors");

        assert_eq!(preview.articles.len(), 1);
        assert_eq!(preview.articles[0].title, "Video title");
        assert_eq!(
            preview.articles[0].url,
            "https://video.example/index.php/vod/play/id/81060/sid/1/nid/1.html"
        );
        assert_eq!(
            preview.articles[0].image_url.as_deref(),
            Some("https://img.example/poster.png")
        );
    }

    #[test]
    fn rejects_json_like_xpath_source_body() {
        let error = reject_non_html_body("{\"items\":[]}", "application/json").unwrap_err();
        assert!(error.contains("returned JSON-like content"));
    }

    #[test]
    fn rejects_json_like_normalized_document() {
        let error = normalize_html_document("{\"items\":[]}").unwrap_err();
        assert!(error.contains("returned JSON-like content"));
    }

    #[test]
    fn rejects_cloudflare_challenge_page() {
        let error =
            normalize_html_document("<html><head><title>Just a moment...</title></head><body><script>window._cf_chl_opt={}</script></body></html>")
                .unwrap_err();
        assert!(error.contains("anti-bot"));
    }

    #[test]
    fn allows_content_pages_with_cloudflare_footer_scripts() {
        let normalized = normalize_html_document(
            r#"<html><head><title>Real page</title></head><body><article><h2><a href="/one">First</a></h2></article><script src="/cdn-cgi/challenge-platform/scripts/jsd/main.js"></script></body></html>"#,
        )
        .expect("normal content pages may include Cloudflare scripts");

        let feed = parse_xpath_source("https://example.com/", &normalized, &selectors())
            .expect("xpath extracts from real content page");
        assert_eq!(feed.articles.len(), 1);
    }
}
