//! SQLite persistence for Feader sources and articles.

use std::path::Path;
use std::sync::Mutex;

use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, ToSql};

use crate::models::{
    AiSettings, AiSettingsInput, Article, ArticleFilter, ParsedArticle, Source,
    WalletLoginChallenge, WalletSession, XPathSelectors,
};

const WALLET_LOGIN_STATEMENT: &str = "Sign in to Feader with your Ethereum wallet.";
const WALLET_CHALLENGE_TTL_MINUTES: i64 = 10;

/// Thread-safe application database handle.
pub struct AppDatabase {
    connection: Mutex<Connection>,
}

impl AppDatabase {
    /// Open or create the SQLite database at the provided path.
    pub fn open(path: &Path) -> Result<Self, String> {
        let connection = Connection::open(path).map_err(|error| error.to_string())?;
        initialize_schema(&connection).map_err(|error| error.to_string())?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Open an in-memory database for tests.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self, String> {
        let connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
        initialize_schema(&connection).map_err(|error| error.to_string())?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Return all known sources with article counters.
    pub fn list_sources(&self) -> Result<Vec<Source>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        list_sources_with_connection(&connection)
    }

    /// Insert a source if it does not exist, or update its title when provided.
    pub fn add_source(&self, url: &str, title: Option<&str>) -> Result<Source, String> {
        self.add_source_with_kind("rss", url, title, None)
    }

    /// Insert an XPath source and persist its selector configuration.
    pub fn add_xpath_source(
        &self,
        url: &str,
        title: &str,
        selectors: &XPathSelectors,
    ) -> Result<Source, String> {
        let config_json = serde_json::to_string(selectors).map_err(|error| error.to_string())?;
        self.add_source_with_kind("xpath", url, Some(title), Some(&config_json))
    }

    /// Create a single-use SIWE challenge for local wallet login.
    pub fn create_wallet_login_challenge(
        &self,
        domain: &str,
        uri: &str,
        nonce: &str,
    ) -> Result<WalletLoginChallenge, String> {
        let domain = domain.trim();
        let uri = uri.trim();
        if domain.is_empty() {
            return Err("Wallet login domain is required".to_string());
        }
        if uri.is_empty() {
            return Err("Wallet login URI is required".to_string());
        }

        let issued_at = Utc::now();
        let expires_at = issued_at + Duration::minutes(WALLET_CHALLENGE_TTL_MINUTES);
        let issued_at = issued_at.to_rfc3339();
        let expires_at = expires_at.to_rfc3339();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;

        connection
            .execute(
                "
                INSERT INTO wallet_login_challenges (
                    nonce, domain, uri, statement, issued_at, expires_at, consumed_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)
                ",
                params![
                    nonce,
                    domain,
                    uri,
                    WALLET_LOGIN_STATEMENT,
                    issued_at,
                    expires_at
                ],
            )
            .map_err(|error| error.to_string())?;

        Ok(WalletLoginChallenge {
            nonce: nonce.to_string(),
            domain: domain.to_string(),
            uri: uri.to_string(),
            statement: WALLET_LOGIN_STATEMENT.to_string(),
            issued_at,
            expires_at,
        })
    }

    /// Consume a SIWE challenge by nonce, rejecting expired or replayed nonces.
    pub fn consume_wallet_login_challenge(
        &self,
        nonce: &str,
        domain: &str,
        uri: &str,
    ) -> Result<(), String> {
        let now = Utc::now().to_rfc3339();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let updated = connection
            .execute(
                "
                UPDATE wallet_login_challenges
                SET consumed_at = ?1
                WHERE nonce = ?2
                  AND domain = ?3
                  AND uri = ?4
                  AND consumed_at IS NULL
                  AND expires_at > ?1
                ",
                params![now, nonce, domain, uri],
            )
            .map_err(|error| error.to_string())?;

        if updated == 0 {
            return Err("Wallet login challenge is expired, replayed, or mismatched".to_string());
        }
        Ok(())
    }

    /// Persist the verified wallet session and mark it as current.
    pub fn save_wallet_session(
        &self,
        address: &str,
        chain_id: u64,
        siwe_message: &str,
        signature: &str,
    ) -> Result<WalletSession, String> {
        let signed_in_at = Utc::now().to_rfc3339();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "
                INSERT INTO wallet_sessions (
                    id, address, chain_id, siwe_message, signature, signed_in_at, expires_at,
                    revoked_at
                )
                VALUES (1, ?1, ?2, ?3, ?4, ?5, NULL, NULL)
                ON CONFLICT(id) DO UPDATE SET
                    address = excluded.address,
                    chain_id = excluded.chain_id,
                    siwe_message = excluded.siwe_message,
                    signature = excluded.signature,
                    signed_in_at = excluded.signed_in_at,
                    expires_at = excluded.expires_at,
                    revoked_at = NULL
                ",
                params![
                    address,
                    chain_id as i64,
                    siwe_message,
                    signature,
                    signed_in_at
                ],
            )
            .map_err(|error| error.to_string())?;

        Ok(WalletSession {
            address: address.to_string(),
            chain_id,
            signed_in_at,
            expires_at: None,
        })
    }

