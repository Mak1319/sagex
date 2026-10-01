# sagex-chatsrv

Simple chat REST + WebSocket server in Rust (Axum), MongoDB-backed.

- **Auth:** signup, login, OTP verify/resend (`signup|login|reset`), refresh rotation,
  logout, logout-all, forgot-password, reset-password, ML-DSA-65 JWKS.
- **Tokens:** JOSE JWS-compact JWTs with `alg: "ML-DSA-65"` per **RFC 9964**.
  Signing via `rustpq` ML-DSA-65, verification via workspace `sagex-crypto`.
- **Chat:** rooms + DMs, join/leave, message history (paginated, enriched with
  reactions/polls/pins), rich message POST (text/image/video/voice/document/
  contact/poll/event/sticker + replies, fans out to WS), WebSocket realtime
  (`/ws/chat`).
- **Engagement:** poll votes, emoji reactions, pins, stars, read watermarks,
  message edit/delete (tombstones), message search, per-user room settings
  (mute/fav/archive/pin/disappearing), member removal, room delete/export,
  user lookup, DM lookup-or-create, reports, blocks.
- **Media:** MinIO object storage (S3 SigV4 presigned PUT/GET URLs); the
  server never handles file bytes. Needs the `minio` compose service
  (repo root) or the endpoints fall back to an in-memory fake (dev only).

## Quickstart

```bash
# 1. start the ROOT-level mongodb service (repo root; db sagexcadb there is
#    unrelated — the chat server uses its own unique `chatdbx` database,
#    created lazily on first write)
docker compose up -d mongodb  # (service name in root docker-compose.yml)

# 2. configure (explicit env wins over any .env files)
export MONGODB_URI='mongodb://sagexca:change-me-in-dot-env@127.0.0.1:27017/chatdbx?authSource=admin'
export DB_NAME=chatdbx
export BIND_ADDR=0.0.0.0:8080
# media (MinIO must be up: `docker compose up -d minio` from repo root)
export MINIO_ENDPOINT=http://127.0.0.1:9000 MINIO_BUCKET=sagex-media
export MINIO_ROOT_USER=sagexminio MINIO_ROOT_PASSWORD=change-me-minio-too
# or: cp crates/sagex-chatsrv/.env.example crates/sagex-chatsrv/.env

# 3. run
cargo run -p sagex-chatsrv
# listening on 0.0.0.0:8080
```

Health: `curl localhost:8080/health` · readiness: `curl localhost:8080/ready`.

## Auth flow (happy path)

```bash
curl -X POST localhost:8080/api/v1/auth/signup \
  -H 'Content-Type: application/json' \
  -d '{"email":"a@x.com","username":"alice","password":"s3cret!!pw"}'
# OTP is mock-delivered: check server logs for [mock-otp] code=...

curl -X POST localhost:8080/api/v1/auth/verify-otp \
  -H 'Content-Type: application/json' \
  -d '{"email":"a@x.com","code":"123456","purpose":"signup"}'
# -> {access_token, refresh_token, ...}

curl localhost:8080/api/v1/users/me -H "Authorization: Bearer $ACCESS"
curl -X POST localhost:8080/api/v1/auth/refresh \
  -H 'Content-Type: application/json' -d "{\"refresh_token\":\"$REFRESH\"}"
```

Forgot password: `POST /api/v1/auth/forgot-password {email}` →
`POST /api/v1/auth/reset-password {email, code, new_password}`.

## Chat

```bash
# room + history
curl -X POST localhost:8080/api/v1/rooms -H "Authorization: Bearer $ACCESS" \
  -H 'Content-Type: application/json' -d '{"name":"general"}'
curl localhost:8080/api/v1/rooms/$ROOM/messages -H "Authorization: Bearer $ACCESS"

# realtime (access JWT as query param — browsers can't set WS headers)
websocat "ws://localhost:8080/ws/chat?token=$ACCESS"
# > {"type":"join","room_id":"..."}
# > {"type":"message","room_id":"...","body":"hello pq world"}
# rich post: {"type":"message","room_id":"...","body":"pic","kind":"image",
#   "metadata":{"object_key":"media/..."}}
```

## Media upload flow (presigned URLs)

