/// Formats a response string with standard wrapper
pub fn format_response(message: &str) -> String {
    format!("Response: {}", message)
}

/// Sanitizes user input by removing special characters
pub fn sanitize_input(input: &str) -> String {
    input.chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect()
}
