# Plugin Cookie & Reader Customization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the plugin cookie a persistent, plugin-level credential used by both list and detail (正文) fetches, add a "check cookie" validity button, and let plugins customize the article reading view.

**Architecture:** A new `plugin_credentials` SQLite table holds one cookie per plugin id. At fetch time the command layer resolves an effective cookie (per-source override → plugin cookie → none) and injects it into `XPathSelectors.cookie`, so the existing `xpath_adapter::fetch_page` path applies it to list and detail fetches unchanged. Plugins declare an optional `auth` block (pack-level) for login probing and an optional `reader` block (on `selectors`) for reading-view customization (server-side DOM transforms + render-time layout/CSS).

**Tech Stack:** Rust (Tauri, rusqlite, sxd-xpath/`parser`, regex, reqwest), TypeScript/React (Vite), JSON Schema. Two repos: Feader app (`/Users/fwmbam4/CodeHub/Frankie/Feader`) and FeaderHub registry (`/Users/fwmbam4/CodeHub/Frankie/FeaderHub`).

**Spec:** `docs/superpowers/specs/2026-05-24-plugin-cookie-and-reader-customization-design.md`

**Verification commands:**
- Rust: `cd src-tauri && cargo test <name>` / `cargo check`
- TS: `npx tsc --noEmit 2>&1 | grep -v "is deprecated"` (deprecation warnings are pre-existing noise)

---

## File Structure

Feader app:
- `src-tauri/src/models.rs` — new types: `PluginCredential`, `CredentialCheck`, `PluginAuth`, `ReaderConfig`, `ReaderLayout`; add `auth` to pack models; add `reader` to `XPathSelectors`.
- `src-tauri/src/db.rs` — `plugin_credentials` table + repo methods.
- `src-tauri/src/plugin_registry.rs` — thread `auth` through pack construction.
- `src-tauri/src/xpath_adapter.rs` — pure helpers: `resolve_cookie`, `evaluate_logged_in`, `apply_reader_transforms`; wire transforms into detail fetch; a `check_login_state` fetch wrapper.
- `src-tauri/src/lib.rs` — cookie resolution wiring + commands `get_plugin_credential`, `set_plugin_credential`, `check_plugin_credential`.
- `src/App.tsx` — TS types; plugin cookie UI + check button; reader Layers A(render)/B/C.
- `src/App.css` — reader custom-field + scoped-CSS wrapper + cookie field/button styles.

FeaderHub registry:
- `schemas/xpath-rule-pack.schema.json` — `auth` (pack) + `reader` (inside `selectors`).
- `plugins/official.naixi-forum.xpath/xpath-rule-pack.json` — populate `auth` + `reader`.
- `plugins/official.naixi-forum.xpath/manifest.json` + `registry/index.json` — updated entry sha256.

---

## Phase 0 — FeaderHub schema & Naixi pack

### Task 0.1: Add `auth` + `reader` to the rule-pack schema

**Files:**
- Modify: `/Users/fwmbam4/CodeHub/Frankie/FeaderHub/schemas/xpath-rule-pack.schema.json`

- [ ] **Step 1: Read the current schema to find the `selectors` and top-level property objects**

Run: `sed -n '1,200p' /Users/fwmbam4/CodeHub/Frankie/FeaderHub/schemas/xpath-rule-pack.schema.json`
Expected: locate the `properties` for the pack and the `selectors` sub-schema (note whether `additionalProperties` is set).

- [ ] **Step 2: Add the pack-level `auth` property**

In the pack's top-level `properties` object, add:

```json
"auth": {
  "type": "object",
  "properties": {
    "checkUrl": { "type": "string", "format": "uri" },
    "loggedInXPath": { "type": "string" }
  },
  "required": ["checkUrl", "loggedInXPath"],
  "additionalProperties": false
}
```

- [ ] **Step 3: Add the `reader` property inside the `selectors` sub-schema**

```json
"reader": {
  "type": "object",
  "properties": {
    "removeSelectors": { "type": "array", "items": { "type": "string" } },
    "resolveRelativeUrls": { "type": "boolean" },
    "rewriteLinks": { "type": "boolean" },
    "showCustomFields": { "type": "boolean" },
    "layout": {
      "type": "object",
      "properties": {
        "typography": { "enum": ["system", "serif", "large"] },
        "width": { "enum": ["narrow", "normal", "wide"] },
        "immersive": { "type": "boolean" }
      },
      "additionalProperties": false
    },
    "css": { "type": "string" }
  },
  "additionalProperties": false
}
```

- [ ] **Step 4: Validate the JSON parses**