    /// Return the current local wallet session, if present.
    pub fn current_wallet_session(&self) -> Result<Option<WalletSession>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        match connection.query_row(
            "
            SELECT address, chain_id, signed_in_at, expires_at
            FROM wallet_sessions
            WHERE id = 1
              AND revoked_at IS NULL
              AND (expires_at IS NULL OR expires_at > ?1)
            ",
            [Utc::now().to_rfc3339()],
            |row| {
                Ok(WalletSession {
                    address: row.get(0)?,
                    chain_id: row.get::<_, i64>(1)? as u64,
                    signed_in_at: row.get(2)?,
                    expires_at: row.get(3)?,
                })
            },
        ) {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    /// Revoke the current local wallet session.
    pub fn disconnect_wallet_session(&self) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE wallet_sessions SET revoked_at = ?1 WHERE id = 1",
                [Utc::now().to_rfc3339()],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Read AI settings with the API key masked (literal hidden, env reference shown).
    pub fn get_ai_settings(&self) -> Result<AiSettings, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        read_ai_settings(&connection)
    }

    /// Return the raw stored API key string (literal or `$NAME` reference) for backend use only.
    pub fn raw_ai_api_key(&self) -> Result<String, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let key = connection
            .query_row("SELECT api_key FROM ai_settings WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        Ok(key)
    }

    /// Cache a value from the remote registry.
    pub fn set_cache(&self, key: &str, value: &str) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO registry_cache (key, value, fetched_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, fetched_at = excluded.fetched_at",
                params![key, value, now_string()],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Read a cached registry value. Returns None if the key is not found or
    /// the cache is older than `max_age_seconds`.
    pub fn get_cache(&self, key: &str, max_age_seconds: i64) -> Result<Option<String>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let row = connection
            .query_row(
                "SELECT value, fetched_at FROM registry_cache WHERE key = ?1",
                params![key],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;

        let Some((value, fetched_at)) = row else {
            return Ok(None);
        };

        let fetched =
            chrono::DateTime::parse_from_rfc3339(&fetched_at).map_err(|error| error.to_string())?;
        let age = Utc::now() - fetched.with_timezone(&Utc);
        if age.num_seconds() > max_age_seconds {
            return Ok(None);
        }

        Ok(Some(value))
    }

    /// Upsert AI settings; a blank `api_key` keeps the existing stored key.
    pub fn set_ai_settings(&self, input: &AiSettingsInput) -> Result<AiSettings, String> {
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;

        let existing_key = connection
            .query_row("SELECT api_key FROM ai_settings WHERE id = 1", [], |row| {
                row.get::<_, String>(0)
            })
            .optional()
            .map_err(|error| error.to_string())?
            .unwrap_or_default();
        let new_key = match input.api_key.as_deref().map(str::trim) {
            Some(key) if !key.is_empty() => key.to_string(),
            _ => existing_key,
        };
        let enabled = if input.enabled { 1 } else { 0 };

        connection
            .execute(
                "
                INSERT INTO ai_settings (id, provider, base_url, model, api_key, enabled, updated_at)
                VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)
                ON CONFLICT(id) DO UPDATE SET
                    provider = excluded.provider,
                    base_url = excluded.base_url,
                    model = excluded.model,
                    api_key = excluded.api_key,
                    enabled = excluded.enabled,
                    updated_at = excluded.updated_at
                ",
                params![
                    &input.provider,
                    &input.base_url,
                    &input.model,
                    &new_key,
                    enabled,
                    &now
                ],
            )
            .map_err(|error| error.to_string())?;

        read_ai_settings(&connection)
    }

