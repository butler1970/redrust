use crate::client::RedditClient;
use log::{error, info};

/// Configuration options for creating a post with browser-based authentication
#[derive(Debug, Clone)]
pub struct BrowserCreateOptions {
    /// The name of the subreddit to post to
    pub subreddit: String,
    /// Title of the post
    pub title: String,
    /// Text content of the post
    pub text: String,
    /// Reddit client ID for OAuth
    pub client_id: String,
    /// Port to use for the localhost callback (default: 8080)
    pub port: Option<u16>,
}

/// Result of a browser-authenticated post creation operation
#[derive(Debug)]
pub struct BrowserCreateResult {
    /// Whether the post was successfully created
    pub success: bool,
    /// URL of the created post (if successful)
    pub post_url: Option<String>,
    /// Formatted message for CLI output
    pub message: String,
    /// Whether existing stored tokens were used instead of browser auth
    pub used_stored_tokens: bool,
}

/// Operation for creating a post on Reddit using browser-based OAuth authentication
pub struct BrowserCreateOperation {
    /// Configuration options for the operation
    options: BrowserCreateOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl BrowserCreateOperation {
    /// Create a new browser-authenticated post creation operation with the provided options
    pub fn new(options: BrowserCreateOptions) -> Self {
        // Use stored tokens if available
        let client = RedditClient::with_stored_tokens(&options.client_id);
        Self { options, client }
    }

    /// Create a new browser-authenticated post creation operation with a custom Reddit client
    pub fn with_client(options: BrowserCreateOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the browser-authenticated post creation operation
    pub async fn execute(
        &mut self,
    ) -> Result<BrowserCreateResult, crate::client::RedditClientError> {
        // Prepare subreddit display format
        let display_sub = if self.options.subreddit.starts_with("r/") {
            self.options.subreddit.clone()
        } else {
            format!("r/{}", self.options.subreddit)
        };

        info!(
            "Creating a new post in {} via browser authentication: '{}'",
            display_sub, self.options.title
        );

        // Try to authenticate with stored tokens first, falling back to browser OAuth
        info!("Checking for stored OAuth tokens...");

        let used_stored_tokens;
        match self
            .client
            .authenticate_with_stored_or_browser(
                &self.options.client_id,
                self.options.port,
                Some("identity submit read"),
            )
            .await
        {
            Ok(_) => {
                if self
                    .client
                    .token_storage
                    .as_ref()
                    .map_or(false, |s| s.is_access_token_valid())
                {
                    info!("Using existing OAuth token (no browser login required)");
                    used_stored_tokens = true;
                } else if self
                    .client
                    .token_storage
                    .as_ref()
                    .map_or(false, |s| s.has_refresh_token())
                {
                    info!("Successfully refreshed OAuth token (no browser login required)");
                    used_stored_tokens = true;
                } else {
                    info!("Successfully authenticated with Reddit API via browser");
                    used_stored_tokens = false;
                }
            }
            Err(err) => {
                let message = format!("Failed to authenticate with Reddit API: {:?}", err);
                error!("{}", message);

                return Ok(BrowserCreateResult {
                    success: false,
                    post_url: None,
                    message,
                    used_stored_tokens: false,
                });
            }
        }

        // Now create the post
        info!("Authentication successful! Creating post...");
        match self
            .client
            .create_post(
                &self.options.subreddit,
                &self.options.title,
                &self.options.text,
            )
            .await
        {
            Ok(url) => {
                let message = format!("Post created successfully! URL: {}", url);
                info!("{}", message);

                Ok(BrowserCreateResult {
                    success: true,
                    post_url: Some(url),
                    message,
                    used_stored_tokens,
                })
            }
            Err(err) => {
                let message = format!("Error creating post: {:?}", err);
                error!("{}", message);

                Ok(BrowserCreateResult {
                    success: false,
                    post_url: None,
                    message,
                    used_stored_tokens,
                })
            }
        }
    }
}

/// CLI handler function for browser_create command
pub async fn handle_browser_create_command(
    subreddit: String,
    title: String,
    text: String,
    client_id: String,
    port: Option<u16>,
) -> Result<(), crate::client::RedditClientError> {
    let options = BrowserCreateOptions {
        subreddit,
        title,
        text,
        client_id,
        port,
    };

    let mut operation = BrowserCreateOperation::new(options);
    match operation.execute().await {
        Ok(result) => {
            if result.success {
                info!("{}", result.message);
            } else {
                error!("{}", result.message);
            }
            Ok(())
        }
        Err(err) => {
            error!("Error executing browser_create operation: {:?}", err);
            Err(err)
        }
    }
}
