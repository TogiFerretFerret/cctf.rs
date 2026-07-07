# cctf.rs — CTFd Feature Parity Tracker

Goal: **minimum of full feature parity** with CTFd (idkCTF/rCTF flavor), then exceed it.

**Architecture note:** the frontend is a **separate Astro app** (kept out of this repo for
modularity), so `cctf.rs` is a **headless API/platform**. This tracker covers backend/platform
capability only — theme/UI/rendering is Astro's job.

Legend: `[x]` done · `[~]` partial · `[ ]` todo · **➕** = already beyond CTFd core
Last reconciled: 2026-07-06

---

## Build order (dependency-driven, frontend = Astro, out of scope here)

- **Phase 0 — KEYSTONE:** `main.rs` bootstrap → nothing ships without it.
- **Phase 1 — Core challenge features** (mostly model→wire): hints, files, visibility, max attempts, prereq enforcement, first blood + rhai scoring, case-insensitive flags, ordering.
- **Phase 2 — Account flows** (email-unblocked): verification, password reset, settings, API tokens, captcha, ban/hide.
- **Phase 3 — Admin backend & ops:** CRUD + authz, submissions browse, stats, awards, notifications (SSE), reset, import/export, backup.
- **Phase 4 — GitOps & content API:** minijinja templating, GitOps challenge loader (rCDS parity), incremental docker builds, registry cache, pruning, (optional) pages API for Astro.

---

## Auth & Users
- [x] Local registration
- [x] Local login / session (JWT)
- [x] CTFtime OAuth SSO
- [x] Username collision auto-handling ➕
- [x] Roles (Admin / Player / Spectator)
- [x] Custom user fields (JSONB `fields`)
- [~] Account settings / profile edit (fields exist; no endpoints)
- [~] Individual vs team mode (team built; solo path partial)
- [ ] Email verification flow (config toggle exists)
- [ ] Password reset flow
- [ ] Ban / hide users
- [ ] API / access tokens
- [ ] Captcha / reCAPTCHA

## Teams
- [x] Create / join (signed invite) / captain / members
- [x] Registration brackets (divisions)
- [x] Bracket-join ACL via rhai ➕
- [x] Custom team fields (JSONB)
- [~] Disband / transfer captain
- [ ] Ban / hide teams
- [ ] Team size cap

## Challenges
- [x] Static / standard challenges
- [x] Categories
- [x] Tags (modeled)
- [x] Dynamic-decay challenges
- [x] Hints — unlock endpoint, idempotent charge, 4 deduction modes; `HintCost::{Fixed, Script}` (rhai dynamic cost) ➕
- [x] Files / attachments — upload/download endpoints, pluggable `FileStorage` (local + rclone → S3/gdrive/etc.), sha256 checksum ➕
- [x] Prerequisites / next-unlock (`Requirement::Solve`) — enforced on submit, hint unlock, and challenge view
- [x] Hidden / visible / locked state (`ChallengeVisibility`; `Locked` uses per-field `LockedReveal`) ➕
- [x] Max attempts (+ enforcement) — `MaxAttempts` with `AttemptCountMode::{All, Unique, IncorrectOnly, UniqueIncorrect}`
- [ ] Challenge ordering
- [x] Per-team dynamic instancing + subdomain reverse proxy ➕ (beyond CTFd core)
- [x] Shared HTTP deployment (`ChallengeDeployment::Shared { url }` — one platform endpoint, no instancer)
- [x] Admin challenge CRUD (`AdminUser`-gated) + delete with optional solve-wipe
- [x] Player challenge view (`PublicChallenge` — flag & hint-content stripped)
- [x] Per-user (team-aware) solved status
- [x] Live decayed points in challenge view

## Flags & Scoring
- [x] Static flags (constant-time compare ➕)
- [x] Regex flags
- [x] Multiple flags, weighted partials ➕
- [x] Script / rhai flags ➕
- [x] Instanced (per-team) flags ➕
- [x] Static point value
- [x] Dynamic decay
- [~] Custom scoring equation (`equation` parsed as plain number; `PointAttribution` == `PointValue` today)
- [ ] **First blood** (NOT implemented; plan: replace numeric equation with rhai-evaluated scoring, reuse sandboxed engine)
- [ ] Case-insensitive flag toggle
- [ ] Awards / manual points / medals

## Submissions & Solves
- [x] Submit + correctness
- [x] Correct/incorrect tracking (`is_correct`)
- [x] Already-solved dedup
- [x] Team-consensus solves ➕
- [x] Rate limiting (IP + account)
- [x] IP audit logging
- [x] Max-attempt enforcement (see Challenges → Max attempts)

## Scoreboard
- [x] Standings / ranks
- [x] Tie-break (last-solve / accuracy)
- [x] Brackets on scoreboard
- [x] CTFtime export
- [x] Freeze (config-driven: `ScoreboardService.freeze_time` fed from `ConfigService`)
- [ ] Visibility toggle (public / private / hidden / admin-only)
- [ ] Score progression data endpoint (chart itself is Astro)

