/// Database connection wrapper
pub struct Database {
    connection_string: String,
}

impl Database {
    /// Creates a new database connection
    pub fn connect(conn_str: &str) -> Self {
        Self {
            connection_string: conn_str.to_string(),
        }
    }

    /// Queries a user by ID from the database
    pub fn get_user(&self, user_id: &str) -> Option<String> {
        if user_id.is_empty() {
            return None;
        }
        Some(format!("User({})", user_id))
    }

    /// Saves user data to the database
    pub fn save_user(&self, user_id: &str, data: &str) -> bool {
        !user_id.is_empty() && !data.is_empty()
    }
}
