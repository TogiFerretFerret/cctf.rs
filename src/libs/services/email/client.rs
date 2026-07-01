use super::{EmailError, EmailService};
use async_trait::async_trait;
use base64::{Engine as _, prelude::BASE64_STANDARD};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{self, RootCertStore, pki_types::ServerName},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    None, 
    StartTls,
}

#[derive(Clone)]
pub struct SmtpCredentials {
    pub username: String,
    pub password: String,
}

pub struct SmtpSenderClient {
    pub smtp_host: String,
    pub smtp_port: u16, 
    pub from_email: String,
    pub ehlo_name: String,
    pub tls_mode: TlsMode,
    pub credentials: Option<SmtpCredentials>,
}

impl SmtpSenderClient {
    pub fn new(smtp_host: String, smtp_port: u16, from_email: String) -> Self {
        Self {
            smtp_host,
            smtp_port,
            from_email,
            ehlo_name: "localhost".to_string(),
            tls_mode: TlsMode::None,
            credentials: None,
        }
    }
    pub fn with_starttls(mut self) -> Self {
        self.tls_mode = TlsMode::StartTls;
        self
    }
    pub fn with_credentials(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.credentials = Some(SmtpCredentials {
            username: username.into(),
            password: password.into(),
        });
        self
    }
    pub fn with_ehlo_name(mut self, name: impl Into<String>) -> Self {
        self.ehlo_name = name.into();
        self
    }
}

enum SmtpStream {
    Plain(BufReader<TcpStream>),
    Tls(BufReader<TlsStream<TcpStream>>),
}

impl AsyncRead for SmtpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            SmtpStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
            SmtpStream::Tls(s) => Pin::new(s).poll_read(cx, buf)
        }
    }
}

impl AsyncBufRead for SmtpStream {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<&[u8]>> {
        match self.get_mut() {
            SmtpStream::Plain(s) => Pin::new(s).poll_fill_buf(cx),
            SmtpStream::Tls(s) => Pin::new(s).poll_fill_buf(cx),
        }
    }
    fn consume(self: Pin<&mut Self>, amt: usize) {
        match self.get_mut() {
            SmtpStream::Plain(s) => Pin::new(s).consume(amt),
            SmtpStream::Tls(s) => Pin::new(s).consume(amt),
        }
    }
}

impl AsyncWrite for SmtpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            SmtpStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
            SmtpStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
        }
    }
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>>{
        match self.get_mut() {
            SmtpStream::Plain(s) => Pin::new(s).poll_flush(cx),
            SmtpStream::Tls(s) => Pin::new(s).poll_flush(cx),
        }
    }
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            SmtpStream::Plain(s) => Pin::new(s).poll_shutdown(cx),
            SmtpStream::Tls(s) => Pin::new(s).poll_shutdown(cx),
        }
    }
}

impl SmtpStream {
    async fn read_reply(&mut self) -> Result<(u16, Vec<String>), EmailError> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let n = self
                .read_line(&mut line)
                .await
                .map_err(|e| EmailError::Io(e.to_string()))?;
            if n == 0 {
                return Err(EmailError::UnexpectedEof);
            }
            let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
            if trimmed.len() < 3 {
                return Err(EmailError::InvalidResponse);
            }
            let code: u16 = trimmed[..3].parse().map_err(|_| EmailError::InvalidResponse)?;
            let is_last = trimmed.len() == 3 || trimmed.as_bytes()[3] == b' ';
            lines.push(trimmed);
            if is_last {
                return Ok((code, lines));
            }
        }
    }
    async fn send_line(&mut self, line: &str) -> Result<(), EmailError> {
        self.write_all(line.as_bytes())
            .await
            .map_err(|e| EmailError::Io(e.to_string()))?;
        self.write_all(b"\r\n")
            .await
            .map_err(|e| EmailError::Io(e.to_string()))?;
        self.flush().await.map_err(|e| EmailError::Io(e.to_string()))?;
        Ok(())
    }
    async fn command(
        &mut self, 
        line: &str,
        expected: u16,
        phase: &str,
    ) -> Result<Vec<String>, EmailError> {
        self.send_line(line).await?;
        let (code, lines) = self.read_reply().await?;
        if code != expected {
            return Err(EmailError::Rejected {
                code, 
                phase: phase.to_string(),
            });
        }
        Ok(lines)
    }
    async fn upgrade(self, host: &str) -> Result<SmtpStream, EmailError> {
        let tcp = match self {
            SmtpStream::Plain(buf) => buf.into_inner(),
            SmtpStream::Tls(_) => {
                return Err(EmailError::Tls("connection already secured".to_string())); // TODO: localize 
            }
        };
        let connector = build_tls_connector()?;
        let server_name = ServerName::try_from(host.to_string())
            .map_err(|e| EmailError::InvalidServerName(e.to_string()))?;
        let tls = connector
            .connect(server_name, tcp)
            .await
            .map_err(|e| EmailError::Tls(e.to_string()))?;
        Ok(SmtpStream::Tls(BufReader::new(tls)))
    }
}