## Config / Settings
- [x] Typed config storage (`CtfConfig` JSONB singleton + repo + service)
- [x] Start / end / freeze times stored (`is_running` / `is_frozen` helpers)
- [x] Registration-open + require-email-verification toggles (stored)
- [ ] Admin config endpoints + authz surface
- [ ] Config import / export

## Notifications
- [ ] Announcements / broadcast
- [ ] Live delivery (SSE)
- [ ] Notification emails

## Email
- [x] SMTP send (hand-rolled STARTTLS + AUTH LOGIN) ➕
- [x] Dev SMTP catcher ➕
- [x] HTTP / Cloudflare-Worker inbound ingress ➕
- [x] Fluent-localized `EmailError` ➕
- [ ] Verification / reset / notification email flows (transport ready)
- [ ] Templated email bodies

## Admin (backend)
- [x] Admin authz (`AdminUser` extractor — role-gated, localized 403)
- [x] Challenge CRUD endpoints (`AdminUser`-gated create/update/delete)
- [ ] User / team management
- [ ] Submissions browse
- [ ] Statistics
- [ ] Event reset

## API
- [x] REST endpoints (auth, teams, scoreboard, challenges, submit, hints, files) served + integration-tested
- [x] OpenAPI spec — hand-written `openapi.yaml`, Fluent-localized at serve time (`/openapi.yaml`, `/openapi.json`, `/docs`), drift-guarded by `tests/openapi.rs` (openapi ≡ `API_ROUTES` ≡ router). Not annotation-generated (no `utoipa`) — a deliberate trade for a curated, localized spec.
- [ ] API / access tokens (also under Auth)

## Import / Export / Backup
- [x] CTFtime scoreboard export
- [ ] CTF import (zip)
- [ ] CTF export (zip)
- [ ] Backup / restore
- [ ] GitOps challenge loader (rCDS parity): minijinja description templating, incremental docker builds, registry metadata cache, obsolete-resource pruning

## Pages / CMS
- [?] **TBD — frontend is Astro (separate).** Static pages likely live in Astro. A thin backend
  Pages API (store markdown, serve via API) is only needed if runtime admin-editable pages are
  wanted. Decision pending.

## Platform / Security / Infra
- [x] Rate-limiting engine
- [x] Hand-rolled alg-confusion-safe HS256 JWT + constant-time verify ➕
- [x] Trusted-proxy IP handling ➕
- [x] Fluent i18n from day 1 ➕
- [ ] **Translation delivery to the Astro frontend** — server-side Fluent localizes API errors + the OpenAPI spec (via `Accept-Language`), but there is no channel for the frontend's own UI strings (nav, buttons, page copy). Decide: expose the Fluent bundles as a per-locale catalog endpoint so backend + frontend share one `locales/` source of truth, vs. Astro-native i18n. Prereq either way: locales beyond `en-US` (only `en-US` exists today).
- [x] Postgres storage (`PgStore`) + schema init
- [x] k8s instancer: pod/svc spawn, timed reaping, lifespan renewal ➕
- [x] lib + bin crate split (0 warnings), doctests + unit tests green; `api/` and `repos/pg/` split into per-domain modules
- [x] **`main.rs` server bootstrap** — axum serve, `.env`, `AppState`, merged HttpCatcher router, freeze from config
- [~] Docker deploy

---

## Beyond CTFd core (already ahead)
k8s per-team instancing + subdomain proxy · rhai flag/bracket scripting · weighted multi-flag
partials · team-consensus solves · Fluent i18n · alg-confusion-safe JWT + constant-time compare ·
SMTP catcher + Cloudflare HTTP email ingress.

## Notes
- **First blood**: not implemented. Plan = swap the numeric `equation` for rhai-evaluated scoring
  (reuse the sandboxed engine in `flags.rs`), enabling first-blood bonuses + arbitrary curves in one move.
- **Hints / files**: done — hints have an unlock endpoint, idempotent charging, 4 deduction modes, and dynamic rhai cost; files have upload/download endpoints over a pluggable `FileStorage` (local + subprocess rclone → S3/gdrive/etc.) with sha256 checksums.
- **Freeze**: done — `ScoreboardService.freeze_time` fed from `ConfigService` in `main.rs`.
- **Frontend**: separate Astro app — everything here is a headless API it consumes. UI-string translation delivery to it is still unbuilt (see Platform → Translation delivery).
- **Tests**: 24 unit + 9 doctests (always run) + rclone-gated (`RCLONE_TEST_REMOTE`) and Postgres-gated integration in `tests/pg.rs` & `tests/http.rs` (`#[ignore]`, need `TEST_DATABASE_URL` + fresh schema). Schema DDL is syntax-checked without a DB via `sqlparser`.
- **OpenAPI**: done — hand-written `openapi.yaml`, Fluent-localized at serve time, drift-guarded against the router + `API_ROUTES`. Deliberately not annotation-generated.
- **Next**: Notifications (announcements + solve/first-blood broadcast over SSE).
