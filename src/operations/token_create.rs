use crate::client::RedditClient;
use log::{error, info};

/// Configuration options for creating a post with manual tokens
#[derive(Debug, Clone)]
pub struct TokenCreateOptions {
    /// The name of the subreddit to post to
    pub subreddit: String,
    /// Title of the post
    pub title: String,
    /// Text content of the post
    pub text: String,
    /// Reddit client ID for OAuth
    pub client_id: String,
    /// The access token obtained from Reddit OAuth
    pub access_token: String,
    /// The refresh token obtained from Reddit OAuth (if available)
    pub refresh_token: Option<String>,
    /// Time in seconds until the access token expires
    pub expires_in: u64,
}

/// Result of a token-authenticated post creation operation
#[derive(Debug)]
pub struct TokenCreateResult {
    /// Whether the post was successfully created
    pub success: bool,
    /// URL of the created post (if successful)
    pub post_url: Option<String>,
    /// Formatted message for CLI output
    pub message: String,
}

/// Operation for creating a post on Reddit using manual tokens
pub struct TokenCreateOperation {
    /// Configuration options for the operation
    options: TokenCreateOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl TokenCreateOperation {
    /// Create a new token-authenticated post creation operation with the provided options
    pub fn new(options: TokenCreateOptions) -> Self {
        let client = RedditClient::new();
        Self { options, client }
    }

    /// Create a new token-authenticated post creation operation with a custom Reddit client
    pub fn with_client(options: TokenCreateOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the token-authenticated post creation operation
    pub async fn execute(&mut self) -> Result<TokenCreateResult, crate::client::RedditClientError> {
        // Prepare subreddit display format
        let display_sub = if self.options.subreddit.starts_with("r/") {
            self.options.subreddit.clone()
        } else {
            format!("r/{}", self.options.subreddit)
        };

        info!(
            "Creating a new post in {} using provided token: '{}'",
            display_sub, self.options.title
        );

        // Set the tokens directly
        match self.client.set_tokens(
            &self.options.client_id,
            &self.options.access_token,
            self.options.refresh_token.as_deref(),
            self.options.expires_in,
        ) {
            Ok(_) => info!("Successfully set manual tokens"),
            Err(err) => {
                let message = format!("Failed to set tokens: {:?}", err);
                error!("{}", message);

                return Ok(TokenCreateResult {
                    success: false,
                    post_url: None,
                    message,
                });
            }
        }

        // Now create the post
        info!("Using provided token to create post...");
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

                Ok(TokenCreateResult {
                    success: true,
                    post_url: Some(url),
                    message,
                })
            }
            Err(err) => {
                let message = format!("Error creating post: {:?}", err);
                error!("{}", message);

                Ok(TokenCreateResult {
                    success: false,
                    post_url: None,
                    message,
                })
            }
        }
    }
}

/// CLI handler function for token_create command
pub async fn handle_token_create_command(
    subreddit: String,
    title: String,
    text: String,
    client_id: String,
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
) -> Result<(), crate::client::RedditClientError> {
    let options = TokenCreateOptions {
        subreddit,
        title,
        text,
        client_id,
        access_token,
        refresh_token,
        expires_in,
    };

    let mut operation = TokenCreateOperation::new(options);
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
            error!("Error executing token_create operation: {:?}", err);
            Err(err)
        }
    }
}
