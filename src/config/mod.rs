//! Configuration module for handling environment variables and .env files

use crate::client::RedditClient;
use dotenv::dotenv;
use log::info;
use std::env;

/// Application configuration derived from environment variables and .env file
#[derive(Debug, Clone)]
pub struct AppConfig {
    // Reddit API credentials
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,

    // Reddit API settings
    pub user_agent: String,
    pub oauth_port: Option<u16>,

    // OAuth tokens (if provided directly)
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_in: u64,

    // Reddit IDs for operations
    pub thing_id: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            username: None,
            password: None,
            user_agent: "".to_string(),
            oauth_port: None,
            access_token: None,
            refresh_token: None,
            token_expires_in: 3600,
            thing_id: None,
        }
    }
}

impl AppConfig {
    /// Load configuration from environment variables and .env file
    pub fn load() -> Self {
        // Try to load .env file, but continue even if it doesn't exist
        match dotenv() {
            Ok(_) => info!("Loaded environment from .env file"),
            Err(_) => info!("No .env file found, using system environment variables only"),
        }

        let mut config = Self::default();

        // Load configuration from environment variables
        if let Ok(client_id) = env::var("REDDIT_CLIENT_ID") {
            config.client_id = Some(client_id);
        }

        if let Ok(client_secret) = env::var("REDDIT_CLIENT_SECRET") {
            config.client_secret = Some(client_secret);
        }

        if let Ok(username) = env::var("REDDIT_USERNAME") {
            config.username = Some(username);
        }

        if let Ok(password) = env::var("REDDIT_PASSWORD") {
            config.password = Some(password);
        }

        // User agent - use environment variable if available, otherwise use default
        if let Ok(user_agent) = env::var("REDDIT_USER_AGENT") {
            config.user_agent = user_agent;
        }

        // OAuth port - parse as u16 if provided
        if let Ok(port_str) = env::var("REDDIT_OAUTH_PORT") {
            if let Ok(port) = port_str.parse::<u16>() {
                config.oauth_port = Some(port);
            }
        }

        // OAuth tokens
        if let Ok(access_token) = env::var("REDDIT_ACCESS_TOKEN") {
            config.access_token = Some(access_token);
        }

        if let Ok(refresh_token) = env::var("REDDIT_REFRESH_TOKEN") {
            config.refresh_token = Some(refresh_token);
        }

        // Token expiration - parse as u64 if provided
        if let Ok(expires_str) = env::var("REDDIT_TOKEN_EXPIRES_IN") {
            if let Ok(expires) = expires_str.parse::<u64>() {
                config.token_expires_in = expires;
            }
        }

        // Thing ID for commands
        if let Ok(thing_id) = env::var("REDDIT_THING_ID") {
            config.thing_id = Some(thing_id);
        }

        config
    }

    /// Get client ID, panicking if not set
    pub fn require_client_id(&self) -> String {
        self.client_id
            .clone()
            .expect("REDDIT_CLIENT_ID environment variable must be set")
    }

    /// Get client secret, panicking if not set
    pub fn require_client_secret(&self) -> String {
        self.client_secret
            .clone()
            .expect("REDDIT_CLIENT_SECRET environment variable must be set")
    }

    /// Get username, panicking if not set
    pub fn require_username(&self) -> String {
        self.username
            .clone()
            .expect("REDDIT_USERNAME environment variable must be set")
    }

    /// Get password, panicking if not set
    pub fn require_password(&self) -> String {
        self.password
            .clone()
            .expect("REDDIT_PASSWORD environment variable must be set")
    }

    /// Get thing ID, panicking if not set
    pub fn require_thing_id(&self) -> String {
        self.thing_id
            .clone()
            .expect("REDDIT_THING_ID environment variable must be set")
    }

    /// Create a RedditClient from this configuration
    pub fn create_client(&self) -> RedditClient {
        // Use the RedditClient's from_config method, which handles all configuration aspects
        RedditClient::from_config(self)
    }
}
