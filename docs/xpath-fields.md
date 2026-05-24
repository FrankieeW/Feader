# XPath Source Fields

XPath sources extract articles from Feader's normalized static HTML DOM. Selectors should be written for that normalized DOM, not for a live browser DOM after JavaScript has changed the page.

## Core Selectors

| Field | Scope | Purpose |
| --- | --- | --- |
| `items` | document | Required repeating node for each article/list item. |
| `title` | item | Required article title. |
| `url` | item | Required article URL or relative link. |
| `summary` | item | Optional short summary or listing metadata. |
| `publishedAt` | item | Optional date/time string. |
| `author` | item | Optional author name. |
| `content` | item | Optional body extracted from the list item itself. |
| `detailContent` | detail document | Optional body selector evaluated after fetching the article URL. |
| `image` | item | Optional image URL. |
| `nextPage` | document | Optional next page URL for pagination. |
| `cookie` | request | Optional Cookie header. Accepts `name=value; ...`, a JSON object, `$ENV_NAME`, or `${ENV_NAME}`. |
| `maxItems` | refresh | Optional positive integer limiting articles fetched per refresh. |
| `plugin` | metadata | Plugin provenance copied from Hub-created sources. |

## Content Cleanup

`contentCleanup` is an ordered array of regex replacement rules applied after `content` or `detailContent` extraction.

```json
[
  {
    "pattern": "(?is)<aside[^>]*>.*?</aside>",
    "replacement": ""
  }
]
```

Each rule has:

| Field | Required | Purpose |
| --- | --- | --- |
| `pattern` | yes | Rust regex pattern. Use `(?s)` for multi-line HTML blocks and `(?i)` for case-insensitive matches. |
| `replacement` | no | Replacement string. Empty string removes the matched content. |

Use cleanup for repeated ads, quote blocks, tracking snippets, or site boilerplate inside the extracted body. Keep rules narrow so they do not remove user content.

## Custom Fields

`customFields` stores non-standard metadata such as tags, view counts, reply counts, score, rating, section, or duration. These fields are not universal, so plugins and users define their own keys.

```json
[
  {
    "key": "views",
    "label": "Views",
    "xpath": ".//span[contains(@class, 'views')]",
    "scope": "item"
  },
  {
    "key": "tags",
    "label": "Tags",
    "xpath": "//*[@class='post-tags']//a",
    "scope": "detail"
  }
]
```

Each custom field has:

| Field | Required | Purpose |
| --- | --- | --- |
| `key` | yes | Stable machine key stored on the article. Use lowercase identifiers such as `views`, `replies`, `section`, `rating`. |
| `label` | no | Human label shown in the UI. Falls back to `key`. |
| `xpath` | yes | XPath selector. Relative to `items` when `scope` is `item`; document-level on the article detail page when `scope` is `detail`. |
| `scope` | no | `item` by default. Use `detail` when the value only exists on the article page. |

Extracted values are stored in `tagsJson` as:

```json
{
  "views": {
    "label": "Views",
    "value": "123"
  }
}
```

The UI shows custom fields on article cards, the reader view, preview cards, and source configuration summaries.

