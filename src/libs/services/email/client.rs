use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpService;
use super::EmailService;
// CHONKER TODO: LOCALIZE EVERYTHING HERE
pub struct SmtpSenderClient {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub from_email: String,
}

impl SmtpSenderClient {
    pub fn new(smtp_host: String, smtp_port: u16, from_email: String) -> Self {
        Self {
            smtp_host,
            smtp_port,
            from_email,
        }
    }
    
    async fn read_smtp_response<R: AsyncBufReadExt + Unpin>(
        reader: &mut R,
    ) -> Result<(u16, Vec<String>), std::io::Error> {
        let mut lines = Vec::new();
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 {
                return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "SMTP server closed prematurely", // TODO: localize
                ));
            }
            let trimmed = line.trim_end_matches(|c| c=='\r'||c=='\n').to_string();
            if trimmed.len() < 3 {
                return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid SMTP line length",
                ));
            }
            lines.push(trimmed.clone());
            if trimmed.len() >= 4 && trimmed.as_bytes()[3] == b' ' {
                let code_str = &trimmed[..3];
                let code = code_str.parse::<u16>().map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid status code")
                })?;
                return Ok((code, lines));
            }
        }
    }
}

