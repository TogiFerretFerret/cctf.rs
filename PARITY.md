# cctf.rs — CTFd Feature Parity Tracker

Goal: **minimum of full feature parity** with CTFd (idkCTF/rCTF flavor), then exceed it.

**Architecture note:** the frontend is a **separate Astro app** (kept out of this repo for
modularity), so `cctf.rs` is a **headless API/platform**. This tracker covers backend/platform
capability only — theme/UI/rendering is Astro's job.

Legend: `[x]` done · `[~]` partial · `[ ]` todo · **➕** = already beyond CTFd core
Last reconciled: 2026-07-01

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
- [~] Hints (modeled `ChallengeHint{content,cost}`; unlock/deduction not wired)
- [~] Files / attachments (modeled `ChallengeFile{name,url,checksum}`; no upload/storage)
- [~] Prerequisites / next-unlock (modeled `Requirement::Solve`; enforcement not wired)
- [ ] Hidden / visible state
- [ ] Max attempts (+ enforcement)
- [ ] Challenge ordering
- [x] Per-team dynamic instancing + subdomain reverse proxy ➕ (beyond CTFd core)

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
- [ ] Max-attempt enforcement

## Scoreboard
- [x] Standings / ranks
- [x] Tie-break (last-solve / accuracy)
- [x] Brackets on scoreboard
- [x] CTFtime export
- [~] Freeze (logic done in `ScoreboardService.freeze_time`; wire from config at Phase 0)
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
- [ ] Admin authz middleware (role exists; no gate)
- [ ] Challenge CRUD endpoints (repo `save` exists; no route/authz)
- [ ] User / team management
- [ ] Submissions browse
- [ ] Statistics
- [ ] Event reset

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
- [x] Postgres storage (`PgStore`) + schema init
- [x] k8s instancer: pod/svc spawn, timed reaping, lifespan renewal ➕
- [x] lib + bin crate split (0 warnings), doctests + unit tests green
- [ ] **`main.rs` server bootstrap (KEYSTONE)** — axum serve, `.env` config, `AppState`, merge HttpCatcher router, wire freeze from config, admin authz
- [~] Docker deploy

---

## Beyond CTFd core (already ahead)
k8s per-team instancing + subdomain proxy · rhai flag/bracket scripting · weighted multi-flag
partials · team-consensus solves · Fluent i18n · alg-confusion-safe JWT + constant-time compare ·
SMTP catcher + Cloudflare HTTP email ingress.

## Notes
- **First blood**: not implemented. Plan = swap the numeric `equation` for rhai-evaluated scoring
  (reuse the sandboxed engine in `flags.rs`), enabling first-blood bonuses + arbitrary curves in one move.
- **Hints / files**: data models exist; unlock/deduction logic, upload/storage, and endpoints are unbuilt.
- **Freeze**: mechanism done; feed `freeze_time` from `ConfigService` at `ScoreboardService` construction (Phase 0).
- **Frontend**: separate Astro app — everything here is a headless API it consumes.
