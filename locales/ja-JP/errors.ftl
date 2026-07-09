# Database & server errors
server-db-connection-failed = データベース接続に失敗しました
admin-db-internal-error = データベース内部エラー: {$reason}

# Challenge & submission errors
ctf-challenge-not-found = チャレンジが見つかりません
ctf-already-solved = このチャレンジはすでに解かれています
ctf-hint-not-found = ヒントが見つかりません
ctf-insufficient-points = このヒントを解放するにはポイントが足りません
ctf-challenge-locked = このチャレンジはロックされています
ctf-max-attempts-reached = 挑戦回数の上限に達しました
ctf-requirements-not-met = 先に前提となるチャレンジを解く必要があります
ctf-incorrect-flag = フラグが正しくありません
ctf-instance-expired-or-not-found = チャレンジインスタンスが見つからないか、期限切れです
ctf-rate-limit-exceeded = リクエストが多すぎます。しばらくしてからもう一度お試しください。

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
ctf-file-too-large = File exceeds the maximum upload size
ctf-file-not-found = File not found
ctf-file-invalid-id = Invalid file id
ctf-file-missing = No file provided in the upload

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

