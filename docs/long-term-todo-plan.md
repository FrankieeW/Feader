# Feader Long-Term TODO Plan

Status: Draft
Last updated: 2026-05-23

This plan tracks design and product work worth borrowing from Folo and MrRSS while keeping Feader's identity: local-first, technical, source-adapter aware, and lightweight.

## Direction

Feader should evolve into a compact desktop reader for mixed information sources:

- Folo-inspired where the pattern improves high-density reading: resizable panes, fast navigation, command surfaces, contextual AI actions, timeline organization.
- MrRSS-inspired where the pattern improves practical source coverage: XPath/script/newsletter adapters, plugin visibility, translation/summary actions, source health and diagnostics.
- Feader-specific in data posture: local-first storage, explicit adapter state, no account/social/discovery dependency as the default path.

## Planning Principles

- Keep the three-pane workspace as the core product surface.
- Improve scanning density before adding new top-level routes.
- Put AI beside the reading workflow, not in front of it.
- Treat RSS, XPath, script/plugin, and newsletter sources as variants of one source model.
- Prefer small stateful UI additions over large rewrites.
- Preserve plain React, TypeScript, CSS custom properties, and Tauri/Rust boundaries unless a specific dependency pays for itself.

## Phase 1: Workspace Foundations

Goal: make the current reader feel stable and configurable under real feed data.

TODO:

- Add resizable pane splitters for sidebar, timeline, and reader.
- Persist pane widths in `localStorage`.
- Add layout reset action in Settings.
- Add article list density modes: `Compact` and `Comfortable`.
- Add selected article keyboard navigation: next, previous, mark read, save.
- Add a sticky reader action bar that remains visible while scrolling long articles.
- Improve empty, loading, error, and slow-refresh states with consistent status language.

Acceptance criteria:

- Long Chinese titles do not overlap controls at common desktop widths.
- A user can scan 20+ articles without the article cards feeling oversized.
- Pane widths survive reload and can be reset.
- Keyboard-only reading is possible for source selection, article selection, read/unread, save, and open original.

Risks:

- Splitter implementation can add fragile pointer handling. Keep it simple and avoid new dependencies initially.
- Density modes can create duplicated CSS. Use shared variables/classes rather than separate component trees.

## Phase 2: Reader Quality

Goal: make Feader's article panel feel like a serious reading surface.

TODO:

- Add reader typography modes: `System`, `Serif`, `Large text`.
- Add readable article extraction state: raw feed body, summary-only, extracted content unavailable.
- Add reader controls for copy link, open source, mark unread/read, save, and font mode.
- Add article metadata line: source, author, published date, canonical URL state.
- Add image rendering when `imageUrl` exists, with safe sizing.
- Add in-reader search within current article.
- Add source-specific reading preferences later if needed.

Acceptance criteria:

- Chinese and English long-form content have comfortable line length and line height.
- Reader controls do not wrap into unreadable clusters at desktop widths.
- Missing content is explained as source/feed limitation, not as a blank panel.

Risks:

- Better extraction may require backend adapter work. UI should expose state without pretending extraction exists.

## Phase 3: Source Manager Upgrade

Goal: turn source management into an operational dashboard, closer to MrRSS's practical source workflow.

TODO:

- Replace simple source cards with a source health table/card hybrid.
- Show adapter kind, URL, homepage, last refresh, last error, article count, unread count, enabled state.
- Add source enable/disable toggle.
- Add source refresh diagnostics: HTTP status, parse error, empty feed, selector miss.
- Add favicon/homepage support to the data model.
- Add bulk actions: refresh selected, mark selected read, disable selected.
- Add OPML import/export.

Acceptance criteria:

- A failed source tells the user what failed and when.
- Users can manage 50+ sources without opening each one.
- OPML export provides a portable backup path.

Risks:

- Source diagnostics touch Rust adapter boundaries. Add tests in `src-tauri` before changing persistence or command payloads.

## Phase 4: Source Creation and Adapter Workbench

Goal: make non-RSS sources first-class without overwhelming normal RSS setup.

TODO:

- Redesign source composer into tabs: RSS/Atom, XPath, Script/Plugin, Newsletter.
- Add XPath preview side-by-side with extracted title, URL, date, summary, content, image.
- Save draft XPath rules before final confirmation.
- Add selector validation messages per field.
- Add pagination/next-page preview for XPath.
- Add script/plugin source placeholder UI with a clear contract.
- Add adapter output inspector that shows normalized article JSON for debugging.

Acceptance criteria:

- RSS setup remains one URL field and one submit action.
- XPath setup can be validated before writing a source.
- Adapter output shape is visible enough for technical users to debug extraction.

Risks:

- XPath UI can become form-heavy. Keep advanced fields collapsed until the basic selectors produce articles.

## Phase 5: Article Actions and Rules

Goal: borrow Folo's action concept in a local-first way.

TODO:

- Add local article actions: summarize, translate, readability, copy link, open original, open source.
- Add local source/article rules:
  - title contains -> hide
  - title contains -> mark read
  - source -> auto mark read after refresh
  - source -> keep unread
  - source -> add tag
- Add rule management screen under Settings or Sources.
- Add rule preview against current articles before enabling.
- Add per-rule audit note: last matched article count and last run time.

Acceptance criteria:

