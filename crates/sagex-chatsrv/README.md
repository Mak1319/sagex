# sagex-chatsrv

Simple chat REST + WebSocket server in Rust (Axum), MongoDB-backed.

- **Auth:** signup, login, OTP verify/resend (`signup|login|reset`), refresh rotation,
  logout, logout-all, forgot-password, reset-password, ML-DSA-65 JWKS.
- **Tokens:** JOSE JWS-compact JWTs with `alg: "ML-DSA-65"` per **RFC 9964**.
  Signing via `rustpq` ML-DSA-65, verification via workspace `sagex-crypto`.
- **Chat:** rooms + DMs, join/leave, message history (paginated), message POST
  (also fans out to WS), WebSocket realtime (`/ws/chat`).

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
| POST/GET | `/rooms` | access | create/list mine |
| POST | `/rooms/:id/join`, `/rooms/:id/leave` | access | self or member-adds-other |
| GET/POST | `/rooms/:id/messages?limit=&before=` | member | history + post (WS fan-out) |
| GET | `/ws/chat?token=` | access JWT | WS upgrade |

## Notes

- Passwords & OTP codes: argon2id hashes at rest. OTP: 6 digits, 10 min TTL,
  5 attempts max, TTL-index auto-expiry.
- `CorsLayer::permissive()` is dev default — tighten for production.
- Ephemeral ML-DSA-65 keypair at boot if `MLDSA65_*` unset (tokens die on
  restart); set both vars in production.
