# CA ↔ chatsrv contract (`sagex-authsrv` + `sagex-chatsrv`)

This doc is the shared standard both services follow at the CSR hand-off.
Read it before touching `tokens.rs` (chatsrv) or `auth.rs`/`config.rs` (authsrv).

## 1. Service JWT (chatsrv → CA)

- **Minted by**: `POST /ca/token` and `POST /service/token` in
  `crates2/sagex-chatsrv/src/tokens.rs` (`mint_service_token`).
- **Verified by**: `verify_jwt` in `crates2/sagex-authsrv/src/auth.rs`.
- **Envelope**: compact JWT — `b64u(header).b64u(payload).b64u(sig)`.
  - `b64u` = base64 **URL_SAFE_NO_PAD** everywhere (header, payload, signature).
  - Header JSON (fixed): `{"alg":"ML-DSA-65","typ":"JWT"}`.
  - Signing input = ASCII bytes of `"header_b64.payload_b64"`.
  - Signature = ML-DSA-65 over the signing input, with the chatsrv
    service DSA key (`dsa::Implement::sign_from_password`).
- **Payload JSON schema** (field names are binding):
  `iss, sub, username, aud, iat, exp`
  - `iss` = `"sagex-chatsrv"` (authsrv `chatsrv_iss` must equal this).
  - `aud` = `"sagex-authsrv"` on both sides (chatsrv `[ca].audience`,
    authsrv `audience`). Any other `aud` is rejected.
  - `sub` = chatsrv internal `object_key` (UUID, **not** the username).
  - `username` = human username; **authoritative** for CSR binding.
  - `iat`/`exp` = unix seconds; authsrv allows ±60s skew.
- **Response shape**: `{ "token_type": "mldsa65", "token": "<compact>" }`.
- **History**: before this contract, chatsrv returned `{payload, signature}`
  (raw-JSON bytes signed, STANDARD b64) which authsrv could not parse.
  Do not reintroduce that envelope.

## 2. CSR binding (`POST /csr/issue`)

Body: `{ jwt, csr: PublicFileFormatExternal, pop_signature }`.
1. Verify JWT per §1 (anti-junk: proves the requester is a chatsrv user).
2. Proof-of-possession: `pop_signature` (hex, b64u, or STANDARD b64) must be
   the user's ML-DSA signature over the canonical
   `postcard::to_allocvec(csr.internal)` bytes, verified with
   `csr.internal.key_dsa`. Also requires `magic == 0x00106E5A`, `version == 1`.
3. `csr.internal.user_name` **must equal** `jwt.username`.
   `jwt.sub` is informational only (it is the chatsrv object_key).

## 3. chatsrv key pinning at the CA

- Preferred: `chatsrv_pub_path` in authsrv `config.toml` points to a **copy**
  of the chatsrv `*.pub` file (postcard `PublicFileFormatExternal`).
  Authsrv extracts `.internal.key_dsa` itself (`config::chatsrv_verify_key`).
- Fallback: inline hex in `chatsrv_dsa_pubkey_hex` (leave empty when the
  file is used; the `REPLACE_WITH_*` placeholder is rejected).
- Provisioning: `cp environments/sagex-chatsrv/*.pub <authsrv-env>/chatsrv.pub`.
- Rotation: re-copy the `.pub` and restart authsrv. Never hand-convert
  b64↔hex; the file flow exists to eliminate that error class.
- chatsrv also serves its key live at `GET /service/pubkey`
  (`{key_dsa: <STANDARD b64>}`), useful for out-of-band comparison.

## 4. Certificates + key lookup

- Authsrv issues X.509 (id-ml-dsa-65, OID `2.16.840.1.101.3.4.3.18`),
  subject `CN=<username>`, user `key_kem`/`key_dsa` in private extensions
  `1.3.6.1.4.1.61095.1.{1,2}`, signed by the CA ML-DSA key, PEM response.
- Minimal Mongo record per user:
  `{user_name, key_kem, key_dsa, cert_pem, issued_at}` (unique on `user_name`).
- `GET /keys/:user_name` requires a presenter certificate
  (`X-Client-Cert-PEM` header, `\n`-escaped OK): must chain to the CA
  (ML-DSA TBS verify) and be within validity; returns the stored keys.

## 5. Operational lessons (2026-09-30 live bring-up)

- **Sandbox DNS**: under `run-isolate.sh` (bwrap `--unshare-all`) the name
  `localhost` does NOT resolve. All mongo URIs (and any in-sandbox
  connection string) must use `127.0.0.1`, e.g.
  `mongodb://sagex:sagex-dev-only@127.0.0.1:27017`. The chatsrv
  `config.toml.example` already uses the IP form — keep it.
- **Presenter header**: ML-DSA certs are ~7KB DER / ~12KB PEM, which is
  fine for axum, but the PEM must be `\n`-escaped in one header line.
  Server order matters: unescape `\n` first, detect `BEGIN/END` markers,
  and only then fall back to space→newline repair (a blanket
  space-replace corrupts `BEGIN CERTIFICATE` itself).
- **Mongo key storage**: `key_kem`/`key_dsa` are stored as BSON Binary and
  the `UserRecord` struct uses `mongodb::bson::Binary` (not `Vec<u8>`,
  which fails to deserialize from Binary).
- **Axum 0.8 routes**: path captures use `{name}`, not `:name`.
- **CA key format**: `CaSigner::load` reads a keygen `.prv` (postcard
  `PrivateExternal`, `key_encapsulation_dsa`) with the password from
  `.env` — same pattern as chatsrv's `load_service_keys`.
