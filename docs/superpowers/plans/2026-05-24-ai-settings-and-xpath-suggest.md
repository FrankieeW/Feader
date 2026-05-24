# AI Settings + AI-Suggested XPath Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add backend-stored AI settings (Anthropic + OpenAI-compatible, with `$ENV` key references) and a "Suggest with AI" action that proposes a full XPath selector set for a page, which the user validates with the existing preview.

**Architecture:** A SQLite singleton `ai_settings` table (mirrors `wallet_sessions`) holds provider/base_url/model/api_key/enabled; the key is never returned raw to the renderer. `$ENV` references are the recommended zero-secret-at-rest path; literal keys are an MVP fallback stored in local app-data SQLite and must be described as less secure. A new `ai.rs` resolves `$ENV` references in the backend, calls the chosen provider via `reqwest`, and parses/validates the JSON selectors. The XPath form gets a Suggest button; settings get an AI card with a docs link.

**Tech Stack:** Rust (rusqlite, reqwest, serde_json) + React/TypeScript. Backend uses `cargo test`; the live LLM path is verified via `npm run tauri dev`.

---

## Spec reference

`docs/superpowers/specs/2026-05-24-ai-settings-and-xpath-suggest-design.md`

## File map

- `src-tauri/src/models.rs` — `AiSettings`, `AiSettingsInput`, `env_reference_name`/`is_env_reference`.
- `src-tauri/src/db.rs` — `ai_settings` table, `get_ai_settings`/`set_ai_settings`/`raw_ai_api_key`, tests.
- `src-tauri/Cargo.toml` — enable `reqwest`'s `json` feature for provider calls.
- `src-tauri/src/xpath_adapter.rs` — `pub fetch_normalized`, `pub is_valid_xpath`.
- `src-tauri/src/ai.rs` — new module: key resolution, prompt, provider calls, JSON parse/validate, tests.
- `src-tauri/src/lib.rs` — `mod ai;` + `get_ai_settings`/`set_ai_settings`/`suggest_xpath_source` commands.
- `src/App.tsx` — AI types/state, Settings AI card, Suggest button, test-mode parity.
- `src/App.css` — minor AI card styling (reuse existing classes where possible).
- `docs/ai-configuration.md` — new doc; linked from the AI card.
- `DESIGN.md` — note AI config + AI-assisted XPath.

---

### Task 1: ai_settings storage + models

**Files:** `src-tauri/src/models.rs`, `src-tauri/src/db.rs`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/db.rs`:

```rust
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
        assert_eq!(saved.api_key_reference, None); // literal key is masked

        // Blank key on a later save keeps the existing key.
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

        // Env-reference form is exposed (it is not a secret).
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ai_settings_round_trip_and_key_masking`
Expected: FAIL to compile — `AiSettings*`, `set_ai_settings`, `raw_ai_api_key` don't exist.

- [ ] **Step 3: Add models + env-reference helpers**

In `src-tauri/src/models.rs`, add:

```rust
/// AI provider configuration exposed to the renderer (never carries a literal secret).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    pub api_key_set: bool,
    pub api_key_reference: Option<String>,
    pub updated_at: String,
}

/// AI settings input from the renderer.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettingsInput {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub enabled: bool,
    pub api_key: Option<String>,
}

/// Return the variable name if `value` is an env reference like `$NAME` or `${NAME}`.
pub fn env_reference_name(value: &str) -> Option<String> {
    let rest = value.trim().strip_prefix('$')?;
    let name = match rest.strip_prefix('{') {
        Some(inner) => inner.strip_suffix('}')?,
        None => rest,
    };
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .map_or(false, |c| c == '_' || c.is_ascii_alphabetic());
    if first_ok && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric()) {
        Some(name.to_string())
    } else {
        None
    }
}

/// True when `value` is an env reference (`$NAME` / `${NAME}`).
pub fn is_env_reference(value: &str) -> bool {
    env_reference_name(value).is_some()
}
```

- [ ] **Step 4: Add the table to the schema**

In `src-tauri/src/db.rs` `initialize_schema`, inside the `execute_batch("...")` SQL (after the `wallet_sessions` table), add:

```sql
        CREATE TABLE IF NOT EXISTS ai_settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            provider TEXT NOT NULL DEFAULT 'openai',
            base_url TEXT NOT NULL DEFAULT '',
            model TEXT NOT NULL DEFAULT '',
            api_key TEXT NOT NULL DEFAULT '',
            enabled INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );
