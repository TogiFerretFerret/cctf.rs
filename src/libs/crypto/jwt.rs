use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use std::{collections::HashMap, borrow::Cow, fmt};
use serde::{Serialize, Deserialize};
use hmac::{Hmac, Mac, KeyInit};
use sha2::Sha256;
use unic_langid::langid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    pub alg: String,
    pub typ: Option<String>,
}

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en-US",
    };
}

fn jwt64_encode(payload: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(payload)
}

fn jwt64_decode(payload: &[u8]) -> Result<Vec<u8>, JwtError> {
    URL_SAFE_NO_PAD.decode(payload).map_err(JwtError::Base64DecodeError)
}

fn sign_hs256(message: &[u8], secret: &[u8]) -> Result<Vec<u8>, JwtError> {
    // only fails if the key format/size is completely invalid for the hash
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|_| JwtError::InvalidSignature)?;
    mac.update(message);
    Ok(mac.finalize().into_bytes().to_vec())
}

#[derive(Debug)]
pub enum JwtError {
    InvalidFormat,
    InvalidSignature,
    Expired,
    NotYetValid,
    InvalidJson(serde_json::Error),
    Base64DecodeError(base64::DecodeError)
}

impl JwtError {
    pub fn localize(&self, lang: &str) -> String {
        let lang_id = lang.parse().unwrap_or_else(|_| langid!("en-US"));
        match self {
            JwtError::InvalidFormat => LOCALES.lookup(&lang_id, "jwt-invalid-format"),
            JwtError::InvalidSignature => LOCALES.lookup(&lang_id, "jwt-invalid-signature"),
            JwtError::Expired => LOCALES.lookup(&lang_id, "jwt-token-expired"),
            JwtError::NotYetValid => LOCALES.lookup(&lang_id, "jwt-not-yet-valid"),
            JwtError::InvalidJson(err) => {
                let args = HashMap::from([(Cow::Borrowed("reason"),FluentValue::from(err.to_string())),]);
                LOCALES.lookup_with_args(&lang_id, "jwt-invalid-json", &args)
            },
            JwtError::Base64DecodeError(err) => {
                let args = HashMap::from([(Cow::Borrowed("reason"),FluentValue::from(err.to_string())),]);
                LOCALES.lookup_with_args(&lang_id, "jwt-base64-error", &args)
            }
        }
    }
}

// display fallback
impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.localize("en-US"))
    }
}


pub fn encode<P: Serialize>(header: &Header, payload: &P, secret: &[u8]) -> Result<String, JwtError> {

}
