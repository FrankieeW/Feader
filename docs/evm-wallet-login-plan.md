# EVM Wallet Login Plan

Feader should support wallet login as an optional identity layer, not as a blocker for local reading. The first version should prove ownership of an EVM address, bind it to local app state, and leave room for future sync, token-gated sources, and community identity features.

## Goals

- Let users connect an EVM wallet and sign in without an email/password account.
- Use a standard Sign-In with Ethereum flow instead of an app-specific message format.
- Keep RSS/XPath reading usable when the user is not signed in.
- Store the minimum local identity state needed for personalization and future sync.
- Make authentication auditable: every signed message should include domain, address, URI, nonce, issued time, and statement.

## Non-Goals

- Do not require wallet login during first-run onboarding.
- Do not custody keys or ask for seed phrases.
- Do not use wallet login as proof of personhood.
- Do not gate local article data behind a remote session in the first version.
- Do not add token-gated source fetching until basic wallet identity is stable.

## Recommended Baseline

Use ERC-4361 / Sign-In with Ethereum (SIWE) semantics:

1. Frontend asks the wallet for the active account and chain.
2. App generates a short-lived nonce.
3. App builds a SIWE message containing domain, address, URI, chain id, nonce, issued-at, and an explicit statement.
4. User signs the message with the wallet.
5. App verifies the signature against the claimed address.
6. App creates a local session record.
7. Optional future sync backend can verify the same signed message server-side and issue a remote session token.

External references:

- ERC-4361 defines the SIWE message format and verification semantics: https://eips.ethereum.org/EIPS/eip-4361
- Reown AppKit documents wallet connection and SIWE helpers for broad wallet coverage: https://docs.reown.com/appkit/react/core/siwe
- WalletConnect recommends SDK partners such as Reown AppKit for app integrations: https://docs.walletconnect.network/app-sdk/overview

## Architecture

### Frontend

Responsibilities:

- Show a wallet connect button in Settings and future account surfaces.
- Display active address, ENS name/avatar if resolved, chain, and session status.
- Request signature only after showing the exact purpose of the login.
- Let users disconnect locally.
- Avoid repeating signature prompts while a valid local session exists.

Possible frontend adapters:

- Reown AppKit for broad wallet support, QR flow, embedded wallet options, and SIWE integration.
- wagmi/viem for lower-level React hooks and typed EVM primitives.
- Minimal `window.ethereum` adapter for the smallest dependency surface.

### Tauri Backend

Responsibilities:

- Generate and persist login nonces with expiry.
- Verify signed SIWE messages.
- Persist local account/session state in SQLite.
- Expose narrow Tauri commands:
  - `create_wallet_login_challenge`
  - `verify_wallet_login`
  - `get_current_account`
  - `disconnect_wallet_account`
- Never receive private keys or seed phrases.

### Storage

Suggested tables:

```sql
wallet_accounts(
  id integer primary key,
  address text not null unique,
  ens_name text,
  avatar_url text,
  first_seen_at text not null,
  last_seen_at text not null
);

wallet_sessions(
  id integer primary key,
  account_id integer not null references wallet_accounts(id) on delete cascade,
  address text not null,
  chain_id integer,
  nonce text not null unique,
  siwe_message text not null,
  signature text not null,
  issued_at text not null,
  expires_at text,
  created_at text not null,
  revoked_at text
);
```

## Flow

### Local-Only Login

Use this for the first implementation.

1. User opens Settings -> Account.
2. User clicks Connect Wallet.
3. Frontend connects wallet and reads address.
4. Backend creates nonce.
5. Frontend builds/signs SIWE message.
6. Backend verifies signature and stores a local session.
7. UI shows signed-in wallet identity.

Pros:

- Works with Feader's local-first model.
- Does not require a hosted backend.
- Easier to ship and test.
- Keeps wallet login optional.

Cons:

- Does not prove identity across devices by itself.
- Cannot support cloud sync sessions yet.
- Nonce/session security is limited to the local app boundary.

### Remote Session Login

Use this later if Feader adds sync or hosted services.

1. Local app requests a challenge from a Feader service.
2. User signs SIWE message.
3. Service verifies signature and issues a short-lived session token.
4. Local app stores token in the OS credential store.

Pros:

- Supports cross-device sync.
- Enables server-side access control for future services.
- Lets the backend revoke sessions.

Cons:

- Adds backend infrastructure and privacy obligations.
- Requires secure token storage and refresh handling.
- Wallet login becomes a network-dependent feature.

### Token-Gated Source Authorization

Use only after wallet login and source adapters are stable.

Pros:

- Enables DAO/community-specific feeds.
- Fits Feader's Web3 research audience.

Cons:

- Requires chain reads, caching, and failure handling.
- Token ownership can change; authorization needs refresh semantics.
- Increases privacy risk because source access may reveal wallet/community membership.

## Implementation Phases

### Phase 1: Local SIWE Identity

- Add Account section in Settings.
- Add connect/disconnect UI.
- Implement local nonce generation and signature verification.
- Persist `wallet_accounts` and `wallet_sessions`.
- Show address and chain in UI.

Acceptance criteria:

- User can sign in with an EVM wallet and disconnect.
- Invalid signature, mismatched address, expired nonce, and replayed nonce are rejected.
- Reader, RSS, and XPath work without login.

### Phase 2: Identity Enrichment

- Add ENS reverse lookup and optional avatar display.
- Add account-scoped preferences where useful.
- Add export shape for wallet identity metadata.

Acceptance criteria:

- ENS failures do not break login.
- Address remains the canonical identity key.

### Phase 3: Remote-Ready Session Boundary

- Abstract session verification so local and remote verification use the same message shape.
- Add optional backend verifier interface.
- Store remote tokens only when sync is configured.

Acceptance criteria:

- Local login remains fully functional without remote services.
- Remote token handling is isolated from wallet signature handling.

## Method Tradeoffs

| Method | Pros | Cons | Recommendation |
| --- | --- | --- | --- |
| Plain `window.ethereum` + Rust verifier | Small dependency surface; easy to reason about; no vendor UI | Weak wallet coverage; QR/mobile flows are hard; more UI work | Good fallback, not best default |
| wagmi/viem frontend + Rust verifier | Strong EVM primitives; flexible; good React ergonomics | Still need wallet modal/connectors; more integration decisions | Good if Feader wants maximum control |
| Reown AppKit + SIWE verifier | Broad wallet support; WalletConnect QR flow; faster UX delivery | Adds SDK dependency and vendor-shaped UX; project configuration needed | Best first full-featured wallet UX |
| Server-only auth | Cross-device sessions and revocation | Requires hosted backend; not local-first | Defer until sync exists |
| Local-only SIWE | Matches current app; no server needed; privacy-preserving | No cross-device session | Recommended Phase 1 |

## Security Rules

- The app must never ask for a seed phrase or private key.
- The signed message must include domain, URI, nonce, chain id, and issued-at.
- Nonces must be single-use and expire.
- Signature verification must bind the signature to the claimed address.
- Disconnect should revoke the local session but should not delete reading data.
- Any remote service must treat wallet addresses as personal data.

## Open Questions

- Should Feader support only Ethereum mainnet identity initially, or accept any EVM chain id?
- Should account-specific preferences be separate from global local settings?
- Should future sync use wallet-only auth or allow email/OAuth fallback?
- Should token-gated source checks run locally through public RPC, through user-configured RPC, or through a Feader service?
