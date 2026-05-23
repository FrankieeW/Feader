# Folo-inspired Hybrid UI Redesign — Design

- Date: 2026-05-23
- Status: Approved (pending spec review)
- Scope: Feader desktop reader UI (React + plain CSS) plus a contained Rust/SQLite addition for feed categories.

## Goal

Adopt Folo's information architecture and a softer, premium visual language while keeping Feader's local-first, technical identity and copper/warm-editorial palette. Approved direction: **Hybrid (B)** — borrow Folo's structure and interactions, restyle the skin, update `DESIGN.md` to match.

## Approved direction (from brainstorm)

- Four-zone layout: **icon rail · grouped feed sidebar · entry list · reader**.
- **Light, refined icon rail** with a thin copper active indicator (no dark block).
- **User-assignable feed folders** (categories) with collapsible groups — requires backend work.
- **Entry list as clean rows** with hairline separators and a soft selected fill, plus a **Card** layout mode using `imageUrl` thumbnails.
- Copper accent used sparingly (unread dots, rail indicator, one primary button); everything else neutral tone + whitespace.
- Medium radius (~12–14px), generous spacing, hairline borders, subtle shadows.

## Out of scope (YAGNI)

- Folo media-type views (Pictures/Videos/Audio), masonry/video grids.
- AI side panel, command palette, global search.
- Drag-and-drop feed reordering; nested folders.
- Any change to RSS/XPath fetching, article schema, or read/saved semantics.

## Architecture

### Layout shell (`App.tsx` + `App.css`)

Replace the current `grid-template-columns` shell with a fixed icon rail plus three resizable/flex zones:

```
[ rail 54px ] [ feeds sidebar ] | resizer | [ entry list ] | resizer | [ reader ]
```

- The rail is fixed width and always visible. It owns view switching (`ViewMode`) and quick theme toggle + Settings entry — replacing today's in-sidebar `workspace-nav`.
- `reader` / `sources` / `settings` views still render in the right zones; for `sources` and `settings` the feeds sidebar is hidden and the page view spans the remaining columns (as today).
- Existing pane-width persistence (`feader.paneWidths`) and resizers are retained; the rail is outside the resizable grid.

### New/changed components

- **`IconRail`** — vertical rail: brand mark, view icons (Reader, Sources), spacer, a quick theme toggle (cycles light → dark → system, reusing `themeMode` state), and Settings. Icons are minimal hand-authored inline SVGs (no npm dependency). Active view shows copper soft-fill + left indicator. The full three-way `ThemeControl` remains in Settings. Buttons remain native `<button>` with `aria-label` and `aria-current`.
- **`FeedSidebar`** — "Library" header + refresh-all affordance, an "All feeds" row, then **collapsible category groups**. Groups derived from distinct `source.category` values; sources with empty category fall under "Uncategorized". Group headers toggle collapse; collapse state persisted per category in `localStorage` (`feader.feedGroups`). Feed rows: unread dot (copper when unread, hollow when all read / error uses danger), title, unread count; hover/selected use soft fill. Error sources keep a danger dot.
- **`EntryList`** (refactor of the current `.timeline` story list) — top bar with source title + a **layout-mode segmented control: List | Card**. List = clean rows (hairline separators, 2-line excerpt, unread dot). Card = same data with a leading thumbnail from `imageUrl` (fallback neutral block). Selected = soft fill, read = muted. Keyboard arrow navigation and r/s shortcuts are preserved.
- **Reader panel** — keep structure; widen margins, refine heading weight/size, keep sticky action toolbar and source/diagnostic panel. `ReaderTypography` (system/serif/large) is preserved.
- **Source category picker** — in the reader-side `source-panel` and in the Sources manager card: a small control to set/clear a source's category (free-text with datalist of existing categories). Calls the new command and reloads.

### State changes (`App.tsx`)