Run: `python3 -m json.tool /Users/fwmbam4/CodeHub/Frankie/FeaderHub/schemas/xpath-rule-pack.schema.json > /dev/null && echo OK`
Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git -C /Users/fwmbam4/CodeHub/Frankie/FeaderHub add schemas/xpath-rule-pack.schema.json
git -C /Users/fwmbam4/CodeHub/Frankie/FeaderHub commit -m "Add auth and reader fields to rule-pack schema"
```

### Task 0.2: Populate `auth` + `reader` in the Naixi pack and refresh hashes

**Files:**
- Modify: `/Users/fwmbam4/CodeHub/Frankie/FeaderHub/plugins/official.naixi-forum.xpath/xpath-rule-pack.json`
- Modify: `/Users/fwmbam4/CodeHub/Frankie/FeaderHub/plugins/official.naixi-forum.xpath/manifest.json`
- Modify: `/Users/fwmbam4/CodeHub/Frankie/FeaderHub/registry/index.json`

- [ ] **Step 1: Read the current pack JSON**

Run: `cat /Users/fwmbam4/CodeHub/Frankie/FeaderHub/plugins/official.naixi-forum.xpath/xpath-rule-pack.json`
Expected: see the candidate `selectors` object and top-level keys.

- [ ] **Step 2: Add the pack-level `auth` block** (top level, next to `id`/`candidates`)

```json
"auth": {
  "checkUrl": "https://forum.naixi.net/home.php?mod=spacecp",
  "loggedInXPath": "//a[contains(@href,'logout') or contains(@href,'action=logout')]"
}
```

- [ ] **Step 3: Add a `reader` block inside the candidate's `selectors`** (next to `contentCleanup`)

```json
"reader": {
  "removeSelectors": ["//ignore_js_op", "//*[contains(@class,'quote')]"],
  "resolveRelativeUrls": true,
  "rewriteLinks": true,
  "showCustomFields": true,
  "layout": { "typography": "serif", "width": "normal", "immersive": false }
}
```

(No `css` for now; it is optional and the official forum needs no extra CSS yet.)

- [ ] **Step 4: Recompute the entry sha256 and update both references**

```bash
cd /Users/fwmbam4/CodeHub/Frankie/FeaderHub
SHA=$(shasum -a 256 plugins/official.naixi-forum.xpath/xpath-rule-pack.json | cut -d' ' -f1)
echo "$SHA"
```
Then set `"sha256": "<SHA>"` in BOTH `plugins/official.naixi-forum.xpath/manifest.json` and the matching entry in `registry/index.json`.

- [ ] **Step 5: Validate JSON + confirm hashes match**

```bash
cd /Users/fwmbam4/CodeHub/Frankie/FeaderHub
python3 -m json.tool plugins/official.naixi-forum.xpath/xpath-rule-pack.json > /dev/null && echo PACK_OK
grep -o '"sha256": "[a-f0-9]*"' plugins/official.naixi-forum.xpath/manifest.json registry/index.json
```
Expected: `PACK_OK` and identical sha256 in both files equal to `$SHA`.

- [ ] **Step 6: Commit**

```bash
git -C /Users/fwmbam4/CodeHub/Frankie/FeaderHub add plugins/official.naixi-forum.xpath/xpath-rule-pack.json plugins/official.naixi-forum.xpath/manifest.json registry/index.json
git -C /Users/fwmbam4/CodeHub/Frankie/FeaderHub commit -m "Add Naixi auth probe and reader customization"
```

---

## Phase 1 — Rust models & registry

All paths in this phase are under `/Users/fwmbam4/CodeHub/Frankie/Feader`.

### Task 1.1: Add reader/auth/credential types

**Files:**
- Modify: `src-tauri/src/models.rs`

- [ ] **Step 1: Add `reader` to `XPathSelectors`** (after the `plugin` field, ~line 164)

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reader: Option<ReaderConfig>,
```

- [ ] **Step 2: Add the reader config structs** (place near `ContentCleanupRule`, ~line 167)

```rust
/// Plugin-authored customization of the article reading view.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderConfig {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove_selectors: Vec<String>,
    #[serde(default)]
    pub resolve_relative_urls: bool,
    #[serde(default)]
    pub rewrite_links: bool,
    #[serde(default)]
    pub show_custom_fields: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<ReaderLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub css: Option<String>,
}

/// Recommended reader presentation defaults from a plugin.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReaderLayout {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typography: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immersive: Option<bool>,
}
```

- [ ] **Step 3: Add the pack-level `auth` struct and field**

Add the struct near the pack models (~line 240):

```rust
/// Login probe declared by a plugin for credential validity checks.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginAuth {
    pub check_url: String,
    pub logged_in_xpath: String,
}
```

Add `auth` to `XPathRulePack` (after `parameters`, ~line 238):

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<PluginAuth>,
```

Add `auth` to `RemoteXPathRulePack` (after `parameters`, ~line 335):

```rust
    #[serde(default)]
    pub auth: Option<PluginAuth>,
```

- [ ] **Step 4: Add credential response types** (place near `AiSettings`, ~line 104)

```rust
/// Plugin credential metadata returned to the renderer (cookie never echoed).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginCredential {
    pub plugin_id: String,
    pub cookie_set: bool,
    pub cookie_reference: Option<String>,
    pub updated_at: Option<String>,
    pub last_checked_at: Option<String>,
    pub last_check_ok: Option<bool>,
    pub last_check_message: Option<String>,
}

/// Result of probing a plugin credential's validity.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCheck {
    pub ok: bool,
    pub message: String,
    pub checked_at: String,
}
```

- [ ] **Step 5: Compile**

Run: `cd src-tauri && cargo check`
Expected: compiles (existing `XPathRulePack {...}` literals in `plugin_registry.rs` will now error for the missing `auth` field — fixed in Task 1.2). If only those errors appear, proceed to 1.2; otherwise fix type errors here.

### Task 1.2: Thread `auth` through plugin_registry

**Files:**
- Modify: `src-tauri/src/plugin_registry.rs`

- [ ] **Step 1: Set `auth` in the merged remote pack** (in `fetch_remote_xpath_rule_pack`, the `Ok(XPathRulePack { ... })` literal ~line 131)

Add after `parameters: pack.parameters,`:

```rust
        auth: pack.auth,