fn build_tls_connector() -> Result<TlsConnector, EmailError> {
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(|e| EmailError::Tls(e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

fn ehlo_supports(caps: &[String], token: &str) -> bool {
    caps.iter().any(|l| {
        let text = if l.len() > 4 { &l[4..] } else { "" };
        text.split_whitespace()
            .next()
            .map(|w| w.eq_ignore_ascii_case(token))
            .unwrap_or(false)
        })
}

fn ehlo_supports_auth_login(caps: &[String]) -> bool {
    caps.iter().any(|l| {
        let text = if l.len() > 4 { &l[4..] } else { "" };
        let mut parts = text.split_whitespace();
        matches!(parts.next(), Some(w) if w.eq_ignore_ascii_case("AUTH"))
            && parts.any(|m| m.eq_ignore_ascii_case("LOGIN"))
    })
}

fn header_safe(value: &str) -> String {
    value.chars().filter(|&c| c != '\r' && c != '\n').collect()
}

fn build_message(from: &str, to: &str, subject: &str, body: &str) -> String {
    let date = chrono::Utc::now().to_rfc2822();
    let domain = match from.rsplit_once('@') {
        Some((_, d)) if !d.is_empty() => d,
        _ => "localhost",
    };
    let message_id = format!("<{}@{}>", uuid::Uuid::new_v4(), domain);
    let mut out = String::new();
    out.push_str(&format!("From: {}\r\n", header_safe(from)));
    out.push_str(&format!("To: {}\r\n", header_safe(to)));
    out.push_str(&format!("Subject: {}\r\n", header_safe(subject)));
    out.push_str(&format!("Date: {}\r\n", date));
    out.push_str(&format!("Message-ID: {}\r\n", message_id));
    out.push_str("MIME-Version: 1.0\r\n");
    out.push_str("Content-Type: text/plain; charset=utf-8\r\n");
    out.push_str("\r\n");
    for raw_line in body.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.starts_with('.') {
            out.push('.'); // rfc 5321 4.5.2: dot-stuffing
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out
}

#[async_trait]
impl EmailService for SmtpSenderClient {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError> {
        if self.credentials.is_some() && self.tls_mode == TlsMode::None {
            return Err(EmailError::AuthRequiresTls);
        }
        let tcp = TcpStream::connect((self.smtp_host.as_str(), self.smtp_port))
            .await
            .map_err(|e| EmailError::Connect(e.to_string()))?;
        let mut stream = SmtpStream::Plain(BufReader::new(tcp));
        let (code, _) = stream.read_reply().await?;
        if code != 220 {
            return Err(EmailError::Rejected { code, phase: "greeting".to_string() });
        }
        let ehlo = format!("EHLO {}", self.ehlo_name);
        let mut caps = stream.command(&ehlo, 250, "EHLO").await?;
        if self.tls_mode == TlsMode::StartTls {
            if !ehlo_supports(&caps, "STARTTLS") {
                return Err(EmailError::StartTlsUnsupported);
            }
            stream.command("STARTTLS", 220, "STARTTLS").await?;
            stream = stream.upgrade(&self.smtp_host).await?;
            caps = stream.command(&ehlo, 250, "EHLO").await?;
        }
        if let Some(creds) = &self.credentials {
            if !ehlo_supports_auth_login(&caps) {
                return Err(EmailError::AuthUnsupported);
            }
            stream.command("AUTH LOGIN", 334, "AUTH").await?;
            stream
                .command(&BASE64_STANDARD.encode(creds.username.as_bytes()), 334, "AUTH username")
                .await?;
            stream
                .send_line(&BASE64_STANDARD.encode(creds.password.as_bytes()))
                .await?;
            let (code, _) = stream.read_reply().await?;
            if code != 235 {
                return Err(EmailError::AuthFailed);
            }
        }
        stream
            .command(&format!("MAIL FROM:<{}>", self.from_email), 250, "MAIL FROM")
            .await?;
        stream
            .command(&format!("RCPT TO:<{}>", to), 250, "RCPT TO")
            .await?;
        stream.command("DATA", 354, "DATA").await?;
        let message = build_message(&self.from_email, to, subject, body);
        stream
            .write_all(message.as_bytes())
            .await
            .map_err(|e| EmailError::Io(e.to_string()))?;
        if !message.ends_with("\r\n") {
            stream.write_all(b"\r\n").await.map_err(|e| EmailError::Io(e.to_string()))?;
        }
        stream.write_all(b".\r\n").await.map_err(|e| EmailError::Io(e.to_string()))?;
        stream.flush().await.map_err(|e| EmailError::Io(e.to_string()))?;
        let (code, _) = stream.read_reply().await?;
        if code != 250 {
            return Err(EmailError::Rejected { code, phase: "message".to_string() });
        }
        let _ = stream.command("QUIT", 221, "QUIT").await;
        Ok(())
    }
}

