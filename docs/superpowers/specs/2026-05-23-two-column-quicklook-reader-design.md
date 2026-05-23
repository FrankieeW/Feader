# Two-Column Reader with Quick Look + Immersive Reading — Design

- Date: 2026-05-23
- Status: Approved (pending spec review)
- Scope: Frontend only — `src/App.tsx`, `src/App.css`, `DESIGN.md`. No backend changes.

## Goal

Replace the three-column reader (feeds · entry list · permanent reader pane) with a two-column workspace (feeds · entry list) and move article reading into two on-demand surfaces modeled on macOS Finder:

- **Quick Look**: select an article, press Space to open a centered floating preview; press Space again to close.
- **Immersive reading**: double-click an article to open full-viewport reading; press Esc to exit.

## Approved decisions

- Quick Look is a **centered floating overlay** with a dimmed backdrop (not a side panel).
- **Source management** (rename, category, delete, health, diagnostics) moves entirely to the **Sources view**; the reader view no longer shows a source-detail panel. The Sources view already hosts these controls.
- The icon rail stays (it is a rail, not a content column).

## Out of scope (YAGNI)

- No backend/schema/command changes.
- No new article data; Quick Look and immersive reuse existing article fields.
- No split/resizable Quick Look; it is a fixed centered panel.
- No multi-article tabs or history.

## Architecture

### Layout

Reader view grid becomes: `54px (rail) | minmax(220px, var(--sidebar-width)) (feeds) | 10px (resizer) | minmax(0, 1fr) (entry list)`.

- The second resizer and the `.reader-panel` column are removed from the reader view.
- The entry list (`.timeline`) becomes the flexible `1fr` main column. Its inner content (`.story-list`) gets a comfortable max-width and centers within wide columns so rows stay readable.
- The second `PaneResizer` (between list and reader) is removed from the reader view. The sidebar stays resizable (`paneWidths.sidebar`). To minimize churn, the `PaneWidths`/`PaneKey` types and `feader.paneWidths` persistence are left as-is; `paneWidths.timeline` simply becomes unused (harmless). Do not refactor those types in this change.
- Sources and Settings views are unchanged.

### Shared component: `ReaderArticle`

Extract the existing `reader-article` markup (kicker, `<h2>`, byline, `reader-actions` with read/save/original, `reader-meta` dl, optional image, `reader-body` with content/summary fallback, `data-typography`) into a single `ReaderArticle` component. Props: `article: Article`, `readerTypography: ReaderTypography`, `onToggleRead`, `onToggleSaved`. Both Quick Look and immersive render `<ReaderArticle>`.

This removes the duplicated reader markup and gives one place to own reading layout.

### Reader view state

Add `readerView: "none" | "preview" | "immersive"` state in `App` (replaces the always-present reader pane). Helpers:
- `openPreview()` → set `"preview"` (only when an article is selected).
- `togglePreview()` → `"preview"` ⇄ `"none"`.
- `enterImmersive()` → set `"immersive"`.
- `closeReader()` → set `"none"`.

`selectedArticle` continues to drive what both surfaces display. When the selection changes while `readerView === "preview"`, the preview content follows the new selection (no reopen needed).

### Quick Look overlay

- Rendered when `readerView === "preview"`: a `position: fixed` backdrop (`role="presentation"`) + a centered panel (`role="dialog"`, `aria-modal="true"`, `aria-label` = article title) containing `<ReaderArticle>`.
- Panel: `width: min(92vw, 760px)`, `max-height: 86vh`, scrollable body, medium radius, panel shadow, copper-restrained.
- Dismiss: Space (toggle), Esc, click on backdrop, or a close button in the panel corner.
- Focus: move focus into the panel on open; restore focus to the previously selected entry row on close. Trap is not required (lightweight), but Esc/Space/backdrop must close.

### Immersive overlay

- Rendered when `readerView === "immersive"`: a full-viewport `position: fixed` surface (opaque background) with a centered `<ReaderArticle>` at reader width and generous margins, plus a minimal top bar (close/Esc affordance, source·date).
- Enter: double-click an entry row (`onDoubleClick`). Exit: Esc or the close affordance.

### Keyboard model (`handleAppKeyDown`, card handlers)

Reconcile with existing handlers (currently: ArrowUp/Down navigate, `r` toggle read, `s` toggle saved; card-level Enter/Space selects):

- **ArrowUp/Down**: navigate selection (unchanged). If preview is open, it follows the selection automatically (selection state already drives `selectedArticle`).
- **Space** (when not typing in an input, `activeView === "reader"`, an article is selected): `event.preventDefault()` then `togglePreview()`. Remove Space from the card-level `handleArticleKeyDown` so it no longer both selects and scrolls; keep Enter there for selection.
- **Esc**: if `immersive` → `closeReader()`; else if `preview` → `closeReader()`. (Handled at app shell; overlays also listen.)
- **Double-click** on an entry row → `enterImmersive()` (selects first, then enters).
- `r` / `s` continue to toggle read/saved on the selected article and also work while preview/immersive is open.

## Data flow

Selection (`selectedArticleId`) is the single source of truth for which article shows. Quick Look and immersive are pure view-state layers over it; no new data fetching. Read/save actions inside the overlays call the existing `handleToggleRead`/`handleToggleSaved`.

## Error/edge handling

- Space/double-click with no selected article: no-op (guard on `selectedArticle`).
- If the selected article is removed (e.g., filter change empties the list), `readerView` resets to `"none"`.
- Reduced motion: overlay open/close transitions are disabled under `prefers-reduced-motion: reduce`.

## Accessibility

- Quick Look panel: `role="dialog"`, `aria-modal="true"`, labelled by the article title; Esc closes; focus moves in on open and restores on close.
- Immersive: focusable container, Esc closes, close button has `aria-label`.
- Entry rows keep `role="button"`, `tabIndex={0}`, Enter to select; double-click is an enhancement, not the only path (Space preview + a visible "Open" affordance remain).
- Maintain ≥34px targets, focus-visible rings.

## DESIGN.md updates

- Information architecture: change to a two-column workspace (icon rail + grouped feed sidebar + entry list); reading happens via a centered Quick Look preview (Space) and an immersive full-view (double-click / Esc); source management lives in the Sources view.
- Components: add Quick Look overlay, immersive reader; note the shared `ReaderArticle`; remove the permanent reader/source panel from the reader view description.
- Interaction states: document Space-to-preview and double-click-to-immerse with Esc exit.

## Testing

- No JS test runner exists (out of scope to add). Verify via `npm run build` (tsc + vite) and manual browser test mode:
  - Reader view shows exactly two content columns (+ rail).
  - Space opens/closes Quick Look on the selected article; Esc and backdrop close it.
  - ArrowUp/Down moves selection and Quick Look follows.
  - Double-click opens immersive; Esc exits; Esc precedence (immersive before preview).
  - r/s still toggle read/saved from within overlays.
  - Sources view still manages rename/category/delete/health.
  - Light/dark both correct; reduced-motion disables overlay transitions.

## Implementation order

1. Extract `ReaderArticle` (pure refactor; build stays green).
2. Add `readerView` state; remove permanent `.reader-panel`; switch reader-view grid to two columns; drop the second resizer + `timeline` pane width usage.
3. Quick Look overlay + Space toggle + backdrop/Esc close + selection-follow.
4. Immersive overlay + double-click enter + Esc precedence + remove card-level Space.
5. Styling polish (overlay/backdrop/immersive, reduced-motion) + DESIGN.md.
6. Verify: build + manual smoke.
