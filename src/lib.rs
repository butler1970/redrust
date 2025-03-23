//! Redrust: A Rust wrapper for the Reddit API
//!
//! This library provides a convenient interface for interacting with the Reddit API,
//! allowing you to fetch posts, create new posts, and more.

pub mod client;
pub mod models;
pub mod operations;

// Re-export the most commonly used types for convenience
pub use client::RedditClient;
pub use client::RedditClientError;
pub use operations::api_create::{ApiCreateOperation, ApiCreateOptions, ApiCreateResult};
pub use operations::browser_create::{
    BrowserCreateOperation, BrowserCreateOptions, BrowserCreateResult,
};
pub use operations::create::{CreateOperation, CreateOptions, CreateResult};
pub use operations::posts::{PostsOperation, PostsOptions, PostsResult};
pub use operations::token_create::{TokenCreateOperation, TokenCreateOptions, TokenCreateResult};
pub use operations::user_create::{UserCreateOperation, UserCreateOptions, UserCreateResult};
