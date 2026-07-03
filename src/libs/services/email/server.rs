use super::{Email, EmailError};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

const MAX_LINE_BYTES: usize = 64 * 1024;
const MAX_MESSAGE_BYTES: usize = 10 * 1024 * 1024;

pub type Mailbox = Arc<Mutex<Vec<Email>>>;
pub struct SmtpCatcherServer {
    listener: TcpListener,
    hostname: String,
    mailbox: Mailbox,
}

impl SmtpCatcherServer {
    pub async fn bind(addr: &str) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            listener,
            hostname: "cctf-catcher".to_string(),
            mailbox: Arc::new(Mutex::new(Vec::new())),
        })
    }
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn mailbox(&self) -> Mailbox {
        self.mailbox.clone()
    }

    pub async fn serve(self) -> std::io::Result<()> {
        loop {
            let (socket, _peer) = self.listener.accept().await?;
            let mailbox = self.mailbox.clone();
            let hostname = self.hostname.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, hostname, mailbox).await {
                    eprintln!("smtp-catcher: conn err: {}", e.localize("en-US"));
                }
            });
        }
    }
}

async fn write_line<W: AsyncWriteExt + Unpin>(w: &mut W, line: &str) -> Result<(), EmailError> {
    w.write_all(line.as_bytes()).await?;
    w.write_all(b"\r\n").await?;
    w.flush().await?;
    Ok(())
}

async fn read_command<R: AsyncBufReadExt + Unpin>(
    r: &mut R,
    buf: &mut String,
) -> Result<usize, EmailError> {
    buf.clear();
    let n = r.read_line(buf).await?;
    if buf.len() > MAX_LINE_BYTES {
        return Err(EmailError::MessageTooLarge);
    }
    Ok(n)
}

async fn read_data<R: AsyncBufReadExt + Unpin>(r: &mut R) -> Result<String, EmailError> {
    let mut data = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let n = r.read_line(&mut line).await?;
        if n == 0 {
            return Err(EmailError::UnexpectedEof);
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "." {
            break;
        }
        let unstuffed = trimmed.strip_prefix('.').unwrap_or(trimmed);
        if data.len() + unstuffed.len() + 1 > MAX_MESSAGE_BYTES {
            return Err(EmailError::MessageTooLarge);
        }
        data.push_str(unstuffed);
        data.push('\n');
    }
    Ok(data)
}

fn split_verb(line: &str) -> (String, &str) {
    let line = line.trim_start();
    match line.find(char::is_whitespace) {
        Some(i) => (line[..i].to_uppercase(), line[i + 1..].trim_start()),
        None => (line.to_uppercase(), ""),
    }
}

fn parse_addr(rest: &str, prefix: &str) -> Option<String> {
    let rest = rest.trim_start();
    if rest.len() < prefix.len() || !rest[..prefix.len()].eq_ignore_ascii_case(prefix) {
        return None;
    }
    let after = rest[prefix.len()..].trim();
    let inside = after
        .strip_prefix('<')
        .and_then(|s| s.split('>').next())
        .unwrap_or(after);
    Some(inside.trim().to_string())
}

fn header_value(block: &str, name: &str) -> Option<String> {
    for line in block.lines() {
        if let Some(colon) = line.find(':')
            && line[..colon].trim().eq_ignore_ascii_case(name)
        {
            return Some(line[colon + 1..].trim().to_string());
        }
    }
    None
}

pub(super) fn parse_email(envelope_from: String, rcpts: &[String], raw: &str) -> Email {
    let (header_block, body) = raw
        .split_once("\r\n\r\n")
        .or_else(|| raw.split_once("\n\n"))
        .map(|(h, b)| (h, b.to_string()))
        .unwrap_or(("", raw.to_string()));
    Email {
        id: uuid::Uuid::new_v4().to_string(),
        from: header_value(header_block, "From").unwrap_or(envelope_from),
        to: header_value(header_block, "To").unwrap_or_else(|| rcpts.join(", ")),
        subject: header_value(header_block, "Subject").unwrap_or_default(),
        body,
        timestamp: chrono::Utc::now().timestamp(),
    }
}

