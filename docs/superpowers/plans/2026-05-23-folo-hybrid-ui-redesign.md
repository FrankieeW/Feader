# Folo Hybrid UI Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure Feader's reader into Folo's four-zone layout (icon rail · grouped feed sidebar · entry list · reader) with user-assignable feed categories and a softer, premium skin, while keeping the local-first identity and copper palette.

**Architecture:** A new Rust/SQLite `category` column + `set_source_category` command drives collapsible feed folders. The React shell gains a fixed icon rail and a grouped sidebar; the entry list becomes clean rows with a List/Card mode; CSS tokens shift to medium-radius + hairline + soft-fill. No change to feed fetching or article schema.

**Tech Stack:** React 19 + TypeScript + plain CSS (`src/App.tsx`, `src/App.css`); Tauri 2 + rusqlite (`src-tauri/src/{db,models,lib}.rs`). Backend uses `cargo test`; frontend verifies via `npm run build` + manual browser test mode.

---

## Spec reference

`docs/superpowers/specs/2026-05-23-folo-hybrid-ui-redesign-design.md`

## File map

- `src-tauri/src/models.rs` — add `category` to `Source`.
- `src-tauri/src/db.rs` — schema column, `list_sources` SELECT/mapping, `set_source_category` method, Rust test.
- `src-tauri/src/lib.rs` — `set_source_category` command + handler registration.
- `src/App.tsx` — `Source.category`, test-mode parity, state (`entryLayout`, `feedGroupCollapse`), IconRail, grouped FeedSidebar, EntryLayout control, category picker, reader polish.
- `src/App.css` — token/skin refresh, rail, grouped sidebar, entry rows/cards, reader polish.
- `DESIGN.md` — brand/IA/components/open-question updates.

## Testing note

There is no JS test runner in this repo. Frontend "verify" steps run `npm run build` (which is `tsc && vite build`) for type/compile safety and use the browser test mode (`npm run dev`, no Tauri backend) for manual checks. Do NOT add a JS test framework — out of scope. Backend uses real `cargo test`.

---

### Task 1: Add `category` to the Source model and schema

**Files:**
- Modify: `src-tauri/src/models.rs:6-20` (Source struct)
- Modify: `src-tauri/src/db.rs` (CREATE TABLE, migration, `list_sources` SELECT + row mapping)
- Test: `src-tauri/src/db.rs` (tests module)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/db.rs` (after `source_insert_is_idempotent_by_url`):

```rust
    #[test]
    fn new_source_has_no_category() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");
        assert_eq!(source.category, None);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test new_source_has_no_category`
Expected: FAIL to compile — `Source` has no field `category`.

- [ ] **Step 3: Add the struct field**

In `src-tauri/src/models.rs`, in `struct Source`, add after the `pub url: String,` line:

```rust
    pub category: Option<String>,
```

- [ ] **Step 4: Add schema column + migration**

In `src-tauri/src/db.rs` `initialize_schema`, change the `sources` CREATE TABLE tail from:

```rust
            last_fetched_at TEXT,
            last_error TEXT
        );
```

to:

```rust
            last_fetched_at TEXT,
            last_error TEXT,
            category TEXT
        );
```

Then, after the existing `add_column_if_missing(... "last_error" ...)?;` call, add:

```rust
    add_column_if_missing(
        connection,
        "sources",
        "category",
        "ALTER TABLE sources ADD COLUMN category TEXT",
    )?;
```

- [ ] **Step 5: Add `category` to `list_sources` query and mapping**

In `src-tauri/src/db.rs` `list_sources_with_connection`, change the end of the SELECT column list from:

```rust
                SUM(CASE WHEN COALESCE(article_states.read, 0) = 0 AND articles.id IS NOT NULL THEN 1 ELSE 0 END) AS unread_count
            FROM sources
```

to:

```rust
                SUM(CASE WHEN COALESCE(article_states.read, 0) = 0 AND articles.id IS NOT NULL THEN 1 ELSE 0 END) AS unread_count,
                sources.category
            FROM sources
```

In the same function's `query_map` closure, change the final field from:

```rust
                unread_count: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
            })
