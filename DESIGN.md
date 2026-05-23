# Design

## Source of truth
- Status: Active
- Last refreshed: 2026-05-23
- Primary product surfaces: Tauri desktop reader, source management, RSS/XPath source creation, article reading panel.
- Evidence reviewed: `README.md`, `docs/plugin-system.md`, `src/App.tsx`, `src/App.css`.

## Brand
- Personality: focused, technical, calm, high-signal.
- Trust signals: local-first data, explicit source adapters, visible refresh/error state, no decorative noise.
- Avoid: marketing-style hero layouts, playful color overload, glassmorphism-heavy panels, crypto casino aesthetics.

## Product goals
- Goals: make mixed RSS/XPath information streams easy to add, scan, triage, and read.
- Non-goals: social feed behavior, public landing page, wallet-first onboarding, AI automation before source mechanics are trustworthy.
- Success signals: users can understand source health, unread volume, selected article context, and current theme at a glance.

## Personas and jobs
- Primary personas: technical readers, crypto/Web3 researchers, AI builders, open-web power users.
- User jobs: follow many sources, add non-RSS pages, identify what changed, save or mark read quickly.
- Key contexts of use: desktop knowledge work, repeated scanning, low-light reading, mixed system theme preferences.

## Information architecture
- Primary navigation: left source rail, center article queue, right reader/details panel.
- Core routes/screens: single desktop workspace; modal-free source creation and reading.
- Content hierarchy: source health and filters first, article title/source/date second, full reading body/details third.

## Design principles
- Dense but breathable: prioritize scanning speed without cramped controls.
- State must be visible: theme, source kind, unread state, errors, selected article, and refresh status should be legible.
- Progressive power: RSS stays simple; XPath exposes advanced controls without hiding the basic flow.
- Tradeoffs: prefer durable controls over decorative imagery; richer theme polish should not reduce information density.

## Visual language
- Color: neutral editorial surfaces with green/graphite structure and copper actions; light/dark/system themes share semantic tokens.
- Typography: system UI sans for compact app chrome; strong but restrained headings.
- Spacing/layout rhythm: 8px radius, 8/12/16/24px spacing steps, stable three-column desktop grid.
- Shape/radius/elevation: low-radius panels, subtle borders, no nested decorative cards.
- Motion: minimal; hover/focus state only unless future settings allow reduced motion handling.
- Imagery/iconography: no stock imagery; use text labels until an icon system is intentionally adopted.

## Components
- Existing components to reuse: source rail, source mode selector, article card, filter tabs, reader panel, source panel.
- New/changed components: theme segmented control with Light/Dark/System choices.
- Variants and states: selected, read, disabled, danger, error, active tab, dark/light/system theme.
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
- Performance constraints: preserve lightweight app shell; no animation or image dependency.
- Compatibility constraints: theme preference stored in `localStorage`, system mode driven by `prefers-color-scheme`.
- Test/screenshot expectations: run build checks; use browser verification when the in-app browser is available.

## Open questions
- [ ] Whether Feader should later adopt an icon library such as Lucide for denser command buttons / owner: product / impact: medium.
- [ ] Whether reader typography should offer serif article mode / owner: product / impact: low.
