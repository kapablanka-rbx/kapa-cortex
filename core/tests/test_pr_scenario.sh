#!/usr/bin/env bash
#
# End-to-end PR splitting scenario.
# Creates a temp Rust repo, builds a feature branch with mixed changes,
# then runs kapa-cortex analyze to verify grouping.
#
set -euo pipefail

BINARY="${1:-$(dirname "$0")/../target/debug/kapa-cortex-core}"
WORKDIR=$(mktemp -d)
trap "rm -rf $WORKDIR; rm -f /tmp/kapa-cortex.sock" EXIT

# Clean stale socket from previous runs
rm -f /tmp/kapa-cortex.sock

echo "=== Setting up test repo in $WORKDIR ==="

cd "$WORKDIR"
git init -b main
git config user.email "test@test.com"
git config user.name "Test"

# ── Main branch: baseline Rust crate ──

mkdir -p src/auth src/api src/db tests docs

cat > Cargo.toml << 'TOML'
[package]
name = "myapp"
version = "0.1.0"
edition = "2021"
TOML

cat > src/main.rs << 'RS'
mod auth;
mod api;
mod db;

fn main() {
    let db = db::Connection::new("sqlite:///app.db");
    let handler = auth::LoginHandler::new(db);
    api::start_server(handler);
}
RS

cat > src/auth/mod.rs << 'RS'
mod login;
mod session;

pub use login::LoginHandler;
pub use session::SessionManager;
RS

cat > src/auth/login.rs << 'RS'
use crate::db::Connection;

pub struct LoginHandler {
    db: Connection,
}

impl LoginHandler {
    pub fn new(db: Connection) -> Self {
        LoginHandler { db }
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Option<String> {
        let user = self.db.find_user(username)?;
        if user.check_password(password) {
            Some(self.create_session(&user))
        } else {
            None
        }
    }

    fn create_session(&self, user: &crate::db::User) -> String {
        format!("session_{}", user.id)
    }
}
RS

cat > src/auth/session.rs << 'RS'
use std::collections::HashMap;

pub struct SessionManager {
    sessions: HashMap<String, SessionData>,
}

pub struct SessionData {
    pub user_id: u64,
    pub created_at: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager { sessions: HashMap::new() }
    }

    pub fn store(&mut self, token: &str, data: SessionData) {
        self.sessions.insert(token.to_string(), data);
    }

    pub fn get(&self, token: &str) -> Option<&SessionData> {
        self.sessions.get(token)
    }

    pub fn revoke(&mut self, token: &str) {
        self.sessions.remove(token);
    }
}
RS

cat > src/api/mod.rs << 'RS'
mod routes;
mod middleware;

pub use routes::start_server;
RS

cat > src/api/routes.rs << 'RS'
use crate::auth::LoginHandler;

pub fn start_server(handler: LoginHandler) {
    println!("Server starting on :8080");
    // POST /login
    // GET /health
}
RS

cat > src/api/middleware.rs << 'RS'
use crate::auth::SessionManager;

pub struct AuthMiddleware {
    session_mgr: SessionManager,
}

impl AuthMiddleware {
    pub fn new(session_mgr: SessionManager) -> Self {
        AuthMiddleware { session_mgr }
    }

    pub fn process(&self, token: &str) -> Option<u64> {
        self.session_mgr.get(token).map(|s| s.user_id)
    }
}
RS

cat > src/db/mod.rs << 'RS'
mod connection;

pub use connection::{Connection, User};
RS

cat > src/db/connection.rs << 'RS'
pub struct Connection {
    url: String,
}

pub struct User {
    pub id: u64,
    pub name: String,
    pub password_hash: String,
}

impl User {
    pub fn check_password(&self, password: &str) -> bool {
        self.password_hash == password
    }
}

impl Connection {
    pub fn new(url: &str) -> Self {
        Connection { url: url.to_string() }
    }

    pub fn find_user(&self, username: &str) -> Option<User> {
        None
    }

    pub fn run_migrations(&self) {
        // CREATE TABLE users ...
        // CREATE TABLE sessions ...
    }
}
RS

cat > docs/README.md << 'MD'
# MyApp

A Rust web server with authentication.

## Setup

1. `cargo build`
2. Run migrations
3. `cargo run`
MD

cat > docs/API.md << 'MD'
# API Reference

## POST /login
Authenticate a user.

## GET /health
Health check.
MD

git add -A
git commit -m "Initial Rust crate"

# ── Feature branch: OAuth + rate limiting ──

git checkout -b feat/oauth-and-rate-limiting

