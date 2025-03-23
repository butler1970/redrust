use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "redrust",
    author = "Robert Butler",
    version = "1.0",
    about = "Rust wrapper for the Reddit API."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand, Debug)]
pub enum Commands {
    /// Command to fetch posts from a subreddit or the public frontpage.
    Posts {
        /// The number of posts to retrieve.
        #[arg(long, short, help = "Number of posts to retrieve", required = true)]
        count: i32,

        /// The name of the subreddit to fetch posts from.
        /// If not provided, posts from the public Reddit frontpage will be retrieved.
        #[arg(long, short, help = "Subreddit name (optional)", required = false)]
        subreddit: Option<String>,

        /// Display posts in a brief, one-line format.
        #[arg(
            long,
            short,
            help = "Show posts in a brief one-line format",
            required = false
        )]
        brief: bool,
    },

    /// Command to create a new post in a subreddit.
    /// Requires app-only authentication which won't allow posting.
    /// Note: REDDIT_CLIENT_ID must be set in your environment or .env file.
    Create {
        /// The name of the subreddit to post to.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        /// Title of the post.
        #[arg(help = "Post title", required = true)]
        title: String,

        /// Text content of the post.
        #[arg(help = "Post text content", required = true)]
        text: String,
    },

    /// Create a post using user authentication (username/password).
    /// For this to work, your app must be registered as a "script" type app.
    /// NOTE: This won't work for accounts that use Google OAuth login.
    /// Note: REDDIT_CLIENT_ID, REDDIT_USERNAME, and REDDIT_PASSWORD must be set in your environment or .env file.
    UserCreate {
        /// The name of the subreddit to post to.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        /// Title of the post.
        #[arg(help = "Post title", required = true)]
        title: String,

        /// Text content of the post.
        #[arg(help = "Post text content", required = true)]
        text: String,
    },

    /// Create a post using browser-based OAuth authentication.
    /// RECOMMENDED for accounts using Google OAuth login.
    /// Requires creating an installed app in Reddit preferences first.
    /// Note: REDDIT_CLIENT_ID must be set in your environment or .env file.
    BrowserCreate {
        /// The name of the subreddit to post to.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        /// Title of the post.
        #[arg(help = "Post title", required = true)]
        title: String,

        /// Text content of the post.
        #[arg(help = "Post text content", required = true)]
        text: String,

        /// Port to use for the localhost callback (default: 8080).
        #[arg(help = "Port to use for the OAuth callback", required = false)]
        port: Option<u16>,
    },

    /// Create a post using manual tokens (for headless environments).
    /// Use this when you have obtained tokens separately and want to use
    /// them without browser authentication.
    /// Note: REDDIT_CLIENT_ID, REDDIT_ACCESS_TOKEN, and optionally REDDIT_REFRESH_TOKEN
    /// must be set in your environment or .env file.
    TokenCreate {
        /// The name of the subreddit to post to.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        /// Title of the post.
        #[arg(help = "Post title", required = true)]
        title: String,

        /// Text content of the post.
        #[arg(help = "Post text content", required = true)]
        text: String,

        /// Time in seconds until the access token expires.
        #[arg(help = "Token expiration time in seconds", default_value = "3600")]
        expires_in: u64,
    },

    /// Create a post using a script application's API credentials.
    /// Works with any Reddit account (including Google OAuth logins).
    /// Requires creating a script app in Reddit preferences first.
    /// Note: REDDIT_CLIENT_ID, REDDIT_CLIENT_SECRET, REDDIT_USERNAME, and REDDIT_PASSWORD
    /// must be set in your environment or .env file.
    ApiCreate {
        /// The name of the subreddit to post to.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        /// Title of the post.
        #[arg(help = "Post title", required = true)]
        title: String,

        /// Text content of the post.
        #[arg(help = "Post text content", required = true)]
        text: String,
    },

    /// Create a comment on a post or another comment.
    /// Requires full OAuth authentication (same as posting).
    /// Note: REDDIT_CLIENT_ID must be set in your environment or .env file.
    Comment {
        /// The fullname of the parent thing (post or comment) to comment on.
        /// Format is "t3_" followed by post ID for posts, or "t1_" followed by comment ID for comments.
        #[arg(
            help = "Reddit thing ID to comment on (e.g., 't3_abcdef' for posts)",
            required = true
        )]
        thing_id: String,

        /// Text content of the comment.
        #[arg(help = "Comment text content", required = true)]
        text: String,
    },

    /// Create a comment using browser-based OAuth authentication.
    /// RECOMMENDED for accounts using Google OAuth login.
    /// Note: REDDIT_CLIENT_ID must be set in your environment or .env file.
    BrowserComment {
        /// The fullname of the parent thing (post or comment) to comment on.
        /// Format is "t3_" followed by post ID for posts, or "t1_" followed by comment ID for comments.
        #[arg(
            help = "Reddit thing ID to comment on (e.g., 't3_abcdef' for posts)",
            required = true
        )]
        thing_id: String,

        /// Text content of the comment.
        #[arg(help = "Comment text content", required = true)]
        text: String,

        /// Port to use for the localhost callback (default: 8080).
        #[arg(help = "Port to use for the OAuth callback", required = false)]
        port: Option<u16>,
    },

    /// Create a comment using user authentication (username/password).
    /// For this to work, your app must be registered as a "script" type app.
    /// NOTE: This won't work for accounts that use Google OAuth login.
    /// Note: REDDIT_CLIENT_ID, REDDIT_USERNAME, and REDDIT_PASSWORD must be set in your environment or .env file.
    UserComment {
        /// The fullname of the parent thing (post or comment) to comment on.
        /// Format is "t3_" followed by post ID for posts, or "t1_" followed by comment ID for comments.
        #[arg(
            help = "Reddit thing ID to comment on (e.g., 't3_abcdef' for posts)",
            required = true
        )]
        thing_id: String,

        /// Text content of the comment.
        #[arg(help = "Comment text content", required = true)]
        text: String,
    },
}
