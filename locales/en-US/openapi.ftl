# OpenAPI spec — human-readable strings (resolved at serve time by localize_spec).
# Keys with no whitespace in openapi.yaml are looked up here; anything else stays literal.

api-title = cctf.rs API
api-description = OpenAPI contract for cctf.rs. Schemas mirror serde encoding: newtype wrappers serialize as their inner value, and Rust enums are externally tagged — unit variants as bare strings, data variants as an object keyed by the variant name.
server-local-desc = Local dev

# Tags
tag-auth = Authentication and sessions
tag-challenges = Challenges
tag-scoreboard = Scoreboard
tag-teams = Teams
tag-notifications = Notifications and announcements

# Operations
op-register-summary = Register a local account
op-register-desc = Rate limited to 5 requests / 60s per IP.
op-login-summary = Log in, returning a JWT
op-login-desc = Rate limited to 5 requests / 60s per IP.
op-oauth-url-summary = Get the CTFtime OAuth authorize URL
op-oauth-callback-summary = CTFtime OAuth callback, exchanges code for a JWT
op-challenges-list-summary = List challenges (player view; flags stripped)
op-challenge-create-summary = Create a challenge (admin only)
op-challenge-get-summary = Get one challenge (player view; flag stripped)
op-challenge-update-summary = Update a challenge (admin only)
op-challenge-delete-summary = Delete a challenge (admin only)
op-challenge-delete-desc = Instances are always removed; solves only when delete_solves=true.
op-submit-summary = Submit a flag
op-submit-desc = Rate limited to 10 / 60s per IP and per account. Team is derived server-side.
op-unlock-hint-summary = Unlock a hint
op-upload-file-summary = Upload a challenge file
op-upload-file-desc = Admin only. Multipart form-data with a single "file" field; returns a ChallengeFile ready to drop into a challenge's files list.
op-download-file-summary = Download a file
op-download-file-desc = Streams the stored file with its original name and content type.
op-unlock-hint-desc = Unlock hint by index. Charges the evaluated cost once (idempotent); the cost is deducted from the team's score when hint deduction is enabled.
op-scoreboard-summary = Get standings
op-scoreboard-export-summary = CTFtime-format scoreboard export
op-teams-invite-summary = Mint a team invite token (captain only)
op-teams-join-summary = Join a team via invite token

# Parameters
param-code-desc = OAuth authorization code
param-challenge-id-desc = Challenge id
param-delete-solves-desc = Also delete solve records for this challenge
param-bracket-desc = Filter standings to a bracket
param-hint-index-desc = Zero-based hint index
param-file-id-desc = Stored file id

# Responses
resp-account-created = Account created
resp-token = Session token
resp-authorize-url = Authorize URL
resp-challenge-list = Player-safe challenge list
resp-challenge-created = Created challenge (full, admin view)
resp-challenge = Player-safe challenge
resp-challenge-updated = Updated challenge
resp-deleted = Deleted
resp-submission = Accepted (correct) submission
resp-hint-unlock = Revealed hint content and the charged cost
resp-file-uploaded = Stored file, as a ChallengeFile reference
resp-file-download = The file's bytes (original content type)
resp-standings = Ranked standings
resp-ctftime-export = CTFtime export
resp-invite = Invite token
resp-joined = Joined
resp-error = Localized error

# Schema descriptions
schema-error-field = Localized (Accept-Language) message
schema-account = password_hash is never serialized.
schema-public-challenge = Player-facing view. Never includes the flag or hint content.
schema-rendered-html = Rendered HTML
schema-points-live = Current (live-decayed) value
schema-challenge = Full challenge (admin create/update body and admin response). Includes the flag.
schema-hint-content = Rendered HTML (admin view only)
schema-lifespan = Clamped between 1 and 168
schema-requirement = Externally tagged enum. Only variant: Solve(challenge_id).
schema-scoring-mode = Externally tagged enum: the bare strings "PointValue" or "PointAttribution", or a "DynamicDecay" object with initial, minimum and decay.
schema-flag-validator = Externally tagged enum: a "Static", "Regex" or "Script" object each wrapping a string, the bare string "Instanced", or a "Multi" object wrapping an array of PartialFlag.
schema-deployment = Externally tagged enum: the bare string "None" or "Instanced", or a "Shared" object with a url.
schema-hint-cost = Externally tagged enum: a "Fixed" object wrapping an integer, or a "Script" object wrapping a rhai expression evaluated with `solves` and `now` in scope.
schema-visibility = Externally tagged enum: the bare string "Visible" or "Hidden", or a "Locked" object wrapping a LockedReveal that controls which fields leak while locked.
schema-locked-reveal = Per-field toggles controlling exactly what a Locked challenge exposes to non-admins.
schema-challenge-locked = True when the viewer sees this challenge in its locked (sealed) form; hidden fields are blanked.
schema-attempt-count-mode = How submissions count toward the attempt limit: All, Unique (dedup identical flags), IncorrectOnly, or UniqueIncorrect.
schema-max-attempts = Optional per-challenge submission cap with a counting mode.
