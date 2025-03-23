use crate::client::RedditClient;
use log::{error, info};

/// Configuration options for creating a post with user authentication
#[derive(Debug, Clone)]
pub struct UserCreateOptions {
    /// The name of the subreddit to post to
    pub subreddit: String,
    /// Title of the post
    pub title: String,
    /// Text content of the post
    pub text: String,
}

/// Result of a user-authenticated post creation operation
#[derive(Debug)]
pub struct UserCreateResult {
    /// Whether the post was successfully created
    pub success: bool,
    /// URL of the created post (if successful)
    pub post_url: Option<String>,
    /// Formatted message for CLI output
    pub message: String,
}

/// Operation for creating a post on Reddit using user authentication (username/password)
pub struct UserCreateOperation {
    /// Configuration options for the operation
    options: UserCreateOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl UserCreateOperation {
    /// Create a new user-authenticated post creation operation with the provided options
    pub fn new(options: UserCreateOptions) -> Self {
        let client = RedditClient::new();
        Self { options, client }
    }

    /// Create a new user-authenticated post creation operation with a custom Reddit client
    pub fn with_client(options: UserCreateOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the user-authenticated post creation operation
    pub async fn execute(&mut self) -> Result<UserCreateResult, crate::client::RedditClientError> {
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

                Ok(UserCreateResult {
                    success: true,
                    post_url: Some(url),
                    message,
                })
            }
            Err(err) => {
                let message = format!("Error creating post: {:?}", err);
                error!("{}", message);

                Ok(UserCreateResult {
                    success: false,
                    post_url: None,
                    message,
                })
            }
        }
    }
}

/// CLI handler function for user_create command that accepts a preconfigured client
pub async fn handle_user_create_command_with_client(
    subreddit: String,
    title: String,
    text: String,
    client: RedditClient,
) -> Result<(), crate::client::RedditClientError> {
    let options = UserCreateOptions {
        subreddit,
        title,
        text,
    };

    let mut operation = UserCreateOperation::with_client(options, client);
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
            error!("Error executing user_create operation: {:?}", err);
            Err(err)
        }
    }
}