    fn add_source_with_kind(
        &self,
        kind: &str,
        url: &str,
        title: Option<&str>,
        config_json: Option<&str>,
    ) -> Result<Source, String> {
        let now = now_string();
        let source_title = title
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(url);
        let connection = self.connection.lock().map_err(|error| error.to_string())?;

        connection
            .execute(
                "
                INSERT INTO sources (kind, title, url, config_json, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                ON CONFLICT(url) DO UPDATE SET
                    kind = excluded.kind,
                    title = CASE
                        WHEN excluded.title = excluded.url THEN COALESCE(sources.title, excluded.title)
                        ELSE excluded.title
                    END,
                    config_json = excluded.config_json,
                    enabled = 1,
                    last_error = NULL,
                    updated_at = excluded.updated_at
                ",
                params![kind, source_title, url, config_json, now],
            )
            .map_err(|error| error.to_string())?;

        let source_id = connection
            .query_row("SELECT id FROM sources WHERE url = ?1", [url], |row| {
                row.get(0)
            })
            .map_err(|error| error.to_string())?;

        get_source_with_connection(&connection, source_id)
    }

    /// Find a source by id.
    pub fn get_source(&self, source_id: i64) -> Result<Source, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        get_source_with_connection(&connection, source_id)
    }

    /// Rename a source.
    pub fn update_source_title(&self, source_id: i64, title: &str) -> Result<Source, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("Source title is required".to_string());
        }

        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE sources SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title, now, source_id],
            )
            .map_err(|error| error.to_string())?;
        get_source_with_connection(&connection, source_id)
    }

    /// Replace the persisted selector configuration for an XPath source.
    pub fn update_xpath_source_config(
        &self,
        source_id: i64,
        selectors: &XPathSelectors,
    ) -> Result<Source, String> {
        let config_json = serde_json::to_string(selectors).map_err(|error| error.to_string())?;
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let updated = connection
            .execute(
                "
                UPDATE sources
                SET config_json = ?1, last_error = NULL, updated_at = ?2
                WHERE id = ?3 AND kind = 'xpath'
                ",
                params![config_json, now, source_id],
            )
            .map_err(|error| error.to_string())?;
        if updated == 0 {
            return Err("XPath source not found".to_string());
        }
        get_source_with_connection(&connection, source_id)
    }

    /// Set or clear a source's category folder. Blank/whitespace clears it.
    pub fn set_source_category(
        &self,
        source_id: i64,
        category: Option<&str>,
    ) -> Result<Source, String> {
        let normalized = category.map(str::trim).filter(|value| !value.is_empty());
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE sources SET category = ?1, updated_at = ?2 WHERE id = ?3",
                params![normalized, now, source_id],
            )
            .map_err(|error| error.to_string())?;
        get_source_with_connection(&connection, source_id)
    }

    /// Delete a source and its articles.
    pub fn delete_source(&self, source_id: i64) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let deleted = connection
            .execute("DELETE FROM sources WHERE id = ?1", [source_id])
            .map_err(|error| error.to_string())?;
        if deleted == 0 {
            return Err(format!("Source {source_id} was not found"));
        }
        Ok(())
    }

    /// Merge parsed articles for a source and update fetch metadata.
    pub fn upsert_articles(
        &self,
        source_id: i64,
        source_title: Option<&str>,
        articles: &[ParsedArticle],
    ) -> Result<usize, String> {
        let mut connection = self.connection.lock().map_err(|error| error.to_string())?;
        let transaction = connection
            .transaction()
            .map_err(|error| error.to_string())?;
        let now = now_string();

        if let Some(title) = source_title.filter(|value| !value.trim().is_empty()) {
            transaction
                .execute(
                    "UPDATE sources SET title = ?1, last_fetched_at = ?2, updated_at = ?2 WHERE id = ?3",
                    params![title, now, source_id],
                )
                .map_err(|error| error.to_string())?;
        } else {
            transaction
                .execute(
                    "UPDATE sources SET last_fetched_at = ?1, updated_at = ?1 WHERE id = ?2",
                    params![now, source_id],
                )
                .map_err(|error| error.to_string())?;
        }

        transaction
            .execute(
                "UPDATE sources SET last_error = NULL, enabled = 1 WHERE id = ?1",
                [source_id],
            )
            .map_err(|error| error.to_string())?;

        for article in articles {
            let content_html = article.content_html.as_deref().map(sanitize_html);
            transaction
                .execute(
                    "
                    INSERT INTO articles (
                        source_id, external_id, title, url, canonical_url, summary,
                        content_html, content_text, author, published_at, image_url,
                        tags_json, created_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
                    ON CONFLICT(source_id, url) DO UPDATE SET
                        external_id = COALESCE(excluded.external_id, articles.external_id),
                        title = excluded.title,
                        canonical_url = excluded.canonical_url,
                        summary = excluded.summary,
                        content_html = excluded.content_html,
                        content_text = excluded.content_text,
                        author = excluded.author,
                        published_at = excluded.published_at,
                        image_url = excluded.image_url,
                        tags_json = excluded.tags_json,
                        updated_at = excluded.updated_at
                    ",
                    params![
                        source_id,
                        article.external_id,
                        article.title,
                        article.url,
                        article.canonical_url,
                        article.summary,
                        content_html,
                        article.content_text,
                        article.author,
                        article.published_at,
                        article.image_url,
                        article.tags_json,
                        now
                    ],
                )
                .map_err(|error| error.to_string())?;
        }

        transaction.commit().map_err(|error| error.to_string())?;
        Ok(articles.len())
    }

    /// Return articles matching an optional filter.
    pub fn list_articles(&self, filter: ArticleFilter) -> Result<Vec<Article>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let mut sql = String::from(
            "
            SELECT
                articles.id,
                articles.source_id,
                sources.title AS source_title,
                articles.external_id,
                articles.title,
                articles.url,
                articles.canonical_url,
                articles.summary,
                articles.content_html,
                articles.content_text,
                articles.author,
                articles.published_at,
                articles.image_url,
                articles.tags_json,
                article_states.read,
                article_states.saved,
                articles.created_at,
                articles.updated_at
            FROM articles
            JOIN sources ON sources.id = articles.source_id
            LEFT JOIN article_states ON article_states.article_id = articles.id
            WHERE 1 = 1
            ",
        );
        let mut params: Vec<Box<dyn ToSql>> = Vec::new();

        if let Some(source_id) = filter.source_id {
            sql.push_str(" AND articles.source_id = ?");
            params.push(Box::new(source_id));
        }
        if filter.unread_only.unwrap_or(false) {
            sql.push_str(" AND COALESCE(article_states.read, 0) = 0");
        }
        if filter.saved_only.unwrap_or(false) {
            sql.push_str(" AND COALESCE(article_states.saved, 0) = 1");
        }

        sql.push_str(
            " ORDER BY COALESCE(articles.published_at, articles.created_at) DESC, articles.id DESC LIMIT 500",
        );

        let params_ref: Vec<&dyn ToSql> = params.iter().map(|value| value.as_ref()).collect();
        let mut statement = connection
            .prepare(&sql)
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map(params_ref.as_slice(), article_from_row)
            .map_err(|error| error.to_string())?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())
    }

    /// Update the read state for one article.
    pub fn mark_article_read(&self, article_id: i64, read: bool) -> Result<(), String> {
        self.update_article_state(article_id, Some(read), None)
    }

    /// Update the saved state for one article.
    pub fn save_article(&self, article_id: i64, saved: bool) -> Result<(), String> {
        self.update_article_state(article_id, None, Some(saved))
    }

    /// Mark all matching articles as read.
    pub fn mark_articles_read(&self, source_id: Option<i64>, read: bool) -> Result<usize, String> {
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let article_ids = if let Some(source_id) = source_id {
            let mut statement = connection
                .prepare("SELECT id FROM articles WHERE source_id = ?1")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([source_id], |row| row.get::<_, i64>(0))
                .map_err(|error| error.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        } else {
            let mut statement = connection
                .prepare("SELECT id FROM articles")
                .map_err(|error| error.to_string())?;
            let rows = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(|error| error.to_string())?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(|error| error.to_string())?
        };

        for article_id in &article_ids {
            connection
                .execute(
                    "
                    INSERT INTO article_states (article_id, read, saved, updated_at)
                    VALUES (?1, ?2, 0, ?3)
                    ON CONFLICT(article_id) DO UPDATE SET
                        read = excluded.read,
                        updated_at = excluded.updated_at
                    ",
                    params![article_id, read, now],
                )
                .map_err(|error| error.to_string())?;
        }

        Ok(article_ids.len())
    }

    /// Store the latest refresh error for a source without deleting old articles.
    pub fn record_source_error(&self, source_id: i64, error: &str) -> Result<(), String> {
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE sources SET last_error = ?1, updated_at = ?2 WHERE id = ?3",
                params![error, now, source_id],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn update_article_state(
        &self,
        article_id: i64,
        read: Option<bool>,
        saved: Option<bool>,
    ) -> Result<(), String> {
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "
                INSERT INTO article_states (article_id, read, saved, updated_at)
                VALUES (?1, 0, 0, ?2)
                ON CONFLICT(article_id) DO NOTHING
                ",
                params![article_id, now],
            )
            .map_err(|error| error.to_string())?;
        connection
            .execute(
                "
                UPDATE article_states
                SET
                    read = COALESCE(?2, read),
                    saved = COALESCE(?3, saved),
                    updated_at = ?4
                WHERE article_id = ?1
                ",
                params![article_id, read, saved, now],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }
}

fn initialize_schema(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind TEXT NOT NULL DEFAULT 'rss',
            title TEXT NOT NULL,
            url TEXT NOT NULL UNIQUE,
            config_json TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_fetched_at TEXT,
            last_error TEXT,
            category TEXT
        );

        CREATE TABLE IF NOT EXISTS articles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
            external_id TEXT,
            title TEXT NOT NULL,
            url TEXT NOT NULL,
            canonical_url TEXT,
            summary TEXT,
            content_html TEXT,
            content_text TEXT,
            author TEXT,
            published_at TEXT,
            image_url TEXT,
            tags_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(source_id, url)
        );

        CREATE TABLE IF NOT EXISTS article_states (
            article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
            read INTEGER NOT NULL DEFAULT 0,
            saved INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS wallet_login_challenges (
            nonce TEXT PRIMARY KEY,
            domain TEXT NOT NULL,
            uri TEXT NOT NULL,
            statement TEXT NOT NULL,
            issued_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            consumed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS wallet_sessions (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            address TEXT NOT NULL,
            chain_id INTEGER NOT NULL,
            siwe_message TEXT NOT NULL,
            signature TEXT NOT NULL,
            signed_in_at TEXT NOT NULL,
            expires_at TEXT,
            revoked_at TEXT
        );

        CREATE TABLE IF NOT EXISTS ai_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            provider TEXT NOT NULL DEFAULT 'openai',
            base_url TEXT NOT NULL DEFAULT '',
            model TEXT NOT NULL DEFAULT '',
            api_key TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS registry_cache (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            fetched_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_articles_source_id ON articles(source_id);
        CREATE INDEX IF NOT EXISTS idx_articles_published_at ON articles(published_at);
        CREATE INDEX IF NOT EXISTS idx_wallet_login_challenges_expires_at
            ON wallet_login_challenges(expires_at);
        ",
    )?;
    add_column_if_missing(
        connection,
        "sources",
        "enabled",
        "ALTER TABLE sources ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(
        connection,
        "sources",
        "last_error",
        "ALTER TABLE sources ADD COLUMN last_error TEXT",
    )?;
    add_column_if_missing(
        connection,
        "sources",
        "category",
        "ALTER TABLE sources ADD COLUMN category TEXT",
    )?;
    Ok(())
}

fn add_column_if_missing(
    connection: &Connection,
    table: &str,
    column: &str,
    statement: &str,
) -> rusqlite::Result<()> {
    let mut columns = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let exists = columns
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|name| name == column);

    if !exists {
        connection.execute(statement, [])?;
    }
    Ok(())
}

