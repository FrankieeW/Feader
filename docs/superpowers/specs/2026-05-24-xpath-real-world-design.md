# XPath Real-World Hardening — Design

- Date: 2026-05-24
- Status: Approved (pending spec review)
- Scope: `src-tauri/src/xpath_adapter.rs`, `src-tauri/Cargo.toml`, `src/App.tsx` (XPath form), `DESIGN.md`. Builds on the existing XPath adapter, preview/diagnostics, and the ammonia ingest sanitization already in `db.rs::upsert_articles`.

## Goal

Make the existing XPath source feature work against real-world (non-well-formed) web pages, capture rich article HTML, follow pagination on refresh, and give users selector presets — so XPath sources are practically usable and testable via `npm run tauri dev`.

## Current state (baseline)

Already implemented and in sync front/back: `XPathSelectors`, `preview_xpath_source` → `XPathPreview { articles, diagnostics, next_page_url }` with per-field `XPathFieldDiagnostic`, `add_xpath_source`, `fetch_xpath_source` (refresh), frontend `XPathSourceForm` with live preview + diagnostics + next-page display, and a browser test-mode stub. Tests exist in `xpath_adapter.rs`.

**The blocker:** parsing uses `sxd_document::parser::parse`, a strict XML parser. Real pages are not well-formed XML, so `parse` fails on most live sites. Secondary gaps: `next_page` is detected in preview but never followed on refresh; XPath only fills `content_text`, never `content_html`.

## Approved approach

- **HTML parsing: Approach A (pure-Rust normalization).** Keep `sxd-xpath`; normalize messy HTML to well-formed, no-namespace XHTML first. No C dependency (keeps Tauri multi-platform builds simple) and preserves all existing selectors/diagnostics/tests/frontend.

## Out of scope (YAGNI)

- No swap to libxml2 / CSS selectors.
- No click-to-inspect selector generation (only static presets + hints).
- No pagination during preview (preview stays single-page; only reports the detected next URL).
- No per-source configurable page cap (fixed constant).

## Design

### 1. Real-world HTML parsing (foundation)

Add an isolated function in `xpath_adapter.rs`:

```
fn normalize_html(raw: &str) -> String
```

- Parse `raw` with `html5ever` into a `markup5ever_rcdom::RcDom` (tolerant; fixes unclosed tags, bare `<br>`, unquoted attrs, etc.).
- Serialize the tree to XML with `xml5ever`'s serializer.
- Strip XHTML/foreign namespace declarations (`xmlns`/`xmlns:*`) from the serialized output so all elements are in **no namespace**, preserving the contract that unprefixed XPath (`//article`, `.//h2/a/@href`) matches.
- Return the normalized XHTML string.

Both `fetch_xpath_source` and `preview_xpath_source` pass the fetched body through `normalize_html` before `parser::parse`. If `sxd` still fails to parse the normalized output (rare), the existing "expects well-formed static HTML/XML" error remains as the last-resort message.

New crates in `Cargo.toml`: `html5ever`, `markup5ever_rcdom`, `xml5ever` (pure Rust; pinned to compatible versions).

**Boundary/interface:** `normalize_html` is a pure `&str -> String` function, independently unit-testable. `parse_xpath_source` / `preview_xpath_document` keep their current signatures (they receive an already-normalized document string); only the two network-fetching functions add the normalization call. This keeps the change localized.

### 2. content_html extraction

- Add `fn node_inner_html(node: Node) -> String` that serializes a matched element's inner markup to an HTML string (recursive walk over sxd element/text nodes).
- In `parse_xpath_source`, when the `content` selector resolves to an element node, set `ParsedArticle.content_html = Some(node_inner_html(node))`; keep `content_text` as the fallback when the selector yields only text (current behavior).
- No new sanitization needed here: `db.rs::upsert_articles` already runs `content_html` through `ammonia` on store, and the reader runs DOMPurify at render — XPath-captured HTML inherits both layers.

### 3. Follow pagination on refresh

- Convert `fetch_xpath_source` into a bounded loop:
  1. Fetch + normalize + `parse_xpath_source` the start URL.
  2. Resolve `selectors.next_page` against the current document to an absolute URL.
  3. If present and not yet visited and under the cap, fetch the next page and append its articles.
  4. Repeat until no next URL, a cycle is detected (visited set), or the cap is reached.
- Hard cap: `const MAX_XPATH_PAGES: usize = 5;`. Visited-URL `HashSet<String>` prevents loops.
- Refresh still de-duplicates at storage via the existing `ON CONFLICT(source_id, url)` upsert, so overlapping pages are safe.
- Preview is unchanged (single page; reports `next_page_url` only).

### 4. Selector UX helpers (frontend)

- In `XPathSourceForm`, add a small **preset dropdown** that, when chosen, calls `onSelectorsChange` with a predefined `XPathSelectors` template. Initial presets (2–3): "Generic blog" (`//article` + `.//h2/a` title/url + `.//p` summary + `.//time/@datetime`), "Listing + links" (`//li` or `//*[contains(@class,'post')]` patterns). Presets are a static const map in `App.tsx`.
- Add concise per-field hint text under each `SelectorInput` (e.g., Items: "Repeating element for each article, e.g. `//article`").
- Frontend-only; no backend or data changes.

## Data flow

Refresh: `refresh_source` → `fetch_xpath_source` (normalize + parse, loop pages) → `upsert_articles` (ammonia-sanitizes `content_html`) → `list_articles`. Preview: `preview_xpath_source` → normalize + `preview_xpath_document` (diagnostics + sample articles, single page). No schema changes; `content_html` column already exists.

## Error handling

- `normalize_html` is infallible for input (html5ever is tolerant); if serialization somehow yields unparseable XML, the existing `parser::parse` error surfaces with the current message.
- Pagination: a failed next-page fetch stops the loop and returns articles gathered so far (partial success is better than total failure); the per-source `last_error` is only set if page 1 fails (via existing refresh error handling).
- Preview keeps returning structured `diagnostics` for invalid/empty selectors (unchanged).

## Testing

- **Rust unit tests** (`xpath_adapter.rs`):
  - `normalize_html` turns malformed HTML (unclosed `<p>`, bare `<br>`, unquoted attr) into a document from which `//article` extraction succeeds.
  - `content` selector on an element yields `content_html` containing inner tags (e.g., `<strong>`), not flattened text.
  - next-page URL resolves to an absolute URL (extend existing preview test or add a parse-level test).
- **Build:** `cargo test --manifest-path src-tauri/Cargo.toml`; `npm run build` for the frontend.
- **Manual (real backend):** `npm run tauri dev`, add an XPath source against a real article-listing page; verify preview diagnostics go green, articles extract, content shows rich HTML in the reader, and a multi-page source pulls more than one page on refresh. (Browser test mode cannot exercise real XPath fetching — it is read-only.)

## Implementation order

1. `normalize_html` + wire into fetch/preview + malformed-HTML test (foundation).
2. `node_inner_html` + `content_html` extraction + test.
3. Bounded pagination loop on refresh.
4. Frontend selector presets + hints.
5. DESIGN.md update + full verification (cargo test + tauri dev smoke).