# 1. New: OAuth provider
cat > src/auth/oauth.rs << 'RS'
pub struct OAuthProvider {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

pub struct OAuthUserInfo {
    pub email: String,
    pub name: String,
}

impl OAuthProvider {
    pub fn new(client_id: &str, client_secret: &str, redirect_uri: &str) -> Self {
        OAuthProvider {
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            redirect_uri: redirect_uri.to_string(),
        }
    }

    pub fn authorization_url(&self) -> String {
        format!("https://oauth.example.com/authorize?client_id={}", self.client_id)
    }

    pub fn exchange_code(&self, code: &str) -> OAuthTokens {
        OAuthTokens {
            access_token: format!("at_{}", code),
            refresh_token: "rt_abc".to_string(),
            expires_in: 3600,
        }
    }

    pub fn get_user_info(&self, access_token: &str) -> OAuthUserInfo {
        OAuthUserInfo {
            email: "user@example.com".to_string(),
            name: "OAuth User".to_string(),
        }
    }

    pub fn refresh(&self, refresh_token: &str) -> OAuthTokens {
        OAuthTokens {
            access_token: "at_refreshed".to_string(),
            refresh_token: refresh_token.to_string(),
            expires_in: 3600,
        }
    }
}
RS

# 2. Update auth/mod.rs to export oauth
cat > src/auth/mod.rs << 'RS'
mod login;
mod session;
mod oauth;

pub use login::LoginHandler;
pub use session::SessionManager;
pub use oauth::OAuthProvider;
RS

# 3. Update login to support OAuth
cat > src/auth/login.rs << 'RS'
use crate::db::Connection;
use crate::auth::oauth::OAuthProvider;

pub struct LoginHandler {
    db: Connection,
    oauth: Option<OAuthProvider>,
}

impl LoginHandler {
    pub fn new(db: Connection) -> Self {
        LoginHandler { db, oauth: None }
    }

    pub fn with_oauth(mut self, provider: OAuthProvider) -> Self {
        self.oauth = Some(provider);
        self
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Option<String> {
        let user = self.db.find_user(username)?;
        if user.check_password(password) {
            Some(self.create_session(&user))
        } else {
            None
        }
    }

    pub fn authenticate_oauth(&self, code: &str) -> Result<String, String> {
        let provider = self.oauth.as_ref().ok_or("OAuth not configured")?;
        let tokens = provider.exchange_code(code);
        let user_info = provider.get_user_info(&tokens.access_token);
        let user = self.db.find_or_create_by_email(&user_info.email)
            .ok_or("Failed to create user")?;
        Ok(self.create_session(&user))
    }

    fn create_session(&self, user: &crate::db::User) -> String {
        format!("session_{}", user.id)
    }
}
RS

# 4. New: Rate limiter
cat > src/api/rate_limiter.rs << 'RS'
use std::collections::HashMap;
use std::time::Instant;

pub struct RateLimiter {
    max_requests: usize,
    window_secs: u64,
    buckets: HashMap<String, Vec<Instant>>,
}

impl RateLimiter {
    pub fn new(max_requests: usize, window_secs: u64) -> Self {
        RateLimiter {
            max_requests,
            window_secs,
            buckets: HashMap::new(),
        }
    }

    pub fn is_allowed(&mut self, client_ip: &str) -> bool {
        let now = Instant::now();
        let bucket = self.buckets.entry(client_ip.to_string()).or_default();

        bucket.retain(|t| now.duration_since(*t).as_secs() < self.window_secs);

        if bucket.len() >= self.max_requests {
            return false;
        }

        bucket.push(now);
        true
    }

    pub fn reset(&mut self, client_ip: &str) {
        self.buckets.remove(client_ip);
    }
}
RS

# 5. Update api/mod.rs
cat > src/api/mod.rs << 'RS'
mod routes;
mod middleware;
mod rate_limiter;

pub use routes::start_server;
pub use rate_limiter::RateLimiter;
RS

# 6. Update routes to use rate limiter + OAuth
cat > src/api/routes.rs << 'RS'
use crate::auth::LoginHandler;
use crate::api::rate_limiter::RateLimiter;

pub fn start_server(handler: LoginHandler) {
    let mut limiter = RateLimiter::new(100, 60);
    println!("Server starting on :8080");
    // POST /login         -> handler.authenticate
    // POST /login/oauth   -> handler.authenticate_oauth
    // GET  /health        -> health check
    // GET  /oauth/callback -> handler.authenticate_oauth
}
RS

# 7. Update middleware for Bearer tokens
cat > src/api/middleware.rs << 'RS'
use crate::auth::SessionManager;

pub enum AuthType {
    Session,
    Bearer,
}

pub struct AuthMiddleware {
    session_mgr: SessionManager,
}

impl AuthMiddleware {
    pub fn new(session_mgr: SessionManager) -> Self {
        AuthMiddleware { session_mgr }
    }