- Rules can be tested before they mutate article state.
- Rules are reversible or disabled without deleting articles.
- AI-backed actions fail gracefully when no provider is configured.

Risks:

- Rule mutation can surprise users. Start with hide/mark-read only and show a preview count.

## Phase 6: Search, Command, and Navigation

Goal: make Feader usable as a daily workstation with many sources.

TODO:

- Add command palette for common actions.
- Add global search across article title, summary, content, source.
- Add quick source switcher.
- Add saved/read/unread smart views.
- Add date filters and source-kind filters.
- Add jump-to-next-unread behavior.

Acceptance criteria:

- A user can find a source or article without leaving the keyboard.
- Search results preserve source/date/read context.
- The command palette only exposes actions that are valid in the current view.

Risks:

- Search requires database indexing work. Keep first version title/summary scoped if full text is too much.

## Phase 7: AI Assistance

Goal: add AI only where it improves reading, triage, or source setup.

TODO:

- Add article summary action.
- Add article translation action.
- Add topic clustering across current queue.
- Add daily brief over unread articles.
- Add XPath selector suggestion from a pasted URL or saved HTML snapshot.
- Add provider settings with explicit local/remote privacy copy.
- Add per-action cost/status visibility.

Acceptance criteria:

- AI features are optional and off by default until configured.
- Article actions preserve original content and store generated output separately.
- Users can distinguish feed text from generated text.

Risks:

- Provider churn and privacy expectations are high. Keep provider layer narrow and document what is sent.

## Phase 8: Plugin and Script Ecosystem

Goal: support complex sources without hardcoding every site.

TODO:

- Define plugin manifest fields for source adapters.
- Add script execution sandbox design.
- Add plugin install/import flow for local files.
- Add plugin diagnostics: logs, last run, emitted articles, errors.
- Add plugin source templates.
- Add docs for normalized article output.

Acceptance criteria:

- A simple plugin can fetch a page and emit normalized articles.
- Plugin errors do not crash the app or corrupt stored articles.
- Users can disable or remove plugin sources cleanly.

Risks:

- Script execution is a security boundary. Do not ship unrestricted execution without a sandbox and explicit permissions.

## Phase 9: Data Portability and Reliability

Goal: make the local-first promise durable.

TODO:

- Add backup/export for sources, rules, settings, and saved articles.
- Add import path for a Feader backup.
- Add database migration tests.
- Add refresh scheduling with backoff.
- Add offline mode indicators.
- Add duplicate detection improvements.
- Add source-level retention settings.

Acceptance criteria:

- A user can move Feader data to another machine.
- Bad migrations are caught in tests before release.
- Refresh failures do not erase existing articles.

Risks:

- Backup/import touches persistence broadly. Use fixtures and migration tests before UI polish.

## Phase 10: Polish and Distribution

Goal: make Feader feel shippable as a desktop app.

TODO:

- Add app icon refinement and window title consistency.
- Add onboarding sample source option using Appinn feed as a test source.
- Add release notes screen or link.
- Add accessibility pass for labels, focus order, contrast, reduced motion.
- Add screenshot/e2e verification for light/dark themes and common widths.
- Add GitHub Actions build checks for frontend and Tauri tests.

Acceptance criteria:

- New users can understand the app with one sample source.
- CI catches TypeScript/build regressions.
- Light and dark themes are visually verified across reader, sources, and settings.

Risks:

- Visual QA needs browser automation availability. Keep manual screenshot checklist until in-app browser tooling is reliable.

## Recommended Execution Order

1. Resizable panes and density modes.
2. Reader typography/settings and sticky action bar.
3. Source health manager.
4. XPath workbench preview improvements.
5. Search and command palette.
6. Rules/actions.
7. AI summaries and translation.
8. Plugin/script runtime.
9. Backup/import and reliability.
10. Distribution polish.

## Near-Term Candidate PRs

### PR 1: Workspace Controls

Scope:

- Add pane splitters.
- Persist widths.
- Add compact/comfortable article list mode.
- Add layout reset setting.

Verification:

- `npm run build`
- Manual desktop width checks at 1024, 1280, 1440.
- Confirm no overlap with long Chinese article titles.

### PR 2: Reader Surface

Scope:

- Add sticky reader action bar.
- Add typography mode state.
- Add metadata row improvements.
- Add image rendering when `imageUrl` exists.

Verification:

- `npm run build`
- Manual checks with Chinese and English article content.
- Confirm reader mode does not affect sidebar/list chrome.

### PR 3: Source Health

Scope:

- Expand source panel.
- Add source manager health table.
- Surface last error and refresh state more clearly.

Verification:

- `npm run build`
- Rust tests if command payloads or persistence change.

## Non-Goals For Now

- Social feed behavior.
- Public discovery network.
- Wallet-first onboarding.
- Copying Folo or MrRSS visual identity.
- Shipping unrestricted plugin script execution.
- Making AI mandatory for basic reading.

## Open Decisions

- Should Feader add an icon library such as Lucide for dense command buttons?
- Should reader typography include a serif default for English content?
- Should rules be source-scoped first or global-first?
- Should AI outputs be stored in SQLite or kept session-only initially?
- Should plugin scripts run in Rust-side sandboxing, a JS runtime, or external command adapters?
