use crate::models::public_feed::PublicFeedResponse;
use crate::models::subreddit_posts::SubredditPostsResponse;
use crate::models::RedditRNewResponse;
use log::{debug, info};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::{Client, Error as ReqwestError};
use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tiny_http::{Response, Server, StatusCode};
use url::Url;
use webbrowser;

// Define a custom error type for handling Reddit API errors
#[derive(Debug)]
pub enum RedditClientError {
    RequestError(ReqwestError),
    ApiError(String),
    ParseError(serde_json::Error),
}

impl fmt::Display for RedditClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RedditClientError::RequestError(err) => write!(f, "Request error: {}", err),
            RedditClientError::ApiError(msg) => write!(f, "Reddit API error: {}", msg),
            RedditClientError::ParseError(err) => write!(f, "Parse error: {}", err),
        }
    }
}

impl std::error::Error for RedditClientError {}

impl From<ReqwestError> for RedditClientError {
    fn from(err: ReqwestError) -> Self {
        RedditClientError::RequestError(err)
    }
}

impl From<serde_json::Error> for RedditClientError {
    fn from(err: serde_json::Error) -> Self {
        RedditClientError::ParseError(err)
    }
}

/// Structure to store OAuth tokens and credentials
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenStorage {
    pub client_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<u64>,
    pub last_updated: u64,
}

impl TokenStorage {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            access_token: None,
            refresh_token: None,
            token_expires_at: None,
            last_updated: chrono::Utc::now().timestamp() as u64,
        }
    }

    pub fn is_access_token_valid(&self) -> bool {
        match (self.access_token.as_ref(), self.token_expires_at) {
            (Some(_), Some(expiry)) => {
                let now = chrono::Utc::now().timestamp() as u64;
                // Add a 5-minute buffer to avoid edge cases
                now + 300 < expiry
            }
            _ => false,
        }
    }

    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }
}

#[derive(Clone)]
pub struct RedditClient {
    pub client: Client,
    pub access_token: Option<String>,
    pub user_agent: String,
    pub token_storage: Option<TokenStorage>,
}

impl RedditClient {
    pub fn new() -> Self {
        let user_agent = format!("redrust/1.0 (by /u/Aggravating-Fix-3871)");
        Self {
            client: Self::get_client(&user_agent).unwrap(),
            access_token: None,
            user_agent,
            token_storage: None,
        }
    }

    pub fn with_user_agent(user_agent: String) -> Self {
        Self {
            client: Self::get_client(&user_agent).unwrap(),
            access_token: None,
            user_agent,
            token_storage: None,
        }
    }

    /// Create a client from a configuration object
    pub fn from_config(config: &crate::config::AppConfig) -> Self {
        debug!(
            "Creating RedditClient with user_agent: {}",
            config.user_agent
        );
        let mut client = Self::with_user_agent(config.user_agent.clone());

        // Use client_id to load token storage if available
        if let Some(client_id) = &config.client_id {
            // Try to load existing tokens first
            if let Some(storage) = Self::load_token_storage(client_id) {
                if storage.is_access_token_valid() {
                    // If we have a valid access token, use it
                    client.access_token = storage.access_token.clone();
                }
                client.token_storage = Some(storage);
            } else {
                // No stored tokens, create new storage
                client.token_storage = Some(TokenStorage::new(client_id));
            }
        }

        // If we have a direct access token, use it
        if let Some(token) = &config.access_token {
            client.access_token = Some(token.clone());
        }

        client
    }

    /// Load stored tokens for a client ID if available
    pub fn with_stored_tokens(client_id: &str) -> Self {
        let mut client = Self::new();

        if let Some(storage) = Self::load_token_storage(client_id) {
            if storage.is_access_token_valid() {
                // If we have a valid access token, use it
                client.access_token = storage.access_token.clone();
            }
            client.token_storage = Some(storage);
        } else {
            // No stored tokens, create a new storage
            client.token_storage = Some(TokenStorage::new(client_id));
        }

        client
    }

    /// Set token values manually (useful for headless environments)
    pub fn set_tokens(
        &mut self,
        client_id: &str,
        access_token: &str,
        refresh_token: Option<&str>,
        expires_in: u64,
    ) -> Result<(), RedditClientError> {
        let now = chrono::Utc::now().timestamp() as u64;
        let expires_at = now + expires_in;

        // Create or update token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }

        if let Some(storage) = &mut self.token_storage {
            storage.client_id = client_id.to_string();
            storage.access_token = Some(access_token.to_string());
            storage.token_expires_at = Some(expires_at);
            storage.last_updated = now;

            if let Some(refresh) = refresh_token {
                storage.refresh_token = Some(refresh.to_string());
            }

            // Save the token storage
            self.save_token_storage()?;
        }

        // Set the token for immediate use
        self.access_token = Some(access_token.to_string());

        Ok(())
    }

    /// Get the directory for storing tokens
    fn get_token_dir() -> PathBuf {
        let mut token_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        token_dir.push(".redrust");

        // Create the directory if it doesn't exist
        if !token_dir.exists() {
            fs::create_dir_all(&token_dir).ok();
        }

        token_dir
    }

