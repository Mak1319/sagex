# sagex-ledger-ui — read-only ledger explorer (GPUI)

Desktop viewer for the blockchain. Talks **only** to the register gateway
over HTTP — never TCP-direct to ledger nodes. Four views:

- **Records**: paged chain browser (newest first), click a row for the full
  record + header + a locally recomputed hash proof badge (✓/✗).
- **Search**: by watermark (`/record/:wm`) or by user (`/records?user_id=`).
- **Logs**: gateway + per-node auditor tails (`GET /logs`), level filter,
  auto-refresh while a permit is configured.
- **Status**: per-node height/view/seq/tip cards, gateway outbox backlog.

## Run

```bash
cargo run -p sagex-ledger-ui
```

Config via env (see root `.env.example` or export directly):

```bash
SAGEX_GATEWAY_URL=http://127.0.0.1:8081   # default when unset
SAGEX_AUDIT_PERMIT=<paste>                # optional; or paste on the Logs tab
```

## Auditor permit (for the Logs tab)

Record/status reads are open. Server logs are restricted: the gateway
requires a CA-issued permit whose subject is the auditor identity
(`[logs].auditor_sub`, default `ledger-auditor`), sent as
`Authorization: Bearer`. Mint it offline with the CA key — no code change,
no new crypto (same `sagex-auth` permits as intake):

```bash
sagex-certauth issue-permit --identity ledger-auditor --ttl-hours 720
```

Paste the token on the Logs tab (or export `SAGEX_AUDIT_PERMIT`). Unlike
intake permits it is multi-use (no JTI burn) — security comes from the
short TTL plus the restricted subject. Rotate by re-issuing. The app never
writes the permit to disk.

## Layout

- `src/main.rs` — window + theme bootstrap (mirrors `sagex_ui`).
- `src/app.rs` — `Explorer` view: tabs, paging, search, log filters,
  status cards, 10s auto-refresh of the visible tab.
- `src/backend.rs` — `GatewayClient` (reqwest) + response models.
- `src/runtime.rs`, `src/net.rs` — dedicated Tokio runtime bridge
  (GPUI thread never does I/O; same pattern as `sagex_ui`).

## Tests

```bash
cargo test -p sagex-ledger-ui   # gateway JSON shape fixtures
```
