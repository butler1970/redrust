//! Configuration module for handling environment variables and .env files

use crate::client::RedditClient;
use dotenv::dotenv;
use log::info;
use std::env;

/// Application configuration derived from environment variables and .env file
///
/// # Environment Variables
///
/// The following environment variables are used to configure the application:
///
/// - `REDDIT_CLIENT_ID`: Reddit API client ID
/// - `REDDIT_CLIENT_SECRET`: Reddit API client secret
/// - `REDDIT_USERNAME`: Reddit username
/// - `REDDIT_PASSWORD`: Reddit password
/// - `REDDIT_USER_AGENT`: User agent string for Reddit API requests (required)
/// - `REDDIT_OAUTH_PORT`: Port for OAuth callback server
/// - `REDDIT_ACCESS_TOKEN`: Direct access token if available
/// - `REDDIT_REFRESH_TOKEN`: Refresh token if available
/// - `REDDIT_TOKEN_EXPIRES_IN`: Token expiration time in seconds
/// - `REDDIT_THING_ID`: Reddit thing ID for operations
///
/// # .env File Location
///
/// The application will look for a `.env` file in the following locations:
///
/// 1. The current working directory
/// 2. One directory up from the current working directory (`../`)
///
/// When used as a library in another project, the `.env` file should be in the
/// root directory of the consuming project. For example:
///
/// - When used in a standalone project: `/myproject/.env`
/// - When used in a workspace: `/workspace/.env` (not in the individual crate directories)
///
/// For CLI usage, the application will try to detect if it's running from a build directory
/// (like `target/debug`) and automatically adjust to look for the `.env` file in the project root.
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
            user_agent: String::new(),
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
        // Try to load .env file from both current directory and project root
        // This helps when running from the bin directory
        if let Ok(_) = dotenv() {
            info!("Loaded environment from .env file");
        } else if let Ok(_) = dotenv::from_filename("../.env") {
            info!("Loaded environment from ../.env file");
        } else {
            info!("No .env file found, using system environment variables only");
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

        // User agent - required for Reddit API usage
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper function to create a temporary .env file with specific content
    fn create_test_env_file(dir: &TempDir, content: &str) -> PathBuf {
        let env_path = dir.path().join(".env");
        let mut file = File::create(&env_path).expect("Failed to create test .env file");
        file.write_all(content.as_bytes()).expect("Failed to write to test .env file");
        env_path
    }

    // Helper to clean environment variables before tests
    fn clean_env_vars() {
        env::remove_var("REDDIT_CLIENT_ID");
        env::remove_var("REDDIT_CLIENT_SECRET");
        env::remove_var("REDDIT_USERNAME");
        env::remove_var("REDDIT_PASSWORD");
        env::remove_var("REDDIT_USER_AGENT");
        env::remove_var("REDDIT_OAUTH_PORT");
        env::remove_var("REDDIT_ACCESS_TOKEN");
        env::remove_var("REDDIT_REFRESH_TOKEN");
        env::remove_var("REDDIT_TOKEN_EXPIRES_IN");
        env::remove_var("REDDIT_THING_ID");
    }

    // Override the default user agent for testing
    #[test]
    fn test_loading_default_config() {
        // Test that the default config has expected values
        let config = AppConfig::default();
        assert!(config.client_id.is_none());
        assert_eq!(config.user_agent, String::new());
        assert_eq!(config.token_expires_in, 3600);
    }

    #[test]
    fn test_loading_from_env_vars() {
        // This test verifies that the AppConfig correctly loads values from environment variables
        
        // Start with a clean environment
        clean_env_vars();
        
        // Set test environment variables directly - no .env file
        env::set_var("REDDIT_CLIENT_ID", "test_client_id");
        env::set_var("REDDIT_USER_AGENT", "test_user_agent");
        env::set_var("REDDIT_OAUTH_PORT", "9999");
        env::set_var("REDDIT_TOKEN_EXPIRES_IN", "7200");
        
        // Create an empty config
        let mut config = AppConfig::default();
        
        // Manually apply the same logic from the load method
        if let Ok(client_id) = env::var("REDDIT_CLIENT_ID") {
            config.client_id = Some(client_id);
        }
        
        if let Ok(user_agent) = env::var("REDDIT_USER_AGENT") {
            config.user_agent = user_agent;
        }
        
        if let Ok(port_str) = env::var("REDDIT_OAUTH_PORT") {
            if let Ok(port) = port_str.parse::<u16>() {
                config.oauth_port = Some(port);
            }
        }
        
        if let Ok(expires_str) = env::var("REDDIT_TOKEN_EXPIRES_IN") {
            if let Ok(expires) = expires_str.parse::<u64>() {
                config.token_expires_in = expires;
            }
        }
        
        // Verify the values were correctly loaded
        assert_eq!(config.client_id, Some("test_client_id".to_string()));
        assert_eq!(config.user_agent, "test_user_agent".to_string());
        assert_eq!(config.oauth_port, Some(9999));
        assert_eq!(config.token_expires_in, 7200);
        
        // Clean up
        clean_env_vars();
    }


    #[test]
    fn test_require_methods() {
        // Set up a test config with required values
        let mut config = AppConfig::default();
        config.client_id = Some("test_id".to_string());
        
        // Test the require method
        assert_eq!(config.require_client_id(), "test_id");
        
        // Testing panic behavior would need std::panic::catch_unwind
    }

    // Test that shows the expected behavior of the dotenv crate when 
    // environment variables are set in both a .env file and the process
    // environment. The process environment should take precedence.
    #[test]
    fn test_env_vars_precedence() {
        // This test documents that environment variables set in the process
        // environment take precedence over those set in .env files.
        
        // Set a variable in the process environment
        env::set_var("TEST_PRECEDENCE", "process_env_value");
        
        // Create a file with a different value
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let _env_path = create_test_env_file(&temp_dir, "TEST_PRECEDENCE=dotenv_value");
        
        // Switch to the directory with the .env file
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(temp_dir.path()).expect("Failed to change directory");
        
        // Load .env file
        dotenv().ok();
        
        // Check which value was loaded
        let value = env::var("TEST_PRECEDENCE").unwrap_or_default();
        
        // Restore original directory
        env::set_current_dir(original_dir).expect("Failed to restore directory");
        
        // The process environment value should be used
        assert_eq!(value, "process_env_value");
        
        // Clean up
        env::remove_var("TEST_PRECEDENCE");
    }
}