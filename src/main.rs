use chrono::DateTime;
use chrono_tz::America::Los_Angeles;
use log::{info, error};
use redrust::client::RedditClient;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "redrust",
    author = "Robert Butler",
    version = "1.0",
    about = "Rust wrapper for the Reddit API.",
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Command to fetch posts from a subreddit.
    Posts {
        /// The name of the subreddit to manage.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        #[arg(help = "Number of posts to retrieve", required = true)]
        count: i32,
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

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Posts { subreddit, count } => {
            info!("Gathering new posts from r/{}", subreddit);
            
            let client = RedditClient::new();

            match client.fetch_new_posts(subreddit, *count).await {
                Ok(response) => {
                    for post in response.data.children {
                        let timestamp: i64 = post.data.created_utc as i64;
                        let timestamp = DateTime::from_timestamp(timestamp, 0).unwrap().with_timezone(&Los_Angeles);
                        let timestamp_str = timestamp.format("%Y-%m-%d %H:%M:%S").to_string();
                        info!("[{}] {} by {}", timestamp_str, post.data.title, post.data.author);
                    }
                }
                Err(err) => error!("Error fetching posts: {:?}", err),
            }
        },
        Commands::Create { subreddit, title, text, client_id } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!("Creating a new post in {}: '{}'", display_sub, title);
            
            let mut client = RedditClient::new();
            
            // First get an access token
            match client.get_access_token(client_id).await {
                Ok(_) => info!("Successfully authenticated with Reddit API"),
                Err(err) => {
                    error!("Failed to authenticate with Reddit API: {:?}", err);
                    return;
                }
            }
            
            // Now create the post
            match client.create_post(subreddit, title, text).await {
                Ok(url) => info!("Post created successfully! URL: {}", url),
                Err(err) => error!("Error creating post: {:?}", err),
            }
        },
        Commands::BrowserCreate { subreddit, title, text, client_id, port } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!("Creating a new post in {} via browser authentication: '{}'", display_sub, title);
            
            let mut client = RedditClient::new();
            
            // Authenticate with browser OAuth
            info!("Starting browser authentication flow. This will open your default web browser.");
            info!("Please log in with your Reddit account (including Google OAuth if that's how you normally log in).");
            info!("After logging in, you'll need to authorize the application to access your Reddit account.");
            
            match client.authenticate_with_browser_oauth(client_id, *port, Some("identity submit read")).await {
                Ok(_) => info!("Successfully authenticated with Reddit API via browser"),
                Err(err) => {
                    error!("Failed to authenticate with Reddit API: {:?}", err);
                    return;
                }
            }
            
            // Now create the post
            info!("Authentication successful! Creating post...");
            match client.create_post(subreddit, title, text).await {
                Ok(url) => info!("Post created successfully! URL: {}", url),
                Err(err) => error!("Error creating post: {:?}", err),
            }
        },
        Commands::UserCreate { subreddit, title, text, client_id, username, password } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!("Creating a new post in {} as user {}: '{}'", display_sub, username, title);
            
            let mut client = RedditClient::new();
            
            // Authenticate with username and password
            match client.authenticate_user(client_id, username, password).await {
                Ok(_) => info!("Successfully authenticated with Reddit API as user {}", username),
                Err(err) => {
                    error!("Failed to authenticate with Reddit API: {:?}", err);
                    return;
                }
            }
            
            // Now create the post
            match client.create_post(subreddit, title, text).await {
                Ok(url) => info!("Post created successfully! URL: {}", url),
                Err(err) => error!("Error creating post: {:?}", err),
            }
        },
        Commands::ApiCreate { subreddit, title, text, client_id, client_secret, username, password } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!("Creating a new post in {} as {} using script app credentials: '{}'", display_sub, username, title);
            
            let mut client = RedditClient::new();
            
            // Authenticate with API credentials
            match client.authenticate_with_api_credentials(client_id, client_secret, username, password).await {
                Ok(_) => info!("Successfully authenticated with Reddit API using script app credentials"),
                Err(err) => {
                    error!("Failed to authenticate with Reddit API: {:?}", err);
                    return;
                }
            }
            
            // Now create the post
            match client.create_post(subreddit, title, text).await {
                Ok(url) => info!("Post created successfully! URL: {}", url),
                Err(err) => error!("Error creating post: {:?}", err),
            }
        }
    }
}