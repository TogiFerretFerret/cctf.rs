use async_trait::async_trait;
use serde::{Deserialize, Serialize};
pub mod client;
pub mod server;
pub use client::SmtpSenderClient;
pub use server::SmtpCatcherServer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Email {
    pub id: String,
    pub from: String,
    pub to: String, 
    pub subject: String,
    pub body: String,
    pub timestamp: i64,
}

#[async_trait]
pub trait EmailService {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), String>;
}
