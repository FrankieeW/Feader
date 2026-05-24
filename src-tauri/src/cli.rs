//! Built-in command-line control surface for Feader.

use std::env;
use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::db::AppDatabase;
use crate::models::{ArticleFilter, Source, SourceRefreshResult};

const DATABASE_FILE: &str = "feader.sqlite";

/// Run the CLI using process arguments and print the result.
pub fn run_from_env() -> i32 {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match run(args) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    let invocation = CliInvocation::parse(args)?;
    if invocation.help {
        return Ok(help_text());
    }

    let database_path = invocation
        .database_path
        .unwrap_or_else(default_database_path);
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let database = AppDatabase::open(&database_path)?;

    tauri::async_runtime::block_on(async move {
        match invocation.command {
            Command::SourcesList => to_json(&database.list_sources()?),
            Command::SourceAdd {
                url,
                title,
                category,
            } => {
                let feed = crate::feed_adapter::fetch_feed(&url).await?;
                let source = database.add_source(
                    &url,
                    title
                        .as_deref()
                        .or(feed.title.as_deref())
                        .filter(|value| !value.trim().is_empty()),
                )?;
                database.upsert_articles(source.id, feed.title.as_deref(), &feed.articles)?;
                let source = if let Some(category) = category {
                    database.set_source_category(source.id, Some(&category))?
                } else {
                    database.get_source(source.id)?
                };
                to_json(&source)
            }
            Command::SourceRename { id, title } => {
                to_json(&database.update_source_title(id, &title)?)
            }
            Command::SourceCategory { id, category } => {
                to_json(&database.set_source_category(id, category.as_deref())?)
            }
            Command::SourceDelete { id, yes } => {
                if !yes {
                    return Err("Deleting a source requires --yes".to_string());
                }
                database.delete_source(id)?;
                to_json(&CommandStatus::ok(format!("Deleted source {id}")))
            }
            Command::SourceRefresh { target } => match target {
                RefreshTarget::One(id) => {
                    let source = database.get_source(id)?;
                    let article_count = crate::refresh_source_record(&database, &source).await?;
                    to_json(&SourceRefreshResult {
                        source_id: source.id,
                        ok: true,
                        article_count,
                        error: None,
                    })
                }
                RefreshTarget::All => {
                    let mut results = Vec::new();
                    for source in database
                        .list_sources()?
                        .into_iter()
                        .filter(|source| source.enabled)
                    {
                        results.push(refresh_for_cli(&database, source).await?);
                    }
                    to_json(&results)
                }
            },
            Command::ArticlesList {
                source_id,
                unread_only,
                saved_only,
                limit,
            } => {
                let mut articles = database.list_articles(ArticleFilter {
                    source_id,
                    unread_only: Some(unread_only),
                    saved_only: Some(saved_only),
                })?;
                articles.truncate(limit);
                to_json(&articles)
            }
        }
    })
}

async fn refresh_for_cli(
    database: &AppDatabase,
    source: Source,
) -> Result<SourceRefreshResult, String> {
    match crate::refresh_source_record(database, &source).await {
        Ok(article_count) => Ok(SourceRefreshResult {
            source_id: source.id,
            ok: true,
            article_count,
            error: None,
        }),
        Err(error) => {
            database.record_source_error(source.id, &error)?;
            Ok(SourceRefreshResult {
                source_id: source.id,
                ok: false,
                article_count: 0,
                error: Some(error),
            })
        }
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string_pretty(value).map_err(|error| error.to_string())
}

fn default_database_path() -> PathBuf {
    if let Some(path) = env::var_os("FEADER_DB") {
        return PathBuf::from(path);
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("com.frankie.feader")
                .join(DATABASE_FILE);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(app_data) = env::var_os("APPDATA") {
            return PathBuf::from(app_data)
                .join("com.frankie.feader")
                .join(DATABASE_FILE);
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
            return PathBuf::from(data_home).join("feader").join(DATABASE_FILE);
        }
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("feader")
                .join(DATABASE_FILE);
        }
    }

    PathBuf::from(DATABASE_FILE)
}

#[derive(Debug)]
struct CliInvocation {
    database_path: Option<PathBuf>,
    help: bool,
    command: Command,
}

impl CliInvocation {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut parser = ArgParser::new(args);
        let mut database_path = None;
        let mut help = false;
        let mut positional = Vec::new();

        while let Some(arg) = parser.next() {
            match arg.as_str() {
                "--db" => database_path = Some(PathBuf::from(parser.value("--db")?)),
                "--json" => {}
                "--help" | "-h" | "help" => help = true,
                value => positional.push(value.to_string()),
            }
        }

        let command = if help && positional.is_empty() {
            Command::SourcesList
        } else {
            Command::parse(positional)?
        };

        Ok(Self {
            database_path,
            help,
            command,
        })
    }
}