```

- [ ] **Step 5: Add the DB methods**

In `src-tauri/src/db.rs`, add `use rusqlite::OptionalExtension;` to the imports if not present, and add to `impl AppDatabase`:

```rust
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
                params![&input.provider, &input.base_url, &input.model, &new_key, enabled, &now],
            )
            .map_err(|error| error.to_string())?;

        read_ai_settings(&connection)
    }
```

Add this free function near the other `*_with_connection` helpers in `db.rs`:

```rust
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
```

Add `AiSettings, AiSettingsInput` to the `use crate::models::{...}` import at the top of `db.rs`.

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ai_settings_round_trip_and_key_masking`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/db.rs
git commit -m "feat: persist AI provider settings with masked keys"
```

---

### Task 2: Expose fetch_normalized + is_valid_xpath

**Files:** `src-tauri/src/xpath_adapter.rs`.

- [ ] **Step 1: Add the helpers**

In `src-tauri/src/xpath_adapter.rs`, add:

```rust
/// Fetch a URL and return its normalized (real-world-tolerant) XHTML.
pub async fn fetch_normalized(url: &str) -> Result<String, String> {
    let body = fetch_page(url).await?;
    Ok(normalize_html(&body))
}

/// True when `expression` compiles as a valid XPath.
pub fn is_valid_xpath(expression: &str) -> bool {
    Factory::new()
        .build(expression)
        .ok()
        .flatten()
        .is_some()
}
```

- [ ] **Step 2: Verify build**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: PASS (no warnings about unused — both are used in Task 3).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/xpath_adapter.rs
git commit -m "refactor: expose fetch_normalized and is_valid_xpath"
```

(If `cargo build` warns that these are unused until Task 3 wires them, proceed — Task 3 lands immediately after and the final suite run covers it.)

---

### Task 3: AI client + suggest command

**Files:** Create `src-tauri/src/ai.rs`; modify `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`.

- [ ] **Step 0: Enable reqwest JSON support**

In `src-tauri/Cargo.toml`, change the `reqwest` dependency to include the `json` feature:

