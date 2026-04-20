# Simple Fixture Project

A minimal Rust project for testing Engram.

## Architecture

The project has 4 modules:

- **auth** — JWT token validation via `AuthService`
- **db** — Database access with `get_user` and `save_user`
- **handler** — HTTP request handlers that call auth + db
- **utils** — Shared utilities like `format_response`

## Authentication Flow

1. Request arrives at `handle_request`
2. Token validated by `AuthService.validate_token`
3. User ID extracted from token
4. User fetched from database

## Configuration

Default secret key is used for token validation.
Maximum retry count is 3.
Database connection string: localhost:5432.
