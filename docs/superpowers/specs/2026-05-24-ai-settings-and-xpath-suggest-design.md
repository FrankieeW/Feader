# AI Settings + AI-Suggested XPath Selectors — Design

- Date: 2026-05-24
- Status: Approved (pending spec review)
- Scope: `src-tauri/src/{db,models,lib,ai,xpath_adapter}.rs`, `src/App.tsx`, `src/App.css`, new `docs/ai-configuration.md`, `DESIGN.md`. No changes to RSS fetching or article schema.

## Goal

Add a general AI configuration surface (provider, endpoint, model, API key) and a first AI consumer: a "Suggest with AI" action that proposes a full set of XPath selectors for a page, which the user then validates with the existing preview/diagnostics. Mirrors the "AI suggests, user verifies" loop and keeps secrets in the backend.

## Approved decisions

- **Providers: both.** A provider selector switches between Anthropic Messages API and an OpenAI-compatible Chat Completions API.
- **Secrets in backend.** API config lives in a SQLite singleton table, not `localStorage`. The LLM call runs in Rust, so the key never reaches the renderer.
- **Env-var references.** The API key field accepts either a literal key or an env reference `$NAME` / `${NAME}`, resolved from the backend process environment at request time. Reference form keeps the real secret out of the database entirely.
- **Docs + UI link.** A `docs/ai-configuration.md` explains setup, env references, and the GUI-launch environment caveat; the AI settings card links to it.

## Out of scope (YAGNI)

- AI summary/translation/chat (the settings are general enough to extend later, but only XPath-suggest consumes AI now).
- Streaming responses, token-budget UI, multi-key/profile management.
- OS keychain integration (SQLite app-data storage is the baseline; env-reference form is the zero-at-rest option).

## Architecture

### Storage: `ai_settings` singleton table (`db.rs`)

Mirror the existing `wallet_sessions` singleton pattern:

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

- `AppDatabase::get_ai_settings() -> AiSettings` and `set_ai_settings(input) -> AiSettings` (UPSERT on `id = 1`).
- `set_ai_settings` only overwrites `api_key` when the input provides a new non-empty value (so saving other fields keeps the existing key).

### Models (`models.rs`)

```rust
// Serialized to the renderer — never carries a literal secret.
struct AiSettings {
    provider: String,        // "anthropic" | "openai"
    base_url: String,
    model: String,
    enabled: bool,
    api_key_set: bool,       // true if a key/reference is stored
    api_key_reference: Option<String>, // the "$NAME" string if stored as a reference, else None
    updated_at: String,
}

// Input from the renderer.
struct AiSettingsInput {
    provider: String,
    base_url: String,
    model: String,
    enabled: bool,
    api_key: Option<String>, // None/"" = keep existing; literal or "$NAME"
}
```

- `get_ai_settings` masking rule: if the stored `api_key` matches an env-reference pattern (`^\$\{?[A-Za-z_][A-Za-z0-9_]*\}?$`), return it in `api_key_reference` (safe to show — it is not the secret). If it is a literal, return `api_key_reference: None` and only `api_key_set: true`. The literal value is never serialized to the renderer.

### Key resolution (`ai.rs`)

```rust
fn resolve_api_key(stored: &str) -> Result<String, String>
```

- If `stored` matches `^\$\{?(NAME)\}?$`, return `std::env::var(NAME)` or error `"Environment variable NAME is not set"`.
- Otherwise return `stored` as the literal key.
- Empty → error `"AI API key is not configured"`.

### AI client (`ai.rs`, new module)

```rust
pub async fn suggest_xpath_selectors(
    settings: &AiSettings,           // resolved internally to a key
    stored_api_key: &str,
    page_html: &str,
) -> Result<XPathSelectors, String>
```

- Resolve the key via `resolve_api_key`.
- Truncate `page_html` to a constant cap (`const AI_HTML_CHAR_CAP: usize = 12_000;`).
- Build a system+user prompt instructing the model to return ONLY a JSON object with keys `items,title,url,summary,publishedAt,author,content,image,nextPage` (XPath strings; empty string when not applicable).
- Dispatch on `settings.provider`:
  - `"anthropic"`: POST `{base_url}/v1/messages` with headers `x-api-key`, `anthropic-version: 2023-06-01`, body `{ model, max_tokens, messages: [...] }`; read `content[0].text`.
  - `"openai"`: POST `{base_url}/chat/completions` with `Authorization: Bearer {key}`, body `{ model, messages: [...] }`; read `choices[0].message.content`.
