use reqwest::{Client, Error as ReqwestError};
use crate::RedditRNewResponse;
use log::{debug, info};
use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use rand::{Rng, distributions::Alphanumeric};
use tiny_http::{Server, Response, StatusCode};
use webbrowser;
use url::Url;

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

#[derive(Clone)]
pub struct RedditClient {
    pub client: Client,
    pub access_token: Option<String>,
    pub user_agent: String,
}

impl RedditClient {
    pub fn new() -> Self {
        let user_agent = format!("redrust/1.0 (by /u/Aggravating-Fix-3871)");
        Self {
            client: Self::get_client(&user_agent).unwrap(),
            access_token: None,
            user_agent,
        }
    }
    
    pub fn with_user_agent(user_agent: String) -> Self {
        Self {
            client: Self::get_client(&user_agent).unwrap(),
            access_token: None,
            user_agent,
        }
    }

    fn get_client(user_agent: &str) -> Result<Client, RedditClientError> {
        Ok(Client::builder()
            .user_agent(user_agent)
            .build()?)
    }

    /// Get an application-only access token for reading public data.
    /// 
    /// This method gets a token that can only be used for reading public data.
    /// It cannot be used for actions that require a user account like posting,
    /// commenting, or voting.
    pub async fn get_access_token(&mut self, client_id: &str) -> Result<String, RedditClientError> {
        let params = [
            ("grant_type", "https://oauth.reddit.com/grants/installed_client"),
            ("device_id", "DO_NOT_TRACK_THIS_DEVICE")
        ];

        // Note: Since there is no client secret, the authorization is created using your client_id followed by a colon.
        let auth = base64::encode(format!("{}:", client_id));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        let json: serde_json::Value = res.json().await?;
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
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
            Err(e) => return Err(RedditClientError::ApiError(
                format!("Failed to start local server: {}", e)
            )),
        };
        
        // Create a channel to receive the authorization code
        let (tx, rx) = mpsc::channel();
        
        // Clone values for the server thread
        let state_clone = state.clone();
        let tx_clone = tx.clone();
        
