# @sagex/chat-server

WhatsApp-like **E2EE chat API server**. This service is a **ciphertext sync relay**:
CA (certificate authority) issues all keys; clients encrypt locally; this server only
syncs **key envelopes, encrypted messages, encrypted backups**. It never sees plaintext.

## Stack

- Express 4 + TypeScript + Mongoose (MongoDB) + Socket.io
- Custom **opaque-token auth (no JWT)**: `cst_access_*` / `cst_refresh_*` / `cst_device_*`
- API versioning: `/api/v1/...`

## Quickstart

```bash
cp .env.example .env
docker compose up -d mongo        # or: docker compose up --build
npm install
npm run dev
```

Health: `GET /health`, `GET /api/v1/health`

## Auth flows

1. **Signup** `POST /api/v1/auth/signup` `{email, phone, password, displayName, deviceId}` → returns `tokens {accessToken, refreshToken}` + one-time `deviceSecret`.
2. **Login** `POST /api/v1/auth/login` `{emailOrPhone, password, deviceId}`.
3. **Non-password entry** `POST /api/v1/auth/device` `{userId, deviceId, deviceSecret}` → sync-scoped tokens.
4. **Refresh** `POST /api/v1/auth/refresh` `{refreshToken}` (rotates).
5. Use `Authorization: Bearer <accessToken>` for all sync routes + socket handshake `{token}`.

## Sync API (all ciphertext opaque base64)

- `PUT /api/v1/keys/:deviceId` `{caKeyId, keyType, envelopeCiphertext, sigRef?}`
- `GET /api/v1/keys/:userId?deviceId=`
- `POST /api/v1/conversations` `{type, participantIds, titleCiphertext?, groupKeyEnvelopeCiphertext?}`
- `POST /api/v1/conversations/:id/messages` `{ciphertext, caKeyIdRef?}` → emits `message:new`
- `GET /api/v1/conversations/:id/messages?limit=&cursor=`
- `PUT /api/v1/backups` `{blobInlineCiphertext? | blobRef?, caKeyIdRef?}`

## Socket.io

Connect with `auth: {token: <accessToken>}`. Join `conv:join(conversationId)`. Events: `message:new`, `keys:updated`, `backup:ready`.

## Security notes

- Passwords: argon2id. Device secrets: argon2id. Tokens stored as SHA-256.
- Server validates sizes + base64 shape only; never decrypts.
- Rate limits on `/auth` (50/15m) and sync (600/min).
- Logs redact `authorization`, `password`, `deviceSecret`, ciphertext bodies.
