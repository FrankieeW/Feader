# AI, MCP, and CLI Control Plan

Feader should add AI as an optional control and understanding layer around the reader. The app should not make AI mandatory for basic reading, and it should expose deterministic operations through internal commands before letting any model or external agent act on user data.

## Goals

- Add built-in AI actions for reading, triage, XPath setup, and source maintenance.
- Support local or remote model providers behind one narrow provider interface.
- Let power users connect external coding/agent tools through MCP where appropriate.
- Expose selected app operations as a CLI so Claude Code, Codex, scripts, and humans can automate Feader safely.
- Make every AI/action boundary inspectable, permissioned, and reversible where possible.

## Non-Goals

- Do not let AI directly mutate the database without an explicit tool/action boundary.
- Do not expose arbitrary SQL, shell, or filesystem access through Feader tools.
- Do not require an AI provider to use RSS, XPath, or reading workflows.
- Do not build a general-purpose agent runtime inside Feader.
- Do not hide generated content as if it were source text.

External references:

- MCP defines a common way for applications to expose tools, resources, and prompts to AI clients: https://modelcontextprotocol.io/docs/learn
- MCP specification and transport details are published at: https://modelcontextprotocol.io/specification/2024-11-05/basic

## Capability Layers

### 1. Built-In AI Actions

Use for first-party reading features where the app controls prompt, context, and output storage.

Initial actions:

- Summarize current article.
- Translate current article.
- Explain selected paragraph.
- Generate topic tags.
- Deduplicate similar articles.
- Build unread digest.
- Suggest XPath selectors from fetched HTML.
- Repair broken XPath selectors using preview diagnostics.

Pros:

- Best user experience inside the app.
- Easier privacy copy because Feader knows exactly what context is sent.
- Easy to store generated outputs separately from source content.
- Works for non-technical users.

Cons:

- Requires provider settings, model selection, retries, and cost visibility.
- Prompt maintenance becomes product surface.
- Remote providers may create privacy concerns.

Recommendation:

- Start here for user-facing AI.
- Keep actions small and task-specific.
- Store generated results in separate columns/tables with provider/model metadata.

### 2. Provider Abstraction

Create one Rust-side provider boundary:

```ts
type AiRequest = {
  action: "summarize" | "translate" | "tag" | "xpathSuggest" | "xpathRepair";
  input: unknown;
  model?: string;
  temperature?: number;
};

type AiResponse = {
  text?: string;
  json?: unknown;
  usage?: {
    inputTokens?: number;
    outputTokens?: number;
    costUsd?: number;
  };
  provider: string;
  model: string;
};
```

Provider options:

- Local model endpoint compatible with OpenAI-style APIs.
- Remote provider API configured by user.
- Future embedded/local runtime if feasible.

Pros:

- Keeps UI independent from provider churn.
- Lets users choose privacy/cost tradeoffs.
- Makes testing easier with a mock provider.

Cons:

- Lowest-common-denominator interfaces can hide provider-specific strengths.
- Streaming, tool calling, and structured output differ across providers.
- Key storage and error normalization need careful handling.

Recommendation:

- Implement non-streaming JSON/text responses first.
- Add streaming only for chat-like surfaces later.
- Store API keys in OS credential storage, not SQLite.

### 3. MCP Server for Feader

Expose Feader as an MCP server so external AI clients can inspect and operate on Feader through permissioned tools.

Candidate resources:

- `feader://sources`
- `feader://articles?filter=unread`
- `feader://article/{id}`
- `feader://xpath/source/{id}/config`

Candidate tools:

- `list_sources`
- `list_articles`
- `get_article`
- `summarize_article`
- `preview_xpath_source`
- `repair_xpath_selectors`
- `mark_article_read`
- `save_article`
- `refresh_source`

Pros:

- Lets Claude Code, Codex, and other MCP-aware clients operate through a standard protocol.
- Tool schemas make agent actions more explicit than UI scraping.
- Good fit for read-heavy workflows and controlled mutations.

Cons:

- MCP tools can become a powerful automation surface; permissions matter.
- Running a local server adds lifecycle and port/transport management.
- External clients may send more data than users expect if resource boundaries are broad.

Recommendation:

- Add MCP after CLI command boundaries are stable.
- Start with read-only resources and preview-only XPath tools.
- Gate mutations behind explicit capability flags or per-session approval.

### 4. Feader CLI

Expose selected operations as a local CLI that talks to the same backend logic as Tauri commands.

Candidate command shape:

```bash
feader sources list --json
feader articles list --unread --limit 20 --json
feader article show 123 --json
feader source refresh 4 --json
feader xpath preview --url https://example.com --selectors selectors.json --json
feader xpath validate --selectors selectors.json --html snapshot.html --json
feader ai summarize --article-id 123 --json
```