```toml
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/ai.rs` with ONLY the test first so it fails to compile, then add code in Step 3. For now add at the bottom of the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_selectors_from_model_text() {
        let text = "Sure:\n```json\n{\"items\":\"//article\",\"title\":\".//h2/a\",\"url\":\".//h2/a/@href\",\"summary\":\"\",\"content\":\".//section\",\"image\":\".//img/@src\",\"author\":null,\"publishedAt\":\".//time/@datetime\",\"nextPage\":\"\"}\n```";
        let selectors = parse_selectors_json(text).expect("parses");
        assert_eq!(selectors.items, "//article");
        assert_eq!(selectors.content.as_deref(), Some(".//section"));
        assert_eq!(selectors.summary, None); // empty dropped
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
        assert_eq!(resolve_api_key("literal-key").unwrap(), "literal-key");
        assert!(resolve_api_key("$FEADER_MISSING_VAR_XYZ").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

First register the module: in `src-tauri/src/lib.rs`, add `mod ai;` near the other `mod` lines.
Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_selectors_from_model_text`
Expected: FAIL to compile — `parse_selectors_json`, `resolve_api_key` not defined.

- [ ] **Step 3: Implement `ai.rs`**

Put this ABOVE the `#[cfg(test)]` block in `src-tauri/src/ai.rs`:

```rust
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
        return std::env::var(&name)
            .map_err(|_| format!("Environment variable {name} is not set"));
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
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && is_valid_xpath(s))
}

/// Parse a model response (possibly wrapped in prose/code fences) into validated selectors.
pub fn parse_selectors_json(text: &str) -> Result<XPathSelectors, String> {
    let json = extract_json_object(text).ok_or("AI response did not contain JSON")?;
    let raw: SuggestedSelectors =
        serde_json::from_str(&json).map_err(|error| error.to_string())?;

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml ai::`
Expected: PASS (3 tests).

- [ ] **Step 5: Add the commands**

In `src-tauri/src/lib.rs`, add `AiSettings, AiSettingsInput` to the `use models::{...}` import, then add:

```rust
/// Return AI settings (API key masked).
#[tauri::command]
fn get_ai_settings(database: tauri::State<'_, AppDatabase>) -> Result<AiSettings, String> {
    database.get_ai_settings()
}

/// Save AI settings (blank api_key keeps the existing key).
#[tauri::command]
fn set_ai_settings(
    input: AiSettingsInput,
    database: tauri::State<'_, AppDatabase>,
) -> Result<AiSettings, String> {
    database.set_ai_settings(&input)
}

/// Suggest XPath selectors for a page using the configured AI provider.
#[tauri::command]
async fn suggest_xpath_source(
    url: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathSelectors, String> {
    let settings = database.get_ai_settings()?;
    if !settings.enabled || !settings.api_key_set {
        return Err("AI is not configured".to_string());
    }
    let raw_key = database.raw_ai_api_key()?;
    let html = xpath_adapter::fetch_normalized(url.trim()).await?;
    ai::suggest_xpath_selectors(&settings, &raw_key, &html).await
}
```

Register `get_ai_settings, set_ai_settings, suggest_xpath_source` in the `tauri::generate_handler![...]` list.

- [ ] **Step 6: Full backend suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS, no unused-warning for `fetch_normalized`/`is_valid_xpath`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/ai.rs src-tauri/src/lib.rs
git commit -m "feat: AI client and suggest_xpath_source command"
```

---

### Task 4: Frontend AI settings card + test-mode parity

**Files:** `src/App.tsx`, `src/App.css`.

- [ ] **Step 1: Add types + the docs link constant**

In `src/App.tsx`, add near the other type aliases:

```tsx
type AiProvider = "anthropic" | "openai";

type AiSettings = {
  provider: AiProvider;
  baseUrl: string;
  model: string;
  enabled: boolean;
  apiKeySet: boolean;
  apiKeyReference?: string | null;
  updatedAt: string;
};

const aiDocsUrl = "https://github.com/FrankieeW/Feader/blob/main/docs/ai-configuration.md";

const defaultAiSettings: AiSettings = {
  provider: "openai",
  baseUrl: "",
  model: "",
  enabled: false,
  apiKeySet: false,
  apiKeyReference: null,
  updatedAt: "",
};
```

- [ ] **Step 2: Add test-mode handlers**

In `testModeInvoke`'s switch, add before `default:`:

```tsx
    case "get_ai_settings":
      return testModeAiSettings as T;
    case "set_ai_settings": {
      const input = args?.input as
        | { provider?: AiProvider; baseUrl?: string; model?: string; enabled?: boolean; apiKey?: string | null }
        | undefined;
      const key = typeof input?.apiKey === "string" ? input.apiKey.trim() : "";
      const hadKey = testModeAiSettings.apiKeySet;
      testModeAiSettings = {
        provider: input?.provider ?? testModeAiSettings.provider,
        baseUrl: input?.baseUrl ?? "",
        model: input?.model ?? "",
        enabled: Boolean(input?.enabled),
        apiKeySet: key.length > 0 ? true : hadKey,
        apiKeyReference: key.startsWith("$") ? key : key.length > 0 ? null : testModeAiSettings.apiKeyReference,
        updatedAt: new Date().toISOString(),
      };
      return testModeAiSettings as T;
    }
    case "suggest_xpath_source":
      throw new Error("AI suggestions require the Tauri app.");
```

And add module-level mutable state near `testModeSourceState`:

```tsx
let testModeAiSettings: AiSettings = { ...defaultAiSettings };
```

- [ ] **Step 3: Load AI settings + add handlers in `App`**

In `App()`, add state and a loader:

```tsx
  const [aiSettings, setAiSettings] = useState<AiSettings>(defaultAiSettings);

  useEffect(() => {
    void invoke<AiSettings>("get_ai_settings").then(setAiSettings).catch(() => undefined);
  }, []);

  async function handleSaveAiSettings(input: {
    provider: AiProvider;
    baseUrl: string;
    model: string;
    enabled: boolean;
    apiKey?: string;
  }): Promise<void> {
    await runTask("Saving AI settings", async () => {
      const next = await invoke<AiSettings>("set_ai_settings", { input });
      setAiSettings(next);
      setStatus("AI settings saved");
    });
  }
```

- [ ] **Step 4: Add the AI settings card**

In the Settings view's `settings-grid`, add a new card (after the existing cards). It uses local form state via a small component — add this component near `ThemeControl`:

```tsx
function AiSettingsCard({
  settings,
  disabled,
  onSave,
}: {
  settings: AiSettings;
  disabled: boolean;
  onSave: (input: {
    provider: AiProvider;
    baseUrl: string;
    model: string;
    enabled: boolean;
    apiKey?: string;
  }) => void;
}) {
  const [provider, setProvider] = useState<AiProvider>(settings.provider);
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl);
  const [model, setModel] = useState(settings.model);
  const [enabled, setEnabled] = useState(settings.enabled);
  const [apiKey, setApiKey] = useState("");

  useEffect(() => {
    setProvider(settings.provider);
    setBaseUrl(settings.baseUrl);
    setModel(settings.model);
    setEnabled(settings.enabled);
  setApiKey("");
  }, [settings]);

  return (
    <article className="settings-card">
      <div className="panel-heading">
        <span>AI</span>
        <span>{settings.enabled && settings.apiKeySet ? "Active" : "Off"}</span>
      </div>
      <form
        className="ai-form"
        onSubmit={(event) => {
          event.preventDefault();
          onSave({ provider, baseUrl, model, enabled, apiKey: apiKey.trim() || undefined });
        }}
      >
        <label className="selector-input">
          <span>Provider</span>
          <select
            disabled={disabled}
            onChange={(event) => setProvider(event.currentTarget.value as AiProvider)}
            value={provider}
          >
            <option value="openai">OpenAI-compatible</option>
            <option value="anthropic">Anthropic (Claude)</option>
          </select>
        </label>
        <label className="selector-input">
          <span>Base URL</span>
          <input
            disabled={disabled}
            onChange={(event) => setBaseUrl(event.currentTarget.value)}
            placeholder={provider === "anthropic" ? "https://api.anthropic.com" : "https://api.openai.com/v1"}
            value={baseUrl}
          />
        </label>
        <label className="selector-input">
          <span>Model</span>
          <input
            disabled={disabled}
            onChange={(event) => setModel(event.currentTarget.value)}
            placeholder={provider === "anthropic" ? "claude-haiku-4-5-20251001" : "gpt-4o-mini"}
            value={model}
          />
        </label>
        <label className="selector-input">
          <span>API key {settings.apiKeySet ? "(set, blank keeps it)" : ""}</span>
          <input
            disabled={disabled}
            onChange={(event) => setApiKey(event.currentTarget.value)}
            placeholder="sk-... or $MY_API_KEY"
            type="password"
            value={apiKey}
          />
          {settings.apiKeyReference ? (
            <small className="selector-hint">Using environment reference {settings.apiKeyReference}</small>
          ) : settings.apiKeySet ? (
            <small className="selector-hint">Literal key is stored locally; leave blank to keep it.</small>
          ) : null}
        </label>
        <label className="ai-enable">
          <input
            checked={enabled}
            disabled={disabled}
            onChange={(event) => setEnabled(event.currentTarget.checked)}
            type="checkbox"
          />
          <span>Enable AI features</span>
        </label>
        <div className="ai-actions">
          <button className="primary-action" disabled={disabled} type="submit">
            Save AI settings
          </button>
          <a href={aiDocsUrl} rel="noreferrer" target="_blank">
            Configuration guide
          </a>
        </div>
      </form>
    </article>
  );
}
```

Mount it in the `settings-grid` (after the last existing `settings-card`):

```tsx
            <AiSettingsCard
              disabled={isBusy}
              onSave={(input) => void handleSaveAiSettings(input)}
              settings={aiSettings}
            />
```

- [ ] **Step 5: Add minimal styling**

In `src/App.css`, add:

```css
.ai-form {
  display: grid;
  gap: 10px;
}

.ai-form select,
.ai-form input[type="password"],
.ai-form input[type="text"],
.ai-form input:not([type]) {
  min-height: 38px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 0 10px;
  color: var(--color-text);
  background: var(--color-panel-strong);
}

.ai-enable {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--color-muted);
  font-size: 12px;
}