- Extract the JSON object from the model text (tolerate surrounding prose / code fences), `serde_json` parse into a selectors struct, map to `XPathSelectors`.
- Validate each non-empty selector compiles (reuse the adapter's compile path); on a compile failure, drop that field to empty rather than failing the whole suggestion. `items`/`title`/`url` empty after validation → error `"AI did not return usable selectors"`.

### Fetch reuse (`xpath_adapter.rs`)

Expose a helper so the AI command does not duplicate fetch/normalize:

```rust
pub async fn fetch_normalized(url: &str) -> Result<String, String> // fetch_page + normalize_html
```

### Command (`lib.rs`)

```rust
#[tauri::command]
async fn get_ai_settings(database: State<AppDatabase>) -> Result<AiSettings, String>

#[tauri::command]
async fn set_ai_settings(input: AiSettingsInput, database: State<AppDatabase>) -> Result<AiSettings, String>

#[tauri::command]
async fn suggest_xpath_source(url: String, database: State<AppDatabase>) -> Result<XPathSelectors, String>
```

`suggest_xpath_source`: load settings → if `!enabled` or no key → error `"AI is not configured"` → `fetch_normalized(url)` → `ai::suggest_xpath_selectors(...)` → return selectors. Because the renderer-facing `AiSettings` masks the key, the command reads the **raw stored `api_key`** via an internal DB getter (e.g. `get_ai_settings_internal()` returning the unmasked row, or a dedicated `raw_api_key()`), and passes that raw string to the client for env-reference resolution. Register all three in `generate_handler!`.

### Frontend (`App.tsx`)

- **Types**: `AiProvider = "anthropic" | "openai"`, `AiSettings` (matching the backend serialized shape), `AiSettingsInput`.
- **State + load**: load AI settings on mount via `get_ai_settings`; hold in `useState`.
- **Settings "AI" card** (in the existing settings grid): provider `<select>`, base URL input, model input, API key input (`type="password"`, placeholder hints `sk-...` or `$MY_API_KEY`), enable toggle, Save button (calls `set_ai_settings`, reloads). A "Configuration guide" link (`<a target="_blank" rel="noreferrer">`) to the docs page. Show `api_key_reference` when present, else a "key set" indicator; leaving the field blank on save keeps the existing key.
- **"Suggest with AI" button** in `XPathSourceForm`: enabled only when `aiSettings.enabled && aiSettings.apiKeySet`; on click calls `suggest_xpath_source(feedUrl)` then `onSelectorsChange(result)` and sets a status; the user then clicks the existing **Preview** to validate.
- **Test mode** (`testModeInvoke`): `get_ai_settings` returns a disabled default; `set_ai_settings` updates an in-memory object and returns it (masking literal keys); `suggest_xpath_source` throws `"AI suggestions require the Tauri app."` (consistent with `add_xpath_source`).

### Docs (`docs/ai-configuration.md`) + DESIGN.md

- `docs/ai-configuration.md`: choosing a provider; base URL + model examples for Anthropic and OpenAI-compatible servers; using a literal key vs `$ENV_VAR`; **the caveat that apps launched from Finder/Dock/Start Menu may not inherit your shell environment**, so env references require launching with the variable exported (or setting it in the app's launch environment); a note that page HTML is sent to the configured provider.
- DESIGN.md: add AI configuration + AI-assisted XPath under information architecture/components, and note the AI-suggest → preview-validate loop.

## Data flow

`Suggest with AI` → `suggest_xpath_source(url)` → load `ai_settings` → `fetch_normalized` → `ai::suggest_xpath_selectors` (resolve key, call provider, parse+validate JSON) → `XPathSelectors` → frontend fills form → user runs existing **Preview** (`preview_xpath_source`) → existing diagnostics. No new persistence of suggestions; nothing auto-added.

## Error handling

- Not configured / disabled / unresolved env var / provider HTTP error / unparseable model output → returned as a `String` error surfaced via the existing `runTask` status line; the form is left unchanged.
- Selector-level compile failures are downgraded to empty fields (partial suggestion) rather than failing the whole call, unless a required field (items/title/url) ends up empty.
- The literal API key is never serialized back to the renderer.

## Security

- Secrets: literal keys stored in the app-data SQLite DB and never returned to the renderer; env-reference form stores no secret at rest and resolves only in the backend at call time.
- The LLM request is made from Rust; the key is not exposed to the web layer.
- Page HTML is sent to the configured provider as data; document this. Prompt-injection from page content can only yield wrong selectors, which the user validates via preview; extraction still flows through ammonia + DOMPurify.

## Testing

- **Rust unit tests**:
  - `ai_settings` round-trip: set then get; blank `api_key` on a second set keeps the prior key; literal key is not exposed (masked) while a `$NAME` reference is exposed via `api_key_reference`.
  - `resolve_api_key`: `$NAME` resolves from env (set a temp var in-test), `${NAME}` form, missing var errors, literal passes through.
  - JSON→`XPathSelectors` mapping: feed a fixed model-response string (with code fences + prose) and assert the parsed selectors; an invalid selector is dropped to empty; empty required fields error.
- **Build**: `cargo test --manifest-path src-tauri/Cargo.toml`; `npm run build`.
- **Manual (`npm run tauri dev`)**: configure a provider + key (and a `$ENV` variant), enable; on the XPath form click "Suggest with AI" for a real page; confirm fields populate and the existing Preview validates; confirm the docs link opens.

## Implementation order

1. `ai_settings` table + models + `get/set_ai_settings` commands + Rust tests.
2. Expose `fetch_normalized` in `xpath_adapter.rs`.
3. `ai.rs`: `resolve_api_key`, prompt, provider dispatch, JSON parse/validate + `suggest_xpath_source` command + mapping tests.
4. Frontend AI settings card + load/save + test-mode parity.
5. "Suggest with AI" button in `XPathSourceForm`.
6. `docs/ai-configuration.md` + UI doc link + DESIGN.md + verification.
