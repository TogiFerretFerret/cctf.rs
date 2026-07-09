use super::*;

pub struct PreferredLang(pub String);

impl<S> FromRequestParts<S> for PreferredLang
where
    S: Send + Sync,
{
    type Rejection = StatusCode;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(PreferredLang(get_lang(&parts.headers)))
    }
}

pub struct LocalizedError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for LocalizedError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(serde_json::json!({"error":self.message}))).into_response()
    }
}

pub trait MapLocalized<T> {
    fn map_localized(self, lang: &str) -> Result<T, LocalizedError>;
}

impl<T> MapLocalized<T> for Result<T, ServiceError> {
    fn map_localized(self, lang: &str) -> Result<T, LocalizedError> {
        self.map_err(|e| {
            let status = match &e {
                ServiceError::Unauthorized => StatusCode::UNAUTHORIZED,
                ServiceError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
                ServiceError::OAuth(_) => StatusCode::BAD_REQUEST,
                ServiceError::Repo(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ServiceError::Kube(_) => StatusCode::INTERNAL_SERVER_ERROR,
                ServiceError::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            };
            LocalizedError {
                status,
                message: e.localize(lang),
            }
        })
    }
}

pub struct AuthenticatedUser {
    pub account_id: AccountId,
}

impl<A, T, C, S> FromRequestParts<AppState<A, T, C, S>> for AuthenticatedUser
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    type Rejection = LocalizedError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<A, T, C, S>,
    ) -> Result<Self, Self::Rejection> {
        let lang = get_lang(&parts.headers);
        let lang_id = lang
            .parse()
            .unwrap_or_else(|_| unic_langid::langid!("en-US"));
        let auth_header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: LOCALES.lookup(&lang_id, "auth-missing-header"),
            })?;
        if !auth_header.starts_with("Bearer ") {
            return Err(LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: LOCALES.lookup(&lang_id, "auth-invalid-scheme"),
            });
        }
        let token = &auth_header["Bearer ".len()..];
        let (_, claims) = crate::libs::crypto::jwt::decode::<crate::libs::crypto::jwt::Claims>(
            token,
            &state.jwt_secret,
        )
        .map_err(|e| LocalizedError {
            status: StatusCode::UNAUTHORIZED,
            message: {
                let args =
                    HashMap::from([(Cow::Borrowed("reason"), FluentValue::from(e.to_string()))]);
                LOCALES.lookup_with_args(&lang_id, "auth-invalid-token", &args)
            },
        })?;
        Ok(AuthenticatedUser {
            account_id: AccountId(claims.sub),
        })
    }
}

pub struct ClientIp(pub String);

impl<S> FromRequestParts<S> for ClientIp
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let trust_proxy = std::env::var("TRUST_PROXY_HEADERS")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if trust_proxy {
            if let Some(ip) = parts
                .headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next_back())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                return Ok(ClientIp(ip));
            }
            if let Some(ip) = parts
                .headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
            {
                return Ok(ClientIp(ip));
            }
        }

        if let Some(ConnectInfo(addr)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
            return Ok(ClientIp(addr.ip().to_string()));
        }

        Ok(ClientIp("127.0.0.1".to_string()))
    }
}

pub struct RateLimiter {
    requests: tokio::sync::Mutex<std::collections::HashMap<String, Vec<i64>>>,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            requests: tokio::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub async fn check_limit(&self, key: &str, limit: usize, window_secs: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        let mut map = self.requests.lock().await;
        let entry = map.entry(key.to_string()).or_insert_with(Vec::new);

        entry.retain(|&ts| now - ts < window_secs);

        if entry.len() >= limit {
            false
        } else {
            entry.push(now);
            true
        }
    }
}

pub struct AdminUser {
    pub account_id: AccountId,
}

impl<A, T, C, S> FromRequestParts<AppState<A, T, C, S>> for AdminUser
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    type Rejection = LocalizedError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<A, T, C, S>,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthenticatedUser::from_request_parts(parts, state).await?;
        let lang = get_lang(&parts.headers);
        let account = state
            .auth_service
            .account_repo
            .find_by_id(&user.account_id)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| LocalizedError {
                status: StatusCode::UNAUTHORIZED,
                message: ServiceError::Unauthorized.localize(&lang),
            })?;
        if !matches!(account.role, AccountRole::Admin) {
            return Err(LocalizedError {
                status: StatusCode::FORBIDDEN,
                message: LOCALES.lookup(&lang_id(&lang), "auth-admin-required"),
            });
        }
        Ok(AdminUser {
            account_id: user.account_id,
        })
    }
}

pub struct OptionalUser(pub Option<AccountId>);

impl<A, T, C, S> FromRequestParts<AppState<A, T, C, S>> for OptionalUser
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<A, T, C, S>,
    ) -> Result<Self, Self::Rejection> {
        Ok(OptionalUser(
            AuthenticatedUser::from_request_parts(parts, state)
                .await
                .ok()
                .map(|user| user.account_id),
        ))
    }
}
