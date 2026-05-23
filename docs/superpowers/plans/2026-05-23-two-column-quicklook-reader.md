# Two-Column Quick Look Reader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the reader from three columns to two (feeds + entry list), and move reading into a centered Quick Look overlay (Space toggles) plus a full-viewport immersive mode (double-click enters, Esc exits).

**Architecture:** Extract the existing reader markup into a shared `ReaderArticle`. Add a `readerView` state ("none"|"preview"|"immersive") rendered as fixed overlays. Relocate source management (rename/delete, already partially there) fully into the Sources view, then remove the permanent reader pane and collapse the reader-view grid to two columns. Frontend only.

**Tech Stack:** React 19 + TypeScript + plain CSS (`src/App.tsx`, `src/App.css`). No backend changes. No JS test runner exists — verify via `npm run build` (tsc + vite) and manual browser test mode (`npm run dev`).

---

## Spec reference

`docs/superpowers/specs/2026-05-23-two-column-quicklook-reader-design.md`

## Sequencing principle

Each task leaves working software. Quick Look + immersive are added **before** the permanent reader pane is removed, and rename/delete are relocated to the Sources view **before** the pane is removed, so no capability is ever temporarily lost.

## File map

- `src/App.tsx` — `ReaderArticle` component; Sources-view rename/delete; `readerView` state + handlers; Quick Look + immersive overlays; keyboard changes; remove `.reader-panel`.
- `src/App.css` — overlay/backdrop/immersive styles; two-column reader grid; remove second resizer placement; reduced-motion.
- `DESIGN.md` — IA + components + interaction states.

---

### Task 1: Extract `ReaderArticle` shared component

**Files:** Modify `src/App.tsx` (reader-panel article markup ~lines 990-1037, and add a component near `ReaderTypographyControl`).

- [ ] **Step 1: Add the `ReaderArticle` component**

Add this top-level component (place near `ReaderTypographyControl`):

```tsx
function ReaderArticle({
  article,
  readerTypography,
  onToggleRead,
  onToggleSaved,
}: {
  article: Article;
  readerTypography: ReaderTypography;
  onToggleRead: (article: Article) => void;
  onToggleSaved: (article: Article) => void;
}) {
  return (
    <article className="reader-article" data-typography={readerTypography}>
      <div className="reader-kicker">
        <span>{article.sourceTitle}</span>
        <span>{formatDate(article.publishedAt ?? article.createdAt)}</span>
      </div>
      <h2>{article.title}</h2>
      {article.author ? <p className="byline">{article.author}</p> : null}
      <div className="reader-actions">
        <button onClick={() => onToggleRead(article)} type="button">
          {article.read ? "Mark unread" : "Mark read"}
        </button>
        <button onClick={() => onToggleSaved(article)} type="button">
          {article.saved ? "Unsave" : "Save"}
        </button>
        <a href={article.url} rel="noreferrer" target="_blank">
          Original
        </a>
      </div>
      <dl className="reader-meta">
        <dt>Source</dt>
        <dd>{article.sourceTitle}</dd>
        <dt>Published</dt>
        <dd>{formatDate(article.publishedAt ?? article.createdAt)}</dd>
        <dt>Body</dt>
        <dd>{articleBodyState(article)}</dd>
        {article.canonicalUrl ? (
          <>
            <dt>Canonical</dt>
            <dd>{article.canonicalUrl}</dd>
          </>
        ) : null}
      </dl>
      {article.imageUrl ? (
        <img alt="" className="reader-image" src={article.imageUrl} />
      ) : null}
      <div className="reader-body">
        {article.contentText ? (
          <p>{article.contentText}</p>
        ) : article.contentHtml ? (
          <p>{stripHtml(article.contentHtml)}</p>
        ) : article.summary ? (
          <p>{stripHtml(article.summary)}</p>
        ) : (
          <p>{articleBodyFallback(article)}</p>
        )}
      </div>
    </article>
  );
}
```

- [ ] **Step 2: Use it in the existing reader pane**

In `App()`'s reader pane, replace the inline `<article className="reader-article" ...> ... </article>` block (the whole thing inside the `{selectedArticle ? (...) : (...)}`) with:

