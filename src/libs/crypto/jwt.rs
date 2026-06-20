use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use fluent_templates::{static_loader, Loader};
use unic_langid::langid;
use std::collections::HashMap;
use std::borrow::Cow;
use fluent_templates::fluent_bundle::FluentValue;
use std::fmt;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_bidi(s: String) -> String {
        s.replace('\u{2068}', "").replace('\u{2069}', "")
    }

    #[test]
    fn test_localization() {
        assert_eq!(
            JwtError::InvalidFormat.localize("ky"),
            "Токендин форматы жараксыз"
        );
        assert_eq!(
            JwtError::InvalidFormat.localize("ru"),
            "Неверный формат токена"
        );
        assert_eq!(
            JwtError::InvalidFormat.localize("vi"),
            "Định dạng token không hợp lệ"
        );
        assert_eq!(
            JwtError::InvalidFormat.localize("ja"),
            "トークンの形式が無効です"
        );
        assert_eq!(
            JwtError::InvalidFormat.localize("lv"),
            "Nederīgs marķiera formāts"
        );

        let json_err = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let jwt_json_err = JwtError::InvalidJson(json_err);
        assert_eq!(
            clean_bidi(jwt_json_err.localize("ky")),
            "Жараксыз JSON берилиштери: EOF while parsing an object at line 1 column 1"
        );
        assert_eq!(
            clean_bidi(jwt_json_err.localize("ru")),
            "Неверная полезная нагрузка JSON: EOF while parsing an object at line 1 column 1"
        );
        assert_eq!(
            clean_bidi(jwt_json_err.localize("vi")),
            "Payload JSON không hợp lệ: EOF while parsing an object at line 1 column 1"
        );
        assert_eq!(
            clean_bidi(jwt_json_err.localize("ja")),
            "JSONペイロードが無効です: EOF while parsing an object at line 1 column 1"
        );
        assert_eq!(
            clean_bidi(jwt_json_err.localize("lv")),
            "Nederīga JSON derīgā slodze: EOF while parsing an object at line 1 column 1"
        );
    }
}

