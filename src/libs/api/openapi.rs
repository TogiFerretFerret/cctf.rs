use super::*;

const OPENAPI_YAML: &str = include_str!("../../../openapi.yaml");

#[derive(Deserialize)]
pub struct SpecLangQuery {
    pub lang: Option<String>,
}

pub(crate) async fn openapi_yaml(
    lang: PreferredLang,
    Query(q): Query<SpecLangQuery>,
) -> impl IntoResponse {
    let lid = lang_id(&q.lang.unwrap_or(lang.0));
    let mut doc: serde_json::Value =
        serde_norway::from_str(OPENAPI_YAML).expect("openapi.yaml must be valid YAML");
    localize_spec(&mut doc, &lid);
    let body = serde_norway::to_string(&doc).expect("serialize localized yaml");
    (
        [(axum::http::header::CONTENT_TYPE, "application/yaml")],
        body,
    )
}

pub(crate) async fn openapi_json(
    lang: PreferredLang,
    Query(q): Query<SpecLangQuery>,
) -> impl IntoResponse {
    let lid = lang_id(&q.lang.unwrap_or(lang.0));
    let mut doc: serde_json::Value =
        serde_norway::from_str(OPENAPI_YAML).expect("openapi.yaml must be valid YAML");
    localize_spec(&mut doc, &lid);
    let body = serde_json::to_string(&doc).expect("serialize localized json");
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        body,
    )
}

pub(crate) async fn api_docs() -> impl IntoResponse {
    match tokio::fs::read_to_string("apidocs/dist/index.html").await {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(_) => (
            StatusCode::NOT_FOUND,
            "docs not built - run `npm run build` in apidocs/",
        )
            .into_response(),
    }
}

fn localize_spec(value: &mut serde_json::Value, lang: &unic_langid::LanguageIdentifier) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if matches!(k.as_str(), "summary" | "description" | "title") {
                    if let serde_json::Value::String(s) = v {
                        if !s.contains(char::is_whitespace) {
                            let t = LOCALES.lookup(lang, s);
                            if !t.is_empty() && &t != s {
                                *s = t;
                            }
                        }
                        continue;
                    }
                } else {
                    localize_spec(v, lang);
                }
            }
        }
        serde_json::Value::Array(arr) => arr.iter_mut().for_each(|v| localize_spec(v, lang)),
        _ => {}
    }
}
