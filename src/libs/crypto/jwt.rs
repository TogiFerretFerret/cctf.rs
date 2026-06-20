use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _, DecodeError};
use serde_json::{Error};

fn jwt64_encode(payload: &str) -> 

#[derive(Debug, Error)]
pub enum JwtError {
    InvalidFormat,
    InvalidSignature,
    Expired,
    NotYetValid,
    InvalidJson(serde_json::Error),
    Base64DecodeError(base64::DecodeError)
}