```

- [ ] **Step 2: Set `auth: None` in the bundled `rule_pack` helper** (the `XPathRulePack { ... }` literal ~line 268)

Add after `parameters: None,`:

```rust
        auth: None,
```

- [ ] **Step 3: Compile**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/plugin_registry.rs
git commit -m "feat: add reader and auth fields to plugin models"
```

---

## Phase 2 — DB: plugin_credentials

**Files:** `src-tauri/src/db.rs`

### Task 2.1: Create the table

- [ ] **Step 1: Add the table to the schema batch** (inside the `execute_batch` string with the other `CREATE TABLE IF NOT EXISTS`, after `registry_cache` ~line 776)

```sql
        CREATE TABLE IF NOT EXISTS plugin_credentials (
            plugin_id          TEXT PRIMARY KEY,
            cookie             TEXT NOT NULL DEFAULT '',
            updated_at         TEXT NOT NULL,
            last_checked_at    TEXT,
            last_check_ok      INTEGER,
            last_check_message TEXT
        );
```

- [ ] **Step 2: Compile**

Run: `cd src-tauri && cargo check`
Expected: compiles.

### Task 2.2: Repo methods (TDD)

- [ ] **Step 1: Write the failing test** (in the `#[cfg(test)] mod tests` block in `db.rs`, near `ai_settings_round_trip_and_key_masking`)

```rust
    #[test]
    fn plugin_credential_round_trip_and_masking() {
        let db = AppDatabase::open_in_memory().expect("open db");
        // unset → cookie_set false
        let empty = db.get_plugin_credential("official.naixi-forum.xpath").unwrap();
        assert!(!empty.cookie_set);

        db.set_plugin_credential("official.naixi-forum.xpath", "sid=abc; uid=1").unwrap();
        let saved = db.get_plugin_credential("official.naixi-forum.xpath").unwrap();
        assert!(saved.cookie_set);
        assert_eq!(saved.cookie_reference, None); // literal not echoed
        assert_eq!(db.raw_plugin_cookie("official.naixi-forum.xpath").unwrap().as_deref(), Some("sid=abc; uid=1"));

        // env reference is surfaced (not masked away)
        db.set_plugin_credential("p2", "$FEADER_NAIXI_COOKIE").unwrap();
        let envref = db.get_plugin_credential("p2").unwrap();
        assert!(envref.cookie_set);
        assert_eq!(envref.cookie_reference.as_deref(), Some("$FEADER_NAIXI_COOKIE"));

        // blank clears
        db.set_plugin_credential("p2", "").unwrap();
        assert!(!db.get_plugin_credential("p2").unwrap().cookie_set);

        // check recording
        db.record_plugin_credential_check("official.naixi-forum.xpath", true, "已登录").unwrap();
        let checked = db.get_plugin_credential("official.naixi-forum.xpath").unwrap();
        assert_eq!(checked.last_check_ok, Some(true));
        assert_eq!(checked.last_check_message.as_deref(), Some("已登录"));
        assert!(checked.last_checked_at.is_some());
    }
```

> Note: confirm an in-memory constructor exists. If `AppDatabase::open_in_memory` is not present, check how existing tests build a DB (search `fn open_in_memory` / `AppDatabase::open` in tests) and use that constructor instead.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src-tauri && cargo test plugin_credential_round_trip_and_masking`
Expected: FAIL — methods `get_plugin_credential` / `set_plugin_credential` / `raw_plugin_cookie` / `record_plugin_credential_check` do not exist.

- [ ] **Step 3: Implement the methods** (add to `impl AppDatabase`, near `set_ai_settings`)

```rust
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
            cookie_reference: crate::models::is_env_reference(trimmed)
                .then(|| trimmed.to_string()),
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
```

Add the import at the top of `db.rs` if not already covered: `use crate::models::PluginCredential;` (the file already imports model types — add `PluginCredential` to that `use` list).

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd src-tauri && cargo test plugin_credential_round_trip_and_masking`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db.rs
git commit -m "feat: add plugin_credentials table and repository methods"
```

---

## Phase 3 — Cookie resolution (items 1 & 4)

**Files:** `src-tauri/src/xpath_adapter.rs`, `src-tauri/src/lib.rs`

### Task 3.1: Pure cookie precedence helper (TDD)

- [ ] **Step 1: Write the failing test** (in the `#[cfg(test)] mod tests` of `xpath_adapter.rs`)

```rust
    #[test]
    fn resolves_cookie_precedence() {
        // source override wins
        assert_eq!(
            resolve_cookie(Some("src=1"), Some("plugin=2")).as_deref(),
            Some("src=1")
        );
        // falls back to plugin cookie
        assert_eq!(
            resolve_cookie(None, Some("plugin=2")).as_deref(),
            Some("plugin=2")
        );
        assert_eq!(resolve_cookie(Some("   "), Some("plugin=2")).as_deref(), Some("plugin=2"));
        // none
        assert_eq!(resolve_cookie(None, None), None);
        assert_eq!(resolve_cookie(Some(""), None), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test resolves_cookie_precedence`
