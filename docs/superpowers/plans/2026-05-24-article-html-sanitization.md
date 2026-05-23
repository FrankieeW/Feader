# Article HTML Sanitization (B-primary + A-fallback) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop rendering untrusted RSS HTML through a hand-rolled sanitizer. Sanitize feed `content_html` at the Rust ingest boundary with `ammonia` (primary), and keep a vetted `DOMPurify` pass at render time (fallback for pre-existing/edge content).

**Architecture:** Mirror the reference projects — MrRSS sanitizes server-side at ingest; Folo uses vetted JS libraries (DOMPurify / rehype-sanitize) at render. Feader does both: `ammonia::clean` in `db.rs::upsert_articles` (the single choke point for both add + refresh) writes clean HTML to SQLite; the frontend `ReaderArticle` replaces its hand-rolled `sanitizeArticleHtml` with `DOMPurify.sanitize`.

**Tech Stack:** Rust + rusqlite + `ammonia` (`src-tauri`); React + TypeScript + `dompurify` (`src`). Backend uses `cargo test`; frontend verifies via `npm run build` + manual browser smoke (no JS test runner).

---

## Spec reference

Security review thread (this conversation): the current `sanitizeArticleHtml` is a hand-rolled blocklist feeding `dangerouslySetInnerHTML`; replace with vetted libraries at two layers.

## File map

- `src-tauri/Cargo.toml` — add `ammonia` dependency.
- `src-tauri/src/db.rs` — `sanitize_html` helper; sanitize `content_html` in `upsert_articles`; Rust test.
- `package.json` / `package-lock.json` — add `dompurify`.
- `src/App.tsx` — replace `sanitizeArticleHtml` body with DOMPurify; remove hand-rolled `isAllowedArticleUrl`.
- `DESIGN.md` — record the two-layer sanitization approach.

## Why both layers

`ammonia` at ingest cleans everything fetched going forward and means the DB never stores dangerous HTML. But rows already stored before this change are not re-sanitized until re-fetched, so the render-time `DOMPurify` pass guarantees safety for existing/edge data. This matches Folo's own defense-in-depth (extraction sanitize + render sanitize).

---

### Task 1: Sanitize `content_html` at ingest with ammonia (backend)

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/db.rs` (helper + `upsert_articles` + test)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/db.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml article_html_is_sanitized_on_upsert`
Expected: FAIL — stored HTML still contains `<script`/`onerror`/`onclick` (no sanitization yet).

- [ ] **Step 3: Add the ammonia dependency**

In `src-tauri/Cargo.toml`, under `[dependencies]`, add:

```toml
ammonia = "4"
```

- [ ] **Step 4: Add the sanitize helper**

In `src-tauri/src/db.rs`, add a module-level function (near `now_string`):

```rust
fn sanitize_html(value: &str) -> String {
    ammonia::clean(value)
}
```

- [ ] **Step 5: Sanitize `content_html` in `upsert_articles`**

In `upsert_articles`, inside the `for article in articles` loop, before the `transaction.execute(INSERT ...)` call, add:

```rust
            let content_html = article.content_html.as_deref().map(sanitize_html);
```

Then in that INSERT's `params![...]`, replace `article.content_html,` with `content_html,`.

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml article_html_is_sanitized_on_upsert`
Expected: PASS.

- [ ] **Step 7: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS (12 tests).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/db.rs
git commit -m "feat: sanitize feed HTML at ingest with ammonia"
```

---

### Task 2: Replace hand-rolled sanitizer with DOMPurify (frontend)

**Files:**
- Modify: `package.json` / `package-lock.json` (add `dompurify`)
- Modify: `src/App.tsx` (import, hook, replace `sanitizeArticleHtml`, remove `isAllowedArticleUrl`)

- [ ] **Step 1: Add the dependency**

Run: `npm install dompurify`
(DOMPurify v3 bundles its own TypeScript types — no `@types/dompurify` needed.)

- [ ] **Step 2: Import DOMPurify and register a link hook**

In `src/App.tsx`, add the import near the top (after the existing imports):

```tsx
import DOMPurify from "dompurify";
```

Add this once at module scope (e.g., directly above the `sanitizeArticleHtml` function), to preserve the previous external-link behavior:

```tsx
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noreferrer");
  }
});
```

- [ ] **Step 3: Replace the `sanitizeArticleHtml` body**

Replace the entire hand-rolled `sanitizeArticleHtml` function with:

```tsx
function sanitizeArticleHtml(value: string): string {
  return DOMPurify.sanitize(value, { USE_PROFILES: { html: true } });
}
```

- [ ] **Step 4: Remove the now-unused helper**

Delete the `isAllowedArticleUrl` function (it was only used by the old hand-rolled sanitizer). Leave `stripHtml` (still used for summary/fallback text).

- [ ] **Step 5: Verify build**

Run: `npm run build`
Expected: PASS — tsc clean (only pre-existing `FormEvent is deprecated` warnings), vite builds. No "unused `isAllowedArticleUrl`" error (it was removed).

- [ ] **Step 6: Commit**

```bash
git add package.json package-lock.json src/App.tsx
git commit -m "feat: render article HTML through DOMPurify instead of hand-rolled sanitizer"
```

---

### Task 3: DESIGN.md note + full verification

**Files:**
- Modify: `DESIGN.md`

- [ ] **Step 1: Record the sanitization approach**

In `DESIGN.md`, under "Implementation constraints", add:

```markdown
- HTML safety: untrusted feed `content_html` is sanitized at the Rust ingest boundary with `ammonia` and again at render time with `DOMPurify` (defense-in-depth); the reader never renders raw feed HTML.
```

- [ ] **Step 2: Commit**

```bash
git add DESIGN.md
git commit -m "docs: record two-layer article HTML sanitization"
```

- [ ] **Step 3: Backend tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS (12 tests including the new sanitization test).

- [ ] **Step 4: Frontend build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 5: Manual smoke (test mode)**

`npm run dev`, open the URL. Open Quick Look / immersive on an article. Confirm:
- A normal HTML body renders with links/lists/images/code intact.
- Links open in a new tab (`target=_blank`, `rel=noreferrer`).
- A crafted body like `<img src=x onerror=alert(1)>` or `<script>alert(1)</script>` does NOT execute (no alert) and is stripped.

(Test-mode articles use `contentText`, so to exercise HTML you can temporarily set a `contentHtml` on a test article, or rely on the Rust test + DOMPurify's vetted behavior.)

---

## Self-review notes

- **Coverage:** ingest sanitization (T1), render sanitization replacing the hand-rolled blocklist (T2), docs + verification (T3). Both layers of the chosen "B-primary + A-fallback" approach are implemented.
- **Placeholder scan:** none — all steps contain concrete code/commands.
- **Type/name consistency:** `sanitize_html(&str) -> String` (Rust) and `sanitizeArticleHtml(value: string): string` (TS) keep their existing call sites; `ReaderArticle`'s `useMemo` + `dangerouslySetInnerHTML` are unchanged and now receive DOMPurify output. `isAllowedArticleUrl` is removed and has no remaining references (it was only called by the old sanitizer).
- **Edge:** existing pre-sanitization rows in SQLite are covered by the render-time DOMPurify pass until they are re-fetched (then ammonia cleans them at ingest).
