use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use thiserror::Error;

fn jwt64_encode(payload: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(payload)
}

fn jwt64_decode(payload: &[u8]) -> Result<Vec<u8>, JwtError> {
    URL_SAFE_NO_PAD.decode(payload).map_err(JwtError::Base64DecodeError)
}



#[derive(Debug, Error)]
pub enum JwtError {
    #[error("Your token was assembled by a Slitheen")]
    InvalidFormat,
    #[error("Your token failed to use its Psychic Paper")]
    InvalidSignature,
    #[error("The token is dead")]
    Expired,
    #[error("Your token has temporarily played the role of TARDIS")]
    NotYetValid,
    #[error("Your token is a human in a Dalek suit: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("Your token refused to be exterminated: {0}")]
    Base64DecodeError(#[from] base64::DecodeError)
}

