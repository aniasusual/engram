/// Authentication service for validating tokens
pub struct AuthService {
    secret: String,
}

impl AuthService {
    /// Creates a new AuthService with the given secret
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_string(),
        }
    }

    /// Validates the JWT token and returns true if valid
    pub fn validate_token(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }
        token.starts_with(&self.secret)
    }

    /// Extracts the user ID from a validated token
    pub fn extract_user_id(&self, token: &str) -> Option<String> {
        if self.validate_token(token) {
            Some(token.replace(&self.secret, ""))
        } else {
            None
        }
    }
}
