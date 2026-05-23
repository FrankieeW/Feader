# Feader

Feader is an AI-native, Web3-friendly RSS reader built for people who follow fast-moving information streams across the open web, crypto networks, and decentralized communities.

## Product Direction

Feader combines a focused RSS reading experience with AI-assisted understanding and Web3-aware source discovery.

Core goals:

- Keep RSS subscriptions, unread state, saved articles, and reading flow simple and fast.
- Use AI for summaries, topic clustering, deduplication, article Q&A, tagging, and personalized reading queues.
- Support Web3-native information sources such as DAO forums, governance feeds, Mirror, Paragraph, Farcaster, Lens, ENS-linked identities, and project updates.
- Support non-RSS sources through built-in adapters, declarative extraction rules, and script/plugin integrations.
- Stay friendly to user-owned data, portable subscriptions, and open web standards.

## Initial Feature Areas

### RSS Reader

- Feed subscription management
- Article aggregation
- Read, unread, saved, and later-reading states
- Search and filtering
- OPML import and export

### AI Native

- Article summaries
- Multi-source topic briefs
- Semantic search
- Automatic tags
- Feed and article deduplication
- Personalized daily reading recommendations

### Web3 Friendly

- Wallet-aware identity options
- ENS, Farcaster, Lens, Mirror, Paragraph, and DAO source support
- Token-gated or community-specific feed support as a future extension
- User-owned exportable data model

### Plugin System

- Native RSS and Atom support for standard feeds
- Declarative XPath extraction for simple HTML or XML sources
- Script-based plugins for complex websites that need custom fetching, parsing, login handling, pagination, or anti-fragile extraction logic
- AI-assisted rule authoring so users can ask Feader to inspect a page and fill XPath selectors for title, link, date, author, content, and next-page fields
- A shared article output contract so RSS, XPath rules, and scripts all feed the same reading pipeline

See [docs/plugin-system.md](docs/plugin-system.md) for the initial architecture.

## Status

This repository has a Tauri, Rust, Vite, React, and TypeScript baseline. The current implementation includes local-first RSS/Atom source management, refresh status tracking, normalized articles, read/saved state, and a basic reader panel backed by SQLite.
