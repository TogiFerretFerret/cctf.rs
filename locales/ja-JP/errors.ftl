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
auth-not-logged-in = ログインしていません
auth-admin-required = この操作には管理者権限が必要です
auth-invalid-credentials = 認証情報が正しくありません
auth-username-taken = このユーザー名はすでに使われています
auth-hash-failed = パスワードのハッシュ化に失敗しました
auth-token-generation-failed = セッショントークンの生成に失敗しました
auth-oauth-token-failed = OAuthトークンの取得に失敗しました
auth-oauth-token-parse-failed = OAuthトークン応答の解析に失敗しました
auth-oauth-token-missing = 応答にOAuthトークンがありません
auth-oauth-profile-failed = OAuthユーザープロフィールの取得に失敗しました
auth-oauth-profile-parse-failed = OAuthユーザープロフィールの解析に失敗しました
oauth-invalid-credentials = OAuth認証情報が正しくありません: {$reason}

# API routing & extractor errors
auth-missing-header = Authorizationヘッダーがありません
auth-invalid-scheme = Authorizationスキームが正しくありません
auth-invalid-token = トークンが無効です: {$reason}

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

