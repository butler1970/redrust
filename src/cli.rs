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

        /// Your Reddit client ID.
        #[arg(help = "Reddit client ID for OAuth", required = true)]
        client_id: String,
    },

    /// Create a post using user authentication (username/password).
    /// For this to work, your app must be registered as a "script" type app.
    /// NOTE: This won't work for accounts that use Google OAuth login.
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

        /// Your Reddit client ID.
        #[arg(help = "Reddit client ID for OAuth", required = true)]
        client_id: String,

        /// Reddit username.
        #[arg(help = "Reddit username", required = true)]
        username: String,

        /// Reddit password.
        #[arg(help = "Reddit password", required = true)]
        password: String,
    },

    /// Create a post using browser-based OAuth authentication.
    /// RECOMMENDED for accounts using Google OAuth login.
    /// Requires creating an installed app in Reddit preferences first.
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

        /// Your Reddit API client ID.
        #[arg(help = "Client ID from your Reddit installed app", required = true)]
        client_id: String,

        /// Port to use for the localhost callback (default: 8080).
        #[arg(help = "Port to use for the OAuth callback", required = false)]
        port: Option<u16>,
    },

    /// Create a post using manual tokens (for headless environments).
    /// Use this when you have obtained tokens separately and want to use
    /// them without browser authentication.
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

        /// Your Reddit API client ID.
        #[arg(help = "Client ID from your Reddit app", required = true)]
        client_id: String,

        /// The access token obtained from Reddit OAuth.
        #[arg(help = "OAuth access token", required = true)]
        access_token: String,

        /// The refresh token obtained from Reddit OAuth (if available).
        #[arg(help = "OAuth refresh token", required = false)]
        refresh_token: Option<String>,

        /// Time in seconds until the access token expires.
        #[arg(help = "Token expiration time in seconds", default_value = "3600")]
        expires_in: u64,
    },

    /// Create a post using a script application's API credentials.
    /// Works with any Reddit account (including Google OAuth logins).
    /// Requires creating a script app in Reddit preferences first.
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

        /// Your Reddit API client ID.
        #[arg(help = "Client ID from your Reddit script app", required = true)]
        client_id: String,

        /// Your Reddit API client secret.
        #[arg(help = "Client secret from your Reddit script app", required = true)]
        client_secret: String,

        /// Your Reddit username.
        #[arg(help = "Your Reddit username", required = true)]
        username: String,

        /// Your Reddit password.
        #[arg(help = "Your Reddit password", required = true)]
        password: String,
    },
}