#[derive(Debug)]
enum Command {
    SourcesList,
    SourceAdd {
        url: String,
        title: Option<String>,
        category: Option<String>,
    },
    SourceRename {
        id: i64,
        title: String,
    },
    SourceCategory {
        id: i64,
        category: Option<String>,
    },
    SourceDelete {
        id: i64,
        yes: bool,
    },
    SourceRefresh {
        target: RefreshTarget,
    },
    ArticlesList {
        source_id: Option<i64>,
        unread_only: bool,
        saved_only: bool,
        limit: usize,
    },
}

impl Command {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        if args.is_empty() {
            return Err(help_text());
        }

        match args[0].as_str() {
            "sources" => parse_sources_command(&args[1..]),
            "source" => parse_source_command(&args[1..]),
            "articles" => parse_articles_command(&args[1..]),
            "article" => parse_articles_command(&args[1..]),
            _ => Err(help_text()),
        }
    }
}

#[derive(Debug)]
enum RefreshTarget {
    One(i64),
    All,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandStatus {
    ok: bool,
    message: String,
}

impl CommandStatus {
    fn ok(message: String) -> Self {
        Self { ok: true, message }
    }
}

fn parse_sources_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("list") | None => Ok(Command::SourcesList),
        Some("add") => parse_source_add(&args[1..]),
        Some("refresh") => parse_source_refresh(&args[1..]),
        _ => Err(help_text()),
    }
}

fn parse_source_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("add") => parse_source_add(&args[1..]),
        Some("list") => Ok(Command::SourcesList),
        Some("rename") => parse_source_rename(&args[1..]),
        Some("category") => parse_source_category(&args[1..]),
        Some("delete") | Some("remove") => parse_source_delete(&args[1..]),
        Some("refresh") => parse_source_refresh(&args[1..]),
        _ => Err(help_text()),
    }
}

fn parse_source_add(args: &[String]) -> Result<Command, String> {
    let mut parser = ArgParser::new(args.to_vec());
    let mut url = None;
    let mut title = None;
    let mut category = None;

    while let Some(arg) = parser.next() {
        match arg.as_str() {
            "--url" => url = Some(parser.value("--url")?),
            "--title" => title = Some(parser.value("--title")?),
            "--category" => category = Some(parser.value("--category")?),
            value if value.starts_with('-') => return Err(format!("Unknown option: {value}")),
            value => {
                if url.is_some() {
                    return Err("Only one source URL can be added at a time".to_string());
                }
                url = Some(value.to_string());
            }
        }
    }

    let url = required_string(url, "Feed URL is required")?;
    Ok(Command::SourceAdd {
        url,
        title,
        category,
    })
}

fn parse_source_rename(args: &[String]) -> Result<Command, String> {
    if args.len() < 2 {
        return Err("Usage: feader source rename <id> <title>".to_string());
    }
    Ok(Command::SourceRename {
        id: parse_id(&args[0])?,
        title: args[1..].join(" "),
    })
}

fn parse_source_category(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("Usage: feader source category <id> [category|--clear]".to_string());
    }
    let id = parse_id(&args[0])?;
    let category = if args.iter().any(|arg| arg == "--clear") || args.len() == 1 {
        None
    } else {
        Some(args[1..].join(" "))
    };
    Ok(Command::SourceCategory { id, category })
}

fn parse_source_delete(args: &[String]) -> Result<Command, String> {
    let mut id = None;
    let mut yes = false;

    for arg in args {
        match arg.as_str() {
            "--yes" | "-y" => yes = true,
            value if value.starts_with('-') => return Err(format!("Unknown option: {value}")),
            value => id = Some(parse_id(value)?),
        }
    }

    Ok(Command::SourceDelete {
        id: id.ok_or_else(|| "Usage: feader source delete <id> --yes".to_string())?,
        yes,
    })
}

fn parse_source_refresh(args: &[String]) -> Result<Command, String> {
    if args.iter().any(|arg| arg == "--all") {
        return Ok(Command::SourceRefresh {
            target: RefreshTarget::All,
        });
    }

    let id = args
        .first()
        .ok_or_else(|| "Usage: feader source refresh <id|--all>".to_string())?;
    Ok(Command::SourceRefresh {
        target: RefreshTarget::One(parse_id(id)?),
    })
}

fn parse_articles_command(args: &[String]) -> Result<Command, String> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            let mut parser = ArgParser::new(args.get(1..).unwrap_or_default().to_vec());
            let mut source_id = None;
            let mut unread_only = false;
            let mut saved_only = false;
            let mut limit = 50;

            while let Some(arg) = parser.next() {
                match arg.as_str() {
                    "--source-id" => source_id = Some(parse_id(&parser.value("--source-id")?)?),
                    "--unread" => unread_only = true,
                    "--saved" => saved_only = true,
                    "--limit" => {
                        limit = parser
                            .value("--limit")?
                            .parse::<usize>()
                            .map_err(|_| "--limit must be a positive integer".to_string())?
                    }
                    value => return Err(format!("Unknown option: {value}")),
                }
            }

            Ok(Command::ArticlesList {
                source_id,
                unread_only,
                saved_only,
                limit,
            })
        }
        _ => Err(help_text()),
    }
}

