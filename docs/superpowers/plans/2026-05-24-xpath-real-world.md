# XPath Real-World Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make XPath sources work on real (non-well-formed) web pages, capture rich `content_html`, follow pagination on refresh, and offer selector presets.

**Architecture:** Keep the `sxd-xpath` engine. Add a pure-Rust `normalize_html` (html5ever parse → xml5ever serialize → strip namespaces) ahead of every `sxd` parse so messy HTML becomes matchable XHTML. Add node→inner-HTML serialization for `content_html`, a bounded next-page fetch loop on refresh, and frontend selector presets/hints.

**Tech Stack:** Rust + `sxd-document`/`sxd-xpath` + new `html5ever`/`markup5ever_rcdom`/`xml5ever`; React + TypeScript. Backend uses `cargo test`; XPath fetching is only exercisable via `npm run tauri dev` (browser test mode is read-only for XPath).

---

## Spec reference

`docs/superpowers/specs/2026-05-24-xpath-real-world-design.md`

## File map

- `src-tauri/Cargo.toml` — add `html5ever`, `markup5ever_rcdom`, `xml5ever`.
- `src-tauri/src/xpath_adapter.rs` — `normalize_html`, `node_inner_html`/content_html, `fetch_page` + pagination loop, tests.
- `src/App.tsx` — XPath preset dropdown + per-field hints in `XPathSourceForm`.
- `src/App.css` — minor styling for the preset row/hints (optional, reuse existing classes).
- `DESIGN.md` — note real-world HTML support, content_html, pagination.

## Notes

