# Database & server errors
server-db-connection-failed = Database connection failed
admin-db-internal-error = Database internal error: {$reason}

# Challenge & submission errors
ctf-challenge-not-found = Challenge not found
ctf-already-solved = Challenge has already been solved
ctf-hint-not-found = Hint not found
ctf-insufficient-points = Not enough points to unlock this hint
ctf-challenge-locked = This challenge is locked
ctf-max-attempts-reached = No attempts remaining for this challenge
ctf-incorrect-flag = Incorrect flag submitted
ctf-instance-expired-or-not-found = Challenge instance not found or expired
ctf-rate-limit-exceeded = Rate limit exceeded. Please try again later.

# Auth & OAuth errors
auth-not-logged-in = User is not logged in
auth-admin-required = This action requires administrator privileges
auth-invalid-credentials = Invalid credentials
auth-username-taken = Username is already taken
auth-hash-failed = Failed to hash password
auth-token-generation-failed = Failed to generate session token
auth-oauth-token-failed = Failed to retrieve OAuth token
auth-oauth-token-parse-failed = Failed to parse OAuth token response
auth-oauth-token-missing = OAuth token is missing in the response
auth-oauth-profile-failed = Failed to retrieve OAuth user profile
auth-oauth-profile-parse-failed = Failed to parse OAuth user profile
oauth-invalid-credentials = Invalid OAuth credentials: {$reason}

# API routing & extractor errors
auth-missing-header = Missing Authorization header
auth-invalid-scheme = Invalid Authorization scheme
auth-invalid-token = Invalid token: {$reason}

# JWT errors
jwt-invalid-format = Malformed token
jwt-invalid-signature = Invalid token signature
jwt-invalid-signature-secret-key = Invalid signing key
jwt-token-expired = Token has expired
jwt-not-yet-valid = Token is not yet valid
jwt-invalid-json = Invalid token payload: {$reason}
jwt-base64-error = Invalid token encoding: {$reason}

# Kubernetes errors
kube-api-error = Kubernetes operation failed: {$reason}

# Team & invite errors
ctf-team-not-found = Team not found
ctf-not-captain = Only the captain can generate invite tokens
ctf-invalid-invite-token = Invalid or expired invite token
ctf-bracket-domain-restricted = Your email domain is not permitted to join this division

# Email / SMTP errors
email-connect-failed = Could not connect to the mail server: {$reason}
email-io-error = Mail server connection error: {$reason}
email-tls-failed = TLS negotiation with the mail server failed: {$reason}
email-invalid-server-name = Invalid mail server name for TLS: {$reason}
email-already-secured = The connection is already secured
email-unexpected-eof = The mail server closed the connection unexpectedly
email-invalid-response = The mail server sent a malformed response
email-command-rejected = The mail server rejected the { $phase } command (code { $code })
email-starttls-unsupported = The mail server does not advertise STARTTLS
email-auth-unsupported = The mail server does not support AUTH LOGIN
email-auth-failed = The mail server rejected the supplied credentials
email-auth-requires-tls = Refusing to send SMTP credentials over an unencrypted connection
email-message-too-large = The message exceeds the maximum allowed size