```tsx
          <ReaderArticle
            article={selectedArticle}
            onToggleRead={(item) => void handleToggleRead(item)}
            onToggleSaved={(item) => void handleToggleSaved(item)}
            readerTypography={readerTypography}
          />
```

Leave the `: (` empty-state branch and the `<section className="source-panel">` untouched.

- [ ] **Step 3: Verify build**

Run: `npm run build`
Expected: PASS (only pre-existing `FormEvent is deprecated` warnings). The reader pane renders identically.

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx
git commit -m "refactor: extract ReaderArticle component"
```

---

### Task 2: Relocate rename + delete into the Sources view

**Files:** Modify `src/App.tsx` (add parametric handlers; add rename/delete to the Sources `.source-card`).

Context: rename/delete currently exist only in the reader pane's `source-panel` (bound to `selectedSource`/`editingTitle`). Before removing the reader pane, the Sources view must offer them per card.

- [ ] **Step 1: Add parametric handlers**

In `App()`, add after `handleDeleteSource`:

```tsx
  async function handleRenameSourceId(sourceId: number, title: string): Promise<void> {
    const nextTitle = title.trim();
    if (!nextTitle) {
      return;
    }
    await runTask("Renaming feed", async () => {
      await invoke<Source>("update_source_title", {
        request: { sourceId, title: nextTitle },
      });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("Feed renamed");
    });
  }

  async function handleDeleteSourceId(sourceId: number, title: string): Promise<void> {
    const confirmed = window.confirm(`Delete "${title}" and its articles?`);
    if (!confirmed) {
      return;
    }
    await runTask("Deleting feed", async () => {
      await invoke("delete_source", { sourceId });
      if (selectedSourceId === sourceId) {
        setSelectedSourceId(undefined);
        setSelectedArticleId(undefined);
        await loadData(undefined, filterMode, undefined);
      } else {
        await loadData(selectedSourceId, filterMode, selectedArticleId);
      }
      setStatus("Feed deleted");
    });
  }
```

- [ ] **Step 2: Add a `SourceCardManage` control**

Add this component (near `CategoryPicker`), giving each Sources card its own rename input:

```tsx
function SourceCardManage({
  source,
  disabled,
  onRename,
  onDelete,
}: {
  source: Source;
  disabled: boolean;
  onRename: (sourceId: number, title: string) => void;
  onDelete: (sourceId: number, title: string) => void;
}) {
  const [title, setTitle] = useState("");
  return (
    <>
      <form
        className="rename-form"
        onSubmit={(event) => {
          event.preventDefault();
          onRename(source.id, title || source.title);
          setTitle("");
        }}
      >
        <input
          aria-label={`Rename ${source.title}`}
          disabled={disabled}
          onChange={(event) => setTitle(event.currentTarget.value)}
          placeholder={source.title}
          value={title}
        />
        <button disabled={disabled} type="submit">
          Rename
        </button>
      </form>
      <button
        className="danger-action"
        disabled={disabled}
        onClick={() => onDelete(source.id, source.title)}
        type="button"
      >
        Delete feed
      </button>
    </>
  );
}
```

- [ ] **Step 3: Mount it in the Sources `.source-card`**

In the Sources view `sources.map((source) => ( ... ))` card, after the `<CategoryPicker ... source={source} />`, add:

```tsx
                  <SourceCardManage
                    disabled={isBusy}
                    onDelete={(id, title) => void handleDeleteSourceId(id, title)}
                    onRename={(id, title) => void handleRenameSourceId(id, title)}
                    source={source}
                  />
```

- [ ] **Step 4: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: in Sources view, rename a feed (title updates), delete a feed (confirm dialog, removed). Reader pane still also has its own rename/delete (unchanged for now).

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "feat: manage rename and delete from the Sources view"
```

---

### Task 3: Quick Look overlay + Space toggle

**Files:** Modify `src/App.tsx` (state, handlers, keyboard, overlay JSX), `src/App.css` (overlay/backdrop).

- [ ] **Step 1: Add the type + state**

In `src/App.tsx`, add the type near the other unions:

```tsx
type ReaderView = "none" | "preview" | "immersive";
```

In `App()`, add state (near `selectedArticleId`):

```tsx
  const [readerView, setReaderView] = useState<ReaderView>("none");
```

- [ ] **Step 2: Reset reader view when selection becomes invalid**

In `App()`, add:

```tsx
  useEffect(() => {
    if (!selectedArticle) {
      setReaderView("none");
    }
  }, [selectedArticle]);
```

- [ ] **Step 3: Add Space toggle + Esc to `handleAppKeyDown`**

Replace the body of `handleAppKeyDown` (lines ~674-698) with:

```tsx
  function handleAppKeyDown(event: KeyboardEvent<HTMLElement>): void {
    if (activeView !== "reader" || isTextInputTarget(event.target)) {
      return;
    }

    if (event.key === "Escape") {
      if (readerView !== "none") {
        event.preventDefault();
        setReaderView("none");
      }
      return;
    }

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      selectRelativeArticle(event.key === "ArrowDown" ? 1 : -1);
      return;
    }

    if (!selectedArticle || event.metaKey || event.ctrlKey || event.altKey) {
      return;
    }

    if (event.key === " ") {
      event.preventDefault();
      setReaderView((current) => (current === "preview" ? "none" : "preview"));
      return;
    }

    if (event.key.toLowerCase() === "r") {
      event.preventDefault();
      void handleToggleRead(selectedArticle);
    }

    if (event.key.toLowerCase() === "s") {
      event.preventDefault();
      void handleToggleSaved(selectedArticle);
    }
  }
```

- [ ] **Step 4: Render the Quick Look overlay**

In `App()`'s returned JSX, immediately before the closing `</main>`, add:

```tsx
      {readerView === "preview" && selectedArticle ? (
        <div
          className="ql-backdrop"
          onClick={() => setReaderView("none")}
          role="presentation"
        >
          <div
            aria-label={selectedArticle.title}
            aria-modal="true"
            className="ql-panel"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
          >
            <button
              aria-label="Close preview"
              className="ql-close"
              onClick={() => setReaderView("none")}
              type="button"
            >
              ✕
            </button>
            <ReaderArticle
              article={selectedArticle}
              onToggleRead={(item) => void handleToggleRead(item)}
              onToggleSaved={(item) => void handleToggleSaved(item)}
              readerTypography={readerTypography}
            />
          </div>
        </div>
      ) : null}
```

- [ ] **Step 5: Add overlay CSS**

In `src/App.css`, add:

```css
.ql-backdrop {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgba(20, 14, 8, 0.42);
  animation: ql-fade 120ms ease;
}

.ql-panel {
  position: relative;
  width: min(92vw, 760px);
  max-height: 86vh;
  overflow: auto;
  padding: 24px 28px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-lg);
  background: var(--color-panel-strong);
  box-shadow: var(--shadow-panel);
  animation: ql-pop 140ms ease;
}

.ql-close {
  position: absolute;
  top: 12px;
  right: 12px;
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  padding: 0;
  border: 1px solid var(--color-border);
  border-radius: 999px;
  color: var(--color-muted);
  background: var(--color-panel);
}

@keyframes ql-fade {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes ql-pop {
  from { opacity: 0; transform: translateY(6px) scale(0.99); }
  to { opacity: 1; transform: none; }
}
```

- [ ] **Step 6: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: select an article, press Space → centered preview opens; Space again, Esc, backdrop click, and ✕ all close it; ArrowUp/Down while open changes the previewed article. Reader pane still visible behind (removed in Task 5).

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: add Quick Look preview overlay with Space toggle"
```

---

### Task 4: Immersive mode + double-click + Esc precedence

**Files:** Modify `src/App.tsx` (double-click handler, immersive overlay, remove card-level Space), `src/App.css` (immersive styles).

- [ ] **Step 1: Remove Space from the card-level handler**

Change `handleArticleKeyDown` (lines ~665-672) so only Enter selects (Space is now an app-level preview toggle):

```tsx
  function handleArticleKeyDown(event: KeyboardEvent<HTMLElement>, articleId: number): void {
    if (event.key !== "Enter") {
      return;
    }

    event.preventDefault();
    setSelectedArticleId(articleId);
  }