fn list_sources_with_connection(connection: &Connection) -> Result<Vec<Source>, String> {
    let mut statement = connection
        .prepare(
            "
            SELECT
                sources.id,
                sources.kind,
                sources.title,
                sources.url,
                sources.config_json,
                sources.enabled,
                sources.created_at,
                sources.last_fetched_at,
                sources.last_error,
                COUNT(articles.id) AS article_count,
                SUM(CASE WHEN COALESCE(article_states.read, 0) = 0 AND articles.id IS NOT NULL THEN 1 ELSE 0 END) AS unread_count,
                sources.category
            FROM sources
            LEFT JOIN articles ON articles.source_id = sources.id
            LEFT JOIN article_states ON article_states.article_id = articles.id
            GROUP BY sources.id
            ORDER BY sources.created_at DESC
            ",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(Source {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                url: row.get(3)?,
                config_json: row.get(4)?,
                enabled: row.get(5)?,
                created_at: row.get(6)?,
                last_fetched_at: row.get(7)?,
                last_error: row.get(8)?,
                article_count: row.get(9)?,
                unread_count: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                category: row.get(11)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn get_source_with_connection(connection: &Connection, source_id: i64) -> Result<Source, String> {
    list_sources_with_connection(connection)?
        .into_iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| format!("Source {source_id} was not found"))
}

fn read_ai_settings(connection: &Connection) -> Result<AiSettings, String> {
    let row = connection
        .query_row(
            "SELECT provider, base_url, model, api_key, enabled, updated_at FROM ai_settings WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;

    let Some((provider, base_url, model, api_key, enabled, updated_at)) = row else {
        return Ok(AiSettings {
            provider: "openai".to_string(),
            base_url: String::new(),
            model: String::new(),
            enabled: false,
            api_key_set: false,
            api_key_reference: None,
            updated_at: String::new(),
        });
    };

    let api_key_set = !api_key.trim().is_empty();
    let api_key_reference = crate::models::is_env_reference(&api_key).then(|| api_key.clone());

    Ok(AiSettings {
        provider,
        base_url,
        model,
        enabled,
        api_key_set,
        api_key_reference,
        updated_at,
    })
}

fn article_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Article> {
    Ok(Article {
        id: row.get(0)?,
        source_id: row.get(1)?,
        source_title: row.get(2)?,
        external_id: row.get(3)?,
        title: row.get(4)?,
        url: row.get(5)?,
        canonical_url: row.get(6)?,
        summary: row.get(7)?,
        content_html: row.get(8)?,
        content_text: row.get(9)?,
        author: row.get(10)?,
        published_at: row.get(11)?,
        image_url: row.get(12)?,
        tags_json: row.get(13)?,
        read: row.get::<_, Option<bool>>(14)?.unwrap_or(false),
        saved: row.get::<_, Option<bool>>(15)?.unwrap_or(false),
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn sanitize_html(value: &str) -> String {
    ammonia::clean(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_insert_is_idempotent_by_url() {
        let database = AppDatabase::in_memory().expect("database opens");

        let first = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let second = database
            .add_source("https://example.com/feed.xml", None)
            .expect("source upserts");

        assert_eq!(first.id, second.id);
        assert_eq!(database.list_sources().expect("sources list").len(), 1);
    }

    #[test]
    fn new_source_has_no_category() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        assert_eq!(source.category, None);
    }

    #[test]
    fn ai_settings_round_trip_and_key_masking() {
        let database = AppDatabase::in_memory().expect("database opens");

        let saved = database
            .set_ai_settings(&crate::models::AiSettingsInput {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-4o-mini".to_string(),
                enabled: true,
                api_key: Some("sk-secret".to_string()),
            })
            .expect("saves");
        assert!(saved.api_key_set);
        assert_eq!(saved.api_key_reference, None);

        let kept = database
            .set_ai_settings(&crate::models::AiSettingsInput {
                provider: "openai".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                model: "gpt-4o-mini".to_string(),
                enabled: true,
                api_key: None,
            })
            .expect("saves");
        assert!(kept.api_key_set);
        assert_eq!(database.raw_ai_api_key().expect("raw key"), "sk-secret");

        let referenced = database
            .set_ai_settings(&crate::models::AiSettingsInput {
                provider: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                model: "claude-haiku-4-5-20251001".to_string(),
                enabled: true,
                api_key: Some("$MY_KEY".to_string()),
            })
            .expect("saves");
        assert_eq!(referenced.api_key_reference.as_deref(), Some("$MY_KEY"));
    }

    #[test]
    fn article_upsert_preserves_state() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let article = ParsedArticle {
            external_id: Some("one".to_string()),
            title: "First".to_string(),
            url: "https://example.com/one".to_string(),
            canonical_url: None,
            summary: Some("Before".to_string()),
            content_html: None,
            content_text: None,
            author: None,
            published_at: None,
            image_url: None,
            tags_json: None,
        };

        database
            .upsert_articles(source.id, None, &[article.clone()])
            .expect("article inserts");
        let inserted = database
            .list_articles(ArticleFilter::default())
            .expect("articles list")[0]
            .clone();
        database
            .mark_article_read(inserted.id, true)
            .expect("read state updates");

        let mut changed = article;
        changed.summary = Some("After".to_string());
        database
            .upsert_articles(source.id, None, &[changed])
            .expect("article updates");
        let updated = database
            .list_articles(ArticleFilter::default())
            .expect("articles list")[0]
            .clone();

        assert!(updated.read);
        assert_eq!(updated.summary.as_deref(), Some("After"));
    }

    #[test]
    fn xpath_source_config_can_be_updated() {
        let database = AppDatabase::in_memory().expect("database opens");
        let selectors = XPathSelectors {
            items: "//article".to_string(),
            title: ".//h2/a".to_string(),
            url: ".//h2/a/@href".to_string(),
            summary: None,
            published_at: None,
            author: None,
            cookie: None,
            content: None,
            detail_content: None,
            image: None,
            next_page: None,
            max_items: None,
            plugin: None,
        };
        let source = database
            .add_xpath_source("https://example.com/list", "Example", &selectors)
            .expect("source inserts");

        let next_selectors = XPathSelectors {
            title: ".//h3/a".to_string(),
            url: ".//h3/a/@href".to_string(),
            ..selectors
        };
        let updated = database
            .update_xpath_source_config(source.id, &next_selectors)
            .expect("config updates");

        let config = updated.config_json.expect("config is stored");
        assert!(config.contains(".//h3/a"));
        assert_eq!(updated.kind, "xpath");
    }

    #[test]
    fn article_html_is_sanitized_on_upsert() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let article = ParsedArticle {
            external_id: None,
            title: "Dirty".to_string(),
            url: "https://example.com/one".to_string(),
            canonical_url: None,
            summary: None,
            content_html: Some(
                "<p onclick=\"x()\">hi</p><script>alert(1)</script><img src=x onerror=alert(1)>"
                    .to_string(),
            ),
            content_text: None,
            author: None,
            published_at: None,
            image_url: None,
            tags_json: None,
        };

        database
            .upsert_articles(source.id, None, &[article])
            .expect("article inserts");
        let stored = database
            .list_articles(ArticleFilter::default())
            .expect("articles list")[0]
            .clone();
        let html = stored.content_html.unwrap_or_default().to_lowercase();

        assert!(!html.contains("<script"));
        assert!(!html.contains("onerror"));
        assert!(!html.contains("onclick"));
    }

    #[test]
    fn deleting_source_cascades_articles() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let article = ParsedArticle {
            external_id: None,
            title: "First".to_string(),
            url: "https://example.com/one".to_string(),
            canonical_url: None,
            summary: None,
            content_html: None,
            content_text: None,
            author: None,
            published_at: None,
            image_url: None,
            tags_json: None,
        };

        database
            .upsert_articles(source.id, None, &[article])
            .expect("article inserts");
        database.delete_source(source.id).expect("source deletes");

        assert!(database.list_sources().expect("sources list").is_empty());
        assert!(database
            .list_articles(ArticleFilter::default())
            .expect("articles list")
            .is_empty());
    }

    #[test]
    fn refresh_error_is_recorded_without_deleting_articles() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let article = ParsedArticle {
            external_id: None,
            title: "First".to_string(),
            url: "https://example.com/one".to_string(),
            canonical_url: None,
            summary: None,
            content_html: None,
            content_text: None,
            author: None,
            published_at: None,
            image_url: None,
            tags_json: None,
        };

        database
            .upsert_articles(source.id, None, &[article])
            .expect("article inserts");
        database
            .record_source_error(source.id, "network failed")
            .expect("error records");

        let source = database.get_source(source.id).expect("source loads");
        let articles = database
            .list_articles(ArticleFilter::default())
            .expect("articles list");
        assert_eq!(source.last_error.as_deref(), Some("network failed"));
        assert_eq!(articles.len(), 1);
    }

    #[test]
    fn source_category_sets_and_clears() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");

        let set = database
            .set_source_category(source.id, Some("Dev"))
            .expect("category sets");
        assert_eq!(set.category.as_deref(), Some("Dev"));

        let cleared = database
            .set_source_category(source.id, Some("   "))
            .expect("blank clears category");
        assert_eq!(cleared.category, None);
    }

    #[test]
    fn mark_articles_read_updates_all_matching_articles() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let articles = [
            ParsedArticle {
                external_id: None,
                title: "First".to_string(),
                url: "https://example.com/one".to_string(),
                canonical_url: None,
                summary: None,
                content_html: None,
                content_text: None,
                author: None,
                published_at: None,
                image_url: None,
                tags_json: None,
            },
            ParsedArticle {
                external_id: None,
                title: "Second".to_string(),
                url: "https://example.com/two".to_string(),
                canonical_url: None,
                summary: None,
                content_html: None,
                content_text: None,
                author: None,
                published_at: None,
                image_url: None,
                tags_json: None,
            },
        ];

        database
            .upsert_articles(source.id, None, &articles)
            .expect("articles insert");
        let changed = database
            .mark_articles_read(Some(source.id), true)
            .expect("articles marked");
        let unread = database
            .list_articles(ArticleFilter {
                source_id: Some(source.id),
                unread_only: Some(true),
                saved_only: None,
            })
            .expect("unread articles list");

        assert_eq!(changed, 2);
        assert!(unread.is_empty());
    }
}
