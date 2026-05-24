# Plugin Cookie & Reader Customization — Design

Date: 2026-05-24
Status: Approved (design)
Scope: Feader app (`src`, `src-tauri`) + FeaderHub registry (`schemas`, plugin packs)

## Goal

Four plugin improvements, centered on credentials and the reading view:

1. Plugin cookie becomes a **persistent, plugin-level** credential stored on the user's machine (set once, shared by all sources from that plugin).
2. A **"check cookie" button** that reports whether the cookie is still valid (logged in) or expired.
3. Plugins can **customize the article (正文) reading view**.
4. The **detail-content (正文) fetch uses the cookie** — verified end to end under the new plugin-level model.

## Current State (verified)

- **Cookie storage**: per-source, embedded in `selectors.cookie` (raw string / JSON object / `$ENV_VAR`), persisted inside `sources.config_json`. No plugin-level cookie exists.
- **Cookie usage**: `xpath_adapter::fetch_page` applies `selectors.cookie` to **both** list and detail fetches. 正文 is fetched at refresh via `enrich_articles_with_detail_content` → `fetch_detail_content` → `fetch_page`, so it already sends the cookie. The reader renders the stored `content_html`/`content_text`.
- **Reading view**: app-controlled only (`ReaderView` none/preview/immersive + `ReaderTypography`). Plugins influence content solely through `detailContent` XPath + `contentCleanup` regex.
- **Persistence precedents**: `ai_settings` (singleton table, api_key masked on read), `registry_cache` (KV). `sources.config_json` holds selectors + persisted plugin info; sources already know their plugin id.
- **Sanitization**: article HTML is sanitized with `ammonia`. `trust` field on packs distinguishes `official` / `bundled-official` from third-party.

## Decisions (from brainstorming)

- Cookie scope: **plugin-level**, with a **per-source override** retained (advanced).
- Storage: **local SQLite, plaintext** (consistent with `ai_settings`).
- Cookie display: **masked by default** (like `ai_settings.api_key`); "edit" to replace.
- Validity check: **probe login state** — plugin declares a check URL + a logged-in XPath marker.
- Reader customization: **all three layers** (A structured rules, B layout defaults, C scoped CSS); **Layer C gated to trusted plugins**.
- Cookie resolution happens in the **command layer**, keeping `xpath_adapter` pure.

## Architecture

### 1. Data model & storage (`src-tauri/src/db.rs`)

New table:

```sql
CREATE TABLE IF NOT EXISTS plugin_credentials (
  plugin_id          TEXT PRIMARY KEY,
  cookie             TEXT NOT NULL DEFAULT '',
  updated_at         TEXT NOT NULL,
  last_checked_at    TEXT,
  last_check_ok      INTEGER,        -- 1 | 0 | NULL (never checked)
  last_check_message TEXT
);
```

Repository methods on `Database`:
- `get_plugin_credential(plugin_id) -> PluginCredential` (cookie masked: present-or-empty flag + never the literal on read, mirroring `get_ai_settings`).
- `set_plugin_credential(plugin_id, cookie)` — blank cookie clears; non-blank replaces; updates `updated_at`.
- `record_plugin_credential_check(plugin_id, ok, message, checked_at)`.
- Internal `read_plugin_cookie_literal(plugin_id) -> Option<String>` for fetch-time resolution (not exposed to the frontend).

### 2. Cookie resolution & usage (items 1 & 4)

Effective cookie precedence, resolved in the command layer (`src-tauri/src/lib.rs`) before calling the adapter:

```
per-source selectors.cookie (non-empty)  →  plugin_credentials.cookie (by source plugin id)  →  none
```

- `$ENV` references continue to resolve on whichever string wins (logic stays in `cookie_header_value`).
- A small helper resolves the effective cookie and writes it into a cloned `XPathSelectors.cookie` before `preview_xpath_source` / refresh / detail fetch. `xpath_adapter` is unchanged, so **list and detail (正文) fetches both inherit the cookie**.
- Refresh path: where a source's selectors are loaded for `parse_xpath_source`/enrichment, inject the resolved cookie.

### 3. Validity check (item 2)

Rule pack gains an optional `auth` block:

```jsonc
"auth": {
  "checkUrl": "https://forum.naixi.net/home.php?mod=spacecp",
  "loggedInXPath": "//a[contains(@href,'logout') or contains(@href,'action=logout')]"
}
```

New command `check_plugin_credential(plugin_id) -> CredentialCheck { ok, message, checkedAt }`:
1. Resolve effective cookie for the plugin (error if none set).
2. Fetch `auth.checkUrl` with the cookie via the shared fetch path.
3. If the response is an interstitial/anti-bot page → `ok=false, message="anti-bot/challenge page"`.
4. Else evaluate `auth.loggedInXPath`: match → `ok=true` ("已登录"); no match → `ok=false` ("cookie 失效或已过期").
5. HTTP errors → `ok=false` with the status.
6. Persist via `record_plugin_credential_check`; return result.

If a pack has no `auth` block, the check falls back to: fetch the source/list URL with the cookie and report HTTP success + interstitial detection only (no login assertion).

### 4. Reader customization (item 3) — layered

Rule pack gains an optional `reader` block. Mirrored in TS types and Rust `models.rs`.

