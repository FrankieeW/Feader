//! Declarative XPath source adapter for static HTML/XML pages.

use sxd_document::parser;
use sxd_xpath::{nodeset::Node, Context, Factory, Value};
use url::Url;

use crate::models::{ParsedArticle, ParsedFeed, XPathSelectors};

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
    parse_xpath_source(url, &body, selectors)
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

fn evaluate_required_string(node: Node<'_>, expression: &str) -> Result<Option<String>, String> {
    evaluate_optional_string(node, Some(expression))
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
}