    /// Get the path to the token file for a client ID
    fn get_token_path(client_id: &str) -> PathBuf {
        let mut path = Self::get_token_dir();
        path.push(format!("{}.json", client_id));
        path
    }

    /// Load token storage from the filesystem
    fn load_token_storage(client_id: &str) -> Option<TokenStorage> {
        let token_path = Self::get_token_path(client_id);

        if !token_path.exists() {
            return None;
        }

        let mut file = match File::open(&token_path) {
            Ok(file) => file,
            Err(_) => return None,
        };

        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_err() {
            return None;
        }

        match serde_json::from_str::<TokenStorage>(&contents) {
            Ok(storage) => Some(storage),
            Err(e) => {
                debug!("Failed to parse token storage: {}", e);
                None
            }
        }
    }

    /// Save token storage to the filesystem
    fn save_token_storage(&self) -> Result<(), RedditClientError> {
        if let Some(storage) = &self.token_storage {
            let token_path = Self::get_token_path(&storage.client_id);

            let json = serde_json::to_string_pretty(storage).map_err(|e| {
                RedditClientError::ApiError(format!("Failed to serialize token storage: {}", e))
            })?;

            let mut file = File::create(&token_path).map_err(|e| {
                RedditClientError::ApiError(format!("Failed to create token file: {}", e))
            })?;

            file.write_all(json.as_bytes()).map_err(|e| {
                RedditClientError::ApiError(format!("Failed to write token file: {}", e))
            })?;

            debug!("Saved token storage to {}", token_path.display());
        }

        Ok(())
    }

    fn get_client(user_agent: &str) -> Result<Client, RedditClientError> {
        Ok(Client::builder().user_agent(user_agent).build()?)
    }

    /// Get an application-only access token for reading public data.
    ///
    /// This method gets a token that can only be used for reading public data.
    /// It cannot be used for actions that require a user account like posting,
    /// commenting, or voting.
    pub async fn get_access_token(&mut self, client_id: &str) -> Result<String, RedditClientError> {
        let params = [
            (
                "grant_type",
                "https://oauth.reddit.com/grants/installed_client",
            ),
            ("device_id", "DO_NOT_TRACK_THIS_DEVICE"),
        ];

        // Note: Since there is no client secret, the authorization is created using your client_id followed by a colon.
        let auth = base64::encode(format!("{}:", client_id));

        let res = self
            .client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        let json: serde_json::Value = res.json().await?;
        let token = json["access_token"]
            .as_str()
            .ok_or_else(|| {
                RedditClientError::ApiError(
                    "Failed to extract access token from response".to_string(),
                )
            })?
            .to_string();

        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!("Application-only access token successfully obtained");

        Ok(token)
    }

    /// Authenticate with Reddit using username and password (password flow).
    ///
    /// This method gets a user-specific token that can be used for actions like posting,
    /// commenting, voting, and other operations that require a user account.
    ///
    /// # Arguments
    /// * `client_id` - Your Reddit API client ID
    /// * `username` - Reddit username
    /// * `password` - Reddit password
    ///
    /// # Note
    /// Your Reddit application must be set up as a "script" type app for this to work.
    /// The scope "submit" is included to allow posting.
    ///
    /// # Important
    /// This method doesn't work with Reddit accounts that use Google OAuth or other
    /// third-party login methods. For those, use authenticate_with_api_credentials() instead.

    /// Authenticate with Reddit using the interactive browser OAuth flow.
    ///
    /// This method works with any Reddit account, including those using Google OAuth.
    /// It will open a web browser where the user can log in with their normal method
    /// and authorize the application.
    ///
    /// # Arguments
    /// * `client_id` - Your Reddit API client ID for an installed app
    /// * `redirect_port` - The port to use for the localhost redirect (default: 8080)
    /// * `scopes` - The permissions to request (default includes read and submit)
    ///
    /// # Returns
    /// A Result containing the access token if successful
    ///
    /// # How this works:
    /// 1. Starts a local web server on localhost to receive the OAuth callback
    /// 2. Opens a browser for the user to log in and authorize the app
    /// 3. Reddit redirects back to localhost with an authorization code
    /// 4. Exchanges this code for an access token
    /// Try to refresh the access token using a stored refresh token
    pub async fn refresh_access_token(&mut self) -> Result<String, RedditClientError> {
        let storage = match &self.token_storage {
            Some(storage) if storage.has_refresh_token() => storage.clone(),
            _ => {
                return Err(RedditClientError::ApiError(
                    "No refresh token available".to_string(),
                ))
            }
        };

        let refresh_token = storage.refresh_token.as_ref().unwrap();
        let client_id = storage.client_id.clone();

        debug!("Refreshing access token using refresh token");

        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ];

        // For the Authorization header, use just the client_id
        let auth = base64::encode(format!("{}:", client_id));