```

to:

```rust
                unread_count: row.get::<_, Option<i64>>(10)?.unwrap_or(0),
                category: row.get(11)?,
            })
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cd src-tauri && cargo test new_source_has_no_category`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/db.rs
git commit -m "feat: add category column to sources"
```

---

### Task 2: `set_source_category` DB method + Tauri command

**Files:**
- Modify: `src-tauri/src/db.rs` (new method + test)
- Modify: `src-tauri/src/lib.rs` (command + handler registration)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/db.rs`:

```rust
    #[test]
    fn source_category_sets_and_clears() {
        let database = AppDatabase::in_memory().expect("database opens");
        let source = database
            .add_source("https://example.com/feed.xml", Some("Example"))
            .expect("source inserts");

        let set = database
            .set_source_category(source.id, Some("Dev"))
            .expect("category sets");
        assert_eq!(set.category.as_deref(), Some("Dev"));

        let cleared = database
            .set_source_category(source.id, Some("   "))
            .expect("blank clears category");
        assert_eq!(cleared.category, None);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test source_category_sets_and_clears`
Expected: FAIL to compile — no method `set_source_category`.

- [ ] **Step 3: Implement the method**

In `src-tauri/src/db.rs`, add this method inside `impl AppDatabase` (after `update_source_title`):

```rust
    /// Set or clear a source's category folder. Blank/whitespace clears it.
    pub fn set_source_category(
        &self,
        source_id: i64,
        category: Option<&str>,
    ) -> Result<Source, String> {
        let normalized = category
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let now = now_string();
        let connection = self.connection.lock().map_err(|error| error.to_string())?;
        connection
            .execute(
                "UPDATE sources SET category = ?1, updated_at = ?2 WHERE id = ?3",
                params![normalized, now, source_id],
            )
            .map_err(|error| error.to_string())?;
        get_source_with_connection(&connection, source_id)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test source_category_sets_and_clears`
Expected: PASS.

- [ ] **Step 5: Add the Tauri command**

In `src-tauri/src/lib.rs`, add after `update_source_title`:

```rust
/// Set or clear a source's category folder.
#[tauri::command]
fn set_source_category(
    source_id: i64,
    category: Option<String>,
    database: tauri::State<'_, AppDatabase>,
) -> Result<Source, String> {
    database.set_source_category(source_id, category.as_deref())
}
```

In the `tauri::generate_handler!` list, add `set_source_category,` after `update_source_title,`.

- [ ] **Step 6: Verify full backend build + tests**

Run: `cd src-tauri && cargo test`
Expected: all tests PASS, no warnings about unused `set_source_category`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat: add set_source_category command"
```

---

### Task 3: Frontend Source type + test-mode parity

**Files:**
- Modify: `src/App.tsx` (Source type, test data, `testModeInvoke`)

- [ ] **Step 1: Add `category` to the Source type**

In `src/App.tsx`, in `type Source`, add after `url: string;`:

```typescript
  category?: string | null;
```

- [ ] **Step 2: Add categories to test data**

In `testModeSources`, set the single source's category. Change the object to include:

```typescript
    category: "News",
```

(place it after the `url:` line). For `upsertTestModeSource`, add `category: null,` to the new `source` object literal (after `url: trimmedUrl,`).

- [ ] **Step 3: Handle `set_source_category` in test mode**

In `testModeInvoke`'s `switch`, add this case before `default:`:

```typescript
    case "set_source_category": {
      const sourceId = Number(args?.sourceId);
      const rawCategory = typeof args?.category === "string" ? args.category.trim() : "";
      const category = rawCategory.length > 0 ? rawCategory : null;
      testModeSourceState = testModeSourceState.map((source) =>
        source.id === sourceId ? { ...source, category } : source,
      );
      return testModeSourceState.find((source) => source.id === sourceId) as T;
    }
```

- [ ] **Step 4: Verify build**

Run: `npm run build`
Expected: PASS (tsc clean, vite builds).

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx
git commit -m "feat: surface source category in frontend types and test mode"
```

---

### Task 4: CSS token + skin refresh

**Files:**
- Modify: `src/App.css` (`:root`, dark theme, shared radius/fill tokens)

- [ ] **Step 1: Add fill + hairline tokens (light)**

In `src/App.css` `:root`, add after `--color-selected-ring: ...;`:

```css
  --color-fill: #f1ece4;
  --color-line: rgba(40, 30, 20, 0.07);
  --color-line-soft: rgba(40, 30, 20, 0.045);
  --radius-lg: 14px;
  --radius-md: 12px;
```

- [ ] **Step 2: Add the same tokens (dark)**

In `:root[data-theme="dark"]`, add after its `--color-selected-ring: ...;`:

```css
  --color-fill: #2c2a28;
  --color-line: rgba(255, 248, 240, 0.09);
  --color-line-soft: rgba(255, 248, 240, 0.055);
```

(The `--radius-lg`/`--radius-md` from `:root` are inherited; no need to repeat.)

- [ ] **Step 3: Soften panel radius globally**

In `src/App.css`, update the shared radius on these rule groups from `border-radius: 8px;` to `border-radius: var(--radius-md);`:
- `.source-stats div, .source-composer, .story-card, .source-panel, .empty-state` block
- `.page-panel, .source-card, .settings-card` block
- `.brand-mark` (use `var(--radius-md)`)

Leave small controls (buttons, inputs, pills, tabs at 8px/999px) unchanged.

- [ ] **Step 4: Verify build**

Run: `npm run build`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/App.css
git commit -m "style: add fill/hairline tokens and medium radius"
```

---

### Task 5: Icon rail + layout shell

**Files:**
- Modify: `src/App.tsx` (IconRail component, shell JSX, remove in-sidebar workspace-nav)
- Modify: `src/App.css` (`.app-shell` grid, `.icon-rail`)

- [ ] **Step 1: Add the IconRail component**

In `src/App.tsx`, add this component (near `ThemeControl`):

```tsx
function IconRail({
  activeView,
  onSelectView,
  themeMode,
  onCycleTheme,
}: {
  activeView: ViewMode;
  onSelectView: (view: ViewMode) => void;
  themeMode: ThemeMode;
  onCycleTheme: () => void;
}) {
  return (
    <nav className="icon-rail" aria-label="Primary">
      <span className="rail-mark" aria-hidden="true">F</span>
      {(["reader", "sources"] as const).map((view) => (
        <button
          aria-current={activeView === view ? "page" : undefined}
          aria-label={viewLabel(view)}
          className={`rail-button ${activeView === view ? "active" : ""}`}
          key={view}
          onClick={() => onSelectView(view)}
          type="button"
        >
          {railIcon(view)}
        </button>
      ))}
      <span className="rail-spacer" />
      <button
        aria-label={`Theme: ${themeLabel(themeMode)}`}
        className="rail-button"
        onClick={onCycleTheme}
        type="button"
      >
        {railIcon("theme")}
      </button>
      <button
        aria-current={activeView === "settings" ? "page" : undefined}
        aria-label="Settings"
        className={`rail-button ${activeView === "settings" ? "active" : ""}`}
        onClick={() => onSelectView("settings")}
        type="button"
      >
        {railIcon("settings")}
      </button>
    </nav>
  );
}

function railIcon(name: ViewMode | "theme"): JSX.Element {
  const paths: Record<string, string> = {
    reader: "M4 6h16M4 12h16M4 18h11",
    sources: "M4 4h16v16H4zM4 9.5h16",
    theme: "M12 7a5 5 0 100 10 5 5 0 000-10zM12 2v2M12 20v2M2 12h2M20 12h2",
    settings: "M12 9a3 3 0 100 6 3 3 0 000-6zM12 2v3M12 19v3M2 12h3M19 12h3",
  };
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <path d={paths[name]} />
    </svg>
  );
}
```

- [ ] **Step 2: Add a theme-cycle helper**

In `src/App.tsx`, add near `applyThemeMode`:

```typescript
function nextThemeMode(mode: ThemeMode): ThemeMode {
  if (mode === "light") {
    return "dark";
  }
  if (mode === "dark") {
    return "system";
  }
  return "light";
}
```

- [ ] **Step 3: Render the rail and drop the in-sidebar nav**

In `src/App.tsx` `App`, inside `<main className="app-shell" ...>`, add as the FIRST child (before `<aside className="sidebar">`):

```tsx
      <IconRail
        activeView={activeView}
        onSelectView={setActiveView}
        themeMode={themeMode}
        onCycleTheme={() => setThemeMode((mode) => nextThemeMode(mode))}
      />
```

Then DELETE the `<nav className="workspace-nav" ...> ... </nav>` block (the `(["reader","sources","settings"] ...).map(...)` nav) from the sidebar — the rail now owns view switching.

- [ ] **Step 4: Update the shell grid for the rail**

In `src/App.css`, change `.app-shell` `grid-template-columns` from:

```css
  grid-template-columns: minmax(220px, var(--sidebar-width)) 10px minmax(360px, var(--timeline-width)) 10px minmax(0, 1fr);
```

to:

```css
  grid-template-columns: 54px minmax(220px, var(--sidebar-width)) 10px minmax(360px, var(--timeline-width)) 10px minmax(0, 1fr);
```

Change `.app-shell:not([data-view="reader"])` from:

```css
  grid-template-columns: minmax(220px, var(--sidebar-width)) minmax(0, 1fr);
```

to:

```css
  grid-template-columns: 54px minmax(220px, var(--sidebar-width)) minmax(0, 1fr);
```

Update the explicit `grid-column` placements: in the `[data-view="reader"]` block change sidebar→`grid-column: 2`, first resizer→`3`, timeline→`4`, second resizer→`5`, reader→`6`. In the `:not([data-view="reader"])` block change sidebar→`grid-column: 2`, `.page-view`→`grid-column: 3`. Also change the top-level `.page-view { grid-column: 2 / -1; }` to `grid-column: 3 / -1;` and the `@media (max-width:1160px)` `.page-view { grid-column: 2; }` to `3`.

- [ ] **Step 4b: Add rail styles**

In `src/App.css`, add:

```css
.icon-rail {
  grid-column: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  padding: 12px 0;
  border-right: 1px solid var(--color-line-soft);
  background: var(--color-bg);
}

.rail-mark {
  display: grid;
  width: 26px;
  height: 26px;
  place-items: center;
  margin-bottom: 14px;
  border-radius: 8px;
  color: var(--color-brand-contrast);
  background: var(--color-brand);
  font-weight: 700;
}

.rail-button {
  position: relative;
  display: grid;
  width: 36px;
  height: 36px;
  place-items: center;
  border: 0;
  border-radius: 11px;
  padding: 0;
  color: var(--color-muted);
  background: transparent;
}

.rail-button svg {
  width: 19px;
  height: 19px;
}

.rail-button.active {
  color: var(--color-action);
  background: color-mix(in srgb, var(--color-action) 12%, transparent);
}

.rail-button.active::before {
  position: absolute;
  left: -12px;
  width: 3px;
  height: 18px;
  border-radius: 3px;
  background: var(--color-action);
  content: "";
}

.rail-spacer {
  flex: 1;
}
```

In the `@media (max-width: 960px)` block, add `.icon-rail { flex-direction: row; justify-content: flex-start; gap: 8px; border-right: 0; border-bottom: 1px solid var(--color-line-soft); }` and add `.icon-rail` to the `grid-column: 1` collapse list, and `.rail-spacer { display: none; }`.

- [ ] **Step 5: Verify build + manual**

Run: `npm run build`
Expected: PASS.
Then `npm run dev`, open the local URL: confirm the rail appears, Reader/Sources/Settings switch views, theme button cycles light→dark→system, and the old text nav is gone.

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: add icon rail and restructure shell grid"
```

---

### Task 6: Grouped, collapsible feed sidebar

**Files:**
- Modify: `src/App.tsx` (group helper, collapse state, FeedSidebar rendering)
- Modify: `src/App.css` (`.feed-group`, refined `.feed-item`)

- [ ] **Step 1: Add the grouping helper**

In `src/App.tsx`, add near other helpers:

```typescript
const uncategorizedLabel = "Uncategorized";

function groupSourcesByCategory(sources: Source[]): { category: string; sources: Source[] }[] {
  const groups = new Map<string, Source[]>();
  for (const source of sources) {
    const key = source.category?.trim() ? source.category.trim() : uncategorizedLabel;
    const bucket = groups.get(key) ?? [];
    groups.set(key, [...bucket, source]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => {
      if (a === uncategorizedLabel) return 1;
      if (b === uncategorizedLabel) return -1;
      return a.localeCompare(b);
    })
    .map(([category, sources]) => ({ category, sources }));
}
```

- [ ] **Step 2: Add collapse state + persistence**

In `src/App.tsx`, add the storage key near the others:

```typescript
const feedGroupStorageKey = "feader.feedGroups";
```

Add state in `App`:

```typescript
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>(() =>
    readInitialCollapsedGroups(),
  );
```

Add the reader + effect:

```typescript
function readInitialCollapsedGroups(): Record<string, boolean> {
  const stored = localStorage.getItem(feedGroupStorageKey);
  if (!stored) {
    return {};
  }
  try {
    return JSON.parse(stored) as Record<string, boolean>;
  } catch {
    return {};
  }
}
```

And inside `App`, an effect:

```typescript
  useEffect(() => {
    localStorage.setItem(feedGroupStorageKey, JSON.stringify(collapsedGroups));
  }, [collapsedGroups]);
```

Add the derived groups near other `useMemo`s:

```typescript
  const sourceGroups = useMemo(() => groupSourcesByCategory(sources), [sources]);
```

- [ ] **Step 3: Replace the flat feed list with grouped rendering**

In `src/App.tsx`, replace the `<nav className="feed-list" ...>` block (the "All feeds" button + `sources.map(...)`) with:

```tsx
            <nav className="feed-list" aria-label="Feeds">
              <button
                className={`feed-item ${selectedSourceId === undefined ? "active" : ""}`}
                onClick={() => void handleSelectSource(undefined)}
                type="button"
              >
                <span className="feed-main">
                  <span className="status-dot mixed" />
                  <span className="feed-name">All feeds</span>
                </span>
                <small>{unreadCount}</small>
              </button>
              {sourceGroups.map((group) => {
                const collapsed = collapsedGroups[group.category] ?? false;
                return (
                  <div className="feed-group" key={group.category}>
                    <button
                      aria-expanded={!collapsed}
                      className="feed-group-header"
                      onClick={() =>
                        setCollapsedGroups((current) => ({
                          ...current,
                          [group.category]: !collapsed,
                        }))
                      }
                      type="button"
                    >
                      <span>{group.category}</span>
                      <span aria-hidden="true">{collapsed ? "▸" : "▾"}</span>
                    </button>
                    {collapsed
                      ? null
                      : group.sources.map((source) => (
                          <button
                            className={`feed-item ${selectedSourceId === source.id ? "active" : ""}`}
                            key={source.id}
                            onClick={() => void handleSelectSource(source.id)}
                            type="button"
                          >
                            <span className="feed-main">
                              <span
                                className={`status-dot ${source.lastError ? "error" : source.unreadCount > 0 ? "healthy" : "muted"}`}
                              />
                              <span className="feed-name">{source.title}</span>
                            </span>
                            <small>{source.unreadCount}</small>
                          </button>
                        ))}
                  </div>
                );
              })}
            </nav>
```

(Note: this drops the per-feed `kind · N articles` sub-label `<em>` for a cleaner Folo-like row; the kind still appears in the source panel.)

- [ ] **Step 4: Add group + refined feed styles**

In `src/App.css`, add:

```css
.feed-group {
  display: grid;
  gap: 2px;
}

.feed-group-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  min-height: 34px;
  border: 0;
  border-radius: 8px;
  padding: 4px 8px;
  color: var(--color-faint);
  background: transparent;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
}

.feed-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-dot.muted {
  background: transparent;
  box-shadow: inset 0 0 0 1.5px var(--color-border-strong);
}
```

In the existing `.feed-item:hover, .feed-item.active` rule, change the hover background to `var(--color-fill)` and keep the active ring. (Replace `background: var(--color-panel-strong);` with `background: var(--color-fill);` in that combined rule, and keep `.feed-item.active { box-shadow: 0 0 0 1px var(--color-selected-ring); }`.)

- [ ] **Step 5: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: confirm the single test source appears under a "News" group, the group header collapses/expands and persists across reload, "All feeds" still works.

- [ ] **Step 6: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: group feeds into collapsible category folders"
```

---

### Task 7: Category picker

**Files:**
- Modify: `src/App.tsx` (handler + CategoryPicker control in source panel and sources manager)
- Modify: `src/App.css` (`.category-picker`)

- [ ] **Step 1: Add the handler**

In `src/App.tsx` `App`, add:

```typescript
  async function handleSetCategory(sourceId: number, category: string): Promise<void> {
    await runTask("Updating category", async () => {
      await invoke<Source>("set_source_category", { sourceId, category });
      await loadData(selectedSourceId, filterMode, selectedArticleId);
      setStatus("Category updated");
    });
  }
```

- [ ] **Step 2: Add the CategoryPicker component**

In `src/App.tsx`, add:

```tsx
function CategoryPicker({
  source,
  categories,
  disabled,
  onSubmit,
}: {
  source: Source;
  categories: string[];
  disabled: boolean;
  onSubmit: (sourceId: number, category: string) => void;
}) {
  const [value, setValue] = useState(source.category ?? "");
  useEffect(() => {
    setValue(source.category ?? "");
  }, [source.id, source.category]);

  return (
    <form
      className="category-picker"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit(source.id, value);
      }}
    >
      <input
        aria-label="Source category"
        disabled={disabled}
        list="feader-category-options"
        onChange={(event) => setValue(event.currentTarget.value)}
        placeholder="Category"
        value={value}
      />
      <datalist id="feader-category-options">
        {categories.map((category) => (
          <option key={category} value={category} />
        ))}
      </datalist>
      <button disabled={disabled} type="submit">
        Set
      </button>
    </form>
  );
}
```

- [ ] **Step 3: Derive the category list**

In `src/App.tsx` `App`, add near other `useMemo`s:

```typescript
  const categoryOptions = useMemo(
    () =>
      [...new Set(sources.map((source) => source.category?.trim()).filter((value): value is string => Boolean(value)))].sort(),
    [sources],
  );
```

- [ ] **Step 4: Mount it in the source panel and sources manager**

In the reader-side `.source-panel`, inside the `selectedSource` branch (after `<SourceHealthStrip .../>`), add:

```tsx
              <CategoryPicker
                categories={categoryOptions}
                disabled={isBusy}
                onSubmit={(id, category) => void handleSetCategory(id, category)}
                source={selectedSource}
              />
```

In the Sources manager `.source-card` (inside `sources.map`, after `<SourceHealthStrip .../>`), add the same element using `source` instead of `selectedSource`:

```tsx
                  <CategoryPicker
                    categories={categoryOptions}
                    disabled={isBusy}
                    onSubmit={(id, category) => void handleSetCategory(id, category)}
                    source={source}
                  />
```

- [ ] **Step 5: Style the picker**

In `src/App.css`, add:

```css
.category-picker {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 8px;
}

.category-picker input {
  min-width: 0;
  min-height: 38px;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  padding: 0 10px;
  color: var(--color-text);
  background: var(--color-panel-strong);
}

.category-picker input:focus {
  border-color: var(--color-action);
  box-shadow: 0 0 0 3px var(--color-selected-ring);
  outline: 0;
}
```

- [ ] **Step 6: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: select the source, type a category (e.g., "Dev"), click Set; confirm the sidebar regroups under "Dev". Clear the field and Set; confirm it moves to "Uncategorized". (Test mode persists within the session.)

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: assign source categories from the UI"
```

---

### Task 8: Entry list — List/Card layout modes

**Files:**
- Modify: `src/App.tsx` (replace `ArticleDensity` with `EntryLayout`, control, story list)
- Modify: `src/App.css` (replace `.story-list.compact` with `.story-list.card`)

- [ ] **Step 1: Replace the density type + storage**

In `src/App.tsx`:
- Change `type ArticleDensity = "comfortable" | "compact";` to `type EntryLayout = "list" | "card";`.
- Change `const densityStorageKey = "feader.articleDensity";` to `const entryLayoutStorageKey = "feader.entryLayout";`.
- Replace `readInitialArticleDensity` with:

```typescript
function readInitialEntryLayout(): EntryLayout {
  const stored = localStorage.getItem(entryLayoutStorageKey);
  if (stored === "list" || stored === "card") {
    return stored;
  }
  return "list";
}
```

- Replace `articleDensityLabel` with:

```typescript
function entryLayoutLabel(layout: EntryLayout): string {
  return layout === "card" ? "Card" : "List";
}
```

- [ ] **Step 2: Swap the state + control component**

In `App`, change the density state to:

```typescript
  const [entryLayout, setEntryLayout] = useState<EntryLayout>(() => readInitialEntryLayout());
```

Change its persistence effect to write `entryLayoutStorageKey` / `entryLayout`. In `handleResetWorkspaceLayout`, replace `setArticleDensity("comfortable")` with `setEntryLayout("list")` and `localStorage.removeItem(densityStorageKey)` with `localStorage.removeItem(entryLayoutStorageKey)`.

Replace `DensityControl` with:

```tsx
function EntryLayoutControl({
  layout,
  onChange,
}: {
  layout: EntryLayout;
  onChange: (layout: EntryLayout) => void;
}) {
  return (
    <div className="entry-layout-control" role="group" aria-label="Entry layout">
      {(["list", "card"] as const).map((next) => (
        <button
          aria-pressed={layout === next}
          className={layout === next ? "active" : ""}
          key={next}
          onClick={() => onChange(next)}
          type="button"
        >
          {entryLayoutLabel(next)}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Update usages**

In the timeline toolbar, replace `<DensityControl density={articleDensity} onChange={setArticleDensity} />` with `<EntryLayoutControl layout={entryLayout} onChange={setEntryLayout} />`.
Change the story list container className from `` `story-list ${articleDensity}` `` to `` `story-list ${entryLayout}` ``.
In the Settings "Workspace" card, replace the `DensityControl` usage and the `<span>{articleDensityLabel(articleDensity)}</span>` with `EntryLayoutControl` + `{entryLayoutLabel(entryLayout)}`.

- [ ] **Step 4: Add a thumbnail in card mode**

In the `articles.map(...)` `story-card`, add — as the first child inside the `<article>` (before `<div className="story-state">`):

```tsx
                {entryLayout === "card" ? (
                  <div
                    className="story-thumb"
                    style={article.imageUrl ? { backgroundImage: `url(${article.imageUrl})` } : undefined}
                  />
                ) : null}
```

- [ ] **Step 5: Replace compact CSS with card CSS**

In `src/App.css`, DELETE the `.story-list.compact ...` rules. Add:

```css
.story-list.card .story-card {
  grid-template-columns: 56px minmax(0, 1fr);
  grid-template-areas:
    "thumb state"
    "thumb meta"
    "thumb title"
    "thumb summary"
    "thumb actions";
  align-items: start;
  column-gap: 12px;
}

.story-thumb {
  grid-area: thumb;
  width: 56px;
  height: 56px;
  border-radius: var(--radius-md);
  background-color: var(--color-fill);
  background-position: center;
  background-size: cover;
}

.story-list.card .story-state { grid-area: state; }
.story-list.card .story-meta { grid-area: meta; }
.story-list.card .story-card h2 { grid-area: title; }
.story-list.card .story-card p { grid-area: summary; }
.story-list.card .story-card .story-actions { grid-area: actions; }
```

- [ ] **Step 6: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: toggle List/Card in the entry toolbar; Card shows a thumbnail block (fallback fill when no image); selection, read, keyboard arrows still work; reload preserves the chosen mode; old `feader.articleDensity` users default cleanly to List.

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.css
git commit -m "feat: replace density toggle with List/Card entry layout"
```

---

### Task 9: Entry-row + reader polish

**Files:**
- Modify: `src/App.css` (`.story-card` rows, `.timeline` toolbar, reader margins)

- [ ] **Step 1: Make List rows hairline-separated, not boxed**

In `src/App.css`, remove `.story-card` from the boxed group `.source-stats div, .source-composer, .story-card, .source-panel, .empty-state { border ...; background ...; box-shadow ...; }` (delete just the `.story-card,` selector from that list). Then add:

```css
.story-list {
  gap: 0;
}

.story-card {
  border: 0;
  border-radius: var(--radius-md);
  background: transparent;
  box-shadow: none;
}

.story-card + .story-card {
  box-shadow: 0 -1px 0 var(--color-line-soft);
}

.story-card.selected {
  background: var(--color-fill);
  border-color: transparent;
  box-shadow: none;
}

.story-card.selected + .story-card {
  box-shadow: none;
}
```

(Keep the existing `.story-card::before` copper selected indicator; it still reads well on the fill.)

- [ ] **Step 2: Widen reader rhythm**

In `src/App.css` `.reader-article`, change `padding: 12px 8px 20px;` to `padding: 8px 12px 28px;` and confirm `gap: 16px;` stays. In `.reader-article h2`, keep clamp but set `letter-spacing: -0.01em;`.

- [ ] **Step 3: Verify build + manual**

Run: `npm run build`
Expected: PASS.
`npm run dev`: List rows are separated by hairlines with a soft fill on the selected row (no heavy card borders); dark theme still reads well; reader has comfortable margins.

- [ ] **Step 4: Commit**

```bash
git add src/App.css
git commit -m "style: refine entry rows and reader rhythm"
```

---

### Task 10: Update DESIGN.md

**Files:**
- Modify: `DESIGN.md`

- [ ] **Step 1: Update brand/visual + IA + components + open question**

Make these edits in `DESIGN.md`:
- In "Shape/radius/elevation", change "low-radius panels" to "medium-radius (12–14px) panels".
- In "Brand" → "Avoid", keep glassmorphism/crypto avoidance but it no longer conflicts (soft skin ≠ glassmorphism).
- In "Information architecture" → "Primary navigation", change to: "far-left icon rail (Reader, Sources, Settings, quick theme), grouped/collapsible feed sidebar by category, center entry list, right reader panel".
- In "Components" → "New/changed components", add: "icon rail, collapsible category feed groups, List/Card entry layout, source category picker".
- In "Open questions", change the Lucide line to: "[x] Adopted a minimal hand-authored inline SVG icon set for the rail (no dependency) / owner: product / impact: medium."
- Update "Last refreshed" to 2026-05-23 and add this plan/spec to "Evidence reviewed".

- [ ] **Step 2: Commit**

```bash
git add DESIGN.md
git commit -m "docs: align DESIGN.md with Folo hybrid redesign"
```

---

### Task 11: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Backend tests**

Run: `cd src-tauri && cargo test`
Expected: all PASS.

- [ ] **Step 2: Frontend build**

Run: `npm run build`
Expected: tsc clean, vite builds, no type errors.

- [ ] **Step 3: Manual smoke (test mode)**

Run: `npm run dev`, open the URL. Verify end-to-end:
- Rail switches Reader/Sources/Settings; theme cycles and persists.
- Feeds show under category groups; collapse persists.
- Set/clear a category; sidebar regroups; "Uncategorized" works.
- List/Card toggle; thumbnails in Card; selection + arrow keys + r/s shortcuts.
- Light and dark both look cohesive; copper stays restrained.

- [ ] **Step 4: Final commit (if any tidy-ups)**

```bash
git add -A
git commit -m "chore: folo hybrid redesign verification tidy-ups"
```

---

## Self-review notes

- **Spec coverage:** layout shell (T5), icon rail (T5), category backend (T1–T2), test-mode parity (T3), grouped sidebar (T6), category picker (T7), List/Card (T8), skin tokens (T4), row/reader polish (T9), DESIGN.md (T10), verification (T11). All spec sections mapped.
- **Type consistency:** `set_source_category` signature consistent across db.rs/lib.rs/test-mode; `EntryLayout` replaces `ArticleDensity` everywhere it was used (control, state, settings, story-list class, reset handler); `Source.category` added in Rust and TS.
- **Accessibility:** rail `aria-current`/`aria-label`, group header `aria-expanded`, layout control `aria-pressed` — consistent with the earlier tabs fix; 34px targets preserved on new controls.