    pub fn process(&self, header: &str) -> Option<(AuthType, u64)> {
        if header.starts_with("Bearer ") {
            let token = &header[7..];
            self.session_mgr.get(token).map(|s| (AuthType::Bearer, s.user_id))
        } else {
            self.session_mgr.get(header).map(|s| (AuthType::Session, s.user_id))
        }
    }
}
RS

# 8. DB: add OAuth support
cat > src/db/connection.rs << 'RS'
pub struct Connection {
    url: String,
}

pub struct User {
    pub id: u64,
    pub name: String,
    pub password_hash: String,
    pub email: Option<String>,
    pub oauth_provider: Option<String>,
}

impl User {
    pub fn check_password(&self, password: &str) -> bool {
        self.password_hash == password
    }
}

impl Connection {
    pub fn new(url: &str) -> Self {
        Connection { url: url.to_string() }
    }

    pub fn find_user(&self, username: &str) -> Option<User> {
        None
    }

    pub fn find_or_create_by_email(&self, email: &str) -> Option<User> {
        Some(User {
            id: 1,
            name: email.to_string(),
            password_hash: String::new(),
            email: Some(email.to_string()),
            oauth_provider: Some("oauth".to_string()),
        })
    }

    pub fn run_migrations(&self) {
        // CREATE TABLE users (id, name, password_hash, email, oauth_provider)
        // CREATE TABLE sessions (token, user_id, created_at)
        // CREATE TABLE oauth_tokens (user_id, provider, access_token, refresh_token, expires_at)
    }
}
RS

# 9. New: Config
cat > src/config.rs << 'RS'
pub struct Config {
    pub db_url: String,
    pub oauth_client_id: String,
    pub oauth_client_secret: String,
    pub oauth_redirect_uri: String,
    pub rate_limit_max: usize,
    pub rate_limit_window: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Config {
            db_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:///app.db".into()),
            oauth_client_id: std::env::var("OAUTH_CLIENT_ID").unwrap_or_default(),
            oauth_client_secret: std::env::var("OAUTH_CLIENT_SECRET").unwrap_or_default(),
            oauth_redirect_uri: std::env::var("OAUTH_REDIRECT_URI")
                .unwrap_or_else(|_| "http://localhost:8000/oauth/callback".into()),
            rate_limit_max: std::env::var("RATE_LIMIT_MAX")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(100),
            rate_limit_window: std::env::var("RATE_LIMIT_WINDOW")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(60),
        }
    }
}
RS

# 10. Update main to use config
cat > src/main.rs << 'RS'
mod auth;
mod api;
mod db;
mod config;

fn main() {
    let cfg = config::Config::from_env();
    let db = db::Connection::new(&cfg.db_url);
    let oauth = auth::OAuthProvider::new(
        &cfg.oauth_client_id,
        &cfg.oauth_client_secret,
        &cfg.oauth_redirect_uri,
    );
    let handler = auth::LoginHandler::new(db).with_oauth(oauth);
    api::start_server(handler);
}
RS

# 11. Update docs
cat > docs/README.md << 'MD'
# MyApp

A Rust web server with authentication.

## Features

- Username/password login
- OAuth 2.0 support
- Rate limiting (token bucket)
- Session management

## Setup

1. `cargo build`
2. Set `OAUTH_CLIENT_ID`, `OAUTH_CLIENT_SECRET` env vars
3. Run migrations
4. `cargo run`
MD

cat > docs/API.md << 'MD'
# API Reference

## POST /login
Authenticate with username and password.

## POST /login/oauth
Authenticate via OAuth 2.0 authorization code.

## GET /oauth/callback
OAuth callback endpoint.

## GET /health
Health check.

## Rate Limiting
100 requests per minute per IP.
MD

git add -A
git commit -m "Add OAuth support and rate limiting"

# ── Run kapa-cortex ──

echo ""
echo "=== Files changed ==="
git diff --stat main

echo ""
echo "=== Index ==="
"$BINARY" index . 2>&1

echo ""
echo "=== analyze --brief ==="
"$BINARY" analyze --base main --brief

echo ""
echo "=== analyze --json ==="
"$BINARY" analyze --base main --json

echo ""
echo "=== extract \"oauth\" --brief ==="
"$BINARY" extract "oauth" --brief

echo ""
echo "=== extract \"*.md\" --brief ==="
"$BINARY" extract "*.md" --brief

echo ""
echo "=== hotspots --brief ==="
"$BINARY" hotspots --brief 2>&1

echo ""
echo "=== symbols src/auth/oauth.rs --brief ==="
"$BINARY" symbols src/auth/oauth.rs --brief 2>&1

echo ""
echo "=== deps src/api/routes.rs --brief ==="
"$BINARY" deps src/api/routes.rs --brief 2>&1

echo ""
echo "=== DONE ==="