.ai-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
```

- [ ] **Step 6: Verify build**

Run: `npm run build`
Expected: PASS (only pre-existing `FormEvent` warnings).

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: AI settings card with provider, key, and docs link"
```

---

### Task 5: "Suggest with AI" button in the XPath form

**Files:** `src/App.tsx`.

- [ ] **Step 1: Add the suggest handler in `App`**

```tsx
  async function handleSuggestXPath(): Promise<void> {
    const url = feedUrl.trim();
    if (!url) {
      setStatus("Enter a page URL first.");
      return;
    }
    await runTask("Suggesting selectors", async () => {
      const suggested = await invoke<XPathSelectors>("suggest_xpath_source", { url });
      setXPathSelectors({ ...defaultXPathSelectors, ...suggested });
      setStatus("AI suggested selectors — run Preview to validate");
    });
  }
```

- [ ] **Step 2: Pass AI availability + handler into `XPathSourceForm`**

At the `<XPathSourceForm .../>` usage, add two props:

```tsx
                        aiAvailable={aiSettings.enabled && aiSettings.apiKeySet}
                        onSuggest={() => void handleSuggestXPath()}
```

In `XPathSourceForm`'s prop type and signature, add:

```tsx
  aiAvailable: boolean;
  onSuggest: () => void;
```

- [ ] **Step 3: Render the button**

In `XPathSourceForm`, immediately before the existing `<button ... onClick={onPreview}>Preview</button>`, add:

