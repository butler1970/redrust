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
    /// Time in seconds until the access token expires (when using with_client)
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

/// CLI handler function for token_create command with client
pub async fn handle_token_create_command_with_client(
    subreddit: String,
    title: String,
    text: String,
    expires_in: u64,
    client: RedditClient,
) -> Result<(), crate::client::RedditClientError> {
    let options = TokenCreateOptions {
        subreddit,
        title,
        text,
        expires_in,
    };

    let mut operation = TokenCreateOperation::with_client(options, client);
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
