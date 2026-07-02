use cctf_rs::libs::api::API_ROUTES;
use std::collections::BTreeSet;

#[test]
fn openapi_spec_matches_api_routes() {
    let yaml = include_str!("../openapi.yaml");
    let doc: serde_norway::Value =
        serde_norway::from_str(yaml).expect("openapi.yaml is valid YAML");
    let paths = doc
        .get("paths")
        .and_then(|p| p.as_mapping())
        .expect("spec has a paths mapping");
    let mut spec: BTreeSet<(String, String)> = BTreeSet::new();
    for (path, ops) in paths {
        let path = path.as_str().expect("path key is a string");
        let ops = ops.as_mapping().expect("operations mapping");
        for (method, _op) in ops {
            let method = method.as_str().expect("method key is a string");
            if method.eq_ignore_ascii_case("parameters") {
                continue; //shared path level params, not an operation
            }
            spec.insert((method.to_uppercase(), path.to_string()));
        }
    }
    let expected: BTreeSet<(String, String)> = API_ROUTES
        .iter()
        .map(|(m, p)| (m.to_string(), p.to_string()))
        .collect();
    assert_eq!(
        spec, expected,
        "openapi.yaml drifted from API_ROUTES (left = spec, right = code)"
    );
}