async fn handle_connection(
    socket: TcpStream,
    hostname: String,
    mailbox: Mailbox,
) -> Result<(), EmailError> {
    let mut stream = BufReader::new(socket);
    write_line(
        &mut stream,
        &format!("220 {} ESMTP cctf-rs catcher", hostname),
    )
    .await?;
    let mut mail_from: Option<String> = None;
    let mut rcpts: Vec<String> = Vec::new();
    let mut line = String::new();
    loop {
        let n = read_command(&mut stream, &mut line).await?;
        if n == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let (verb, rest) = split_verb(trimmed);
        match verb.as_str() {
            "HELO" => write_line(&mut stream, &format!("250 {}", hostname)).await?,
            "EHLO" => {
                write_line(&mut stream, &format!("250-{}", hostname)).await?;
                write_line(&mut stream, "250-SIZE 10485760").await?;
                write_line(&mut stream, "250 SMTPUTF8").await?;
            }
            "MAIL" => {
                mail_from = parse_addr(rest, "FROM:");
                rcpts.clear();
                write_line(&mut stream, "250 2.1.0 OK").await?;
            }
            "RCPT" => match parse_addr(rest, "TO:") {
                Some(addr) => {
                    rcpts.push(addr);
                    write_line(&mut stream, "250 2.1.5 OK").await?;
                }
                None => write_line(&mut stream, "501 5.5.4 Syntax: RCPT TO:<address>").await?,
            },
            "DATA" => {
                if mail_from.is_none() || rcpts.is_empty() {
                    write_line(&mut stream, "503 5.5.1 Need MAIL and RCPT first").await?;
                    continue;
                }
                write_line(&mut stream, "354 End data with <CR><LF>.<CR><LF>").await?;
                let raw = read_data(&mut stream).await?;
                let email = parse_email(mail_from.clone().unwrap_or_default(), &rcpts, &raw);
                mailbox.lock().await.push(email);
                mail_from = None;
                rcpts.clear();
                write_line(&mut stream, "250 2.0.0 OK: message queued").await?;
            }
            "RSET" => {
                mail_from = None;
                rcpts.clear();
                write_line(&mut stream, "250 2.0.0 OK").await?;
            }
            "NOOP" => write_line(&mut stream, "250 2.0.0 OK").await?,
            "VRFY" => write_line(&mut stream, "252 2.5.2 Cannot VRFY user").await?,
            "QUIT" => {
                write_line(
                    &mut stream,
                    &format!("221 2.0.0 {} closing connection", hostname),
                )
                .await?;
                break;
            }
            "" => write_line(&mut stream, "500 5.5.2 Error: bad syntax").await?,
            _ => write_line(&mut stream, "502 5.5.1 Command not implemented").await?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libs::services::email::{EmailService, SmtpSenderClient};
    #[tokio::test]
    async fn catcher_receives_plaintext_email() {
        let server = SmtpCatcherServer::bind("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        let mailbox = server.mailbox();
        tokio::spawn(server.serve());
        let client = SmtpSenderClient::new(
            addr.ip().to_string(),
            addr.port(),
            "noreply@cctf.rs".to_string(),
        );
        client
            .send_email(
                "captain@chordjack.dev",
                "Verify your cctf.rs account",
                "Welcome to your rebirth arc.\n.leading dot survives\nGLHF",
            )
            .await
            .unwrap();
        let caught = mailbox.lock().await;
        assert_eq!(caught.len(), 1);
        assert_eq!(caught[0].subject, "Verify your cctf.rs account");
        assert_eq!(caught[0].to, "captain@chordjack.dev");
        assert!(caught[0].body.contains(".leading dot survives"));
    }
}