        // Start the server in a separate thread
        let server_thread = thread::spawn(move || {
            info!("Waiting for authorization callback on http://localhost:{}/callback", port);
            
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
                            let pairs: HashMap<String, String> = url.query_pairs()
                                .map(|(k, v)| (k.to_string(), v.to_string()))
                                .collect();
                            pairs
                        },
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
                        tx_clone.send(Err(format!("Authorization error: {}", error))).unwrap();
                        
                        // Return an error page
                        let response = Response::from_string(format!(
                            "<html><body><h1>Authentication Error</h1><p>{}</p></body></html>",
                            error
                        )).with_status_code(StatusCode(400));
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
                                tx_clone.send(Err("No authorization code received".to_string())).unwrap();
                                
                                // Return an error page
                                let response = Response::from_string(
                                    "<html><body><h1>Authentication Error</h1><p>No authorization code received</p></body></html>"
                                ).with_status_code(StatusCode(400));
                                request.respond(response).ok();
                                break;
                            }
                        },
                        Some(_) => {
                            tx_clone.send(Err("State mismatch - possible CSRF attack".to_string())).unwrap();
                            
                            // Return an error page
                            let response = Response::from_string(
                                "<html><body><h1>Authentication Error</h1><p>State mismatch - possible CSRF attack</p></body></html>"
                            ).with_status_code(StatusCode(400));
                            request.respond(response).ok();
                            break;
                        },
                        None => {
                            tx_clone.send(Err("No state parameter received".to_string())).unwrap();
                            
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
                    let response = Response::from_string(
                        "<html><body><h1>404 Not Found</h1></body></html>"
                    ).with_status_code(StatusCode(404));
                    request.respond(response).ok();
                }
            }
        });
        
        // Open the browser to the authorization URL
        info!("Opening browser for Reddit OAuth authorization...");
        if let Err(e) = webbrowser::open(&auth_url) {
            tx.send(Err(format!("Failed to open browser: {}", e))).unwrap();
        }
        
        // Print the URL in case the browser doesn't open
        info!("If your browser doesn't open automatically, please visit this URL:");
        info!("{}", auth_url);
        
        // Wait for the authorization code
        let auth_result = match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(result) => result,
            Err(_) => {
                return Err(RedditClientError::ApiError(
                    "Timed out waiting for authorization".to_string()
                ))
            }
        };
        
        // Process the authorization code
        let code = match auth_result {
            Ok(code) => code,
            Err(e) => {
                return Err(RedditClientError::ApiError(e))
            }
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
        
        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Token exchange failed: HTTP {}: {}", status, body)
            ));
        }
        
        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Token exchange failed: {}", error)
            ));
        }
        
        // Get the access token
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the refresh token if available
        if let Some(refresh_token) = json["refresh_token"].as_str() {
            debug!("Received refresh token: {}", refresh_token);
            // You could store this for later use
        }
        
        // Store the token for future use
        self.access_token = Some(token.clone());
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
        password: &str
    ) -> Result<String, RedditClientError> {
        // For script apps, you must use the password grant type with your
        // actual Reddit username and password
        let params = [
            ("grant_type", "password"),
            ("username", username),
            ("password", password),
            // Include the scopes needed for posting
            ("scope", "submit identity read")
        ];

        // For the Authorization header, use the client_id and client_secret
        let auth = base64::encode(format!("{}:{}", client_id, client_secret));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: HTTP {}: {}", status, body)
            ));
        }

        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: {}", error)
            ));
        }
        
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!("API authentication successful, token obtained with scopes: {:?}", json["scope"].as_str());
        
        Ok(token)
    }
    
    pub async fn authenticate_user(
        &mut self,
        client_id: &str,
        username: &str,
        password: &str
    ) -> Result<String, RedditClientError> {
        // The password grant requires these parameters
        let params = [
            ("grant_type", "password"),
            ("username", username),
            ("password", password),
            // Include the scopes needed for posting
            ("scope", "submit identity read")
        ];

        // For script apps, you use client_id as both username and password
        let auth = base64::encode(format!("{}:", client_id));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;
            
        // Check for HTTP errors
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await?;
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: HTTP {}: {}", status, body)
            ));
        }

        let json: serde_json::Value = res.json().await?;
        
        // Check for API errors
        if let Some(error) = json["error"].as_str() {
            return Err(RedditClientError::ApiError(
                format!("Authentication failed: {}", error)
            ));
        }
        
        let token = json["access_token"].as_str()
            .ok_or_else(|| RedditClientError::ApiError(
                "Failed to extract access token from response".to_string()
            ))?.to_string();
        
        // Store the token for future use
        self.access_token = Some(token.clone());
        debug!("User authentication successful, token obtained with scopes: {:?}", json["scope"].as_str());
        
        Ok(token)
    }

    pub async fn fetch_new_posts(&self, subreddit: &str, limit: i32) -> Result<RedditRNewResponse, RedditClientError> {
        let url = format!("https://www.reddit.com/r/{}/new.json?limit={}", subreddit, limit);
        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;

        let response: RedditRNewResponse = serde_json::from_str(&body)?;
        Ok(response)
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
    pub async fn create_post(&self, subreddit: &str, title: &str, text: &str) -> Result<String, RedditClientError> {
        // Ensure we have an access token
        let token = match &self.access_token {
            Some(token) => token,
            None => return Err(RedditClientError::ApiError(
                "No access token available. Call get_access_token() first.".to_string()
            )),
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
        params.insert("kind", "self");  // "self" for text post, "link" for link post
        
        let url = "https://oauth.reddit.com/api/submit";
        
        let response = self.client
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
            return Err(RedditClientError::ApiError(
                format!("Failed to create post: HTTP {}: {}", status, text)
            ));
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
                        if call_args.len() > 0 && call_args[0].as_str() == Some(".error.USER_REQUIRED") {
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
                                        return Err(RedditClientError::ApiError(
                                            format!("Reddit API error: {}", err_msg)
                                        ));
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
                return Err(RedditClientError::ApiError(
                    format!("Reddit API returned an error: {:?}", errors)
                ));
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
                           && jquery[next_index][3].as_array().unwrap().len() > 0 {
                            
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
        debug!("Full response structure: {}", serde_json::to_string_pretty(&json).unwrap_or_default());
        
        // If we got this far, check if we can at least tell if it was successful
        if json["success"].as_bool() == Some(true) {
            // The post was successful but we couldn't extract the URL for some reason
            return Ok("Post was successful, but couldn't extract the URL".to_string());
        }
        
        Err(RedditClientError::ApiError(
            "Failed to create post. Reddit requires user authentication with proper scopes for this operation.".to_string()
        ))
    }
}