- The current GET logic is duplicated in `fetch_xpath_source` and `preview_xpath_source`; Task 3 extracts a shared `fetch_page` helper (a reasonable improvement while we're here).
- `parse_xpath_source` and `preview_xpath_document` keep their signatures (they receive an already-normalized document string).

---

### Task 1: Normalize real-world HTML before XPath parsing

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/xpath_adapter.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/xpath_adapter.rs`:

```rust
    #[test]
    fn normalizes_malformed_html_for_extraction() {
        let messy = r#"<article><h2><a href="/one">First</a></h2><p>Summary one<br>more</article>"#;
        let feed = parse_xpath_source(
            "https://example.com/blog/",
            &normalize_html(messy),
            &selectors(),
        )
        .expect("xpath extracts from normalized html");

        assert_eq!(feed.articles.len(), 1);
        assert_eq!(feed.articles[0].title, "First");
        assert_eq!(feed.articles[0].url, "https://example.com/one");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml normalizes_malformed_html_for_extraction`
Expected: FAIL to compile — `normalize_html` is not defined.

- [ ] **Step 3: Add the parser crates**

In `src-tauri/Cargo.toml` `[dependencies]`, add (use these versions as a starting point; if cargo reports a `markup5ever` version conflict between the three, align them to a common `markup5ever` by adjusting versions until `cargo tree -p markup5ever` shows a single version):

```toml
html5ever = "0.29"
markup5ever_rcdom = "0.5"
xml5ever = "0.22"
```

- [ ] **Step 4: Implement `normalize_html`**

In `src-tauri/src/xpath_adapter.rs`, add near the other free functions:

```rust
fn normalize_html(raw: &str) -> String {
    use html5ever::tendril::TendrilSink;

    let dom = html5ever::parse_document(markup5ever_rcdom::RcDom::default(), Default::default())
        .one(raw);
    let handle: markup5ever_rcdom::SerializableHandle = dom.document.clone().into();

    let mut buffer = Vec::new();
    if xml5ever::serialize::serialize(
        &mut buffer,
        &handle,
        xml5ever::serialize::SerializeOpts::default(),
    )
    .is_err()
    {
        return raw.to_string();
    }

    let xml = String::from_utf8(buffer).unwrap_or_else(|_| raw.to_string());
    // sxd-xpath unprefixed name tests only match the no-namespace; html5ever puts
    // elements in the XHTML (and SVG/MathML) namespaces, so drop those declarations.
    xml.replace(" xmlns=\"http://www.w3.org/1999/xhtml\"", "")
        .replace(" xmlns=\"http://www.w3.org/2000/svg\"", "")
        .replace(" xmlns=\"http://www.w3.org/1998/Math/MathML\"", "")
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml normalizes_malformed_html_for_extraction`
Expected: PASS. (If it fails because the namespace is still present, inspect `normalize_html(messy)` output and extend the `replace` calls to cover the emitted `xmlns` literal.)

- [ ] **Step 6: Wire normalization into the fetching paths**

In `src-tauri/src/xpath_adapter.rs`, in `fetch_xpath_source`, change:

```rust
    let body = response.text().await.map_err(|error| error.to_string())?;
    parse_xpath_source(url, &body, selectors)
```

to:

```rust
    let body = response.text().await.map_err(|error| error.to_string())?;
    parse_xpath_source(url, &normalize_html(&body), selectors)
```

In `preview_xpath_source`, change:

```rust
    let body = response.text().await.map_err(|error| error.to_string())?;
    preview_xpath_document(url, &body, selectors)
```

to:

```rust
    let body = response.text().await.map_err(|error| error.to_string())?;
    preview_xpath_document(url, &normalize_html(&body), selectors)
```

- [ ] **Step 7: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS (existing XPath tests still green — they pass well-formed XHTML directly to `parse_xpath_source`, which is unaffected).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/xpath_adapter.rs
git commit -m "feat: normalize real-world HTML before XPath extraction"
```

---

### Task 2: Capture content_html from the content selector

**Files:**
- Modify: `src-tauri/src/xpath_adapter.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/xpath_adapter.rs`:

```rust
    #[test]
    fn content_selector_captures_inner_html() {
        let mut selectors = selectors();
        selectors.content = Some(".//section".to_string());

        let feed = parse_xpath_source(
            "https://example.com/blog/",
            r#"
            <html><body>
              <article>
                <h2><a href="/one">First</a></h2>
                <section><strong>Bold</strong> and <em>italic</em></section>
              </article>
            </body></html>
            "#,
            &selectors,
        )
        .expect("xpath extracts");

        let html = feed.articles[0].content_html.as_deref().unwrap_or_default();
        assert!(html.contains("<strong>"), "expected inner tags, got: {html}");
        assert!(html.contains("Bold"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml content_selector_captures_inner_html`
Expected: FAIL — `content_html` is `None` (the content selector currently only fills `content_text`).

- [ ] **Step 3: Add node→inner-HTML serialization**

In `src-tauri/src/xpath_adapter.rs`, add these imports at the top (extend the existing `use sxd_*` lines):

```rust
use sxd_document::dom::{ChildOfElement, Element};
```

Add these functions near the other helpers:

```rust
fn node_inner_html(element: Element<'_>) -> String {
    let mut out = String::new();
    for child in element.children() {
        serialize_child(child, &mut out);
    }
    out
}

fn serialize_child(child: ChildOfElement<'_>, out: &mut String) {
    match child {
        ChildOfElement::Element(element) => {
            let name = element.name().local_part();
            out.push('<');
            out.push_str(name);
            for attribute in element.attributes() {
                out.push(' ');
                out.push_str(attribute.name().local_part());
                out.push_str("=\"");
                out.push_str(&escape_html(attribute.value(), true));
                out.push('"');
            }
            out.push('>');
            for grandchild in element.children() {
                serialize_child(grandchild, out);
            }
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        ChildOfElement::Text(text) => out.push_str(&escape_html(text.text(), false)),
        _ => {}
    }
}

fn escape_html(value: &str, in_attribute: bool) -> String {
    let mut escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    if in_attribute {
        escaped = escaped.replace('"', "&quot;");
    }
    escaped
}

fn evaluate_content_html(item: Node<'_>, expression: Option<&str>) -> Result<Option<String>, String> {
    let Some(expression) = expression.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let xpath = compile_xpath(expression)?;
    let value = xpath
        .evaluate(&Context::new(), item)
        .map_err(|error| error.to_string())?;
    if let Value::Nodeset(nodeset) = value {
        if let Some(Node::Element(element)) = nodeset.document_order().into_iter().next() {
            let html = node_inner_html(element);
            return Ok((!html.trim().is_empty()).then_some(html));
        }
    }
    Ok(None)
}
```

- [ ] **Step 4: Use it in `parse_xpath_source`**

In `parse_xpath_source`, inside the `for item in items` loop, replace the single `content_text` line in the `ParsedArticle { ... }` literal. Currently it builds:

```rust
            content_html: None,
            content_text: evaluate_optional_string(item, selectors.content.as_deref())?,
```

Change to:

```rust
            content_html: evaluate_content_html(item, selectors.content.as_deref())?,
            content_text: match evaluate_content_html(item, selectors.content.as_deref())? {
                Some(_) => None,
                None => evaluate_optional_string(item, selectors.content.as_deref())?,
            },
```

(When the content selector points at an element, capture its inner HTML; otherwise fall back to text, preserving today's behavior. `content_html` is sanitized by `ammonia` at ingest in `db.rs::upsert_articles`.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml content_selector_captures_inner_html`
Expected: PASS.

- [ ] **Step 6: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/xpath_adapter.rs
git commit -m "feat: capture content_html from XPath content selector"
```

---

### Task 3: Follow pagination on refresh

**Files:**
- Modify: `src-tauri/src/xpath_adapter.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module (verifies next-page URL resolution at parse level; the multi-page network loop is verified manually in `tauri dev`):

```rust
    #[test]
    fn resolves_absolute_next_page_url() {
        let mut selectors = selectors();
        selectors.next_page = Some("//a[@rel='next']/@href".to_string());

        let next = next_page_url(
            "https://example.com/blog/",
            r#"<html><body><a rel="next" href="/page/2">Next</a></body></html>"#,
            &selectors,
        )
        .expect("next page resolves");

        assert_eq!(next.as_deref(), Some("https://example.com/page/2"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml resolves_absolute_next_page_url`
Expected: FAIL to compile — `next_page_url` is not defined.

- [ ] **Step 3: Add the page cap, fetch helper, and next-page resolver**

In `src-tauri/src/xpath_adapter.rs`, add near the top of the file (after imports):

```rust
const MAX_XPATH_PAGES: usize = 5;
```

Add these functions near the other helpers:

```rust
async fn fetch_page(url: &str) -> Result<String, String> {
    let response = reqwest::Client::new()
        .get(url)
        .header("user-agent", "Feader/0.1")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("XPath source request failed with status {status}"));
    }
    response.text().await.map_err(|error| error.to_string())
}

fn next_page_url(
    base_url: &str,
    document: &str,
    selectors: &XPathSelectors,
) -> Result<Option<String>, String> {
    let package = parser::parse(document).map_err(|error| {
        format!("XPath adapter currently expects well-formed static HTML/XML: {error}")
    })?;
    let document = package.as_document();
    let raw = preview_optional_string(
        Node::Root(document.root()),
        selectors.next_page.as_deref(),
    );
    raw.map(|value| absolutize_url(base_url, &value)).transpose()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml resolves_absolute_next_page_url`
Expected: PASS.

- [ ] **Step 5: Rewrite `fetch_xpath_source` as a bounded loop**

Replace the body of `fetch_xpath_source` with:

```rust
pub async fn fetch_xpath_source(
    url: &str,
    selectors: &XPathSelectors,
) -> Result<ParsedFeed, String> {
    let mut visited = std::collections::HashSet::new();
    let mut current = url.to_string();
    let mut articles = Vec::new();

    for _ in 0..MAX_XPATH_PAGES {
        if !visited.insert(current.clone()) {
            break;
        }

        let body = fetch_page(&current).await?;
        let normalized = normalize_html(&body);
        let feed = parse_xpath_source(&current, &normalized, selectors)?;
        articles.extend(feed.articles);

        match next_page_url(&current, &normalized, selectors)? {
            Some(next) if !visited.contains(&next) => current = next,
            _ => break,
        }
    }

    Ok(ParsedFeed {
        title: None,
        articles,
    })
}
```

Also update `preview_xpath_source` to reuse `fetch_page` (replace its inline GET block):

```rust
pub async fn preview_xpath_source(
    url: &str,
    selectors: &XPathSelectors,
) -> Result<XPathPreview, String> {
    let body = fetch_page(url).await?;
    preview_xpath_document(url, &normalize_html(&body), selectors)
}
```

- [ ] **Step 6: Run the full suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS. (Overlapping pages are de-duplicated at storage by the existing `ON CONFLICT(source_id, url)` upsert; a failed next-page fetch returns the `?` error, but page-1 success with a later failure returns gathered articles only if the failure is on the next fetch — note `fetch_page` errors propagate via `?`, stopping refresh; acceptable since page 1 is the primary content.)

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/xpath_adapter.rs
git commit -m "feat: follow XPath pagination up to a page cap on refresh"
```

---

### Task 4: Frontend selector presets + hints

**Files:**
- Modify: `src/App.tsx` (`XPathSourceForm`, presets const)

- [ ] **Step 1: Add a presets constant**

In `src/App.tsx`, near `defaultXPathSelectors`, add:

```tsx
const xpathPresets: Record<string, XPathSelectors> = {
  "Generic blog": {
    items: "//article",
    title: ".//h2/a | .//h3/a",
    url: ".//h2/a/@href | .//h3/a/@href",
    summary: ".//p",
    publishedAt: ".//time/@datetime",
    author: "",
    content: ".//section",
    image: ".//img/@src",
    nextPage: "//a[@rel='next']/@href",
  },
  "Listing + links": {
    items: "//li[.//a]",
    title: ".//a",
    url: ".//a/@href",
    summary: "",
    publishedAt: "",
    author: "",
    content: "",
    image: ".//img/@src",
    nextPage: "",
  },
};
```

- [ ] **Step 2: Add a preset dropdown to `XPathSourceForm`**

In `XPathSourceForm`, immediately after the opening `<section className="xpath-form">` and before the title `<input>`, add:

```tsx
      <label className="selector-input">
        <span>Preset</span>
        <select
          aria-label="Selector preset"
          disabled={isBusy}
          onChange={(event) => {
            const preset = xpathPresets[event.currentTarget.value];
            if (preset) {
              onSelectorsChange(preset);
            }
          }}
          value=""
        >
          <option value="">Choose a preset…</option>
          {Object.keys(xpathPresets).map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </label>
```

(`XPathSourceForm` already receives `isBusy`, `onSelectorsChange`; no new props needed.)

- [ ] **Step 3: Add per-field hints**

In the `SelectorInput` component, add an optional `hint` prop and render it. Change the component signature/body to:

```tsx
function SelectorInput({
  disabled,
  label,
  name,
  hint,
  onChange,
  selectors,
}: {
  disabled: boolean;
  label: string;
  name: keyof XPathSelectors;
  hint?: string;
  onChange: (selectors: XPathSelectors) => void;
  selectors: XPathSelectors;
}) {
  return (
    <label className="selector-input">
      <span>{label}</span>
      <input
        disabled={disabled}
        onChange={(event) =>
          onChange({
            ...selectors,
            [name]: event.currentTarget.value,
          })
        }
        value={selectors[name] ?? ""}
      />
      {hint ? <small className="selector-hint">{hint}</small> : null}
    </label>
  );
}
```

Then pass hints on the three required fields in `XPathSourceForm` (add the `hint` prop to those `<SelectorInput>` usages):

- Items: `hint="Repeating element per article, e.g. //article"`
- Title: `hint="Text or link inside an item, e.g. .//h2/a"`
- URL: `hint="Link href inside an item, e.g. .//h2/a/@href"`

- [ ] **Step 4: Add minimal hint styling**

In `src/App.css`, add:

```css
.selector-hint {
  color: var(--color-faint);
  font-size: 10px;
  line-height: 1.35;
}

.selector-input select {
  min-height: 38px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 0 10px;
  color: var(--color-text);
  background: var(--color-panel-strong);
}
```

- [ ] **Step 5: Verify build**

Run: `npm run build`
Expected: PASS (only pre-existing `FormEvent is deprecated` warnings).

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: add XPath selector presets and field hints"
```

---

### Task 5: DESIGN.md + full verification

**Files:**
- Modify: `DESIGN.md`

- [ ] **Step 1: Update DESIGN.md**

In `DESIGN.md`, under "Implementation constraints", add:

```markdown
- XPath sources: real-world HTML is normalized (html5ever -> XHTML) before sxd-xpath extraction; the content selector can capture sanitized `content_html`; refresh follows `nextPage` up to a fixed page cap.
```

- [ ] **Step 2: Commit**

```bash
git add DESIGN.md
git commit -m "docs: record XPath real-world parsing, content_html, and pagination"
```

- [ ] **Step 3: Backend tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all PASS (including the three new tests).

- [ ] **Step 4: Frontend build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 5: Manual smoke (real backend)**

Run: `npm run tauri dev`. In the Sources view, add an XPath source against a real article-listing page (e.g., a blog index). Verify:
- Preview diagnostics resolve (items/title/url go green) on a real, non-XHTML page.
- Articles extract; opening one shows rich HTML (lists/links/images) in Quick Look / immersive, with scripts stripped.
- A source whose page has a "next" link pulls more than one page of articles on refresh (capped at 5).
- A preset fills the selector fields and then previews successfully.

---

## Self-review notes

- **Spec coverage:** HTML normalization (T1), content_html (T2), pagination on refresh (T3), presets + hints (T4), DESIGN.md + verification (T5). All four spec parts mapped.
- **Type/name consistency:** `normalize_html(&str)->String`, `node_inner_html(Element)->String`, `evaluate_content_html(Node, Option<&str>)->Result<Option<String>,String>`, `fetch_page(&str)`, `next_page_url(&str,&str,&XPathSelectors)`, `MAX_XPATH_PAGES`, `xpathPresets` — each defined once and referenced consistently. `parse_xpath_source`/`preview_xpath_document` signatures unchanged.
- **Risk:** the one uncertainty is the `html5ever`/`xml5ever`/`markup5ever_rcdom` version trio resolving to a single `markup5ever`; Step 1.3 calls this out with the `cargo tree` check, and the TDD test in Step 1.5 proves the full normalize→parse→extract pipeline works before moving on.
- **Placeholder scan:** none — every code step contains the actual code.