fn parse_id(value: &str) -> Result<i64, String> {
    value
        .parse::<i64>()
        .map_err(|_| format!("Invalid numeric id: {value}"))
}

fn required_string(value: Option<String>, message: &str) -> Result<String, String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| message.to_string())
}

#[derive(Debug)]
struct ArgParser {
    args: Vec<String>,
    index: usize,
}

impl ArgParser {
    fn new(args: Vec<String>) -> Self {
        Self { args, index: 0 }
    }

    fn next(&mut self) -> Option<String> {
        let value = self.args.get(self.index).cloned();
        if value.is_some() {
            self.index += 1;
        }
        value
    }

    fn value(&mut self, option: &str) -> Result<String, String> {
        self.next()
            .filter(|value| !value.starts_with('-'))
            .ok_or_else(|| format!("{option} requires a value"))
    }
}

fn help_text() -> String {
    r#"Feader CLI

Usage:
  feader --db <path> sources list --json
  feader --db <path> source add <feed-url> [--title <title>] [--category <name>] --json
  feader --db <path> source refresh <id|--all> --json
  feader --db <path> source rename <id> <title> --json
  feader --db <path> source category <id> [category|--clear] --json
  feader --db <path> source delete <id> --yes --json
  feader --db <path> articles list [--source-id <id>] [--unread] [--saved] [--limit <n>] --json

FEADER_DB can also set the database path. JSON output is the stable default."#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ParsedArticle;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn source_list_reads_selected_database_as_json() {
        let database_path = test_database_path("list");
        let database = AppDatabase::open(&database_path).expect("database opens");
        database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");

        let output = run(vec![
            "--db".to_string(),
            database_path.to_string_lossy().to_string(),
            "sources".to_string(),
            "list".to_string(),
        ])
        .expect("sources list succeeds");

        assert!(output.contains("\"title\": \"Example\""));
        assert!(output.contains("\"url\": \"https://example.com/feed.xml\""));
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn articles_list_applies_filters_and_limit() {
        let database_path = test_database_path("articles");
        let database = AppDatabase::open(&database_path).expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let articles = [
            parsed_article("First", "https://example.com/1"),
            parsed_article("Second", "https://example.com/2"),
        ];
        database
            .upsert_articles(source.id, None, &articles)
            .expect("articles insert");

        let output = run(vec![
            "--db".to_string(),
            database_path.to_string_lossy().to_string(),
            "articles".to_string(),
            "list".to_string(),
            "--source-id".to_string(),
            source.id.to_string(),
            "--limit".to_string(),
            "1".to_string(),
        ])
        .expect("articles list succeeds");

        assert_eq!(output.matches("\"sourceId\"").count(), 1);
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn source_add_fetches_feed_and_persists_articles() {
        let database_path = test_database_path("add");
        let feed_url = serve_once(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>CLI Feed</title>
    <link>https://example.com/</link>
    <item>
      <title>CLI Article</title>
      <link>https://example.com/article</link>
      <guid>cli-article</guid>
      <description>Added from the CLI.</description>
    </item>
  </channel>
</rss>"#,
        );

        let output = run(vec![
            "--db".to_string(),
            database_path.to_string_lossy().to_string(),
            "source".to_string(),
            "add".to_string(),
            feed_url,
            "--category".to_string(),
            "Automation".to_string(),
        ])
        .expect("source add succeeds");

        assert!(output.contains("\"title\": \"CLI Feed\""));
        assert!(output.contains("\"category\": \"Automation\""));

        let database = AppDatabase::open(&database_path).expect("database reopens");
        let articles = database
            .list_articles(ArticleFilter::default())
            .expect("articles list");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "CLI Article");
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn delete_requires_explicit_yes() {
        let database_path = test_database_path("delete");
        let database = AppDatabase::open(&database_path).expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");

        let error = run(vec![
            "--db".to_string(),
            database_path.to_string_lossy().to_string(),
            "source".to_string(),
            "delete".to_string(),
            source.id.to_string(),
        ])
        .expect_err("delete is gated");

        assert_eq!(error, "Deleting a source requires --yes");
        assert_eq!(database.list_sources().expect("sources list").len(), 1);
        let _ = fs::remove_file(database_path);
    }

    fn test_database_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "feader-cli-{name}-{}-{}.sqlite",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn parsed_article(title: &str, url: &str) -> ParsedArticle {
        ParsedArticle {
            external_id: None,
            title: title.to_string(),
            url: url.to_string(),
            canonical_url: None,
            summary: None,
            content_html: None,
            content_text: None,
            author: None,
            published_at: None,
            image_url: None,
            tags_json: None,
        }
    }

    fn serve_once(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test server binds");
        let address = listener.local_addr().expect("server has address");
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("server accepts request");
            let mut buffer = [0; 1024];
            let _ = stream.read(&mut buffer);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("response writes");
        });
        format!("http://{address}/feed.xml")
    }
}
