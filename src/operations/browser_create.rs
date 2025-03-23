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
    pub fn new(options: BrowserCreateOptions, client_id: &str) -> Self {
        // Use stored tokens if available
        let client = RedditClient::with_stored_tokens(client_id);
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
            "Creating a new post in {}: '{}'",
            display_sub, self.options.title
        );

        // Assume client is already properly authenticated
        let used_stored_tokens = self.client.token_storage.as_ref().map_or(false, |s| {
            s.is_access_token_valid() || s.has_refresh_token()
        });

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
                // We don't need to log here as the handler function will log the message
                // Removed: info!("{}", message);

                Ok(BrowserCreateResult {
                    success: true,
                    post_url: Some(url),
                    message,
                    used_stored_tokens,
                })
            }
            Err(err) => {
                let message = format!("Error creating post: {:?}", err);
                // We don't need to log here as the handler function will log the message
                // Removed: error!("{}", message);

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

/// CLI handler function for browser_create command with client
pub async fn handle_browser_create_command_with_client(
    subreddit: String,
    title: String,
    text: String,
    port: Option<u16>,
    client: RedditClient,
) -> Result<(), crate::client::RedditClientError> {
    let options = BrowserCreateOptions {
        subreddit,
        title,
        text,
        port,
    };

    let mut operation = BrowserCreateOperation::with_client(options, client);
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
