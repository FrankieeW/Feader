# AI Configuration

Feader can use an AI model to suggest XPath selectors for a page. The model is also the foundation for future AI features. Configuration lives in Settings -> AI.

## Providers

- **OpenAI-compatible**: works with OpenAI and servers exposing `/chat/completions`, including OpenRouter, Ollama, LM Studio, and gateways. Request URL example: `https://api.openai.com/v1`. Model example: `gpt-4o-mini`.
- **Anthropic**: native Messages API. Request URL example: `https://api.anthropic.com`. Model example: `claude-haiku-4-5`.

You can enter either a provider base URL or a complete API URL. If the URL does not already end in `/chat/completions` or `/v1/messages`, Feader appends the standard path for the selected provider. For example, `https://api.minimaxi.com/anthropic` with provider Anthropic becomes `https://api.minimaxi.com/anthropic/v1/messages`.

## API Key

Enter either:

- an **environment-variable reference** like `$MY_API_KEY` or `${MY_API_KEY}`. Feader stores only the reference, resolves it from the backend process environment when it calls the provider, and keeps the real secret out of the database. This is the recommended path.
- a **literal key** like `sk-...`. Feader stores it locally in the app-data SQLite database and never shows it back in the UI. This is convenient, but less secure than an environment reference because the local database contains the key.

### Environment Caveat

Desktop apps launched from Finder, Dock, or the Windows Start Menu often do not inherit the environment from your shell. For a `$VAR` reference to resolve, the variable must exist in the environment Feader is launched with. Launch from a terminal where the variable is exported, or set it in your OS login or launch environment.

## Privacy

When you use Suggest with AI, Feader fetches the page and normalizes it with the same backend DOM pipeline used by the XPath adapter. A truncated copy of that normalized HTML is sent to the provider you configured. Suggested selectors are validated locally and shown for you to confirm with Preview before anything is saved.
