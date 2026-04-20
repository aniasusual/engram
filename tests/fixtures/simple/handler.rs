use crate::auth::AuthService;
use crate::db::Database;
use crate::utils::format_response;

/// Handles incoming API requests
pub fn handle_request(token: &str) -> String {
    let auth = AuthService::new("secret");
    let db = Database::connect("localhost:5432");

    if !auth.validate_token(token) {
        return format_response("unauthorized");
    }

    let user_id = auth.extract_user_id(token).unwrap_or_default();
    match db.get_user(&user_id) {
        Some(user) => format_response(&user),
        None => format_response("not found"),
    }
}

/// Handles batch processing of multiple requests
pub fn handle_batch(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|t| handle_request(t)).collect()
}
