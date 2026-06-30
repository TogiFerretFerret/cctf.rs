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
