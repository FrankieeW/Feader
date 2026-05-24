# Feader Plugin System

Feader should treat RSS as the best path, not the only path. Many useful sources do not expose RSS, expose broken feeds, or require site-specific extraction. The plugin system exists to bring those sources into the same article pipeline without hardcoding every website into the app.

## Implementation Status

Feader now supports the first plugin layer: **static XPath rule packs**. These are data-only packs that contribute:

- XPath selector candidates.
- AI prompt rules for page families.
- Page-type detection markers.

The built-in packs are bundled in Feader for bootstrapping, but the schema is designed to move provider-specific packs into **FeaderHub**, the official registry repository. Static packs are intentionally not executable. More powerful script/runtime plugins remain a later layer with stricter permissions and signing.

Current bundled packs:

- `official.discuz.xpath` — Discuz-style forum thread lists.
- `official.maccms.xpath` — MacCMS video list and detail pages.
- `official.generic-html.xpath` — generic fallback article listings.

The Tauri command `list_xpath_plugin_packs` exposes bundled static packs to the renderer. During AI XPath suggestion, Feader uses matching pack prompt rules and then scores all matching selector candidates with the existing preview diagnostics.

## Registry Direction

The official registry should live outside Feader Core in `FrankieeW/FeaderHub`. FeaderHub should publish:

- Registry index files.
- Plugin manifests.
- Static XPath rule pack JSON files.
- Checksums and future signatures.
- Compatibility metadata.

Users should eventually be able to add third-party registries or import local packs. Feader Core remains responsible for checksum verification, signature verification, permission display, and refusing silent permission expansion on update.

## Goals

- Prefer native RSS or Atom when a source supports it well.
- Support simple websites with declarative extraction rules.
- Support complex websites with script-based plugins.
- Let AI help users author and repair extraction rules.
- Keep every source type normalized into one article contract.
- Keep user data portable and inspectable.

## Source Adapter Layers

### 1. Native Feed Adapter

Use for standard RSS, Atom, JSON Feed, and OPML workflows.

Implementation status: RSS and Atom parsing are live, with source management and refresh error tracking backed by SQLite. JSON Feed and OPML remain follow-up work.

Responsibilities:

- Fetch feed documents.
- Parse feed metadata and entries.
- Normalize entries into Feader articles.
- Preserve canonical URLs, GUIDs, authors, dates, and content.

### 2. Declarative XPath Adapter

Use for simple HTML or XML pages where the article list and fields can be extracted from static markup.

The user or AI defines XPath expressions for fields such as:

- Article container
- Title
- URL
- Summary
- Published date
- Author
- Content
- Image
- Next page

The rule should be stored as data, not code. This makes it inspectable, exportable, syncable, and safer than arbitrary scripts.

Implementation status: static, well-formed HTML/XML pages can be previewed, saved, refreshed, and normalized into Feader articles. Preview includes field-level diagnostics for required and optional selectors, extracted article samples, and next-page URL preview. JavaScript-rendered pages, authentication, full pagination traversal, draft rule storage, and AI-assisted selector generation remain follow-up work.

Example shape:

```json
{
  "kind": "xpath",
  "source": {
    "name": "Example Blog",
    "url": "https://example.com/articles"
  },
  "selectors": {
    "items": "//article",
    "title": ".//h2/a/text()",
    "url": ".//h2/a/@href",
    "summary": ".//p[contains(@class, 'summary')]/text()",
    "publishedAt": ".//time/@datetime",
    "author": ".//*[contains(@class, 'author')]/text()",
    "nextPage": "//a[@rel='next']/@href"
  }
}
```

### 3. Script Plugin Adapter

Use for complex websites where declarative XPath is not enough.

Typical cases:

- Multiple request steps
- Pagination that is not visible in plain HTML
- Site-specific date or URL cleanup
- Authenticated pages
- API-backed pages
- Web3/community sources with custom protocols
- Resilient fallback logic when markup changes

Scripts should return the same normalized article shape as native feeds and XPath rules. The runtime should expose a narrow host API for fetching, logging, storage, and article emission rather than giving scripts unrestricted app access.

## AI-Assisted XPath Authoring

AI should help with rule creation, but the generated rule should remain explicit and editable.

Target workflow:

1. User enters a website URL.
2. Feader fetches representative HTML or XML.
3. AI proposes XPath selectors for article fields.
4. Feader previews extracted articles.
5. User confirms, edits, or asks AI to repair selectors.
6. The final rule is saved as a declarative source adapter.

AI can also repair broken rules by comparing the old selector output with current page markup and proposing a minimal selector update.

## Normalized Article Contract

All adapters should emit the same shape:

```ts
type NormalizedArticle = {
  sourceId: string;
  externalId?: string;
  title: string;
  url: string;
  canonicalUrl?: string;
  summary?: string;
  contentHtml?: string;
  contentText?: string;
  author?: string;
  publishedAt?: string;
  imageUrl?: string;
  tags?: string[];
  raw?: unknown;
};
```

## Safety Model

Declarative XPath rules are preferred because they are easier to inspect, sync, and sandbox. Script plugins are more powerful and should have stricter boundaries.

Initial constraints:

- XPath rules cannot execute code.
- Script plugins run through a limited host API.
- Network permissions should be source-scoped.
- Secrets should never be embedded directly in plugin source.
- AI-generated rules must be previewed before activation.

## Product Principle

Feader should make simple sources easy and complex sources possible. The user should not need to become a scraper engineer for common pages, but the system should still have an escape hatch when websites are messy.