```tsx
      {aiAvailable ? (
        <button disabled={isBusy} onClick={onSuggest} type="button">
          Suggest with AI
        </button>
      ) : null}
```

- [ ] **Step 4: Verify build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "feat: suggest XPath selectors with AI from the source form"
```

---

### Task 6: Docs + DESIGN.md + verification

**Files:** Create `docs/ai-configuration.md`; modify `DESIGN.md`.

- [ ] **Step 1: Write the docs page**

Create `docs/ai-configuration.md`:

```markdown
# AI Configuration

Feader can use an AI model to suggest XPath selectors for a page. The model is also the
foundation for future AI features. Configuration lives in **Settings → AI**.

## Providers

- **OpenAI-compatible** — works with OpenAI and any server exposing `/chat/completions`
  (OpenRouter, local servers like Ollama/LM Studio, gateways). Base URL example:
  `https://api.openai.com/v1`. Model example: `gpt-4o-mini`.
- **Anthropic (Claude)** — native Messages API. Base URL: `https://api.anthropic.com`.
  Model example: `claude-haiku-4-5-20251001`.

## API key

Enter either:

- an **environment-variable reference** like `$MY_API_KEY` or `${MY_API_KEY}`. Feader stores
  only the reference (no secret at rest) and resolves it from the environment when it calls
  the provider. This is the recommended path.
- a **literal key** (e.g. `sk-...`), stored locally in Feader's app-data SQLite database and
  never shown back in the UI. This is convenient, but less secure than `$ENV` because the
  local database contains the key.

### Environment-variable caveat

Desktop apps launched from Finder/Dock (macOS) or the Start Menu (Windows) usually do **not**
inherit the environment from your shell. For a `$VAR` reference to resolve, the variable must
be present in the environment Feader is launched with — for example, launch it from a terminal
where the variable is exported, or set it in your OS login/launch environment.

## Privacy

When you use "Suggest with AI", Feader fetches and normalizes the page with the same backend
DOM pipeline used by the XPath adapter. That truncated normalized HTML is sent to the provider you
configured. Suggested selectors are validated locally and shown for you to confirm with
Preview before anything is saved.
```

- [ ] **Step 2: Update DESIGN.md**

In `DESIGN.md`, under "Implementation constraints", add:

```markdown
- AI: optional provider config (Anthropic or OpenAI-compatible) is stored in the backend with literal keys masked and `$ENV` references resolved at call time; the first consumer suggests XPath selectors that the user validates via the existing preview. See `docs/ai-configuration.md`.
```

- [ ] **Step 3: Commit**

```bash
git add docs/ai-configuration.md DESIGN.md
git commit -m "docs: add AI configuration guide and design note"
```

- [ ] **Step 4: Full verification**

Run: `cargo test --manifest-path src-tauri/Cargo.toml` (all PASS) and `npm run build` (PASS).

- [ ] **Step 5: Manual smoke (`npm run tauri dev`)**

- Settings → AI: set OpenAI-compatible base URL + model + a literal key, enable, Save; reopen Settings and confirm it shows "set" and the key field is blank (masked).
- Set the key to `$SOME_VAR` and Save; confirm it shows the reference back.
- On the XPath source form, with AI enabled, click "Suggest with AI" for a real listing page; confirm the selector fields populate and the existing Preview validates them.
- Click "Configuration guide" and confirm the docs page opens.

---

## Self-review notes

- **Spec coverage:** storage + masking + env reference (T1), fetch reuse (T2), provider client + resolution + parse/validate + command (T3), settings card + test-mode parity (T4), suggest button (T5), docs + UI link + DESIGN.md (T6). All spec sections mapped.
- **Type/name consistency:** `AiSettings`/`AiSettingsInput`, `get_ai_settings`/`set_ai_settings`/`raw_ai_api_key`, `env_reference_name`/`is_env_reference`, `resolve_api_key`, `parse_selectors_json`, `suggest_xpath_selectors`, `suggest_xpath_source`, `fetch_normalized`, `is_valid_xpath` — each defined once and referenced consistently across tasks. Frontend `AiSettings` mirrors the backend camelCase serialization (`apiKeySet`, `apiKeyReference`).
- **Secret handling:** literal key never serialized to the renderer (masked in `read_ai_settings`); `suggest_xpath_source` reads the raw key via `raw_ai_api_key` and resolves env refs only in the backend. `$ENV` references are recommended; literal SQLite storage is a documented fallback.
- **Placeholder scan:** none — every code step has concrete code.
- **Note:** `docs/ai-configuration.md` is an explicitly requested doc (user asked for it); creating it is intended, not incidental.
