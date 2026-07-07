use super::brackets::validate_bracket_join_rhai;
use super::*;
use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

#[derive(Deserialize)]
pub struct CreateInvitePayload {
    pub team_id: String,
    pub lifespan_hours: Option<i64>,
}

#[derive(Deserialize)]
pub struct JoinTeamPayload {
    pub token: String,
}

pub(crate) fn generate_invite_token(team_id: &str, expires_at: i64, secret: &[u8]) -> String {
    let message = format!("{}:{}", team_id, expires_at);
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    let signature = BASE64_URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    format!("{}:{}:{}", team_id, expires_at, signature)
}

pub(crate) fn verify_invite_token(token: &str, secret: &[u8]) -> Option<(String, i64)> {
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let team_id = parts[0];
    let expires_at_str = parts[1];
    let signature_base64 = parts[2];

    let expires_at = expires_at_str.parse::<i64>().ok()?;
    let now = chrono::Utc::now().timestamp();
    if now > expires_at {
        return None;
    }

    let message = format!("{}:{}", team_id, expires_at);
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(message.as_bytes());

    let provided_sig = BASE64_URL_SAFE_NO_PAD.decode(signature_base64).ok()?;
    if mac.verify_slice(&provided_sig).is_ok() {
        Some((team_id.to_string(), expires_at))
    } else {
        None
    }
}

pub async fn create_invite<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Json(payload): Json<CreateInvitePayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let team = match state
        .auth_service
        .team_repo
        .find_by_id(&TeamId(payload.team_id.clone()))
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            return LocalizedError {
                status: StatusCode::NOT_FOUND,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-team-not-found"),
            }
            .into_response();
        }
    };
    if team.captain_id != user.account_id {
        return LocalizedError {
            status: StatusCode::FORBIDDEN,
            message: LOCALES.lookup(&lang_id(&lang.0), "ctf-not-captain"),
        }
        .into_response();
    }
    let lifespan = payload.lifespan_hours.unwrap_or(24).clamp(1, 168);
    let expires_at = chrono::Utc::now().timestamp() + (lifespan * 3600);
    let token = generate_invite_token(&team.id.0, expires_at, &state.jwt_secret);
    Json(serde_json::json!({"token":token, "expires_at":expires_at})).into_response()
}

pub async fn join_team<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    user: AuthenticatedUser,
    lang: PreferredLang,
    Json(payload): Json<JoinTeamPayload>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let (team_id_str, _) = match verify_invite_token(&payload.token, &state.jwt_secret) {
        Some(val) => val,
        None => {
            return LocalizedError {
                status: StatusCode::BAD_REQUEST,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-invalid-invite-token"),
            }
            .into_response();
        }
    };
    let mut team = match state
        .auth_service
        .team_repo
        .find_by_id(&TeamId(team_id_str))
        .await
    {
        Ok(Some(t)) => t,
        _ => {
            return LocalizedError {
                status: StatusCode::NOT_FOUND,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-team-not-found"),
            }
            .into_response();
        }
    };
    let account = match state
        .auth_service
        .account_repo
        .find_by_id(&user.account_id)
        .await
    {
        Ok(Some(a)) => a,
        _ => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let scripts = state.bracket_acl_scripts.read().await;
    if let Some(script) = scripts.get(&team.bracket) {
        let email_str = account.email.as_ref().map(|e| e.0.as_str()).unwrap_or("");
        let username_str = account.username.0.as_str();
        let is_allowed = validate_bracket_join_rhai(email_str, username_str, script);
        if !is_allowed {
            return LocalizedError {
                status: StatusCode::FORBIDDEN,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-bracket-domain-restricted"),
            }
            .into_response();
        }
    }
    let mut updated_account = account;
    updated_account.team_id = Some(team.id.clone());
    if state
        .auth_service
        .account_repo
        .update(updated_account)
        .await
        .is_err()
    {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if !team.member_ids.contains(&user.account_id) {
        team.member_ids.push(user.account_id.clone());
        let _ = state.auth_service.team_repo.update(team).await;
    }
    StatusCode::OK.into_response()
}
