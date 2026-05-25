//! RSS and Atom fetching/parsing adapter.

use std::sync::OnceLock;

use feed_rs::parser;
use wreq::header::{ACCEPT, ACCEPT_LANGUAGE};
use wreq::Client;
use wreq_util::Emulation;

use crate::error::Result;
use crate::models::{ParsedArticle, ParsedFeed};

const FEED_ACCEPT: &str = "application/rss+xml, application/atom+xml, application/xml;q=0.9, text/xml;q=0.8, */*;q=0.7";
const FEED_ACCEPT_LANGUAGE: &str = "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7";
static FEED_CLIENT: OnceLock<Client> = OnceLock::new();

/// Fetch and parse a remote RSS or Atom document.
pub async fn fetch_feed(url: &str) -> Result<ParsedFeed> {
    let response = feed_http_client()
        .get(url)
        .header(ACCEPT, FEED_ACCEPT)
        .header(ACCEPT_LANGUAGE, FEED_ACCEPT_LANGUAGE)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Feed request failed with status {status}").into());
    }

    let bytes = response.bytes().await?;
    parse_feed_bytes(&bytes)
}

fn feed_http_client() -> &'static Client {
    FEED_CLIENT.get_or_init(|| {
        Client::builder()
            .emulation(Emulation::Chrome133)
            .http1_only()
            .build()
            .expect("feed HTTP client configuration is valid")
    })
}

/// Parse RSS or Atom bytes into Feader's normalized feed shape.
pub fn parse_feed_bytes(bytes: &[u8]) -> Result<ParsedFeed> {
    let feed = parser::parse(bytes).map_err(|error| error.to_string())?;
    let title = feed.title.map(|text| text.content);
    let articles = feed
        .entries
        .into_iter()
        .filter_map(|entry| {
            let url = entry
                .links
                .iter()
                .find(|link| link.rel.as_deref().unwrap_or("alternate") == "alternate")
                .or_else(|| entry.links.first())
                .map(|link| link.href.clone())?;
            let title = entry
                .title
                .map(|text| text.content)
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| url.clone());
            let author = entry
                .authors
                .first()
                .map(|person| person.name.clone())
                .filter(|value| !value.trim().is_empty());
            let content_html = entry
                .content
                .as_ref()
                .and_then(|content| content.body.clone())
                .filter(|value| !value.trim().is_empty());
            let summary = entry
                .summary
                .map(|text| text.content)
                .filter(|value| !value.trim().is_empty());
            let published_at = entry
                .published
                .or(entry.updated)
                .map(|date| date.to_rfc3339());

            Some(ParsedArticle {
                external_id: Some(entry.id).filter(|value| !value.trim().is_empty()),
                title,
                url,
                canonical_url: None,
                summary,
                content_html,
                content_text: None,
                author,
                published_at,
                image_url: None,
                tags_json: None,
            })
        })
        .collect();

    Ok(ParsedFeed { title, articles })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rss_fixture() {
        let feed = parse_feed_bytes(
            br#"
            <rss version="2.0">
              <channel>
                <title>Example RSS</title>
                <item>
                  <guid>one</guid>
                  <title>First article</title>
                  <link>https://example.com/one</link>
                  <description>Summary</description>
                  <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
                </item>
              </channel>
            </rss>
            "#,
        )
        .expect("rss parses");

        assert_eq!(feed.title.as_deref(), Some("Example RSS"));
        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "First article");
        assert_eq!(feed.articles[0].url, "https://example.com/one");
    }

    #[test]
    fn parses_atom_fixture() {
        let feed = parse_feed_bytes(
            br#"
            <feed xmlns="http://www.w3.org/2005/Atom">
              <title>Example Atom</title>
              <entry>
                <id>tag:example.com,2024:one</id>
                <title>Atom article</title>
                <link href="https://example.com/atom-one" />
                <updated>2024-01-01T00:00:00Z</updated>
                <summary>Atom summary</summary>
              </entry>
            </feed>
            "#,
        )
        .expect("atom parses");

        assert_eq!(feed.title.as_deref(), Some("Example Atom"));
        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "Atom article");
        assert_eq!(feed.articles[0].url, "https://example.com/atom-one");
    }

    #[test]
    fn feed_http_client_is_reused() {
        let first = feed_http_client() as *const Client;
        let second = feed_http_client() as *const Client;
        assert_eq!(first, second);
    }
}
