use super::*;

pub async fn upload_file<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    _admin: AdminUser,
    lang: PreferredLang,
    mut multipart: Multipart,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let mut uploaded: Option<(String, String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "file".to_string());
        let content_type = field
            .content_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        if let Ok(b) = field.bytes().await {
            uploaded = Some((name, content_type, b.to_vec()));
            break;
        }
    }
    let (name, content_type, bytes) = match uploaded {
        Some(v) => v,
        None => {
            return LocalizedError {
                status: StatusCode::BAD_REQUEST,
                message: LOCALES.lookup(&lang_id(&lang.0), "ctf-file-missing"),
            }
            .into_response();
        }
    };
    let now = chrono::Utc::now().timestamp();
    match state
        .file_service
        .upload(&name, &content_type, &bytes, now)
        .await
        .map_localized(&lang.0)
    {
        Ok(cf) => Json(cf).into_response(),
        Err(e) => e.into_response(),
    }
}

pub async fn download_file<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    _user: AuthenticatedUser,
    lang: PreferredLang,
    Path(id): Path<String>,
) -> axum::response::Response
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    match state
        .file_service
        .download(&id)
        .await
        .map_localized(&lang.0)
    {
        Ok((meta, bytes)) => {
            let safe_name = meta.name.replace(['"', '\\', '\r', '\n'], "_");
            let mut headers = axum::http::HeaderMap::new();
            if let Ok(ct) = axum::http::HeaderValue::from_str(&meta.content_type) {
                headers.insert(axum::http::header::CONTENT_TYPE, ct);
            }
            if let Ok(cd) =
                axum::http::HeaderValue::from_str(&format!("attachment;filename=\"{safe_name}\""))
            {
                headers.insert(axum::http::header::CONTENT_DISPOSITION, cd);
            }
            (headers, bytes).into_response()
        }
        Err(e) => e.into_response(),
    }
}