```jsonc
"reader": {
  "removeSelectors": ["//div[contains(@class,'ad')]"],   // Layer A
  "resolveRelativeUrls": true,                              // Layer A
  "rewriteLinks": true,                                     // Layer A (force absolute href/src)
  "showCustomFields": true,                                 // Layer A (render customFields in reader header)
  "layout": { "typography": "serif", "width": "wide", "immersive": false }, // Layer B (recommended defaults)
  "css": ".reader-content table { width:100%; }"            // Layer C (trusted plugins only)
}
```

- **Layer A — structured rules (all plugins).** `removeSelectors` and `resolveRelativeUrls`/`rewriteLinks` run **server-side in Rust during detail enrichment**, operating on the extracted detail HTML before it is stored/sanitized — they reuse the existing `extract_detail_content_html`/cleanup path and resolve URLs against the article base URL. `showCustomFields` is **render-time** in the reader (surfacing existing `customFields` in the header). This split keeps DOM-structural edits server-side and presentation toggles client-side.
- **Layer B — layout defaults (all plugins).** `reader.layout` provides recommended typography/width/immersive defaults applied when opening an article from that plugin's source. User controls still override and the user's explicit choice wins for the session.
- **Layer C — scoped CSS (trusted plugins only).** `reader.css` is injected into a wrapper `<style>` scoped under `.reader-content[data-plugin="<id>"]`, so it cannot affect app chrome. Gated on `trust` ∈ {official, bundled-official}. No raw HTML/JS templates (XSS/sandbox risk; HTML content is already ammonia-sanitized). CSS is applied as-is but confined by the scoping selector; we do not parse/validate CSS beyond wrapping.

### 5. Schema / manifest additions (spans both repos)

**FeaderHub** (`/Users/fwmbam4/CodeHub/Frankie/FeaderHub`):
- `schemas/xpath-rule-pack.schema.json`: add optional `auth` and `reader` objects (with `additionalProperties:false` sub-schemas).
- `plugins/official.naixi-forum.xpath/xpath-rule-pack.json`: populate `auth` (Naixi logout marker) and a starter `reader` block. Recompute the entry `sha256` and update both the manifest and `registry/index.json`.

**Feader app**:
- TS: extend `XPathRulePack` / selectors types with `auth` and `reader`; add `PluginCredential` + `CredentialCheck` types.
- Rust `models.rs`: add `auth` + `reader` to `XPathRulePack` and `RemoteXPathRulePack`; add `PluginCredential`, `CredentialCheck`.
- `plugin_registry.rs`: thread `auth`/`reader` through the merge and bundled constructors (`None`/default for bundled).

### 6. UI surfaces (`src/App.tsx`, `src/App.css`)

- **Add Source dialog & source detail**: the cookie field manages the **plugin-level** cookie — pre-filled (masked) from `get_plugin_credential`, saved via `set_plugin_credential`. A **"检查 cookie"** button calls `check_plugin_credential` and shows ok/expired + last-checked time. Per-source override remains as a secondary "advanced" field bound to `selectors.cookie`.
- **Reader**: apply Layer A transforms, render custom fields when `showCustomFields`, apply Layer B defaults on open, inject Layer C `<style>` when present and trusted.

### 7. Testing

- **Rust unit tests** (extend `xpath_adapter` / new `db` tests):
  - cookie precedence resolution (source override > plugin > none; `$ENV` on the winner).
  - `check_plugin_credential`: logged-in match, no-match (expired), interstitial, HTTP error, no-cookie error.
  - reader transforms: `removeSelectors`, relative→absolute URL rewriting.
  - `plugin_credentials` round-trip + masking + check recording.
- **TS**: `tsc --noEmit`; reader rendering of custom fields and CSS scoping wrapper; masked cookie display.
- **Manual**: build the app, set the Naixi cookie once, verify multiple Naixi sections share it, run "检查 cookie", open an article and confirm 正文 loads with cookie + reader customizations apply.

## Out of Scope

- OS keychain / encrypted-at-rest credential storage (plaintext local now).
- Raw HTML/JS reader templates from plugins.
- Per-domain global credential sharing across different plugins.
- Automatic cookie refresh / re-login flows.

## File-by-File Change Summary

FeaderHub:
- `schemas/xpath-rule-pack.schema.json` — add `auth`, `reader`.
- `plugins/official.naixi-forum.xpath/xpath-rule-pack.json` — add `auth`, `reader`; bump entry sha256.
- `plugins/official.naixi-forum.xpath/manifest.json` + `registry/index.json` — update sha256.

Feader `src-tauri/src`:
- `db.rs` — `plugin_credentials` table + repo methods + migration.
- `models.rs` — credential types; `auth`/`reader` on pack models.
- `xpath_adapter.rs` — reader Layer A transforms; check helper; cookie resolution helper.
- `plugin_registry.rs` — thread `auth`/`reader`.
- `lib.rs` — commands `get/set/check_plugin_credential`; cookie resolution wired into preview/refresh/detail.

Feader `src`:
- `App.tsx` — types; plugin cookie UI + check button; reader Layers A/B/C.
- `App.css` — reader custom-field + scoped-CSS wrapper styles; cookie field/button styles.
