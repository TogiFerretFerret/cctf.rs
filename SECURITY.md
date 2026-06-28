# Security notes — cctf.rs

The mindset for a CTF platform: **every player is hostile, and the prize is the
flag/scoreboard.** Almost every real bug here is "correct code that trusts a
value the client controls." When adding a feature, check it against the
**PR checklist** at the bottom.

## Fixed (done in the security pass)

| # | Issue | Sev | Fix |
|---|-------|-----|-----|
| 1 | `team_id` taken from request body on flag submit → anyone could credit any team (scoreboard forgery / IDOR) | 🔴 | `SubmitFlagPayload.team_id` removed; `submit_flag` derives team from the authenticated account (`api.rs`) |
| 2 | Passwords hashed with single-round salted SHA-256 (GPU-crackable on DB leak) | 🔴 | Argon2id PHC strings (`auth.rs`) |
| 3 | rhai flag/bracket scripts ran with no limits → one `loop{}` pins a worker forever | 🟠 | `set_max_operations`/levels/sizes (`flags.rs` `sandboxed_engine`, `api.rs`) |
| 4 | JWTs never expired (no `exp`); leaked token = permanent compromise | 🟠 | `jwt::Claims` + `jwt::issue()` (24h TTL) + `exp`/`nbf` check in `jwt::decode` |
| 5 | Rate limits bypassable via spoofed `X-Forwarded-For`; stored IP attacker-controlled | 🟠 | `ClientIp` only trusts fwd headers when `TRUST_PROXY_HEADERS=1`, else socket peer; takes last hop |
| 6 | Flag/password compares short-circuited (timing oracle) | 🟡 | `constant_time_eq` for flags; argon2 verify is constant-time |
| 7 | Panic-DoS: `Accept-Language` `.parse().unwrap()` | 🟡 | `lang_id()` helper with en-US fallback |
| 8 | Proxy forwarded player's `Authorization`/`Cookie` to challenge containers (cred theft) | 🟡 | proxy strips auth + hop-by-hop headers (`proxy_handler`) |
| 9 | Invite lifespan unbounded | 🟢 | clamped to 1h..1week (`create_invite`) |

## Still open (need schema / larger work — do these together)

- **Double-solve race (TOCTOU)** — `solve.rs` does check-then-insert; two concurrent
  correct submits can both pass the dup check → double points (worse with dynamic
  decay). Fix: DB `UNIQUE` constraint on correct solves per `(challenge_id, team_id)`
  + do the read+insert in one transaction.
- **`find_all()` per submission** — `solve.rs` loads the whole submissions table
  (sometimes twice) every submit. Add `count_solves`/`has_solved` repo methods backed
  by `SELECT COUNT/EXISTS`. (Touches the repo trait + pg.rs + test mocks.)
- **OAuth has no `state` param** — `auth.rs` CTFtime flow is CSRF-able. Add a signed
  `state` nonce, verify on callback.
- **`main.rs` is a stub** — when wiring the real server: load `jwt_secret` from env
  (≥32 random bytes, fail closed if unset — never hardcode/commit it), and add
  `ConnectInfo` (`into_make_service_with_connect_info::<SocketAddr>`) so `ClientIp`
  has a real peer to fall back to.
- **Admin authz** — `AccountRole::Admin` exists but nothing enforces it. Before adding
  challenge CRUD / rhai-script upload endpoints, build a role-aware extractor
  (`RequireAdmin`) so admin-only routes are gated at the type level.

## PR checklist (run this in your head on every change)

1. Does this trust a value from the request body/headers for **identity or
   authorization**? (→ derive it server-side from the JWT instead.)
2. Does it compare a **secret** with `==`? (→ constant-time.)
3. Does it run **rhai / regex / unbounded work** on the request path? (→ cap it.)
4. Does it `.unwrap()`/`panic` on **attacker-controlled input**? (→ fallback.)
5. Does it **check-then-write** shared state? (→ DB constraint / transaction.)
6. Does a new endpoint need a **role** or **ownership** check? (→ add it explicitly.)
