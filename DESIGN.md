# Design

## Source of truth
- Status: Active
- Last refreshed: 2026-05-23
- Primary product surfaces: Tauri desktop reader, source management, RSS/XPath source creation, article reading panel.
- Evidence reviewed: `README.md`, `docs/plugin-system.md`, `src/App.tsx`, `src/App.css`, `docs/superpowers/plans/2026-05-23-folo-hybrid-ui-redesign.md`, Folo GitHub source (`RSSNext/Folo`), MrRSS GitHub source (`WCY-dt/MrRSS`).

## External references
- Folo: desktop-grade reader architecture with a persistent subscription column, entry column, reader content area, command/search panels, AI chat surfaces, layout state persistence, and route-level separation between timeline, AI, discover/subview, and reader modes.
- Folo visual pattern: high-density but spacious three-pane workspace, unread/source badges, context-aware toolbars, resizable columns, media-aware entry layouts, and AI affordances that sit beside the reading flow instead of replacing it.
- MrRSS: self-hosted AI RSS reader frontend using Vue, Tailwind, CSS variables, dark-mode class support, Sidebar + ArticleList + ArticleDetail workspace, resizable article column, global modals, context menu, toast system, and a practical feature set around AI summary/translation, plugins, XPath/script/newsletter sources, and integrations.
- MrRSS visual pattern: more utility-first than Folo; useful as a reference for layout modes such as normal/compact/card/image gallery and for keeping advanced source/plugin capability visible without making the base reader feel enterprise-heavy.
- Feader adaptation rule: borrow information architecture and interaction ideas, not visual identity. Feader should feel like a local-first technical workstation for feeds, XPath adapters, AI extraction, and future Web3-friendly source verification.

## Brand
- Personality: focused, technical, calm, high-signal.
- Trust signals: local-first data, explicit source adapters, visible refresh/error state, no decorative noise.
- Avoid: marketing-style hero layouts, playful color overload, glassmorphism-heavy panels, crypto casino aesthetics; soft skin tokens are acceptable when they remain flat, functional, and non-glassy.

## Product goals
- Goals: make mixed RSS/XPath information streams easy to add, scan, triage, and read.
- Non-goals: social feed behavior, public landing page, wallet-first onboarding, AI automation before source mechanics are trustworthy.
- Success signals: users can understand source health, unread volume, selected article context, and current theme at a glance.

## Personas and jobs
- Primary personas: technical readers, crypto/Web3 researchers, AI builders, open-web power users.
- User jobs: follow many sources, add non-RSS pages, identify what changed, save or mark read quickly.
- Key contexts of use: desktop knowledge work, repeated scanning, low-light reading, mixed system theme preferences.

## Information architecture
- Primary navigation: far-left icon rail (Reader, Sources, Settings, quick theme), grouped/collapsible feed sidebar by category, center entry list, right reader panel.
- Core routes/screens: single desktop workspace first; source creation, XPath preview, reader, source health, and future plugin/script panels should remain reachable without leaving the workspace.
- Content hierarchy: source health and filters first, article title/source/date second, full reading body/details third, AI/source extraction state fourth.

## Design principles
- Dense but breathable: prioritize scanning speed without cramped controls.
- State must be visible: theme, source kind, unread state, errors, selected article, and refresh status should be legible.
- Progressive power: RSS stays simple; XPath exposes advanced controls without hiding the basic flow.
- AI beside the reader: summaries, XPath fill, extraction diagnostics, and chat should appear as contextual panels/actions, not as a generic chatbot-first product.
- Advanced sources are first-class: RSS, XPath, script/plugin, newsletter, and future Web3-friendly sources should share a common health/status language.
- Tradeoffs: prefer durable controls over decorative imagery; richer theme polish should not reduce information density.

## Visual language
- Color: neutral editorial surfaces with graphite structure and restrained copper/green accents; light/dark/system themes share semantic tokens and should avoid one-note blue/purple SaaS styling.
- Typography: system UI sans for compact app chrome; strong but restrained headings; reader body uses a dedicated typography mode (system/serif/large) kept separate from app chrome.
- Spacing/layout rhythm: stable three-column desktop grid with compact source/list rows, larger reader rhythm, and resizable sidebar/timeline splitters with persisted widths.
- Shape/radius/elevation: medium-radius (12-14px) panels, subtle borders, no nested decorative cards.
- Motion: minimal; hover/focus state only unless future settings allow reduced motion handling.
- Imagery/iconography: no stock imagery; use text labels until an icon system is intentionally adopted.

## Components
- Existing components to reuse: source rail, source mode selector, article card, filter tabs, reader panel, source panel, theme segmented control.
- New/changed components: icon rail, collapsible category feed groups, List/Card entry layout, source category picker, command toolbar, source health row, AI/action strip, XPath/plugin configuration panel, empty/error/loading states with consistent icon treatment.
- Variants and states: selected, read, unread, saved, disabled, danger, error, warning, syncing, active tab, dark/light/system theme, source kind badge.
- Token/component ownership: `src/App.css` owns CSS custom properties; `src/App.tsx` owns theme preference state.

## Accessibility
- Target standard: practical WCAG AA contrast for text and controls.
- Keyboard/focus behavior: buttons and form inputs remain native focusable elements.
- Contrast/readability: theme tokens must keep body text, muted text, borders, selected states, and danger states readable.
- Screen-reader semantics: keep existing labels, nav/section landmarks, and tablist roles.
- Reduced motion and sensory considerations: avoid motion-heavy theme transitions.

## Responsive behavior
- Supported breakpoints/devices: desktop-first Tauri window, tablet two-column, mobile single-column fallback.
- Layout adaptations: reader panel moves below at medium widths; rails stack at narrow widths.
- Touch/hover differences: all controls remain text-labeled and at least 34px tall.

## Interaction states
- Loading: status line and disabled controls.
- Empty: explicit empty-state copy by article filter.
- Error: source panel `lastError` and status line.
- Success: status line summarizes add/refresh/mark-read actions.
- Disabled: opacity and cursor state.
- Offline/slow network: refresh errors persist per source.

## Content voice
- Tone: concise, operational, technically precise.
- Terminology: use "source", "feed", "XPath", "RSS/Atom", "unread", "saved".
- Microcopy rules: describe outcomes, not instructions; avoid onboarding prose inside the main workspace.

## Implementation constraints
- Framework/styling system: React + TypeScript + plain CSS modules via `src/App.css`.
- Design-token constraints: use CSS custom properties, not a new design dependency.
- Reference constraints: Folo uses a much larger modular route/layout system and MrRSS uses Vue/Tailwind; Feader should adapt only the design patterns that fit the current Tauri + React + plain CSS stack.
- Performance constraints: preserve lightweight app shell; no animation or image dependency.
- Compatibility constraints: theme preference stored in `localStorage`, system mode driven by `prefers-color-scheme`.
- Test/screenshot expectations: run build checks; use browser verification when the in-app browser is available.

## Open questions
- [x] Adopted a minimal hand-authored inline SVG icon set for the rail (no dependency) / owner: product / impact: medium.
- [x] Reader typography offers system/serif/large article modes (shipped) / owner: product / impact: low.