- Replace `ArticleDensity` (`comfortable | compact`) with `EntryLayout` (`list | card`). Migrate the persisted key: read old `feader.articleDensity`, map to `list`, store under `feader.entryLayout`. Remove `DensityControl`; add `EntryLayoutControl`.
- Add `feedGroupCollapse: Record<string, boolean>` state persisted to `feader.feedGroups`.
- `Source` type gains `category?: string | null`.

## Backend changes (Rust / SQLite)

Contained, uses the existing `add_column_if_missing` migration helper.

1. **Schema** (`db.rs` `initialize_schema`): add `category TEXT` to `sources` (via `add_column_if_missing` so existing DBs migrate). Default `NULL`.
2. **`list_sources` SELECT** + row mapping: include `sources.category`.
3. **`Source` struct** (`models.rs`): add `pub category: Option<String>` (serde `camelCase` → `category`).
4. **New command** `set_source_category(source_id: i64, category: Option<String>)`:
   - Method on `AppDatabase`: `UPDATE sources SET category = ?, updated_at = ? WHERE id = ?`; empty/whitespace string normalizes to `NULL`. Returns the updated `Source`.
   - `#[tauri::command]` wrapper in `lib.rs`, registered in `generate_handler!`.
5. **Test-mode parity** (`App.tsx` `testModeInvoke`): implement `set_source_category`; add `category` to `testModeSources`; group rendering works in browser preview.

## Data flow

`set category` → invoke `set_source_category` → reload `list_sources` → `FeedSidebar` regroups by `category`. Collapse state is purely client-side. No change to article fetch/refresh flow.

## Visual tokens (`App.css`)

Evolve the existing custom properties (already copper/graphite after the prior fix):

- Increase panel radius from `8px` to `12–14px`; introduce hairline border tokens (`--line`, `--line-soft`) at low alpha; add a soft-fill token (`--color-fill`) for hover/selected.
- Reduce reliance on boxed cards in the entry list — use hairline separators + selected fill instead of per-item borders/shadows.
- Keep copper `--color-action` / `--color-brand`; ensure it appears only as dots, rail indicator, primary button, focus ring.
- Light and dark themes both updated to keep the refined low-contrast chrome.

## DESIGN.md updates

- **Brand / Visual language**: change "low-radius panels" → "medium-radius (12–14px) panels"; allow a "soft modern skin" while still avoiding glassmorphism-heavy/crypto aesthetics.
- **Information architecture**: document the icon rail + collapsible category folders.
- **Components**: add IconRail, FeedSidebar groups, EntryLayout (List/Card), category picker.
- **Open questions**: close the Lucide question by recording that a minimal hand-authored inline SVG icon set was adopted (no dependency).
- Refresh "Last refreshed" date and Evidence list.

## Error handling

- `set_source_category` failures surface through the existing `runTask` status line; no partial UI state (reload on success only).
- Backend normalizes empty category to `NULL`; no new error paths in fetch/refresh.
- Test mode mirrors the command so browser preview never calls a missing handler.

## Accessibility

- Rail buttons: `aria-label` + `aria-current="page"` for active view.
- Group headers: real `<button>` with `aria-expanded` controlling the group region.
- Layout-mode control: `role="group"` with `aria-pressed`/`aria-selected` on options (consistent with the just-fixed tabs).
- Maintain ≥34px control targets, focus-visible rings, and `prefers-reduced-motion` handling.

## Testing

- **Rust unit test** (`db.rs`): set category persists and clears (empty → NULL), and `list_sources` returns it.
- **Build check**: `npm run build` (tsc + vite) and `cargo test` in `src-tauri`.
- **Manual UI verification**: browser preview (test mode) for rail switching, group collapse, List/Card toggle, category assignment, light/dark; confirm copper restraint and spacing.

## Implementation order (high level)

1. Backend: schema column + struct + `list_sources` + `set_source_category` + Rust test.
2. Frontend types/state + test-mode parity.
3. Layout shell + IconRail.
4. FeedSidebar grouping + collapse + category picker.
5. EntryList List/Card modes.
6. Token/skin refresh + reader polish.
7. DESIGN.md update + verification.