Expected: FAIL — `resolve_cookie` not defined.

- [ ] **Step 3: Implement** (add as a `pub` fn in `xpath_adapter.rs`)

```rust
/// Effective cookie precedence: non-empty source override → plugin cookie → none.
pub fn resolve_cookie(source_cookie: Option<&str>, plugin_cookie: Option<&str>) -> Option<String> {
    let pick = |value: Option<&str>| {
        value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    pick(source_cookie).or_else(|| pick(plugin_cookie))
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd src-tauri && cargo test resolves_cookie_precedence`
Expected: PASS.

### Task 3.2: Wire effective cookie into commands

- [ ] **Step 1: Add a resolution helper in `lib.rs`** (near `parse_xpath_selectors`, ~line 453)

```rust
/// Fill `selectors.cookie` with the plugin-level cookie when the source has no override.
fn apply_plugin_cookie(database: &AppDatabase, mut selectors: XPathSelectors) -> XPathSelectors {
    let plugin_id = selectors
        .plugin
        .as_ref()
        .map(|plugin| plugin.id.clone());
    let plugin_cookie = plugin_id
        .as_deref()
        .and_then(|id| database.raw_plugin_cookie(id).ok().flatten());
    selectors.cookie = xpath_adapter::resolve_cookie(
        selectors.cookie.as_deref(),
        plugin_cookie.as_deref(),
    );
    selectors
}
```

> Confirm `XPathSourcePluginInfo` exposes `id` (it does — used by `pluginSourceInfo`). If the field name differs, adjust `plugin.id`.

- [ ] **Step 2: Use it in the refresh path** — in `refresh_source_record` (~line 431), replace:

```rust
            let selectors = parse_xpath_selectors(source)?;
```
with:
```rust
            let selectors = apply_plugin_cookie(database, parse_xpath_selectors(source)?);
```

- [ ] **Step 3: Give `preview_xpath_source` DB access and resolve** — change the command signature (~line 259):

```rust
async fn preview_xpath_source(
    request: PreviewXPathSourceRequest,
    database: tauri::State<'_, AppDatabase>,
) -> Result<XPathPreview, String> {
    let url = request.url.trim();
    if url.is_empty() {
        return Err("XPath source URL is required".to_string());
    }
    let selectors = apply_plugin_cookie(&database, request.selectors);
    xpath_adapter::preview_xpath_source(url, &selectors).await
}
```

- [ ] **Step 4: Resolve in add/update too** — in `add_xpath_source` (~line 284) and `update_xpath_source` (~line 305), wrap the selectors before fetching:

```rust
    let selectors = apply_plugin_cookie(&database, request.selectors);
    let feed = xpath_adapter::fetch_xpath_source(url, &selectors).await?;
```
(For `update_xpath_source`, the fetch uses `&source.url`; keep that and pass the resolved `selectors`.) Persist the ORIGINAL `request.selectors` (do not store the injected cookie): keep `database.add_xpath_source(url, title, &request.selectors)` / `update_xpath_source_config(source.id, &request.selectors)` unchanged so the plugin cookie is not duplicated into the source config.

- [ ] **Step 5: Compile**

