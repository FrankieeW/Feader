# Plugin Marketplace UI + Remote Registry + Parameter Dialog

## Overview

Redesign the Hub into a discovery-first plugin marketplace, connect to the GitHub-hosted FeaderHub registry for remote pack fetching, add a naixi.net forum plugin, and introduce a unified parameter dialog for source creation from plugins.

## Part 1 — Marketplace UI (Discovery-first)

The Hub transforms from a static 3-card grid into a browse-able marketplace.

### Components

- **SearchBar** — text input at top, filters packs by name/description/capability as you type. Debounced 200ms.
- **CategoryChips** — horizontal chip row: "All", "XPath Rules", "Forum", "Video", "Article". Derived from pack `kind` and `candidates[].pageType`.
- **StatsBar** — compact row: "N packs available", "M official", "Last synced: ..."
- **PluginCard** — each card shows:
  - Icon/avatar (first letter of name, colored)
  - Name, version badge
  - Description (2-line clamp)
  - Capability pills
  - Trust badge ("official" / "community")
  - "Add Source" button
- **PluginCardGrid** — responsive grid: 3 cols desktop, 2 tablet, 1 mobile
- **FeaturedSection** — first 2-3 "official" packs get larger banner-style cards above the grid

### Data flow

1. On Hub mount, fetch registry index from GitHub
2. Merge with bundled packs, deduplicate by `id`
3. Apply search + category filters client-side
4. Render filtered cards

## Part 2 — Remote Registry Connection

### Registry source

- URL: `https://raw.githubusercontent.com/FrankieeW/FeaderHub/main/registry/index.json`
- Individual manifests: `https://raw.githubusercontent.com/FrankieeW/FeaderHub/main/plugins/{id}/manifest.json`
- Rule packs: `https://raw.githubusercontent.com/FrankieeW/FeaderHub/main/plugins/{id}/xpath-rule-pack.json`

### Rust backend

New Tauri commands:

- `fetch_registry_index() -> RegistryIndex` — fetches and parses the registry index.json
- `fetch_plugin_pack(id: String) -> XPathRulePack` — fetches manifest + rule pack, merges into an `XPathRulePack`

### Caching

- Registry index cached in SQLite with a TTL (24h default)
- Individual packs cached on first fetch
- "Refresh" button in Hub header to force re-fetch
- Graceful fallback to bundled packs when offline

### Trust model

- `trust: "official"` — packs from the `FrankieeW/FeaderHub` repo with valid checksums
- `trust: "community"` — packs from third-party registries (future)
- `trust: "bundled"` — packs compiled into the binary (existing 3)

## Part 3 — naixi.net Forum Plugin

New plugin `official.naixi-forum.xpath` authored in the local FeaderHub clone.

### Structure

```
plugins/official.naixi-forum.xpath/
  manifest.json
  xpath-rule-pack.json
```

### Forum section hierarchy

The plugin metadata includes a `sections` tree for forum navigation:

```
板块
  内容区
    茶馆
      日常 (forum-64-1)
      交易 (forum-64-2)
    技术
      ...
  站务区
    ...
```

### Detection

- Domain: `forum.naixi.net`
- Discuz markers: `threadlisttableid`, `km_subject`, Discuz template structure
- Priority: 90 (high, same as discuz pack)

### Selectors

Based on Discuz XPath patterns:
- Items: `//tbody[contains(@id, 'normalthread_')]`
- Title: `.//a[contains(@class, 'xst')]`
- URL: `.//a[contains(@class, 'xst')]/@href`
- Author: `.//cite/a`
- Published date: `.//em/span/@title` or `.//em/span`
- Next page: `//a[contains(@class, 'nxt')]/@href`

### Registration

Add entry to `registry/index.json` with placeholder sha256.

## Part 4 — Unified Parameter Dialog

A modal dialog shown when clicking "Add Source" on any plugin card in the Hub.

### Dialog layout

```
┌─────────────────────────────────────────────────┐
│  Add Source: Naixi Forum                    [X] │
├─────────────────────────────────────────────────┤
│                                                 │
│  URL                                            │
│  ┌─────────────────────────────────────────┐   │
│  │ https://forum.naixi.net/forum-64-1.html  │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
│  Section (for forum plugins)                    │
│  ┌ 板块 > 内容区 > 茶馆 > 日常 ──────── [▾] ┐  │
│  │                                         │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
│  Max items ────●──────────── 20                 │
│  5                                  100         │
│                                                 │
│  Max pages  ────●──────────── 3                  │
│  1                                   10         │
│                                                 │
│  Source title                                   │
│  ┌─────────────────────────────────────────┐   │
│  │ Naixi Forum · 日常                      │   │
│  └─────────────────────────────────────────┘   │
│                                                 │
│  [Preview: 20 articles found]                   │
│                                                 │
│  [Cancel]                    [Add Source]       │
└─────────────────────────────────────────────────┘
```

### Key behaviors

- **URL** — pre-filled from plugin's section URL if one is selected, editable
- **Section selector** — dropdown tree showing plugin's section hierarchy; selecting a section updates the URL
- **Max items slider** — range 5-100, default 20, step 5
- **Max pages slider** — range 1-10, default 3, step 1 (how many nextPage cycles)
- **Source title** — auto-generated from plugin name + section, editable
- **Preview** — inline button that calls `preview_xpath_source` and shows article count
- **Add Source** — calls `add_xpath_source` with the configured selectors and URL, then closes dialog and navigates to the new source

### Plugin schema extension

Add optional `parameters` field to `xpath-rule-pack.json` schema:

```json
{
  "parameters": {
    "urlTemplate": "https://forum.naixi.net/{sectionId}.html",
    "sections": [
      {
        "id": "forum-64-1",
        "path": ["板块", "内容区", "茶馆", "日常"],
        "url": "https://forum.naixi.net/forum-64-1.html"
      }
    ],
    "defaults": {
      "maxItems": 20,
      "maxPages": 3
    }
  }
}
```

## Scope boundaries

### In scope
- Hub marketplace UI (search, categories, cards, featured section)
- GitHub-based registry fetching with local caching
- naixi.net forum plugin in FeaderHub
- Unified parameter dialog for plugin→source flow
- Plugin schema extension for parameters/sections

### Out of scope
- Third-party registries
- Checksum/signature verification (schema has placeholders, not yet enforced)
- Plugin enable/disable or uninstall
- Plugin update checking
- Advanced source plugins with sandboxed execution

## Implementation phases

1. **FeaderHub** — add naixi.net plugin + extend schema with parameters
2. **Rust backend** — registry fetch commands + caching
3. **Frontend Hub** — marketplace UI redesign
4. **Frontend Dialog** — unified parameter dialog component
