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
}

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
        phase; &str,
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
    async fn upgrade(self, hsot: &str) -> Result<SmtpStream, EmailError> {
        let tcp = match self {
            SmtpStream::Plain(buf) => buf.into_inner(),
            SmtpStream::Tls(_) => {
                return Err(EmailError::Tls("connection already secured".to_string())); // TODO: localize 
