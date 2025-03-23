use crate::client::RedditClient;
use log::{error, info};

/// Configuration options for creating a post with API credentials
#[derive(Debug, Clone)]
pub struct ApiCreateOptions {
    /// The name of the subreddit to post to
    pub subreddit: String,
    /// Title of the post
    pub title: String,
    /// Text content of the post
    pub text: String,
    /// Reddit client ID for OAuth
    pub client_id: String,
    /// Reddit client secret
    pub client_secret: String,
    /// Reddit username
    pub username: String,
    /// Reddit password
    pub password: String,
}

/// Result of an API-authenticated post creation operation
#[derive(Debug)]
pub struct ApiCreateResult {
    /// Whether the post was successfully created
    pub success: bool,
    /// URL of the created post (if successful)
    pub post_url: Option<String>,
    /// Formatted message for CLI output
    pub message: String,
}

/// Operation for creating a post on Reddit using API credentials
pub struct ApiCreateOperation {
    /// Configuration options for the operation
    options: ApiCreateOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl ApiCreateOperation {
    /// Create a new API-authenticated post creation operation with the provided options
    pub fn new(options: ApiCreateOptions) -> Self {
        let client = RedditClient::new();
        Self { options, client }
    }

    /// Create a new API-authenticated post creation operation with a custom Reddit client
    pub fn with_client(options: ApiCreateOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the API-authenticated post creation operation
    pub async fn execute(&mut self) -> Result<ApiCreateResult, crate::client::RedditClientError> {
        // Prepare subreddit display format
        let display_sub = if self.options.subreddit.starts_with("r/") {
            self.options.subreddit.clone()
        } else {
            format!("r/{}", self.options.subreddit)
        };

        info!(
            "Creating a new post in {} as {} using script app credentials: '{}'",
            display_sub, self.options.username, self.options.title
        );

        // Authenticate with API credentials
        match self
            .client
            .authenticate_with_api_credentials(
                &self.options.client_id,
                &self.options.client_secret,
                &self.options.username,
                &self.options.password,
            )
            .await
        {
            Ok(_) => {
                info!("Successfully authenticated with Reddit API using script app credentials")
            }
            Err(err) => {
                let message = format!("Failed to authenticate with Reddit API: {:?}", err);
                error!("{}", message);

                return Ok(ApiCreateResult {
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

                Ok(ApiCreateResult {
                    success: true,
                    post_url: Some(url),
                    message,
                })
            }
            Err(err) => {
                let message = format!("Error creating post: {:?}", err);
                error!("{}", message);

                Ok(ApiCreateResult {
                    success: false,
                    post_url: None,
                    message,
                })
            }
        }
    }
}

/// CLI handler function for api_create command
pub async fn handle_api_create_command(
    subreddit: String,
    title: String,
    text: String,
    client_id: String,
    client_secret: String,
    username: String,
    password: String,
) -> Result<(), crate::client::RedditClientError> {
    let options = ApiCreateOptions {
        subreddit,
        title,
        text,
        client_id,
        client_secret,
        username,
        password,
    };

    let mut operation = ApiCreateOperation::new(options);
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
            error!("Error executing api_create operation: {:?}", err);
            Err(err)
        }
    }
}