```

- [ ] **Step 2: Add double-click to enter immersive on each entry row**

On the `<article className={\`story-card ...\`} ...>` element, add an `onDoubleClick` handler alongside the existing `onClick`:

```tsx
                onDoubleClick={() => {
                  setSelectedArticleId(article.id);
                  setReaderView("immersive");
                }}
```

- [ ] **Step 3: Render the immersive overlay**

In `App()`'s returned JSX, before the closing `</main>` (after the Quick Look block), add:

```tsx
      {readerView === "immersive" && selectedArticle ? (
        <div aria-label="Immersive reader" aria-modal="true" className="immersive" role="dialog">
          <div className="immersive-bar">
            <span>{selectedArticle.sourceTitle}</span>
            <button
              aria-label="Exit immersive reading"
              className="secondary-action"
              onClick={() => setReaderView("none")}
              type="button"
            >
              Close
            </button>
          </div>
          <div className="immersive-body">
            <ReaderArticle
              article={selectedArticle}
              onToggleRead={(item) => void handleToggleRead(item)}
              onToggleSaved={(item) => void handleToggleSaved(item)}
              readerTypography={readerTypography}
            />
          </div>
        </div>
      ) : null}
```

(Esc already exits via `handleAppKeyDown`: it sets `readerView` to "none" from either "immersive" or "preview". Immersive cannot be open at the same time as preview because both are driven by the single `readerView` value, so precedence is inherent.)

- [ ] **Step 4: Add immersive CSS**

In `src/App.css`, add:

```css
.immersive {
  position: fixed;
  inset: 0;
  z-index: 60;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
  background: var(--color-bg);
  animation: ql-fade 120ms ease;
}

.immersive-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 22px;
  border-bottom: 1px solid var(--color-line-soft);
  color: var(--color-muted);
  font-size: 12px;
}

.immersive-body {
  overflow: auto;
  padding: 24px;
}
```

- [ ] **Step 5: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: double-click an entry → full-viewport immersive reading; Esc exits; Space still toggles Quick Look (single click then Space); Enter selects a focused row; r/s toggle read/saved from both overlays.

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: add immersive reading on double-click with Esc exit"
```

---

### Task 5: Remove the permanent reader pane; collapse to two columns

**Files:** Modify `src/App.tsx` (remove second `PaneResizer` + `.reader-panel` aside), `src/App.css` (reader-view grid → two columns).

- [ ] **Step 1: Remove the reader pane + its resizer from the reader view**

In `App()`, delete the second `<PaneResizer label="Resize reader panel" ... />` element AND the entire `<aside className="reader-panel" aria-label="Reader panel"> ... </aside>` block (the reader-article-or-empty-state plus the `source-panel` section). The reader content now lives only in the Quick Look / immersive overlays; source management lives in the Sources view (Task 2).

After this, the reader view's `<>` fragment contains just the first `PaneResizer` (sidebar) and the `<section className="timeline">`.

- [ ] **Step 2: Switch the reader-view grid to two columns**

In `src/App.css`, change `.app-shell` `grid-template-columns` from:

```css
  grid-template-columns: 54px minmax(220px, var(--sidebar-width)) 10px minmax(360px, var(--timeline-width)) 10px minmax(0, 1fr);
```

to:

```css
  grid-template-columns: 54px minmax(220px, var(--sidebar-width)) 10px minmax(0, 1fr);
```

Remove the now-obsolete reader-view placement rules `.app-shell[data-view="reader"] .reader-panel { grid-column: 6; }` and `.app-shell[data-view="reader"] .pane-resizer:last-of-type { grid-column: 5; }`. Change `.app-shell[data-view="reader"] .timeline { grid-column: 4; }` to `grid-column: 4;` is now the last column — keep it `4`. (Grid: 1=rail, 2=sidebar, 3=resizer, 4=timeline.)

In the `@media (max-width: 1160px)` reader override, change:

```css
    grid-template-columns: 54px minmax(204px, var(--sidebar-width)) 10px minmax(360px, var(--timeline-width)) 10px minmax(0, 1fr);
```

to:

```css
    grid-template-columns: 54px minmax(204px, var(--sidebar-width)) 10px minmax(0, 1fr);
```

- [ ] **Step 3: Give the entry list a readable max-width in the wide column**

In `src/App.css`, the `.story-list` rule already sets layout; add a centering max-width so rows stay readable in the wide column:

```css
.timeline .story-list {
  width: min(100%, 760px);
  margin-inline: auto;
}
```

- [ ] **Step 4: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: reader view now shows exactly rail + feeds + entry list (no third pane). Space opens Quick Look; double-click opens immersive; Sources view manages rename/category/delete/health. Resize the window to ≤1160px and ≤960px — layout stays coherent (rail present, no leftover empty column).

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: collapse reader to two columns; reading via overlays"
```

---

### Task 6: Reduced-motion + DESIGN.md

**Files:** Modify `src/App.css` (reduced-motion), `DESIGN.md`.

- [ ] **Step 1: Disable overlay animations under reduced motion**

In `src/App.css`, in the existing `@media (prefers-reduced-motion: reduce)` block, add:

```css
  .ql-backdrop,
  .ql-panel,
  .immersive {
    animation: none;
  }
```

- [ ] **Step 2: Update DESIGN.md**

In `DESIGN.md`:
- Information architecture → Primary navigation: change to "far-left icon rail (Reader, Sources, Settings, quick theme), grouped/collapsible feed sidebar by category, and a center entry list (two-column reader). Reading opens in a centered Quick Look preview (Space) or full-viewport immersive mode (double-click; Esc exits). Source management lives in the Sources view."
- Components → New/changed: add "Quick Look preview overlay, immersive reader, shared ReaderArticle"; remove the permanent reader/source panel from the reader-view description.
- Interaction states: add "Preview: Space toggles a centered Quick Look for the selected article. Immersive: double-click an entry; Esc exits (Esc closes immersive or preview)."

- [ ] **Step 3: Verify build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/App.css DESIGN.md
git commit -m "docs: align design with two-column Quick Look reader; reduced motion"
```

---

### Task 7: Full verification

**Files:** none.

- [ ] **Step 1: Build**

Run: `npm run build`
Expected: tsc clean (only pre-existing FormEvent warnings), vite builds.

- [ ] **Step 2: Backend tests (unchanged, sanity)**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 11 passed.

- [ ] **Step 3: Manual smoke (test mode)**

`npm run dev`, open the URL:
- Reader view = rail + feeds + entry list only.
- Space toggles Quick Look on the selected article; Esc / backdrop / ✕ close; ArrowUp/Down moves selection and preview follows.
- Double-click → immersive; Esc exits.
- r/s toggle read/saved from overlays; Enter selects a focused row.
- Sources view: rename, category, delete, health all work.
- Light/dark cohesive; reduced-motion disables overlay animation.

- [ ] **Step 4: Final commit (if tidy-ups)**

```bash
git add -A
git commit -m "chore: two-column Quick Look reader verification tidy-ups"
```

---

## Self-review notes

- **Spec coverage:** two-column grid (T5), ReaderArticle (T1), Quick Look + Space (T3), immersive + double-click + Esc precedence (T4), source management relocation (T2 — fills the spec's "management lives in Sources view" promise, which required adding rename/delete there), reduced-motion + DESIGN.md (T6), verification (T7).
- **Sequencing:** overlays and Sources-view management land before the reader pane is removed (T5), so no capability gap between commits.
- **Type/name consistency:** `ReaderView` ("none"|"preview"|"immersive") and `setReaderView` used identically across T3–T5; `ReaderArticle` props (`article`, `readerTypography`, `onToggleRead`, `onToggleSaved`) consistent in pane, Quick Look, and immersive; parametric `handleRenameSourceId`/`handleDeleteSourceId` distinct from the existing `handleRenameSource`/`handleDeleteSource` (which are removed with the pane in T5 — verify no remaining references after T5; if `handleRenameSource`/`handleDeleteSource`/`editingTitle` become unused, delete them in T5 to keep tsc clean).
