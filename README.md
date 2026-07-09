# cctf.rs

**custom ctf infra made with lots of love** :heart:

A CTF platform built from scratch in Rust — think CTFd, rebuilt
from zero as a **headless API** (for now)
Contains a lot of features nobody asked for, and a lot of features I really wanted.

---

## Why

Most CTF platforms are monoliths. `cctf.rs` is a **headless platform**: everything here
is a JSON API + OpenAPI spec, so the UI (a separate Astro app) — or a script, or a bot —
can be whatever you want. And a chunk of it is deliberately hand-rolled (auth, email,
scripting) both to avoid heavy deps and because building it is the fun part.

## Features

**Challenges**
- Flag types: static (constant-time compare), regex, weighted multi-flag partials, **rhai-scripted**, and per-team **instanced** flags
- Scoring: fixed value + **dynamic decay** (first-blood earns full value)
- **Hints** — unlock endpoint, idempotent charging, 4 deduction modes, and dynamic **rhai-computed cost**
- **Files** — upload/download over a pluggable storage backend (local disk or rclone → S3 / Google Drive / anything), sha256-checksummed
- **Visibility** — visible / hidden / `Locked` with per-field reveal control
- **Max attempts** with configurable counting modes; **prerequisite** enforcement (unlock chains)

**Teams & players**
- Local accounts (Argon2id) + **CTFtime OAuth** SSO
- Teams with signed invite tokens, captains, and **registration brackets** (divisions) gated by **rhai ACL scripts**
- Roles: Admin / Player / Spectator; custom user/team fields (JSONB)

**Scoreboard**
- Live standings, tie-breaks (last-solve / accuracy), per-bracket boards
- **Freeze** support + **CTFtime scoreboard export**

**Real-time notifications (SSE)**
- Admin announcements + automatic **solve / first-blood** broadcasts
- **Targeted**: everyone / specific teams / specific accounts / a **rhai filter** ("everyone who solved X"), resolved at publish time
- Fully config-driven (auth requirement + which events broadcast)

**Platform**
- Per-team **Kubernetes challenge instancing** with subdomain reverse-proxy routing
- Hand-rolled **SMTP client** (STARTTLS + AUTH LOGIN) + dev catcher + Cloudflare-Worker email ingress
- **Fluent i18n** from day one (errors *and* the OpenAPI spec are localized via `Accept-Language`)
- **Hand-written OpenAPI spec**, served localized at `/openapi.yaml`, `/openapi.json`, `/docs`, and drift-guarded against the router by a test

## Built from scratch (on purpose)

No auth framework, no SMTP library. Notable hand-rolled bits:
- **HS256 JWT** hardened against algorithm-confusion (verifies the HMAC before honoring the header alg) + constant-time compare
- **Argon2id** password hashing
- **SMTP sender** — EHLO/STARTTLS/AUTH LOGIN/MAIL/RCPT/DATA + dot-stuffing, over rustls
- **Sandboxed rhai** engine (bounded ops/stack/string size) powering flag validators, hint costs, bracket ACLs, and notification filters

## Architecture

- **axum** + **sqlx / Postgres**, repo/service/trait layered (`Arc<dyn Trait>` to keep `AppState` sane)
- Headless: the frontend is a **separate Astro app** — this repo is a pure API/platform
- `src/libs/{api,services,repos,types,crypto}` split into per-domain modules; lib + thin binary; 0 warnings

## Quickstart

Needs Docker (+ compose). Config via a `.env` next to the compose file:

```sh
# .env  — set a real secret before prod
JWT_SECRET=$(openssl rand -hex 32)

docker compose up -d --build        # Postgres + the app
curl -s localhost:6767/openapi.json | head
# open http://localhost:6767/docs   for the live API explorer
```

The app runs `init_db` on boot. It listens on `:6767` (override with `BIND_ADDR`).

## Configuration

**Environment** (see `.env.example`):

| var | purpose |
|---|---|
| `DATABASE_URL` | Postgres connection string (required) |
| `JWT_SECRET` | signing secret (required — no insecure default in prod) |
| `BIND_ADDR` | listen address (default `0.0.0.0:8080`; compose uses `:6767`) |
| `CTFTIME_CLIENT_ID` / `_SECRET` / `_REDIRECT_URI` | CTFtime OAuth (optional) |
| `RCLONE_REMOTE` / `RCLONE_PATH` / `RCLONE_DRIVE_ROOT_FOLDER_ID` | rclone file storage (optional) |
| `INBOUND_EMAIL_SECRET` | shared secret for the email webhook (optional) |
| `TRUST_PROXY_HEADERS` | set to `1` only behind a trusted reverse proxy |

**Runtime** config lives in Postgres as a typed `CtfConfig` singleton: CTF name, start/end/freeze
times, registration + email-verification toggles, hint-deduction mode, storage backend, upload limit,
and the notification config.

## File storage backends

Pluggable via the `FileStorage` trait:
- **Local disk** (default, `./uploads`)
- **rclone** subprocess → any rclone remote (S3, Google Drive, etc.). Mount your `rclone.conf`
  into the container and set the `RCLONE_*` env.

## Development

```sh
make db            # start Postgres
make run           # cargo run against it
make test          # unit + doctests (no DB)
make test-all      # spin up DB, run pg+http integration + unit/doctests, then wipe the DB
make check         # fmt --check + clippy (warnings = errors)
make build-docs    # build the API docs bundle
make help          # all targets
```

## Testing

- Unit tests + doctests run on every `cargo test` (no DB needed)
- Postgres-gated integration in `tests/pg.rs` + `tests/http.rs` (`#[ignore]`, need `TEST_DATABASE_URL`)
- DDL is syntax-checked without a database via a `sqlparser` unit test
- OpenAPI spec is drift-guarded: spec ≡ `API_ROUTES` ≡ router
- rclone storage roundtrip is gated on `RCLONE_TEST_REMOTE`

`make test-all` runs the whole thing (DB up → integration → unit → wipe) in one shot.

## Status

Feature tracker (and how it stacks against CTFd) lives in [`PARITY.md`](PARITY.md). Shipped:
auth + OAuth, challenges (all flag/scoring types, hints, files, visibility, max-attempts,
prereqs), teams + brackets, scoreboard + freeze + export, SSE notifications, k8s instancing,
email, i18n, OpenAPI. In progress: first-blood scoring curves, admin CRUD, import/export.

## License

See [LICENSE](LICENSE).

---

made by [river](https://github.com/TogiFerretFerret) · feeling so enby btw
