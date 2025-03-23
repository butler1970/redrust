use crate::client::RedditClient;
use log::{error, info};

/// Configuration options for creating a post on Reddit
#[derive(Debug, Clone)]
pub struct CreateOptions {
    /// The name of the subreddit to post to
    pub subreddit: String,
    /// Title of the post
    pub title: String,
    /// Text content of the post
    pub text: String,
    /// Reddit client ID for OAuth
    pub client_id: String,
}

/// Result of a post creation operation
#[derive(Debug)]
pub struct CreateResult {
    /// Whether the post was successfully created
    pub success: bool,
    /// URL of the created post (if successful)
    pub post_url: Option<String>,
    /// Formatted message for CLI output
    pub message: String,
}

/// Operation for creating a post on Reddit using application-only authentication
pub struct CreateOperation {
    /// Configuration options for the operation
    options: CreateOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl CreateOperation {
    /// Create a new post creation operation with the provided options
    pub fn new(options: CreateOptions) -> Self {
        let client = RedditClient::new();
        Self { options, client }
    }

    /// Create a new post creation operation with a custom Reddit client
    pub fn with_client(options: CreateOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the post creation operation
    pub async fn execute(&mut self) -> Result<CreateResult, crate::client::RedditClientError> {
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

        // First get an access token
        match self.client.get_access_token(&self.options.client_id).await {
            Ok(_) => info!("Successfully authenticated with Reddit API"),
            Err(err) => {
                let message = format!("Failed to authenticate with Reddit API: {:?}", err);
                error!("{}", message);

                return Ok(CreateResult {
                    success: false,
                    post_url: None,
                    message,
                });
            }
        }

        // Now create the post
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

                Ok(CreateResult {
                    success: true,
                    post_url: Some(url),
                    message,
                })
            }
            Err(err) => {
                let message = format!("Error creating post: {:?}", err);
                error!("{}", message);

                Ok(CreateResult {
                    success: false,
                    post_url: None,
                    message,
                })
            }
        }
    }
}

/// CLI handler function for create command
pub async fn handle_create_command(
    subreddit: String,
    title: String,
    text: String,
    client_id: String,
) -> Result<(), crate::client::RedditClientError> {
    let options = CreateOptions {
        subreddit,
        title,
        text,
        client_id,
    };

    let mut operation = CreateOperation::new(options);
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
            error!("Error executing create operation: {:?}", err);
            Err(err)
        }
    }
}
