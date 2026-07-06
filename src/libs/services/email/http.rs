use super::Email;
use super::server::{Mailbox, parse_email};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode, header},
    routing::post,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct HttpCatcherConfig {
    pub path: String,
    pub secret: Option<String>,
    pub max_body_bytes: usize,
}

impl Default for HttpCatcherConfig {
    fn default() -> Self {
        Self {
            path: "/api/v1/_inbound/email".to_string(),
            secret: None,
            max_body_bytes: 10 * 1024 * 1024,
        }
    }
}

pub struct HttpCatcher {
    mailbox: Mailbox,
    config: HttpCatcherConfig,
}

#[derive(Clone)]
struct HttpState {
    mailbox: Mailbox,
    secret: Option<Arc<String>>,
}

impl HttpCatcher {
    pub fn new(config: HttpCatcherConfig) -> Self {
        Self {
            mailbox: Arc::new(Mutex::new(Vec::new())),
            config,
        }
    }
    pub fn with_mailbox(mailbox: Mailbox, config: HttpCatcherConfig) -> Self {
        Self { mailbox, config }
    }
    pub fn mailbox(&self) -> Mailbox {
        self.mailbox.clone()
    }

    pub fn router(&self) -> Router {
        let state = HttpState {
            mailbox: self.mailbox.clone(),
            secret: self.config.secret.clone().map(Arc::new),
        };
        Router::new()
            .route(&self.config.path, post(inbound_handler))
            .layer(DefaultBodyLimit::max(self.config.max_body_bytes))
            .with_state(state)
    }

    pub async fn serve(self, addr: &str) -> std::io::Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router()).await
    }
}

#[derive(Deserialize)]
struct InboundJson {
    from: Option<String>,
    to: Option<String>,
    subject: Option<String>,
    text: Option<String>,
    html: Option<String>,
    raw: Option<String>,
}

async fn inbound_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    // Fail closed: with no configured secret the endpoint is disabled, not open.
    let Some(expected) = &state.secret else {
        return StatusCode::UNAUTHORIZED;
    };
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");
    if !constant_time_eq::constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return StatusCode::UNAUTHORIZED;
    }
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let email = if content_type.starts_with("application/json") {
        let Ok(j) = serde_json::from_slice::<InboundJson>(&body) else {
            return StatusCode::BAD_REQUEST;
        };
        match j.raw {
            Some(raw) => {
                let rcpts: Vec<String> = j.to.into_iter().collect();
                parse_email(j.from.unwrap_or_default(), &rcpts, &raw)
            }
            None => Email {
                id: uuid::Uuid::new_v4().to_string(),
                from: j.from.unwrap_or_default(),
                to: j.to.unwrap_or_default(),
                subject: j.subject.unwrap_or_default(),
                body: j.html.or(j.text).unwrap_or_default(),
                timestamp: chrono::Utc::now().timestamp(),
            },
        }
    } else {
        let Ok(raw) = std::str::from_utf8(&body) else {
            return StatusCode::BAD_REQUEST;
        };
        let envelope_from = header_str(&headers, "x-mail-from");
        let rcpts: Vec<String> = header_str(&headers, "x-mail-to")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        parse_email(envelope_from, &rcpts, raw)
    };
    // Bounded mailbox: keep only the most recent entries so it can't grow forever.
    const MAX_MAILBOX: usize = 1000;
    {
        let mut mb = state.mailbox.lock().await;
        mb.push(email);
        if mb.len() > MAX_MAILBOX {
            let excess = mb.len() - MAX_MAILBOX;
            mb.drain(0..excess);
        }
    }
    StatusCode::OK
}

fn header_str(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string()
}