Pros:

- Works for humans, shell scripts, Claude Code, Codex, and CI-like checks.
- Easier to test than UI automation.
- JSON output gives agents a stable contract.
- Does not require MCP clients.

Cons:

- Requires packaging and path management.
- Needs database locking rules while the desktop app is open.
- Mutating commands need confirmation or explicit flags.

Recommendation:

- Build CLI before MCP.
- Treat CLI as the deterministic control plane.
- Let MCP wrap CLI/backend commands later instead of duplicating behavior.

### 5. External Agent Control: Claude Code and Codex

Feader should not assume one coding agent. The stable contract should be CLI and MCP schemas.

Control modes:

- Read-only analysis: external agent reads sources/articles/configs.
- Preview mode: external agent proposes XPath fixes and AI outputs without writing.
- Approved mutation mode: external agent can save source config, mark read/saved, or refresh feeds.
- Developer mode: external agent can run local tests/builds for Feader itself.

Pros:

- Power users can automate repetitive maintenance.
- Claude Code/Codex can repair XPath rules using real diagnostics.
- Same surface can support future agent clients.

Cons:

- Easy to overexpose private reading data.
- Agents may perform many actions quickly; rate limits and audit logs matter.
- Developer-mode control must stay separate from user data operations.

Recommendation:

- Keep external agent control off by default.
- Expose read-only and preview commands first.
- Add audit logs for every mutation.

## Implementation Phases

### Phase 1: Internal Action Registry

- Define canonical app actions independent of UI.
- Reuse existing Tauri commands where possible.
- Add structured result types and errors.
- Keep mutation actions explicit.

Acceptance criteria:

- XPath preview, source listing, article listing, and article show can be called through a shared service layer.
- Actions return JSON-serializable outputs.

### Phase 2: Built-In AI Provider Settings

- Add Settings -> AI.
- Add provider URL/API key/model fields.
- Add test connection action.
- Add summary and translation actions.
- Store AI outputs separately from original articles.

Acceptance criteria:

- AI is off until configured.
- User can see what content will be sent before first use.
- Provider failures do not break reading.

### Phase 3: XPath AI Assistant

- Use current XPath diagnostics as input.
- Add selector suggestion from fetched HTML.
- Add repair flow for broken saved XPath sources.
- Always preview before saving generated selectors.

Acceptance criteria:

- AI-generated selectors are editable.
- Saving is blocked until preview extracts at least one article.

### Phase 4: CLI

- Add a Rust CLI binary or subcommand package that reuses adapter/database code.
- Support JSON output by default for automation.
- Add destructive/mutating flags such as `--yes` where needed.

Acceptance criteria:

- Claude Code/Codex can list sources, preview XPath selectors, and inspect articles without the desktop UI.
- CLI tests cover read-only and preview commands.

### Phase 5: MCP Server

- Add opt-in local MCP server.
- Start with read-only resources.
- Wrap stable action registry/CLI behavior.
- Add capability configuration for mutations.

Acceptance criteria:

- An MCP client can list sources, read articles, and preview XPath extraction.
- Mutating tools are disabled unless explicitly enabled.

## Method Tradeoffs

| Method | Pros | Cons | Recommendation |
| --- | --- | --- | --- |
| Built-in AI only | Best UX; controlled context; simple mental model | Harder for external agents; provider maintenance | Required first user-facing layer |
| MCP only | Standard agent integration; schema-driven tools | Poor standalone UX; server lifecycle/security overhead | Defer until stable internal actions exist |
| CLI only | Scriptable; easy to test; works with Claude/Codex | Less discoverable for normal users; packaging work | Best first automation layer |
| CLI + MCP wrapper | One deterministic backend; supports humans and agents | More surfaces to document and secure | Recommended long-term |
| Direct database access for agents | Fast to build | Unsafe; schema coupling; no permissions/audit | Reject |
| UI automation by agents | No backend work | Fragile and hard to audit | Reject except for visual QA |

## Safety and Privacy Rules

- Generated AI content must be labeled and stored separately from source text.
- Every AI action must define exactly what content is sent to the provider.
- External control surfaces start read-only.
- Mutations require explicit command/tool names and audit records.
- API keys belong in OS credential storage.
- MCP and CLI should never expose arbitrary SQL or shell access.
- XPath AI suggestions must pass preview before activation.

## Open Questions

- Which provider should be the default documented example: local OpenAI-compatible endpoint, OpenAI, Anthropic, or user-provided only?
- Should AI outputs be cached per article/version?
- Should MCP run as a sidecar process, a Tauri-managed process, or a command users start manually?
- Should CLI commands use the app's SQLite database directly or communicate with a running app process when available?
- What is the minimum audit log that is useful without becoming noisy?
