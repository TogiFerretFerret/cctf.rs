use super::*;

pub fn load_bracket_scripts() -> HashMap<String, String> {
    if let Ok(content) = std::fs::read_to_string("brackets.json") {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        let mut map = HashMap::new();
        map.insert(
            "Collegiate".to_string(),
            "email.ends_with(\".edu\")".to_string(),
        );
        map
    }
}

pub fn validate_bracket_join_rhai(email: &str, username: &str, script_content: &str) -> bool {
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(100_000);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 64);
    engine.set_max_string_size(16 * 1024);
    let mut scope = rhai::Scope::new();
    scope.push("email", email.to_string());
    scope.push("username", username.to_string());
    match engine.eval_with_scope::<bool>(&mut scope, script_content) {
        Ok(is_allowed) => is_allowed,
        Err(e) => {
            eprintln!("rhaiBracket!: {:?}", e);
            false
        }
    }
}
