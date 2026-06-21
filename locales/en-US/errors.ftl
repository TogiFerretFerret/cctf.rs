# ==========================================
# AUTHENTICATION & ACCOUNT ERRORS
# ==========================================
auth-invalid-credentials = Invalid username or password. Please try again.
auth-username-taken = This username is already registered.
auth-email-taken = This email address is already in use by another account.
auth-email-invalid = Please enter a valid email address (e.g. name@domain.com).
auth-team-full = This team has reached the maximum allowed member limit.
auth-team-name-taken = This team name is already registered by another team.
auth-not-logged-in = You must be authenticated to perform this action.
auth-invalid-invite = The invite code you provided is invalid or has expired.
auth-already-in-team = You are already a member of a team. Leave your current team first.
auth-banned = Your account has been suspended for violating the rules.

# ==========================================
# CHALLENGE SUBMISSION & PLAYPLAY ERRORS
# ==========================================
ctf-challenge-not-found = The requested challenge could not be found.
ctf-challenge-inactive = This challenge is currently hidden or disabled by the administrators.
ctf-challenge-locked = This challenge is locked. You must solve the prerequisites first.
ctf-incorrect-flag = Incorrect flag! Check for typos and try again.
ctf-already-solved = Your team has already submitted the correct flag for this challenge.
ctf-rate-limited = Too many attempts! Please wait {$seconds} seconds before submitting again.
ctf-hint-locked = This hint is locked. You need to unlock it first.
ctf-hint-insufficient-points = You do not have enough points to unlock this hint.

# ==========================================
# DYNAMIC CONTAINER INSTANCING ERRORS
# ==========================================
ctf-instance-failed = Failed to launch your challenge container instance. Please contact support.
ctf-instance-limit = You have reached the maximum number of concurrent active container instances.
ctf-instance-timeout = Your container instance has timed out and was destroyed. Please spawn a new one.
ctf-file-not-found = The requested challenge attachment file could not be found.

# ==========================================
# SCOREBOARD & TEAM MANAGEMENT ERRORS
# ==========================================
ctf-team-not-found = The specified team could not be found.
ctf-only-captain-action = Only the team captain is authorized to perform this action.
ctf-cannot-kick-captain = You cannot kick the team captain. Transfer captain status first.
ctf-team-not-empty = Cannot delete team while there are still members registered.

# ==========================================
# ADMIN & INFRASTRUCTURE ERRORS
# ==========================================
admin-unauthorized = You do not have administrator permissions to perform this action.
admin-invalid-points = The points configuration formula has a mathematical syntax error.
admin-challenge-failed-create = Failed to register the challenge in the system configuration database.
admin-user-not-found = The specified user account could not be found.
admin-db-write-failed = Database write transaction failed. Please check logs.
