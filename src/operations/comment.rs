use crate::client::RedditClient;
use log::{error, info};

/// Configuration options for creating a comment on Reddit
#[derive(Debug, Clone)]
pub struct CommentOptions {
    /// The fullname of the parent thing (post or comment) to comment on
    /// Format is "t3_" followed by post ID for posts, or "t1_" followed by comment ID for comments
    pub thing_id: String,
    /// Text content of the comment
    pub text: String,
    /// Reddit client ID for OAuth
    pub client_id: String,
}

/// Result of a comment creation operation
#[derive(Debug)]
pub struct CommentResult {
    /// Whether the comment was successfully created
    pub success: bool,
    /// URL or ID of the created comment (if successful)
    pub comment_url: Option<String>,
    /// Formatted message for CLI output
    pub message: String,
}

/// Operation for creating a comment on a post or another comment
pub struct CommentOperation {
    /// Configuration options for the operation
    options: CommentOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl CommentOperation {
    /// Create a new comment operation with the provided options
    pub fn new(options: CommentOptions) -> Self {
        let client = RedditClient::new();
        Self { options, client }
    }

    /// Create a new comment operation with a custom Reddit client
    pub fn with_client(options: CommentOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the comment creation operation
    pub async fn execute(&mut self) -> Result<CommentResult, crate::client::RedditClientError> {
        info!(
            "Creating a new comment on thing_id: {}",
            self.options.thing_id
        );

        // First get an access token
        match self.client.get_access_token(&self.options.client_id).await {
            Ok(_) => info!("Successfully authenticated with Reddit API"),
            Err(err) => {
                let message = format!("Failed to authenticate with Reddit API: {:?}", err);
                error!("{}", message);

                return Ok(CommentResult {
                    success: false,
                    comment_url: None,
                    message,
                });
            }
        }

        // Now create the comment
        match self
            .client
            .create_comment(&self.options.thing_id, &self.options.text)
            .await
        {
            Ok(url) => {
                let message = format!("Comment created successfully! URL or ID: {}", url);
                // We don't need to log here as the handler function will log the message
                // Removed: info!("{}", message);

                Ok(CommentResult {
                    success: true,
                    comment_url: Some(url),
                    message,
                })
            }
            Err(err) => {
                let message = format!(
                    "Error creating comment: {:?}\n\nNote: Commenting requires OAuth authentication with the 'submit' scope.",
                    err
                );
                // We don't need to log here as the handler function will log the message
                // Removed: error!("{}", message);

                Ok(CommentResult {
                    success: false,
                    comment_url: None,
                    message,
                })
            }
        }
    }
}

/// CLI handler function for comment command (attempts app-only auth, but will likely need OAuth)
pub async fn handle_comment_command(
    thing_id: String,
    text: String,
    client_id: String,
) -> Result<(), crate::client::RedditClientError> {
    let options = CommentOptions {
        thing_id,
        text,
        client_id,
    };

    let mut operation = CommentOperation::new(options);
    match operation.execute().await {
        Ok(result) => {
            if result.success {
                println!("{}", result.message);
            } else {
                eprintln!("{}", result.message);
            }
            Ok(())
        }
        Err(err) => {
            error!("Error executing comment operation: {:?}", err);
            Err(err)
        }
    }
}

/// CLI handler function for browser comment command
pub async fn handle_browser_comment_command(
    thing_id: String,
    text: String,
    client_id: String,
    port: Option<u16>,
) -> Result<(), crate::client::RedditClientError> {
    let options = CommentOptions {
        thing_id,
        text,
        client_id: client_id.clone(),
    };

    // Create a new client with stored tokens
    let mut client = RedditClient::with_stored_tokens(&client_id);

    // Authenticate with browser OAuth
    info!("Authenticating with Reddit via browser OAuth...");
    match client
        .authenticate_with_stored_or_browser(&client_id, port, Some("identity read submit"))
        .await
    {
        Ok(_) => {
            info!("Successfully authenticated with Reddit API via browser OAuth");

            // Now that we have an authenticated client, create the comment
            let mut operation = CommentOperation::with_client(options, client);
            match operation.execute().await {
                Ok(result) => {
                    if result.success {
                        println!("{}", result.message);
                    } else {
                        eprintln!("{}", result.message);
                    }
                    Ok(())
                }
                Err(err) => {
                    error!("Error executing comment operation: {:?}", err);
                    Err(err)
                }
            }
        }
        Err(err) => {
            let message = format!("Failed to authenticate with Reddit: {:?}", err);
            error!("{}", message);
            Err(err)
        }
    }
}

/// CLI handler function for user comment command
pub async fn handle_user_comment_command(
    thing_id: String,
    text: String,
    client_id: String,
    username: String,
    password: String,
) -> Result<(), crate::client::RedditClientError> {
    let options = CommentOptions {
        thing_id,
        text,
        client_id: client_id.clone(),
    };

    // Create a new client
    let mut client = RedditClient::new();

    // Authenticate with username/password
    info!("Authenticating with Reddit using username/password...");
    match client
        .authenticate_user(&client_id, &username, &password)
        .await
    {
        Ok(_) => {
            info!("Successfully authenticated with Reddit API using username/password");

            // Now that we have an authenticated client, create the comment
            let mut operation = CommentOperation::with_client(options, client);
            match operation.execute().await {
                Ok(result) => {
                    if result.success {
                        println!("{}", result.message);
                    } else {
                        eprintln!("{}", result.message);
                    }
                    Ok(())
                }
                Err(err) => {
                    error!("Error executing comment operation: {:?}", err);
                    Err(err)
                }
            }
        }
        Err(err) => {
            let message = format!("Failed to authenticate with Reddit: {:?}", err);
            error!("{}", message);
            Err(err)
        }
    }
}
