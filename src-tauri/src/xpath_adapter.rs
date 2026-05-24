//! Declarative XPath source adapter for static HTML/XML pages.

use sxd_document::parser;
use sxd_xpath::{nodeset::Node, Context, Factory, Value};
use url::Url;

use crate::models::{
    ParsedArticle, ParsedFeed, XPathFieldDiagnostic, XPathPreview, XPathSelectors,
};

fn normalize_html(raw: &str) -> String {
    use html5ever::tendril::TendrilSink;

    let dom = html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default())
        .one(raw);
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
    xml.replace(" xmlns=\"http://www.w3.org/1999/xhtml\"", "")
        .replace(" xmlns=\"http://www.w3.org/2000/svg\"", "")
        .replace(" xmlns=\"http://www.w3.org/1998/Math/MathML\"", "")
}

/// Fetch a static page and extract articles with XPath selectors.
pub async fn fetch_xpath_source(
    url: &str,
    selectors: &XPathSelectors,
) -> Result<ParsedFeed, String> {
    let response = reqwest::Client::new()
        .get(url)
        .header("user-agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("XPath source request failed with status {status}"));
    }

    let body = response.text().await.map_err(|error| error.to_string())?;
    parse_xpath_source(url, &normalize_html(&body), selectors)
}

/// Fetch a static page and return extracted article samples plus selector diagnostics.
pub async fn preview_xpath_source(
    url: &str,
    selectors: &XPathSelectors,
) -> Result<XPathPreview, String> {
    let response = reqwest::Client::new()
        .get(url)
        .header("user-agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("XPath source request failed with status {status}"));
    }

    let body = response.text().await.map_err(|error| error.to_string())?;
    preview_xpath_document(url, &normalize_html(&body), selectors)
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

        articles.push(ParsedArticle {
            external_id: Some(url.clone()),
            title,
            url,
            canonical_url: None,
            summary: evaluate_optional_string(item, selectors.summary.as_deref())?,
            content_html: None,
            content_text: evaluate_optional_string(item, selectors.content.as_deref())?,
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
}
