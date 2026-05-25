//! SQLite persistence for Feader sources and articles.

use std::path::Path;
use std::sync::Mutex;

use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, ToSql};
use url::Url;

use crate::models::{
    AiSettings, AiSettingsInput, Article, ArticleFilter, ParsedArticle, PluginCredential,
    PluginPack, PluginRefreshOverride, RssHubSourceConfig, Source, WalletLoginChallenge,
    WalletSession, XPathSelectors, SOURCE_KIND_JSON_API, SOURCE_KIND_RSS, SOURCE_KIND_RSSHUB,
    SOURCE_KIND_XPATH,
};

const WALLET_LOGIN_STATEMENT: &str = "Sign in to Feader with your Ethereum wallet.";
const WALLET_CHALLENGE_TTL_MINUTES: i64 = 10;
const TRACKING_QUERY_PARAMS: &[&str] = &[
    "fbclid",
    "gclid",
    "igshid",
    "mc_cid",
    "mc_eid",
    "spm",
    "utm_campaign",
    "utm_content",
    "utm_medium",
    "utm_source",
    "utm_term",
];

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
        self.add_source_with_kind(SOURCE_KIND_RSS, url, title, None)
    }

    /// Insert an RSSHub route source and persist its selected instance override.
    pub fn add_rsshub_source(
        &self,
        route: &str,
        title: Option<&str>,
        config: &RssHubSourceConfig,
    ) -> Result<Source, String> {
        let config_json = serde_json::to_string(config).map_err(|error| error.to_string())?;
        self.add_source_with_kind(SOURCE_KIND_RSSHUB, route, title, Some(&config_json))
    }

    /// Insert an XPath source and persist its selector configuration.
    pub fn add_xpath_source(
        &self,
        url: &str,
        title: &str,
        selectors: &XPathSelectors,
    ) -> Result<Source, String> {
        let config_json = serde_json::to_string(selectors).map_err(|error| error.to_string())?;
        self.add_source_with_kind(SOURCE_KIND_XPATH, url, Some(title), Some(&config_json))
    }

    /// Insert a JSON API feed source and persist its selector configuration.
    pub fn add_json_api_source(
        &self,
        url: &str,
        title: &str,
        selectors: &XPathSelectors,
    ) -> Result<Source, String> {
        let config_json = serde_json::to_string(selectors)
            .map_err(|e| format!("Serialize JSON selectors: {e}"))?;
        self.add_source_with_kind(SOURCE_KIND_JSON_API, url, Some(title), Some(&config_json))
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

    /// Persist an installed static plugin pack locally.
    pub fn install_plugin_pack(&self, pack: &PluginPack) -> Result<(), String> {
        let json = serde_json::to_string(pack).map_err(|error| error.to_string())?;
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO installed_plugin_packs (plugin_id, version, pack_json, installed_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(plugin_id) DO UPDATE SET
                    version = excluded.version,
                    pack_json = excluded.pack_json,
                    updated_at = excluded.updated_at",
                params![pack.id, pack.version, json, now_string()],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Remove one installed plugin pack.
    pub fn uninstall_plugin_pack(&self, plugin_id: &str) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "DELETE FROM installed_plugin_packs WHERE plugin_id = ?1",
                params![plugin_id],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Return all installed plugin packs.
    pub fn list_installed_plugin_packs(&self) -> Result<Vec<PluginPack>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let mut statement = connection
            .prepare("SELECT pack_json FROM installed_plugin_packs ORDER BY installed_at DESC")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        rows.map(|row| {
            let json = row.map_err(|error| error.to_string())?;
            serde_json::from_str::<PluginPack>(&json).map_err(|error| error.to_string())
        })
        .collect()
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

    /// Read a plugin credential with the cookie literal masked (env reference surfaced).
    pub fn get_plugin_credential(&self, plugin_id: &str) -> Result<PluginCredential, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let row = connection
            .query_row(
                "SELECT cookie, updated_at, last_checked_at, last_check_ok, last_check_message
                 FROM plugin_credentials WHERE plugin_id = ?1",
                params![plugin_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;

        let Some((cookie, updated_at, last_checked_at, last_check_ok, last_check_message)) = row
        else {
            return Ok(PluginCredential {
                plugin_id: plugin_id.to_string(),
                cookie_set: false,
                cookie_reference: None,
                updated_at: None,
                last_checked_at: None,
                last_check_ok: None,
                last_check_message: None,
            });
        };
        let trimmed = cookie.trim();
        Ok(PluginCredential {
            plugin_id: plugin_id.to_string(),
            cookie_set: !trimmed.is_empty(),
            cookie_reference: crate::models::is_env_reference(trimmed).then(|| trimmed.to_string()),
            updated_at,
            last_checked_at,
            last_check_ok: last_check_ok.map(|value| value != 0),
            last_check_message,
        })
    }

    /// Raw stored cookie string (literal or `$NAME`) for backend fetch use only.
    pub fn raw_plugin_cookie(&self, plugin_id: &str) -> Result<Option<String>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let cookie = connection
            .query_row(
                "SELECT cookie FROM plugin_credentials WHERE plugin_id = ?1",
                params![plugin_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(cookie)
    }

    /// Upsert a plugin cookie; a blank cookie clears it.
    pub fn set_plugin_credential(&self, plugin_id: &str, cookie: &str) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO plugin_credentials (plugin_id, cookie, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(plugin_id) DO UPDATE SET cookie = excluded.cookie, updated_at = excluded.updated_at",
                params![plugin_id, cookie.trim(), now_string()],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Record the outcome of a credential validity probe.
    pub fn record_plugin_credential_check(
        &self,
        plugin_id: &str,
        ok: bool,
        message: &str,
    ) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO plugin_credentials (plugin_id, cookie, updated_at, last_checked_at, last_check_ok, last_check_message)
                 VALUES (?1, '', ?2, ?2, ?3, ?4)
                 ON CONFLICT(plugin_id) DO UPDATE SET
                    last_checked_at = excluded.last_checked_at,
                    last_check_ok = excluded.last_check_ok,
                    last_check_message = excluded.last_check_message",
                params![plugin_id, now_string(), if ok { 1 } else { 0 }, message],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
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

    /// Replace the persisted RSSHub route configuration for an RSSHub source.
    pub fn update_rsshub_source_config(
        &self,
        source_id: i64,
        config: &RssHubSourceConfig,
    ) -> Result<Source, String> {
        let config_json = serde_json::to_string(config).map_err(|error| error.to_string())?;
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let updated = connection
            .execute(
                "
                UPDATE sources
                SET config_json = ?1, last_error = NULL, updated_at = ?2
                WHERE id = ?3 AND kind = 'rsshub'
                ",
                params![config_json, now, source_id],
            )
            .map_err(|error| error.to_string())?;
        if updated == 0 {
            return Err("RSSHub source not found".to_string());
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
            let dedupe_key = article_dedupe_key(article);
            let existing_id = transaction
                .query_row(
                    "SELECT id FROM articles WHERE source_id = ?1 AND dedupe_key = ?2",
                    params![source_id, dedupe_key],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(|error| error.to_string())?;

            if let Some(article_id) = existing_id {
                transaction
                    .execute(
                        "
                        UPDATE articles
                        SET
                            external_id = COALESCE(?2, external_id),
                            title = ?3,
                            url = ?4,
                            canonical_url = ?5,
                            summary = ?6,
                            content_html = ?7,
                            content_text = ?8,
                            author = ?9,
                            published_at = ?10,
                            image_url = ?11,
                            tags_json = ?12,
                            dedupe_key = ?13,
                            updated_at = ?14
                        WHERE id = ?1
                        ",
                        params![
                            article_id,
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
                            dedupe_key,
                            now
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            } else {
                transaction
                    .execute(
                        "
                    INSERT INTO articles (
                        source_id, external_id, title, url, canonical_url, summary,
                        content_html, content_text, author, published_at, image_url,
                        tags_json, dedupe_key, created_at, updated_at
                    )
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
                    ON CONFLICT(source_id, url) DO UPDATE SET
                        external_id = COALESCE(excluded.external_id, articles.external_id),
                        title = excluded.title,
                        dedupe_key = excluded.dedupe_key,
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
                            dedupe_key,
                            now
                        ],
                    )
                    .map_err(|error| error.to_string())?;
            }
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

    /// Read an app_settings value by key.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    /// Upsert an app_settings key-value.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Read plugin-level refresh interval override.
    pub fn get_plugin_refresh_interval(&self, plugin_id: &str) -> Result<Option<i64>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .query_row(
                "SELECT refresh_interval_seconds FROM plugin_auto_refresh WHERE plugin_id = ?1",
                params![plugin_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    /// Upsert a plugin-level refresh interval override.
    pub fn set_plugin_refresh_interval(&self, plugin_id: &str, seconds: i64) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "INSERT INTO plugin_auto_refresh (plugin_id, refresh_interval_seconds)
                 VALUES (?1, ?2)
                 ON CONFLICT(plugin_id) DO UPDATE SET refresh_interval_seconds = excluded.refresh_interval_seconds",
                params![plugin_id, seconds],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Set or clear a source-level refresh interval override.
    pub fn set_source_refresh_interval(
        &self,
        source_id: i64,
        seconds: Option<i64>,
    ) -> Result<(), String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE sources SET refresh_interval_seconds = ?1, updated_at = ?2 WHERE id = ?3",
                params![seconds, now_string(), source_id],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    /// List all plugin refresh overrides with display names derived from source configs.
    pub fn list_plugin_refresh_overrides(&self) -> Result<Vec<PluginRefreshOverride>, String> {
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT plugin_id, refresh_interval_seconds FROM plugin_auto_refresh ORDER BY plugin_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(PluginRefreshOverride {
                    plugin_id: row.get(0)?,
                    plugin_name: String::new(), // filled in below
                    refresh_interval_seconds: row.get(1)?,
                })
            })
            .map_err(|error| error.to_string())?;
        let mut overrides: Vec<PluginRefreshOverride> = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;

        // Fill in plugin names by looking at source configs.
        for ov in &mut overrides {
            if let Ok(name) = connection
                .query_row(
                    "SELECT s.config_json FROM sources s
                 WHERE json_extract(s.config_json, '$.plugin.id') = ?1
                 LIMIT 1",
                    params![ov.plugin_id],
                    |row| {
                        let config: String = row.get(0)?;
                        Ok(config)
                    },
                )
                .optional()
                .map_err(|e| e.to_string())
            {
                if let Some(config) = name {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&config) {
                        if let Some(name) = parsed["plugin"]["name"].as_str() {
                            ov.plugin_name = name.to_string();
                        }
                    }
                }
            }
            if ov.plugin_name.is_empty() {
                ov.plugin_name = ov.plugin_id.clone();
            }
        }

        Ok(overrides)
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
            dedupe_key TEXT,
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

        CREATE TABLE IF NOT EXISTS plugin_credentials (
            plugin_id          TEXT PRIMARY KEY,
            cookie             TEXT NOT NULL DEFAULT '',
            updated_at         TEXT NOT NULL,
            last_checked_at    TEXT,
            last_check_ok      INTEGER,
            last_check_message TEXT
        );

        CREATE TABLE IF NOT EXISTS app_settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS plugin_auto_refresh (
            plugin_id                TEXT PRIMARY KEY,
            refresh_interval_seconds INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS installed_plugin_packs (
            plugin_id    TEXT PRIMARY KEY,
            version      TEXT NOT NULL,
            pack_json    TEXT NOT NULL,
            installed_at TEXT NOT NULL,
            updated_at   TEXT NOT NULL
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
    add_column_if_missing(
        connection,
        "sources",
        "refresh_interval_seconds",
        "ALTER TABLE sources ADD COLUMN refresh_interval_seconds INTEGER",
    )?;
    add_column_if_missing(
        connection,
        "articles",
        "dedupe_key",
        "ALTER TABLE articles ADD COLUMN dedupe_key TEXT",
    )?;
    migrate_article_dedupe_keys(connection)?;

    // Seed defaults for auto-refresh settings.
    connection.execute(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('auto_refresh_enabled', 'true')",
        [],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO app_settings (key, value) VALUES ('global_refresh_interval', '1800')",
        [],
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

fn migrate_article_dedupe_keys(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(
        "
        SELECT id, external_id, url, canonical_url
        FROM articles
        WHERE dedupe_key IS NULL OR dedupe_key = ''
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    let articles = rows.collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    for (id, external_id, url, canonical_url) in articles {
        let dedupe_key =
            article_dedupe_key_from_parts(external_id.as_deref(), canonical_url.as_deref(), &url);
        connection.execute(
            "UPDATE articles SET dedupe_key = ?1 WHERE id = ?2",
            params![dedupe_key, id],
        )?;
    }

    merge_duplicate_articles(connection)?;
    connection.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_articles_source_dedupe_key ON articles(source_id, dedupe_key)",
        [],
    )?;
    Ok(())
}

fn merge_duplicate_articles(connection: &Connection) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(
        "
        SELECT source_id, dedupe_key
        FROM articles
        WHERE dedupe_key IS NOT NULL AND dedupe_key != ''
        GROUP BY source_id, dedupe_key
        HAVING COUNT(*) > 1
        ",
    )?;
    let duplicate_keys = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    for (source_id, dedupe_key) in duplicate_keys {
        let ids = {
            let mut ids_statement = connection.prepare(
                "
                SELECT id
                FROM articles
                WHERE source_id = ?1 AND dedupe_key = ?2
                ORDER BY id ASC
                ",
            )?;
            let ids = ids_statement
                .query_map(params![source_id, dedupe_key], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        let Some((&keep_id, redundant_ids)) = ids.split_first() else {
            continue;
        };
        let latest_id = *ids.last().unwrap_or(&keep_id);
        let latest = connection.query_row(
            "
            SELECT
                external_id, title, url, canonical_url, summary, content_html,
                content_text, author, published_at, image_url, tags_json, updated_at
            FROM articles
            WHERE id = ?1
            ",
            [latest_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, String>(11)?,
                ))
            },
        )?;

        let merged_state = connection.query_row(
            "
            SELECT
                MAX(COALESCE(article_states.read, 0)),
                MAX(COALESCE(article_states.saved, 0)),
                MAX(article_states.updated_at)
            FROM articles
            LEFT JOIN article_states ON article_states.article_id = articles.id
            WHERE articles.source_id = ?1 AND articles.dedupe_key = ?2
            ",
            params![source_id, dedupe_key],
            |row| {
                Ok((
                    row.get::<_, Option<bool>>(0)?.unwrap_or(false),
                    row.get::<_, Option<bool>>(1)?.unwrap_or(false),
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )?;
        if merged_state.0 || merged_state.1 {
            connection.execute(
                "
                INSERT INTO article_states (article_id, read, saved, updated_at)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(article_id) DO UPDATE SET
                    read = excluded.read,
                    saved = excluded.saved,
                    updated_at = excluded.updated_at
                ",
                params![
                    keep_id,
                    merged_state.0,
                    merged_state.1,
                    merged_state.2.unwrap_or_else(now_string)
                ],
            )?;
        }

        for id in redundant_ids {
            connection.execute("DELETE FROM articles WHERE id = ?1", [id])?;
        }

        if latest_id != keep_id {
            connection.execute(
                "
                UPDATE articles
                SET
                    external_id = COALESCE(?2, external_id),
                    title = ?3,
                    url = ?4,
                    canonical_url = ?5,
                    summary = ?6,
                    content_html = ?7,
                    content_text = ?8,
                    author = ?9,
                    published_at = ?10,
                    image_url = ?11,
                    tags_json = ?12,
                    updated_at = ?13
                WHERE id = ?1
                ",
                params![
                    keep_id, latest.0, latest.1, latest.2, latest.3, latest.4, latest.5, latest.6,
                    latest.7, latest.8, latest.9, latest.10, latest.11
                ],
            )?;
        }
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
                sources.category,
                sources.refresh_interval_seconds
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
                refresh_interval_seconds: row.get(12)?,
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

fn article_dedupe_key(article: &ParsedArticle) -> String {
    article_dedupe_key_from_parts(
        article.external_id.as_deref(),
        article.canonical_url.as_deref(),
        &article.url,
    )
}

fn article_dedupe_key_from_parts(
    external_id: Option<&str>,
    canonical_url: Option<&str>,
    url: &str,
) -> String {
    if let Some(external_id) = external_id.map(str::trim).filter(|value| !value.is_empty()) {
        return format!("guid:{external_id}");
    }

    let url = canonical_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(url);
    format!("url:{}", normalize_article_url(url))
}

fn normalize_article_url(value: &str) -> String {
    let trimmed = value.trim();
    let Ok(mut url) = Url::parse(trimmed) else {
        return trimmed.split('#').next().unwrap_or(trimmed).to_string();
    };
    url.set_fragment(None);

    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !is_tracking_query_param(key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    pairs.sort();

    url.set_query(None);
    if !pairs.is_empty() {
        let mut serializer = url.query_pairs_mut();
        for (key, value) in pairs {
            serializer.append_pair(&key, &value);
        }
    }

    url.to_string()
}

fn is_tracking_query_param(key: &str) -> bool {
    key.starts_with("utm_") || TRACKING_QUERY_PARAMS.contains(&key)
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
    fn plugin_credential_round_trip_and_masking() {
        let db = AppDatabase::in_memory().expect("open db");
        // unset → cookie_set false
        let empty = db
            .get_plugin_credential("official.naixi-forum.xpath")
            .unwrap();
        assert!(!empty.cookie_set);

        db.set_plugin_credential("official.naixi-forum.xpath", "sid=abc; uid=1")
            .unwrap();
        let saved = db
            .get_plugin_credential("official.naixi-forum.xpath")
            .unwrap();
        assert!(saved.cookie_set);
        assert_eq!(saved.cookie_reference, None); // literal not echoed
        assert_eq!(
            db.raw_plugin_cookie("official.naixi-forum.xpath")
                .unwrap()
                .as_deref(),
            Some("sid=abc; uid=1")
        );

        // env reference is surfaced (not masked away)
        db.set_plugin_credential("p2", "$FEADER_NAIXI_COOKIE")
            .unwrap();
        let envref = db.get_plugin_credential("p2").unwrap();
        assert!(envref.cookie_set);
        assert_eq!(
            envref.cookie_reference.as_deref(),
            Some("$FEADER_NAIXI_COOKIE")
        );

        // blank clears
        db.set_plugin_credential("p2", "").unwrap();
        assert!(!db.get_plugin_credential("p2").unwrap().cookie_set);

        // check recording
        db.record_plugin_credential_check("official.naixi-forum.xpath", true, "已登录")
            .unwrap();
        let checked = db
            .get_plugin_credential("official.naixi-forum.xpath")
            .unwrap();
        assert_eq!(checked.last_check_ok, Some(true));
        assert_eq!(checked.last_check_message.as_deref(), Some("已登录"));
        assert!(checked.last_checked_at.is_some());
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
    fn article_upsert_uses_external_id_when_feed_link_changes() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://www.v2ex.com/feed/openai.xml", Some("OpenAI"))
            .expect("source inserts");
        let article = ParsedArticle {
            external_id: Some("tag:www.v2ex.com,2026-05-24:/t/1215096".to_string()),
            title: "API access".to_string(),
            url: "https://www.v2ex.com/t/1215096#reply21".to_string(),
            canonical_url: None,
            summary: Some("Before".to_string()),
            content_html: None,
            content_text: None,
            author: None,
            published_at: Some("2026-05-24T09:12:25+00:00".to_string()),
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
        database
            .save_article(inserted.id, true)
            .expect("saved state updates");

        let mut changed = article;
        changed.url = "https://www.v2ex.com/t/1215096#reply34".to_string();
        changed.summary = Some("After".to_string());
        database
            .upsert_articles(source.id, None, &[changed])
            .expect("article updates");

        let articles = database
            .list_articles(ArticleFilter::default())
            .expect("articles list");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].url, "https://www.v2ex.com/t/1215096#reply34");
        assert_eq!(articles[0].summary.as_deref(), Some("After"));
        assert!(articles[0].read);
        assert!(articles[0].saved);
    }

    #[test]
    fn article_upsert_normalizes_url_fragment_when_guid_is_missing() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        let mut article = ParsedArticle {
            external_id: None,
            title: "Fragmented".to_string(),
            url: "https://example.com/post?id=42#reply1".to_string(),
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
        article.url = "https://example.com/post?id=42#reply2".to_string();
        article.summary = Some("After".to_string());
        database
            .upsert_articles(source.id, None, &[article])
            .expect("article updates");

        let articles = database
            .list_articles(ArticleFilter::default())
            .expect("articles list");
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].url, "https://example.com/post?id=42#reply2");
        assert_eq!(articles[0].summary.as_deref(), Some("After"));
    }

    #[test]
    fn article_url_normalization_keeps_identity_query_params() {
        assert_eq!(
            normalize_article_url("https://www.huanqiukexue.com/?utm_source=x&p=4137#comments"),
            "https://www.huanqiukexue.com/?p=4137"
        );
        assert_eq!(
            normalize_article_url("https://example.com/thread?utm_medium=rss&tid=7272735"),
            "https://example.com/thread?tid=7272735"
        );
    }

    #[test]
    fn schema_migration_merges_existing_external_id_duplicates() {
        let connection = Connection::open_in_memory().expect("sqlite opens");
        connection
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;
                CREATE TABLE sources (
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
                    category TEXT,
                    refresh_interval_seconds INTEGER
                );
                CREATE TABLE articles (
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
                CREATE TABLE article_states (
                    article_id INTEGER PRIMARY KEY REFERENCES articles(id) ON DELETE CASCADE,
                    read INTEGER NOT NULL DEFAULT 0,
                    saved INTEGER NOT NULL DEFAULT 0,
                    updated_at TEXT NOT NULL
                );
                INSERT INTO sources (id, title, url, created_at, updated_at)
                VALUES (1, 'OpenAI', 'https://www.v2ex.com/feed/openai.xml', 'now', 'now');
                INSERT INTO articles (
                    id, source_id, external_id, title, url, created_at, updated_at
                )
                VALUES
                    (10, 1, 'same-guid', 'Old', 'https://www.v2ex.com/t/1#reply1', 'old', 'old'),
                    (11, 1, 'same-guid', 'New', 'https://www.v2ex.com/t/1#reply2', 'new', 'new');
                INSERT INTO article_states (article_id, read, saved, updated_at)
                VALUES (10, 1, 0, 'old'), (11, 0, 1, 'new');
                ",
            )
            .expect("old schema seeds");

        initialize_schema(&connection).expect("schema migrates");

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM articles", [], |row| row.get(0))
            .expect("count loads");
        let row = connection
            .query_row(
                "
                SELECT articles.id, articles.title, articles.url, article_states.read, article_states.saved
                FROM articles
                LEFT JOIN article_states ON article_states.article_id = articles.id
                ",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, bool>(4)?,
                    ))
                },
            )
            .expect("merged article loads");

        assert_eq!(count, 1);
        assert_eq!(row.0, 10);
        assert_eq!(row.1, "New");
        assert_eq!(row.2, "https://www.v2ex.com/t/1#reply2");
        assert!(row.3);
        assert!(row.4);
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
            content_cleanup: Vec::new(),
            image: None,
            next_page: None,
            custom_fields: Vec::new(),
            max_items: None,
            plugin: None,
            reader: None,
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
        assert_eq!(updated.kind, SOURCE_KIND_XPATH);
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

    #[test]
    fn plugin_pack_install_round_trip_and_uninstall() {
        let database = AppDatabase::in_memory().expect("database opens");
        let pack = crate::plugin_registry::bundled_xpath_rule_packs()
            .into_iter()
            .next()
            .expect("bundled pack exists");
        let plugin = crate::plugin_registry::plugin_pack_from_xpath_rule_pack(pack.clone());

        database
            .install_plugin_pack(&plugin)
            .expect("plugin installs");
        let installed = database
            .list_installed_plugin_packs()
            .expect("installed plugins list");
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, pack.id);

        database
            .uninstall_plugin_pack(&pack.id)
            .expect("plugin uninstalls");
        assert!(database
            .list_installed_plugin_packs()
            .expect("installed plugins list")
            .is_empty());
    }
}