```bash
# 1. mint a PUT url (room membership enforced, type/size validated)
curl -X POST localhost:8080/api/v1/media/presign \
  -H "Authorization: Bearer $ACCESS" -H 'Content-Type: application/json' \
  -d '{"room_id":"'$ROOM'","filename":"photo.png","content_type":"image/png","size_bytes":12345}'
# -> {"object_key":"media/<room>/<uuid>/photo.png","put_url":"...","expires_at":...}
# 2. upload straight to MinIO with the EXACT content type (SigV4-signed in)
curl -X PUT "$PUT_URL" -H 'Content-Type: image/png' --data-binary @photo.png
# 3. post the message referencing the key (server verifies existence)
curl -X POST localhost:8080/api/v1/rooms/$ROOM/messages \
  -H "Authorization: Bearer $ACCESS" -H 'Content-Type: application/json' \
  -d '{"body":"photo","kind":"image","metadata":{"object_key":"media/..."}}'
# download: GET /api/v1/media/<object_key> -> time-boxed get_url (member-only)
```

## Routes (all `/api/v1` JSON unless noted)

| Method | Path | Auth | Notes |
|---|---|---|---|
| GET | `/health`, `/ready` | no | readiness pings mongo |
| POST | `/auth/signup` | no | creates unverified user + OTP |
| POST | `/auth/verify-otp` | no | `signup` activates + returns tokens |
| POST | `/auth/request-otp` | no | resend; cooldown 60s |
| POST | `/auth/login` | no | requires verified email |
| POST | `/auth/refresh` | refresh JWT | rotates (old revoked) |
| POST | `/auth/logout` | access + refresh body | revoke one session |
| POST | `/auth/logout-all` | access | revoke all sessions |
| POST | `/auth/forgot-password` | no | generic 200 (anti-enumeration) |
| POST | `/auth/reset-password` | OTP code | new password, sessions revoked |
| GET | `/auth/jwks.json` | no | ML-DSA-65 public JWK |
| GET/PUT | `/users/me` | access | profile |
| GET | `/users?q=&page=&limit=` | access | search |
| GET | `/users/:id` | access | public profile lookup |
| POST | `/dms/lookup {user_id}` | access | find-or-create DM (block-aware) |
| POST/GET | `/rooms` | access | create/list mine |
| POST | `/rooms/:id/join`, `/rooms/:id/leave` | access | self or member-adds-other |
| POST | `/rooms/:id/remove {user_id}` | member | kick (not self) |
| DELETE/GET | `/rooms/:id`, `/rooms/:id/export` | member | delete (DM: member, group: creator) / full export |
| GET/PUT | `/users/me/rooms/:id/settings` | member | mute/fav/archive/pin/disappearing |
| GET/POST | `/rooms/:id/messages?limit=&before=` | member | enriched history + rich post (WS fan-out) |
| GET | `/rooms/:id/messages/search?q=` | member | substring search |
| PATCH/DELETE | `/rooms/:id/messages/:mid` | author | edit / tombstone delete (WS fan-out) |
| POST/DELETE | `/rooms/:id/messages/:mid/vote` | member | poll vote / retract (WS `poll.result`) |
| POST/DELETE | `/rooms/:id/messages/:mid/reactions` | member | react / unreact `?emoji=` (WS `reaction`) |
| GET/POST | `/rooms/:id/pins` | member | list (embedded) / pin (WS `pin.updated`) |
| DELETE | `/rooms/:id/pins/:mid` | member | unpin |
| GET/POST | `/users/me/starred` | access | list / star |
| DELETE | `/users/me/starred/:mid` | access | unstar |
| POST | `/rooms/:id/read {message_id}` | member | read watermark (WS `read`) |
| POST | `/users/:id/report` | access | report (409 dup) |
| POST/DELETE | `/users/:id/blocks` | access | block (409 dup) / unblock |
| POST | `/media/presign` | member(room) | PUT-URL mint (type/size validated) |
| GET | `/media/*key` | member(room) | GET-URL mint (no public URLs) |
| GET | `/ws/chat?token=` | access JWT | WS upgrade |

## Notes

- Passwords & OTP codes: argon2id hashes at rest. OTP: 6 digits, 10 min TTL,
  5 attempts max, TTL-index auto-expiry.
- `CorsLayer::permissive()` is dev default — tighten for production.
- Ephemeral ML-DSA-65 keypair at boot if `MLDSA65_*` unset (tokens die on
  restart); set both vars in production.
