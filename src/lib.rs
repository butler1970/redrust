//! Redrust: A Rust wrapper for the Reddit API
//!
//! This library provides a convenient interface for interacting with the Reddit API,
//! allowing you to fetch posts, create new posts, add comments, and more.

pub mod client;
pub mod config;
pub mod models;
pub mod operations;

// Re-export the most commonly used types for convenience
pub use client::RedditClient;
pub use client::RedditClientError;
pub use config::AppConfig;
pub use operations::api_create::{ApiCreateOperation, ApiCreateOptions, ApiCreateResult};
pub use operations::browser_create::{
    BrowserCreateOperation, BrowserCreateOptions, BrowserCreateResult,
};
pub use operations::comment::{CommentOperation, CommentOptions, CommentResult};
pub use operations::create::{CreateOperation, CreateOptions, CreateResult};
pub use operations::posts::{PostsOperation, PostsOptions, PostsResult};
pub use operations::token_create::{TokenCreateOperation, TokenCreateOptions, TokenCreateResult};
pub use operations::user_create::{UserCreateOperation, UserCreateOptions, UserCreateResult};

// Re-export the client-aware handler functions for convenient usage
pub use operations::api_create::handle_api_create_command_with_client;
pub use operations::browser_create::handle_browser_create_command_with_client;
pub use operations::comment::{
    handle_browser_comment_command_with_client, handle_comment_command_with_client,
    handle_user_comment_command_with_client,
};
pub use operations::create::handle_create_command_with_client;
pub use operations::posts::handle_posts_command_with_client;
pub use operations::token_create::handle_token_create_command_with_client;
pub use operations::user_create::handle_user_create_command_with_client;