Run: `cd src-tauri && cargo check`
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/xpath_adapter.rs src-tauri/src/lib.rs
git commit -m "feat: resolve plugin-level cookie for list and detail fetches"
```

---

## Phase 4 — Validity check (item 2)

**Files:** `src-tauri/src/xpath_adapter.rs`, `src-tauri/src/lib.rs`

### Task 4.1: Pure login-state evaluation (TDD)

- [ ] **Step 1: Write the failing test** (in `xpath_adapter.rs` tests)

```rust
    #[test]
    fn evaluates_logged_in_marker() {
        let logged_in = r#"<html><body><a href="member.php?action=logout">退出</a></body></html>"#;
        let logged_out = r#"<html><body><a href="member.php?action=login">登录</a></body></html>"#;
        let xpath = "//a[contains(@href,'logout') or contains(@href,'action=logout')]";
        assert!(evaluate_logged_in(logged_in, xpath).unwrap());
        assert!(!evaluate_logged_in(logged_out, xpath).unwrap());
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test evaluates_logged_in_marker`
Expected: FAIL — `evaluate_logged_in` not defined.

- [ ] **Step 3: Implement** — reuse the existing normalize+parse+xpath pattern (mirror `extract_detail_content_html`):

```rust
/// True when `logged_in_xpath` matches at least one node in `document`.
pub fn evaluate_logged_in(document: &str, logged_in_xpath: &str) -> Result<bool, String> {
    let normalized = normalize_html_document(document)?;
    let package = parser::parse(&normalized).map_err(|error| {
        format!("XPath adapter currently expects well-formed static HTML/XML: {error}")
    })?;
    let root = Node::Root(package.as_document().root());
    let factory = Factory::new();
    let xpath = factory
        .build(logged_in_xpath)
        .map_err(|error| format!("Invalid login XPath: {error}"))?
        .ok_or_else(|| "Invalid login XPath".to_string())?;
    let context = Context::new();
    match xpath.evaluate(&context, root).map_err(|error| error.to_string())? {
        Value::Nodeset(nodes) => Ok(!nodes.document_order().is_empty()),
        Value::Boolean(value) => Ok(value),
        Value::String(value) => Ok(!value.is_empty()),
        Value::Number(value) => Ok(value != 0.0),
    }
}
```

> Match the exact `Value`/`Context`/`Factory` imports already used in this file (search existing `Factory::new()` / `Context` usage near the top and reuse the same paths). If a `compile_xpath` helper already exists, build with that instead of `Factory` directly.

- [ ] **Step 4: Run to verify it passes**

Run: `cd src-tauri && cargo test evaluates_logged_in_marker`
Expected: PASS.

### Task 4.2: `check_plugin_credential` + get/set commands

- [ ] **Step 1: Add a fetch+evaluate wrapper in `xpath_adapter.rs`**

```rust
/// Fetch `check_url` with the given cookie and report whether the login marker is present.
pub async fn check_login_state(
    check_url: &str,
    cookie: Option<&str>,
    logged_in_xpath: &str,
) -> Result<(bool, String), String> {
    let mut selectors = XPathSelectors::default();
    selectors.cookie = cookie.map(str::to_string);
    let body = fetch_page(check_url, &selectors).await?;
    if looks_like_interstitial_document(&body) {
        return Ok((false, "返回了反爬/浏览器校验页,无法确认登录态".to_string()));
    }
    match evaluate_logged_in(&body, logged_in_xpath) {
        Ok(true) => Ok((true, "cookie 有效,已登录".to_string())),
        Ok(false) => Ok((false, "cookie 失效或已过期(未检测到登录标志)".to_string())),
        Err(error) => Ok((false, format!("校验失败: {error}"))),
    }
}
```

- [ ] **Step 2: Add the commands in `lib.rs`** (near `get_ai_settings`/`set_ai_settings`)

```rust
/// Read a plugin credential (cookie masked).
#[tauri::command]
fn get_plugin_credential(
    plugin_id: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<models::PluginCredential, String> {
    database.get_plugin_credential(&plugin_id)
}

/// Save (or clear, when blank) a plugin-level cookie.
#[tauri::command]
fn set_plugin_credential(
    plugin_id: String,
    cookie: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<models::PluginCredential, String> {
    database.set_plugin_credential(&plugin_id, &cookie)?;
    database.get_plugin_credential(&plugin_id)
}

/// Probe whether the stored cookie is still valid for a plugin.
#[tauri::command]
async fn check_plugin_credential(
    plugin_id: String,
    check_url: String,
    logged_in_xpath: String,
    database: tauri::State<'_, AppDatabase>,
) -> Result<models::CredentialCheck, String> {
    let cookie = database.raw_plugin_cookie(&plugin_id)?;
    if cookie.is_none() {
        return Err("尚未设置该插件的 cookie".to_string());
    }
    let (ok, message) =
        xpath_adapter::check_login_state(check_url.trim(), cookie.as_deref(), logged_in_xpath.trim())
            .await?;
    database.record_plugin_credential_check(&plugin_id, ok, &message)?;
    Ok(models::CredentialCheck {
        ok,
        message,
        checked_at: chrono_now_rfc3339(),
    })
}
```

> For `checked_at`, reuse whatever timestamp helper `lib.rs`/`db.rs` already uses (e.g., `now_string()` is in `db.rs`). Simplest: have the command return the value `record_plugin_credential_check` stored by reading back `get_plugin_credential(...).last_checked_at`. Replace the `chrono_now_rfc3339()` placeholder with that read-back to avoid adding a new helper:
>
> ```rust
> let checked_at = database
>     .get_plugin_credential(&plugin_id)?
>     .last_checked_at
>     .unwrap_or_default();
> Ok(models::CredentialCheck { ok, message, checked_at })
> ```

- [ ] **Step 3: Register the three commands** in `generate_handler!` (~line 489), adding to the list:

```rust
            get_plugin_credential,
            set_plugin_credential,
            check_plugin_credential,
```

- [ ] **Step 4: Compile + run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: all tests pass (including Phase 2/3/4 additions).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/xpath_adapter.rs src-tauri/src/lib.rs
git commit -m "feat: add plugin credential commands and login-state check"
```

---

## Phase 5 — Reader Layer A (server-side transforms)

**Files:** `src-tauri/src/xpath_adapter.rs`

### Task 5.1: `apply_reader_transforms` (TDD)

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn applies_reader_transforms() {
        use crate::models::ReaderConfig;
        let html = r#"<div><a href="/thread-1.html">x</a><img src="img/a.png"/><ignore_js_op>junk</ignore_js_op></div>"#;
        let reader = ReaderConfig {
            remove_selectors: vec!["//ignore_js_op".to_string()],
            resolve_relative_urls: true,
            rewrite_links: true,
            show_custom_fields: false,
            layout: None,
            css: None,
        };
        let out = apply_reader_transforms(html, "https://forum.naixi.net/forum-64-1.html", &reader).unwrap();
        assert!(!out.contains("junk"));
        assert!(out.contains("https://forum.naixi.net/thread-1.html"));
        assert!(out.contains("https://forum.naixi.net/img/a.png"));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test applies_reader_transforms`
Expected: FAIL — `apply_reader_transforms` not defined.

- [ ] **Step 3: Implement** — parse the fragment, strip `remove_selectors` nodes, and resolve relative `href`/`src` against the base URL. Use the same `parser`/`Node` machinery already in this file for parsing, and the `url` crate if available (check `Cargo.toml`; `reqwest` pulls in `url`, so `use url::Url;` should resolve) for absolutization. If node mutation via `sxd` is awkward, implement URL rewriting with a targeted regex pass over `href="..."` / `src="..."` and node removal by re-serializing without matched subtrees. Concrete regex-based approach (robust, no DOM mutation):

```rust
/// Apply plugin reader transforms to extracted detail HTML.
pub fn apply_reader_transforms(
    html: &str,
    base_url: &str,
    reader: &ReaderConfig,
) -> Result<String, String> {
    let mut out = html.to_string();

    // Remove nodes matched by removeSelectors (XPath) by extracting their serialized
    // HTML and deleting those substrings.
    for selector in &reader.remove_selectors {
        let selector = selector.trim();
        if selector.is_empty() {
            continue;
        }
        for fragment in evaluate_node_html_fragments(&out, selector)? {
            out = out.replace(&fragment, "");
        }
    }

    if reader.resolve_relative_urls || reader.rewrite_links {
        if let Ok(base) = url::Url::parse(base_url) {
            out = rewrite_attr_urls(&out, &base, "href");
            out = rewrite_attr_urls(&out, &base, "src");
        }
    }
    Ok(out)
}

fn rewrite_attr_urls(html: &str, base: &url::Url, attr: &str) -> String {
    let pattern = format!(r#"(?i){attr}\s*=\s*"([^"]*)""#);
    let regex = match Regex::new(&pattern) {
        Ok(regex) => regex,
        Err(_) => return html.to_string(),
    };
    regex
        .replace_all(html, |caps: &regex::Captures| {
            let raw = &caps[1];
            match base.join(raw) {
                Ok(joined) => format!("{attr}=\"{}\"", joined),
                Err(_) => caps[0].to_string(),
            }
        })
        .to_string()
}
```

For `evaluate_node_html_fragments`, add a helper that evaluates the XPath to a nodeset and serializes each node back to HTML using the same approach as `evaluate_content_html` (which already produces node HTML). Reuse `evaluate_content_html`'s serialization path generalized to return all matched nodes' HTML strings:

```rust
fn evaluate_node_html_fragments(document: &str, selector: &str) -> Result<Vec<String>, String> {
    let normalized = normalize_html_document(document)?;
    let package = parser::parse(&normalized).map_err(|error| {
        format!("XPath adapter currently expects well-formed static HTML/XML: {error}")
    })?;
    let root = Node::Root(package.as_document().root());
    let factory = Factory::new();
    let xpath = factory
        .build(selector)
        .map_err(|error| format!("Invalid removeSelector XPath: {error}"))?
        .ok_or_else(|| "Invalid removeSelector XPath".to_string())?;
    let context = Context::new();
    let mut fragments = Vec::new();
    if let Value::Nodeset(nodes) = xpath.evaluate(&context, root).map_err(|e| e.to_string())? {
        for node in nodes.document_order() {
            if let Some(html) = serialize_node_html(node) {
                fragments.push(html);
            }
        }
    }
    Ok(fragments)
}
```

> `serialize_node_html` should reuse the exact node→HTML serialization already used inside `evaluate_content_html` (extract that inner serialization into a shared `serialize_node_html(node) -> Option<String>` and call it from both places — DRY). Read `evaluate_content_html` first and factor its body out rather than duplicating.

- [ ] **Step 4: Run to verify it passes**

Run: `cd src-tauri && cargo test applies_reader_transforms`
Expected: PASS.

> If `normalize_html_document` re-wraps fragments such that `.replace(&fragment, "")` misses, switch the remove step to operate on the parsed+reserialized `normalized` document consistently (normalize once, then remove, then rewrite URLs) so the strings match.

### Task 5.2: Wire transforms into detail fetch

- [ ] **Step 1: Apply transforms after content extraction** — in `fetch_detail_content` (~line 485), after the `apply_content_cleanup` step, chain the reader transform when present:

```rust
    let content = selector
        .map(|selector| evaluate_content_html(root, Some(selector)))
        .transpose()?
        .flatten()
        .map(|html| apply_content_cleanup(&html, selectors))
        .transpose()?
        .map(|html| match &selectors.reader {
            Some(reader) => apply_reader_transforms(&html, url, reader),
            None => Ok(html),
        })
        .transpose()?;
```

- [ ] **Step 2: Compile + test**

Run: `cd src-tauri && cargo test`
Expected: all pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/xpath_adapter.rs
git commit -m "feat: apply plugin reader transforms to detail content"
```

---

## Phase 6 — Frontend types & cookie UI

**Files:** `src/App.tsx`, `src/App.css`

### Task 6.1: TypeScript types

- [ ] **Step 1: Add `reader` to the selectors type** — find the selectors type (the object with `items`/`title`/`cookie`/`contentCleanup`/`customFields`; search `customFields` in `App.tsx`). Add:

```ts
  reader?: ReaderConfig | null;
```

And define the types near `XPathRulePack` (~line 163):

```ts
type ReaderLayout = {
  typography?: "system" | "serif" | "large";
  width?: "narrow" | "normal" | "wide";
  immersive?: boolean;
};

type ReaderConfig = {
  removeSelectors?: string[];
  resolveRelativeUrls?: boolean;
  rewriteLinks?: boolean;
  showCustomFields?: boolean;
  layout?: ReaderLayout | null;
  css?: string | null;
};

type PluginAuth = {
  checkUrl: string;
  loggedInXPath: string;
};

type PluginCredential = {
  pluginId: string;
  cookieSet: boolean;
  cookieReference?: string | null;
  updatedAt?: string | null;
  lastCheckedAt?: string | null;
  lastCheckOk?: boolean | null;
  lastCheckMessage?: string | null;
};

type CredentialCheck = { ok: boolean; message: string; checkedAt: string };
```

- [ ] **Step 2: Add `auth` to `XPathRulePack`** (after `parameters`, ~line 174):

```ts
  auth?: PluginAuth | null;
```

- [ ] **Step 3: Populate `auth`/`reader` in the test-mode Naixi pack** (`testModeXPathRulePacks`, ~line 454) — mirror the FeaderHub pack: add `reader` inside `selectors` and `auth` at pack level, matching the JSON from Task 0.2.

- [ ] **Step 4: Add test-mode `invoke` cases** — in `testModeInvoke` (~line 532), add cases so the UI works without Tauri:

```ts
    case "get_plugin_credential":
      return { pluginId: String(args?.pluginId ?? ""), cookieSet: false } as T;
    case "set_plugin_credential":
      return { pluginId: String(args?.pluginId ?? ""), cookieSet: Boolean(String(args?.cookie ?? "").trim()) } as T;
    case "check_plugin_credential":
      return { ok: true, message: "测试模式:已登录", checkedAt: new Date().toISOString() } as T;
```

- [ ] **Step 5: Typecheck**

Run: `npx tsc --noEmit 2>&1 | grep -v "is deprecated"`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx
git commit -m "feat: add reader/auth/credential types to frontend"
```

### Task 6.2: Plugin cookie field + check button

- [ ] **Step 1: Load the plugin credential when the Add Source dialog opens** — in `openPluginDialog` (~line 1030) and the dialog state, fetch and store the credential:

```ts
const [pluginCredential, setPluginCredential] = useState<PluginCredential | null>(null);
const [credentialCheck, setCredentialCheck] = useState<CredentialCheck | null>(null);
```

In `openPluginDialog(pack)`:
```ts
  setCredentialCheck(null);
  invoke<PluginCredential>("get_plugin_credential", { pluginId: pack.id })
    .then(setPluginCredential)
    .catch(() => setPluginCredential(null));
```

- [ ] **Step 2: Replace the dialog's per-source cookie field binding with the plugin cookie** — the dialog cookie input (`dialogCookie`, ~line 2208) now represents the plugin-level cookie. Show masked state when `pluginCredential.cookieSet` and the field is untouched; on save, call `set_plugin_credential`. Add a save handler:

```ts
async function savePluginCookie(pack: XPathRulePack, cookie: string) {
  const updated = await invoke<PluginCredential>("set_plugin_credential", {
    pluginId: pack.id,
    cookie,
  });
  setPluginCredential(updated);
}
```

- [ ] **Step 3: Add the "检查 cookie" button** next to the cookie field (only when `pack.auth` exists):

```tsx
{showPluginDialog?.auth ? (
  <button
    type="button"
    className="hub-cookie-check"
    onClick={async () => {
      try {
        const result = await invoke<CredentialCheck>("check_plugin_credential", {
          pluginId: showPluginDialog.id,
          checkUrl: showPluginDialog.auth!.checkUrl,
          loggedInXpath: showPluginDialog.auth!.loggedInXPath,
        });
        setCredentialCheck(result);
      } catch (error) {
        setCredentialCheck({ ok: false, message: String(error), checkedAt: new Date().toISOString() });
      }
    }}
  >
    检查 cookie
  </button>
) : null}
{credentialCheck ? (
  <small className={credentialCheck.ok ? "cookie-status ok" : "cookie-status bad"}>
    {credentialCheck.message}
  </small>
) : null}
```

> Tauri snake_cases command args from camelCase, so `loggedInXpath` maps to the `logged_in_xpath` param. Verify the casing against an existing camelCase arg (e.g. `sourceId` → `source_id`); if the bridge expects `loggedInXPath`, match that exactly.

- [ ] **Step 4: Keep the per-source override** — leave the existing source-detail selectors cookie field (`selectors.cookie`, ~line 3516) as-is; it remains the advanced per-source override. Add a one-line hint that empty means "use the plugin cookie".

- [ ] **Step 5: Typecheck**

Run: `npx tsc --noEmit 2>&1 | grep -v "is deprecated"`
Expected: no errors.

- [ ] **Step 6: Add styles** — in `src/App.css`, add `.hub-cookie-check`, `.cookie-status.ok`, `.cookie-status.bad` (reuse `--color-success`/`--color-danger`).

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: plugin cookie management and validity check button"
```

---

## Phase 7 — Frontend reader Layers A(render)/B/C

**Files:** `src/App.tsx`, `src/App.css`

> Reader config for a saved article comes from its source's selectors. Resolve the active source for the open article (the reader already knows `selectedSourceId`/the article's `sourceId`) and read `source.config`/selectors `.reader`. If the source config is not already in frontend state, look it up from the loaded sources list. The plugin trust comes from `selectors.plugin.trust`.

### Task 7.1: Render custom fields in the reader (Layer A render-time)

- [ ] **Step 1: Locate the reader content render** (search `contentText`/`content_html` render in the reader view, ~line 1718+).

- [ ] **Step 2: When `reader.showCustomFields` is true**, render the article's custom field values (already parsed from `tagsJson`) in the reader header. Reuse the existing custom-field parsing used elsewhere (search `tagsJson` usage). Render:

```tsx
{readerConfig?.showCustomFields && customFields.length > 0 ? (
  <dl className="reader-custom-fields">
    {customFields.map((field) => (
      <div key={field.label ?? field.value}>
        <dt>{field.label}</dt>
        <dd>{field.value}</dd>
      </div>
    ))}
  </dl>
) : null}
```

- [ ] **Step 3: Typecheck**

Run: `npx tsc --noEmit 2>&1 | grep -v "is deprecated"`
Expected: no errors.

### Task 7.2: Apply layout defaults on open (Layer B)

- [ ] **Step 1: When an article opens**, if its source's `reader.layout` is set and the user has not explicitly chosen a typography this session, apply `layout.typography` as the default `readerTypography`, and `layout.immersive` to choose the initial `readerView`. Track an explicit-choice flag so user overrides win:

```ts
const [userChoseTypography, setUserChoseTypography] = useState(false);
// in the typography control onChange: setUserChoseTypography(true);
// on open article:
if (!userChoseTypography && readerConfig?.layout?.typography) {
  setReaderTypography(readerConfig.layout.typography);
}
```

- [ ] **Step 2: Typecheck**

Run: `npx tsc --noEmit 2>&1 | grep -v "is deprecated"`
Expected: no errors.

### Task 7.3: Inject scoped CSS for trusted plugins (Layer C)

- [ ] **Step 1: When `reader.css` is present AND the source's plugin trust is `official`/`bundled-official`**, render a scoped `<style>` plus a wrapper attribute on the content container:

```tsx
{readerConfig?.css && isTrustedPlugin(pluginTrust) ? (
  <style>{scopeCss(readerConfig.css, pluginId)}</style>
) : null}
<div className="reader-content" data-plugin={pluginId} /* existing content render */ />
```

```ts
function isTrustedPlugin(trust?: string): boolean {
  return trust === "official" || trust === "bundled-official";
}

// Naive scoping: prefix every top-level selector with the plugin wrapper.
function scopeCss(css: string, pluginId: string): string {
  const scope = `.reader-content[data-plugin="${pluginId}"]`;
  return css.replace(/(^|\})\s*([^{}]+)\{/g, (_m, brace, selector) =>
    `${brace} ${selector
      .split(",")
      .map((part: string) => `${scope} ${part.trim()}`)
      .join(", ")} {`);
}
```

- [ ] **Step 2: Typecheck**

Run: `npx tsc --noEmit 2>&1 | grep -v "is deprecated"`
Expected: no errors.

### Task 7.4: Reader styles + manual verification

- [ ] **Step 1: Add `.reader-custom-fields` styling** in `src/App.css` (mirror `.xpath-selector-summary` / `.hub-card-meta`).

- [ ] **Step 2: Build the app and verify end to end**

Run: `npm run tauri dev` (or the project's run command; check `package.json` scripts).
Manual checks:
- Set the Naixi cookie once in the Add Source dialog; add two Naixi sections; confirm both refresh without re-entering the cookie.
- Click "检查 cookie": shows 已登录 with a valid cookie, and 失效/过期 after clearing it.
- Open a Naixi thread: 正文 loads (cookie applied), `ignore_js_op`/quote blocks are stripped, links/images are absolute, custom fields show, serif layout applies.

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: plugin-driven reader customization (fields, layout, scoped css)"
```

---

## Self-Review Notes (spec coverage)

- Item 1 (persistent plugin-level cookie): Phase 2 (table) + Phase 3 (resolution) + Phase 6 (UI). Per-source override retained (Task 3.4 / 6.2 Step 4).
- Item 2 (validity check button): Phase 4 + Task 6.2 Step 3.
- Item 3 (reader customization, all 3 layers): Phase 0 (schema/pack), Phase 1 (types), Phase 5 (Layer A server-side), Phase 7 (Layer A render + B + C, C trust-gated).
- Item 4 (正文 uses cookie): Phase 3 wires the resolved cookie into the refresh/detail path; `fetch_detail_content` already calls `fetch_page` with the (now cookie-bearing) selectors. Manual verification in Task 7.4 Step 2.
- Masked display: Task 2.2 (backend) + Task 6.2 (UI).

## Risks / Watch-outs

- `apply_reader_transforms` remove-by-string-replace depends on the removed fragment serializing identically to what appears in the cleaned HTML. If normalization differs, normalize once up front and operate on that single normalized string (noted inline in Task 5.1).
- Tauri arg casing (camelCase → snake_case) for `loggedInXPath`/`logged_in_xpath` — verify against an existing command before assuming (noted in Task 6.2).
- `XPathSourcePluginInfo` must carry `trust` and `id` to the frontend for Layer C gating and credential lookup; both are already persisted, but confirm they survive the source round-trip.
