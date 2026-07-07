use super::*;

fn extract_instance_id(host: &str) -> Option<String> {
    let first_part = host.split('.').next()?;
    if first_part.starts_with("inst-") {
        Some(first_part.to_string())
    } else {
        None
    }
}

pub async fn proxy_handler<A, T, C, S>(
    State(state): State<AppState<A, T, C, S>>,
    req: Request,
) -> Result<Response, StatusCode>
where
    A: AccountRepo + Send + Sync + 'static,
    T: TeamRepo + Send + Sync + 'static,
    C: ChallengeRepo + Send + Sync + 'static,
    S: SubmissionRepo + Send + Sync + 'static,
{
    let host = req
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let instance_id = match extract_instance_id(host) {
        Some(id) => id,
        None => return Err(StatusCode::NOT_FOUND),
    };
    let cluster_ip = state
        .solve_service
        .challenge_repo
        .get_instance_ip(&instance_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("");
    let target_url = format!("http://{}{}", cluster_ip, path_and_query);
    let method = req.method().clone();
    let mut headers = req.headers().clone();
    for h in [
        "authorization",
        "cookie",
        "connection",
        "keep-alive",
        "proxy-authorization",
        "proxy-authenticate",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(h);
    }
    let body = req.into_body();
    let bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let reqwest_body = reqwest::Body::from(bytes);
    let res = state
        .http_client
        .request(method, &target_url)
        .headers(headers)
        .body(reqwest_body)
        .send()
        .await
        .map_err(|e| {
            eprintln!("Proxy gateway error: {:?}", e);
            StatusCode::BAD_GATEWAY
        })?;
    let mut response_builder = Response::builder().status(res.status());
    if let Some(headers_mut) = response_builder.headers_mut() {
        for (key, value) in res.headers() {
            headers_mut.insert(key, value.clone());
        }
    }
    let response_stream = res.bytes_stream();
    let body = axum::body::Body::from_stream(response_stream);
    let response = response_builder
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(response)
}