        let res = self
            .client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(format!(
                "Token refresh failed: HTTP {}: {}",
                status, body
            )));
        }

        let json: serde_json::Value = res.json().await?;

        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(format!(
                "Token refresh failed: {}",
                error
            )));
        }

        // Get the new access token
        let token = json["access_token"]
            .as_str()
            .ok_or_else(|| {
                RedditClientError::ApiError(
                    "Failed to extract access token from response".to_string(),
                )
            })?
            .to_string();

        // Update expiration time if provided
        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);
        let now = chrono::Utc::now().timestamp() as u64;
        let token_expires_at = now + expires_in;

        // Update our token storage
        if let Some(storage) = &mut self.token_storage {
            storage.access_token = Some(token.clone());
            storage.token_expires_at = Some(token_expires_at);
            storage.last_updated = now;

            // Save the updated token storage
            self.save_token_storage()?;
        }

        // Store the token for immediate use
        self.access_token = Some(token.clone());
        debug!("Access token refreshed successfully");

        Ok(token)
    }

    /// Authenticate with browser OAuth, but first try to use a stored refresh token
    pub async fn authenticate_with_stored_or_browser(
        &mut self,
        client_id: &str,
        redirect_port: Option<u16>,
        scopes: Option<&str>,
    ) -> Result<String, RedditClientError> {
        // Make sure we have token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }

        // First try to use an existing token if it's still valid
        if let Some(storage) = &self.token_storage {
            if storage.is_access_token_valid() {
                debug!("Using existing valid access token");
                if let Some(token) = &storage.access_token {
                    self.access_token = Some(token.clone());
                    return Ok(token.clone());
                }
            }

            // If we have a refresh token, try to use it
            if storage.has_refresh_token() {
                debug!("Trying to refresh access token");
                match self.refresh_access_token().await {
                    Ok(token) => {
                        debug!("Successfully refreshed token");
                        return Ok(token);
                    }
                    Err(e) => {
                        debug!("Failed to refresh token: {}, will try browser auth", e);
                        // Continue to browser auth
                    }
                }
            }
        }

        // If we get here, we need browser authentication
        debug!("Proceeding with browser authentication");
        self.authenticate_with_browser_oauth(client_id, redirect_port, scopes)
            .await
    }

    pub async fn authenticate_with_browser_oauth(
        &mut self,
        client_id: &str,
        redirect_port: Option<u16>,
        scopes: Option<&str>,
    ) -> Result<String, RedditClientError> {
        // Setup parameters
        let port = redirect_port.unwrap_or(8080);
        let scopes = scopes.unwrap_or("identity read submit");
        let redirect_uri = format!("http://localhost:{}/callback", port);

        // Generate a random state token to prevent CSRF
        let state: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        // Create the authorization URL
        let auth_url = format!(
            "https://www.reddit.com/api/v1/authorize?client_id={}&response_type=code&state={}&redirect_uri={}&duration=permanent&scope={}",
            client_id, state, redirect_uri, scopes
        );

        // Start the local server to receive the callback
        let server = match Server::http(format!("127.0.0.1:{}", port)) {
            Ok(server) => server,
            Err(e) => {
                return Err(RedditClientError::ApiError(format!(
                    "Failed to start local server: {}",
                    e
                )))
            }
        };

        // Create a channel to receive the authorization code
        let (tx, rx) = mpsc::channel();

        // Clone values for the server thread
        let state_clone = state.clone();
        let tx_clone = tx.clone();

        // Start the server in a separate thread
        let server_thread = thread::spawn(move || {
            info!(
                "Waiting for authorization callback on http://localhost:{}/callback",
                port
            );

            // Set a timeout value - we'll exit the loop after this duration
            let _timeout_duration = Duration::from_secs(300); // 5 minutes

            // Handle incoming requests
            for request in server.incoming_requests() {
                let path = request.url();

                // Only handle the expected callback path
                if path.starts_with("/callback") {
                    debug!("Received callback: {}", path);

                    // Parse the query parameters
                    let url_str = format!("http://localhost{}", path);
                    let query = match Url::parse(&url_str) {
                        Ok(url) => {
                            // Collect into a HashMap to avoid borrowing issues
                            let pairs: HashMap<String, String> = url
                                .query_pairs()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect();
                            pairs
                        }
                        Err(_) => {
                            // Failed to parse URL, return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Error</h1><p>Invalid callback URL</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            continue;
                        }
                    };

                    // Check for errors from Reddit
                    if let Some(error) = query.get("error") {
                        tx_clone
                            .send(Err(format!("Authorization error: {}", error)))
                            .unwrap();

                        // Return an error page
                        let response = Response::from_string(format!(
                            "<html><body><h1>Authentication Error</h1><p>{}</p></body></html>",
                            error
                        ))
                        .with_status_code(StatusCode(400));
                        request.respond(response).ok();
                        break;
                    }

                    // Check for the state parameter
                    match query.get("state") {
                        Some(received_state) if received_state == &state_clone => {
                            // State matches, check for the code
                            if let Some(code) = query.get("code") {
                                tx_clone.send(Ok(code.to_string())).unwrap();

                                // Return a success page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Successful</h1><p>You can now close this window and return to the application.</p></body></html>"
                                ).with_status_code(StatusCode(200));
                                request.respond(response).ok();
                                break;
                            } else {
                                tx_clone
                                    .send(Err("No authorization code received".to_string()))
                                    .unwrap();

                                // Return an error page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Error</h1><p>No authorization code received</p></body></html>"
                                ).with_status_code(StatusCode(400));
                                request.respond(response).ok();
                                break;
                            }
                        }
                        Some(_) => {
                            tx_clone
                                .send(Err("State mismatch - possible CSRF attack".to_string()))
                                .unwrap();

                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>State mismatch - possible CSRF attack</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        }
                        None => {
                            tx_clone
                                .send(Err("No state parameter received".to_string()))
                                .unwrap();

                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>No state parameter received</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        }
                    }
                } else {
                    // Not the callback endpoint
                    let response =
                        Response::from_string("<html><body><h1>404 Not Found</h1></body></html>")
                            .with_status_code(StatusCode(404));
                    request.respond(response).ok();
                }
            }
        });

        // Open the browser to the authorization URL
        info!("Opening browser for Reddit OAuth authorization...");
        if let Err(e) = webbrowser::open(&auth_url) {
            tx.send(Err(format!("Failed to open browser: {}", e)))
                .unwrap();
        }

        // Print the URL in case the browser doesn't open
        info!("If your browser doesn't open automatically, please visit this URL:");
        info!("{}", auth_url);

        // Wait for the authorization code
        let auth_result = match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(result) => result,
            Err(_) => {
                return Err(RedditClientError::ApiError(
                    "Timed out waiting for authorization".to_string(),
                ))
            }
        };

        // Process the authorization code
        let code = match auth_result {
            Ok(code) => code,
            Err(e) => return Err(RedditClientError::ApiError(e)),
        };

        // Wait for the server thread to complete
        let _ = server_thread.join();

        // Exchange the code for an access token
        info!("Exchanging authorization code for access token...");

        let params = [
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
        ];

        // For installed apps, the auth header uses just the client_id followed by a colon
        let auth = base64::encode(format!("{}:", client_id));

        let res = self
            .client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(format!(
                "Token exchange failed: HTTP {}: {}",
                status, body
            )));
        }

        let json: serde_json::Value = res.json().await?;

        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(format!(
                "Token exchange failed: {}",
                error
            )));
        }

        // Get the access token
        let token = json["access_token"]
            .as_str()
            .ok_or_else(|| {
                RedditClientError::ApiError(
                    "Failed to extract access token from response".to_string(),
                )
            })?
            .to_string();

        // Store the refresh token if available
        if let Some(refresh_token) = json["refresh_token"].as_str() {
            debug!("Received refresh token: {}", refresh_token);
            // You could store this for later use
        }

        // Store the token for future use
        self.access_token = Some(token.clone());

        // Update token storage
        let now = chrono::Utc::now().timestamp() as u64;
        let expires_in = json["expires_in"].as_u64().unwrap_or(3600);
        let expires_at = now + expires_in;

        // Create or update our token storage
        if self.token_storage.is_none() {
            self.token_storage = Some(TokenStorage::new(client_id));
        }

        if let Some(storage) = &mut self.token_storage {
            storage.client_id = client_id.to_string();
            storage.access_token = Some(token.clone());
            storage.token_expires_at = Some(expires_at);
            storage.last_updated = now;

            // Store the refresh token if provided
            if let Some(refresh_token) = json["refresh_token"].as_str() {
                storage.refresh_token = Some(refresh_token.to_string());
                debug!("Received and stored refresh token");
            }

            // Save the token storage
            self.save_token_storage()?;
        }

        debug!("Browser OAuth authentication successful, token obtained");

        Ok(token)
    }

    /// Authenticate with Reddit using API credentials for a script app.
    ///
    /// This method is for Reddit accounts that use any login method (including Google OAuth)
    /// but requires you to create a "script" type app and provide your actual Reddit username
    /// and password.
    ///
    /// # Arguments
    /// * `client_id` - Your Reddit API client ID
    /// * `client_secret` - Your Reddit API client secret
    /// * `username` - Your Reddit username
    /// * `password` - Your Reddit password
    ///
    /// # How to get these credentials:
    /// 1. Go to https://www.reddit.com/prefs/apps
    /// 2. Click "create another app..." at the bottom
    /// 3. Select "script" as the app type (important!)
    /// 4. Fill in the name and description
    /// 5. For the redirect URI, you can use http://localhost:8080
    /// 6. After creation, the client ID is under the app name
    /// 7. The client secret is listed as "secret"
    pub async fn authenticate_with_api_credentials(
        &mut self,
        client_id: &str,
        client_secret: &str,
        username: &str,
        password: &str,
    ) -> Result<String, RedditClientError> {
        // For script apps, you must use the password grant type with your
        // actual Reddit username and password
        let params = [
            ("grant_type", "password"),
            ("username", username),
            ("password", password),
            // Include the scopes needed for posting
            ("scope", "submit identity read"),
        ];

        // For the Authorization header, use the client_id and client_secret
        let auth = base64::encode(format!("{}:{}", client_id, client_secret));

        let res = self
            .client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(format!(
                "Authentication failed: HTTP {}: {}",
                status, body
            )));
        }

        let json: serde_json::Value = res.json().await?;

        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(format!(
                "Authentication failed: {}",
                error
            )));
        }

        let token = json["access_token"]
            .as_str()
            .ok_or_else(|| {
                RedditClientError::ApiError(
                    "Failed to extract access token from response".to_string(),
                )
            })?
            .to_string();

        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!(
            "API authentication successful, token obtained with scopes: {:?}",
            json["scope"].as_str()
        );

        Ok(token)
    }

    pub async fn authenticate_user(
        &mut self,
        client_id: &str,
        username: &str,
        password: &str,
    ) -> Result<String, RedditClientError> {
        // The password grant requires these parameters
        let params = [
            ("grant_type", "password"),
            ("username", username),
            ("password", password),
            // Include the scopes needed for posting
            ("scope", "submit identity read"),
        ];

        // For script apps, you use client_id as both username and password
        let auth = base64::encode(format!("{}:", client_id));

        let res = self
            .client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(format!(
                "Authentication failed: HTTP {}: {}",
                status, body
            )));
        }

        let json: serde_json::Value = res.json().await?;

        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(format!(
                "Authentication failed: {}",
                error
            )));
        }

        let token = json["access_token"]
            .as_str()
            .ok_or_else(|| {
                RedditClientError::ApiError(
                    "Failed to extract access token from response".to_string(),
                )
            })?
            .to_string();

        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!(
            "User authentication successful, token obtained with scopes: {:?}",
            json["scope"].as_str()
        );

        Ok(token)
    }

    /// Fetch new posts from a specific subreddit
    pub async fn fetch_new_posts(
        &self,
        subreddit: &str,
        limit: i32,
    ) -> Result<RedditRNewResponse, RedditClientError> {
        // Check if we have an access token and use OAuth endpoint if we do
        let base_url = if self.access_token.is_some() {
            debug!("Using OAuth API endpoint with access token");
            "https://oauth.reddit.com/r"
        } else {
            debug!("Using public API endpoint (no access token)");
            "https://www.reddit.com/r"
        };

        let url = format!("{}/{}/new.json?limit={}", base_url, subreddit, limit);
        debug!("Fetching from subreddit URL: {}", url);
        debug!("Using User-Agent: {}", self.user_agent);

        // Create request builder
        let mut req_builder = self.client.get(&url);

        // Add authorization header if we have a token
        if let Some(token) = &self.access_token {
            debug!("Adding Authorization header with token");
            req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
        }

        // Send the request
        let response = req_builder.send().await?;
        let status = response.status();
        debug!("Response status: {}", status);

        if !status.is_success() {
            return Err(RedditClientError::ApiError(format!(
                "Server returned error status: {}",
                status
            )));
        }

        let body = response.text().await?;
        debug!("Response body length: {} bytes", body.len());

        // Parse using our specialized SubredditPostsResponse model
        let parsed = match serde_json::from_str::<SubredditPostsResponse>(&body) {
            Ok(parsed) => parsed,
            Err(e) => {
                debug!("Error parsing subreddit posts: {}", e);
                debug!("First 100 chars: {}", &body[..body.len().min(100)]);
                return Err(RedditClientError::ParseError(e));
            }
        };

        // Convert from SubredditPostsResponse to RedditRNewResponse for backwards compatibility
        let post_count = parsed.data.children.len();
        debug!("Successfully parsed {} posts from subreddit", post_count);

        // Create a RedditRNewResponse from the SubredditPostsResponse
        let result = RedditRNewResponse {
            kind: parsed.kind,
            data: crate::models::RedditPostCollection {
                after: parsed.data.after,
                dist: parsed.data.dist,
                modhash: parsed.data.modhash,
                geo_filter: parsed.data.geo_filter,
                before: parsed.data.before,
                // Convert each post entity
                children: parsed
                    .data
                    .children
                    .into_iter()
                    .map(|post| {
                        // Convert subreddit post to regular post
                        crate::models::RedditPostEntity {
                            kind: post.kind,
                            data: crate::models::RedditPostData {
                                id: post.data.id,
                                name: post.data.name,
                                title: post.data.title,
                                author: post.data.author,
                                author_fullname: post.data.author_fullname,
                                permalink: post.data.permalink,
                                url: post.data.url,
                                created_utc: post.data.created_utc,
                                is_self: post.data.is_self,
                                selftext: post.data.selftext,
                                selftext_html: post.data.selftext_html,
                                is_video: post.data.is_video,
                                is_original_content: post.data.is_original_content,
                                is_reddit_media_domain: post.data.is_reddit_media_domain,
                                is_meta: post.data.is_meta,
                                is_crosspostable: post.data.is_crosspostable,
                                thumbnail: post.data.thumbnail,
                                thumbnail_width: post.data.thumbnail_width,
                                thumbnail_height: post.data.thumbnail_height,
                                secure_media: None, // Convert if needed
                                secure_media_embed: crate::models::RedditMediaEmbed {
                                    content: post.data.secure_media_embed.content,
                                    width: post.data.secure_media_embed.width,
                                    height: post.data.secure_media_embed.height,
                                },
                                media: None, // Convert if needed
                                media_embed: crate::models::RedditMediaEmbed {
                                    content: post.data.media_embed.content,
                                    width: post.data.media_embed.width,
                                    height: post.data.media_embed.height,
                                },
                                preview: None,      // Convert if needed
                                gallery_data: None, // Convert if needed
                                media_metadata: post.data.media_metadata,
                                score: post.data.score,
                                upvote_ratio: post.data.upvote_ratio,
                                ups: post.data.ups,
                                downs: post.data.downs,
                                num_comments: post.data.num_comments,
                                num_crossposts: post.data.num_crossposts,
                                total_awards_received: post.data.total_awards_received,
                                subreddit: post.data.subreddit,
                                subreddit_id: post.data.subreddit_id,
                                subreddit_subscribers: post.data.subreddit_subscribers,
                                subreddit_type: post.data.subreddit_type,
                                subreddit_name_prefixed: post.data.subreddit_name_prefixed,
                                archived: post.data.archived,
                                locked: post.data.locked,
                                hidden: post.data.hidden,
                                removed_by_category: post.data.removed_by_category,
                                removed_by: post.data.removed_by,
                                stickied: post.data.stickied,
                                pinned: post.data.pinned,
                                spoiler: post.data.spoiler,
                                over_18: post.data.over_18,
                                hide_score: post.data.hide_score,
                                contest_mode: post.data.contest_mode,
                                edited: post.data.edited.clone(),
                                distinguished: post.data.distinguished,
                                link_flair_text: post.data.link_flair_text,
                                link_flair_type: post.data.link_flair_type,
                                link_flair_background_color: post.data.link_flair_background_color,
                                link_flair_text_color: post.data.link_flair_text_color,
                                author_flair_text: post.data.author_flair_text,
                                author_flair_type: post.data.author_flair_type,
                                author_flair_background_color: post
                                    .data
                                    .author_flair_background_color,
                                author_flair_text_color: post.data.author_flair_text_color,
                                additional_fields: post.data.additional_fields,
                            },
                        }
                    })
                    .collect(),
            },
        };

        Ok(result)
    }

    /// Fetch new posts from the public Reddit frontpage
    pub async fn fetch_public_new_posts(
        &self,
        limit: i32,
    ) -> Result<RedditRNewResponse, RedditClientError> {
        // Check if we have an access token and use OAuth endpoint if we do
        let base_url = if self.access_token.is_some() {
            debug!("Using OAuth API endpoint with access token");
            "https://oauth.reddit.com"
        } else {
            debug!("Using public API endpoint (no access token)");
            "https://www.reddit.com"
        };

        // Using the URL that shows new posts on the main feed
        let url = format!("{}/new.json?feed=home&limit={}", base_url, limit);
        debug!("Fetching from URL: {}", url);
        debug!("Using User-Agent: {}", self.user_agent);

        // Create request builder
        let mut req_builder = self.client.get(&url);

        // Add authorization header if we have a token
        if let Some(token) = &self.access_token {
            debug!("Adding Authorization header with token");
            req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
        }

        // Try to get a response from this endpoint
        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(e) => {
                debug!("Error fetching {}: {:?}", url, e);
                // If this fails, fall back to r/popular/new
                let fallback_url = format!("{}/r/popular/new.json?limit={}", base_url, limit);
                debug!("Falling back to URL: {}", fallback_url);

                let mut fallback_req = self.client.get(&fallback_url);
                if let Some(token) = &self.access_token {
                    fallback_req =
                        fallback_req.header("Authorization", format!("Bearer {}", token));
                }
                fallback_req.send().await?
            }
        };

        let status = response.status();
        debug!("Response status: {}", status);

        if !status.is_success() {
            return Err(RedditClientError::ApiError(format!(
                "Server returned error status: {}",
                status
            )));
        }

        let body = response.text().await?;
        debug!("Response body length: {} bytes", body.len());

        // Parse using our more forgiving PublicFeedResponse model
        let parsed = match serde_json::from_str::<PublicFeedResponse>(&body) {
            Ok(parsed) => parsed,
            Err(e) => {
                debug!("Error parsing public feed: {}", e);
                debug!("First 100 chars: {}", &body[..body.len().min(100)]);
                return Err(RedditClientError::ParseError(e));
            }
        };

        // Convert from PublicFeedResponse to RedditRNewResponse
        let post_count = parsed.data.children.len();
        debug!("Successfully parsed {} posts from public feed", post_count);

        // Create a RedditRNewResponse from the PublicFeedResponse
        let result = RedditRNewResponse {
            kind: parsed.kind,
            data: crate::models::RedditPostCollection {
                after: parsed.data.after,
                dist: parsed.data.dist,
                modhash: parsed.data.modhash,
                geo_filter: parsed.data.geo_filter,
                before: parsed.data.before,
                // Convert each post entity
                children: parsed
                    .data
                    .children
                    .into_iter()
                    .map(|post| {
                        // Convert public feed post to regular post
                        crate::models::RedditPostEntity {
                            kind: post.kind,
                            data: crate::models::RedditPostData {
                                id: post.data.id,
                                name: post.data.name,
                                title: post.data.title,
                                author: post.data.author,
                                author_fullname: None,
                                permalink: post.data.permalink,
                                url: post.data.url,
                                created_utc: post.data.created_utc,
                                is_self: post.data.is_self,
                                selftext: post.data.selftext,
                                selftext_html: post.data.selftext_html,
                                is_video: post.data.is_video,
                                is_original_content: post.data.is_original_content,
                                is_reddit_media_domain: post.data.is_reddit_media_domain,
                                is_meta: post.data.is_meta,
                                is_crosspostable: post.data.is_crosspostable,
                                thumbnail: post.data.thumbnail,
                                thumbnail_width: post.data.thumbnail_width,
                                thumbnail_height: post.data.thumbnail_height,
                                secure_media: None,
                                secure_media_embed: crate::models::RedditMediaEmbed {
                                    content: None,
                                    width: None,
                                    height: None,
                                },
                                media: None,
                                media_embed: crate::models::RedditMediaEmbed {
                                    content: None,
                                    width: None,
                                    height: None,
                                },
                                preview: None,
                                gallery_data: None,
                                media_metadata: None,
                                score: post.data.score,
                                upvote_ratio: post.data.upvote_ratio,
                                ups: post.data.ups,
                                downs: post.data.downs,
                                num_comments: post.data.num_comments,
                                num_crossposts: post.data.num_crossposts,
                                total_awards_received: post.data.total_awards_received,
                                subreddit: post.data.subreddit,
                                subreddit_id: post.data.subreddit_id,
                                subreddit_subscribers: post.data.subreddit_subscribers,
                                subreddit_type: post.data.subreddit_type,
                                subreddit_name_prefixed: post.data.subreddit_name_prefixed,
                                archived: post.data.archived,
                                locked: post.data.locked,
                                hidden: post.data.hidden,
                                removed_by_category: post.data.removed_by_category,
                                removed_by: post.data.removed_by,
                                stickied: post.data.stickied,
                                pinned: post.data.pinned,
                                spoiler: post.data.spoiler,
                                over_18: post.data.over_18,
                                hide_score: post.data.hide_score,
                                contest_mode: post.data.contest_mode,
                                edited: post.data.edited.clone(),
                                distinguished: post.data.distinguished,
                                link_flair_text: post.data.link_flair_text,
                                link_flair_type: post.data.link_flair_type,
                                link_flair_background_color: post.data.link_flair_background_color,
                                link_flair_text_color: post.data.link_flair_text_color,
                                author_flair_text: post.data.author_flair_text,
                                author_flair_type: post.data.author_flair_type,
                                author_flair_background_color: post
                                    .data
                                    .author_flair_background_color,
                                author_flair_text_color: post.data.author_flair_text_color,
                                additional_fields: post.data.additional_fields,
                            },
                        }
                    })
                    .collect(),
            },
        };

        Ok(result)
    }

    /// Create a new text post in a subreddit.
    ///
    /// IMPORTANT: This method requires full OAuth user authentication with the 'submit' scope.
    /// The application-only auth from get_access_token() is not sufficient for posting.
    ///
    /// To post content, you need to:
    /// 1. Create a Reddit OAuth app (script type) at https://www.reddit.com/prefs/apps
    /// 2. Get username and password credentials from your Reddit account
    /// 3. Implement the password OAuth flow with the 'submit' scope
    ///
    /// This method will attempt to post, but will return a helpful error if the token lacks
    /// the required permissions.
    pub async fn create_post(
        &self,
        subreddit: &str,
        title: &str,
        text: &str,
    ) -> Result<String, RedditClientError> {
        // Ensure we have an access token
        let token = match &self.access_token {
            Some(token) => token,
            None => {
                return Err(RedditClientError::ApiError(
                    "No access token available. Call get_access_token() first.".to_string(),
                ))
            }
        };

        // Clean up the subreddit name - remove r/ if it's there
        let subreddit_clean = if subreddit.starts_with("r/") {
            &subreddit[2..]
        } else {
            subreddit
        };

        let mut params = HashMap::new();
        params.insert("sr", subreddit_clean);
        params.insert("title", title);
        params.insert("text", text);
        params.insert("kind", "self"); // "self" for text post, "link" for link post

        let url = "https://oauth.reddit.com/api/submit";

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        // Check if request was successful
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(RedditClientError::ApiError(format!(
                "Failed to create post: HTTP {}: {}",
                status, text
            )));
        }

        // Parse the response
        let json: serde_json::Value = response.json().await?;
        debug!("Post creation response: {:?}", json);

        // Check for common error messages
        if json["success"].as_bool() == Some(false) {
            // Check for user required error
            if let Some(jquery) = json["jquery"].as_array() {
                for item in jquery {
                    if let Some(call_args) = item[3].as_array() {
                        if call_args.len() > 0
                            && call_args[0].as_str() == Some(".error.USER_REQUIRED")
                        {
                            return Err(RedditClientError::ApiError(
                                "Reddit requires user authentication with 'submit' scope to create posts. The current authentication method (application-only) only supports reading public data. You need to implement the full OAuth flow with a Reddit account.".to_string()
                            ));
                        }
                    }

                    // Extract error message if present
                    if item[2].as_str() == Some("call") {
                        if let Some(call_args) = item[3].as_array() {
                            if call_args.len() > 0 {
                                if let Some(err_msg) = call_args[0].as_str() {
                                    if err_msg.starts_with("Please") || err_msg.contains("error") {
                                        return Err(RedditClientError::ApiError(format!(
                                            "Reddit API error: {}",
                                            err_msg
                                        )));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If there's an explicit error in the response, return it
        if let Some(errors) = json["json"]["errors"].as_array() {
            if !errors.is_empty() {
                return Err(RedditClientError::ApiError(format!(
                    "Reddit API returned an error: {:?}",
                    errors
                )));
            }
        }

        // Check if the post was successful
        if json["success"].as_bool() == Some(true) {
            // In the success case, look for a redirect URL in the jQuery response
            if let Some(jquery) = json["jquery"].as_array() {
                for item in jquery {
                    // Look for the jquery call with redirect attribute
                    if item[2].as_str() == Some("attr") && item[3].as_str() == Some("redirect") {
                        // The next item contains the URL in the call parameter
                        let next_index = item[1].as_u64().unwrap_or(0) as usize;
                        if next_index < jquery.len()
                            && jquery[next_index][2].as_str() == Some("call")
                            && jquery[next_index][3].as_array().is_some()
                            && jquery[next_index][3].as_array().unwrap().len() > 0
                        {
                            if let Some(url) = jquery[next_index][3][0].as_str() {
                                return Ok(url.to_string());
                            }
                        }
                    }
                }
            }
        }

        // The standard way to extract the URL
        if let Some(url) = json["json"]["data"]["url"].as_str() {
            return Ok(url.to_string());
        }

        // For debugging purposes, print the entire response
        debug!(
            "Full response structure: {}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );

        // If we got this far, check if we can at least tell if it was successful
        if json["success"].as_bool() == Some(true) {
            // The post was successful but we couldn't extract the URL for some reason
            return Ok("Post was successful, but couldn't extract the URL".to_string());
        }

        Err(RedditClientError::ApiError(
            "Failed to create post. Reddit requires user authentication with proper scopes for this operation.".to_string()
        ))
    }

    /// Create a comment on a post or another comment.
    ///
    /// # Arguments
    /// * `thing_id` - The fullname of the parent thing (post or comment) to comment on
    ///   Format is "t3_" followed by post ID for posts, or "t1_" followed by comment ID for comments
    /// * `text` - The comment text content
    ///
    /// # Note
    /// This method requires full OAuth user authentication with the 'submit' scope.
    /// The application-only auth from get_access_token() is not sufficient for commenting.
    pub async fn create_comment(
        &self,
        thing_id: &str,
        text: &str,
    ) -> Result<String, RedditClientError> {
        // Ensure we have an access token
        let token = match &self.access_token {
            Some(token) => token,
            None => {
                return Err(RedditClientError::ApiError(
                    "No access token available. Call get_access_token() first.".to_string(),
                ))
            }
        };

        let mut params = HashMap::new();
        params.insert("api_type", "json");
        params.insert("thing_id", thing_id);
        params.insert("text", text);

        let url = "https://oauth.reddit.com/api/comment";

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await?;

        // Check if request was successful
        if !response.status().is_success() {
            let status = response.status();
            let response_text = response.text().await?;
            return Err(RedditClientError::ApiError(format!(
                "Failed to create comment: HTTP {}: {}",
                status, response_text
            )));
        }

        // Parse the response
        let json: serde_json::Value = response.json().await?;
        debug!("Comment creation response: {:?}", json);

        // Check for API errors
        if let Some(errors) = json["json"]["errors"].as_array() {
            if !errors.is_empty() {
                return Err(RedditClientError::ApiError(format!(
                    "Reddit API returned an error: {:?}",
                    errors
                )));
            }
        }

        // Check for user required error
        if json.get("error").is_some() && json["error"].as_i64() == Some(403) {
            return Err(RedditClientError::ApiError(
                "Reddit requires user authentication with 'submit' scope to create comments. The current authentication method (application-only) only supports reading public data.".to_string()
            ));
        }

        // Extract the comment ID and permalink if available
        if let Some(things) = json["json"]["data"]["things"].as_array() {
            if !things.is_empty() {
                if let (Some(_), Some(permalink)) = (
                    things[0]["data"]["name"].as_str(),
                    things[0]["data"]["permalink"].as_str(),
                ) {
                    return Ok(format!("https://reddit.com{}", permalink));
                }

                // Fallback if permalink is not available
                if let Some(thing_id) = things[0]["data"]["name"].as_str() {
                    return Ok(format!(
                        "Comment created successfully with ID: {}",
                        thing_id
                    ));
                }
            }
        }

        // For debugging purposes, print the entire response
        debug!(
            "Full comment response structure: {}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );

        // Fallback success message if we couldn't extract the details
        Ok("Comment was created successfully, but couldn't extract the details".to_string())
    }
}
