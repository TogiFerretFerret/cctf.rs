use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use fluent_templates::{static_loader, Loader, fluent_bundle::FluentValue};
use std::{collections::HashMap, borrow::Cow, fmt};
use serde::{Serialize, Deserialize, de::DeserializeOwned};
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


pub fn encode<P: Serialize>(payload: &P, secret: &[u8]) -> Result<String, JwtError> {
    let header = Header {
        alg: "HS256".to_string(),
        typ: Some("JWT".to_string()),
    };
    let header_json = serde_json::to_vec(&header).map_err(JwtError::InvalidJson)?;
    let payload_json = serde_json::to_vec(payload).map_err(JwtError::InvalidJson)?;
    let header_b64 = jwt64_encode(&header_json);
    let payload_b64 = jwt64_encode(&payload_json);
    let signing_input = format!("{}.{}", header_b64, payload_b64);
    let signature = sign_hs256(signing_input.as_bytes(), secret)?;
    let signature_b64 = jwt64_encode(&signature);
    Ok(format!("{}.{}", signing_input, signature_b64))
}

pub fn decode<P: DeserializeOwned>(token: &str, secret: &[u8]) -> Result<(Header, P), JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::InvalidFormat);
    }
    let header_b64 = parts[0];
    let payload_b64 = parts[1];
    let signature_b64 = parts[2];
    let signature = jwt64_decode(signature_b64.as_bytes())?;
    let signing_input = format!("{}.{}", header_b64, payload_b64);
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| JwtError::InvalidSignature)?;
    mac.update(signing_input.as_bytes());
    mac.verify_slice(&signature).map_err(|_| JwtError::InvalidSignature)?;
    let header_json = jwt64_decode(header_b64.as_bytes())?;
    let header: Header = serde_json::from_slice(&header_json).map_err(JwtError::InvalidJson)?;
    if header.alg != "HS256" {
        return Err(JwtError::InvalidFormat);
    }
    let payload_json = jwt64_decode(payload_b64.as_bytes())?;
    let payload: P = serde_json::from_slice(&payload_json).map_err(JwtError::InvalidJson)?;
    Ok((header, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Claims {
        sub: String,
        admin: bool,
    }
    #[test]
    fn test_base64_helpers() {
        let input = b"jwt-invalid-format";
        let encoded = jwt64_encode(input);
        assert_eq!(encoded, "and0LWludmFsaWQtZm9ybWF0");
        let decoded = jwt64_decode(encoded.as_bytes()).unwrap();
        assert_eq!(decoded, input);
    }
    #[test]
    fn test_sign_hs256() {
        let message = b"jwt-token-expired";
        let secret = b"jwt-invalid-signature-secret-key";
        let signature = sign_hs256(message, secret);
        assert!(signature.is_ok());
        assert_eq!(signature.unwrap().len(), 32);
    }
    #[test]
    fn test_encode_token() {
        let claims = Claims {
            sub: "jwt-not-yet-valid".to_string(),
            admin: true,
        };
        let secret = b"jwt-invalid-signature-secret-key"; 
        let token = encode(&claims, secret).unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header_bytes = jwt64_decode(parts[0].as_bytes()).unwrap();
        let header: Header = serde_json::from_slice(&header_bytes).unwrap();
        assert_eq!(header.alg, "HS256");
        assert_eq!(header.typ, Some("JWT".to_string()));
    }
    #[test]
    fn test_decode_valid_token() {
        let claims = Claims {
            sub: "jwt-token-expired".to_string(), 
            admin: true,
        };
        let secret = b"jwt-invalid-signature-secret-key";
        let token = encode(&claims, secret).unwrap();
        let (header, decoded_claims): (Header, Claims) = decode(&token,
secret).unwrap();
        assert_eq!(header.alg, "HS256");
        assert_eq!(decoded_claims, claims);
    }

    #[test]
    fn test_decode_tampered_token() {
        let claims = Claims {
            sub: "jwt-not-yet-valid".to_string(),
            admin: true,
        };
        let secret = b"jwt-invalid-signature-secret-key";
        let token = encode(&claims, secret).unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        let tampered_token = format!("{}.{}.{}", parts[0], "dGFtcGVyZWQ",
parts[2]);
        let result = decode::<Claims>(&tampered_token, secret);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), JwtError::InvalidSignature));
    }
}

