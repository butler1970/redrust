use chrono::DateTime;
use chrono_tz::America::Los_Angeles;
use clap::Parser;
use log::{error, info};
use redrust::client::RedditClient;

#[derive(Parser, Debug)]
#[command(
    name = "redrust",
    author = "Robert Butler",
    version = "1.0",
    about = "Rust wrapper for the Reddit API."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
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

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Posts {
            subreddit,
            count,
            brief,
        } => {
            // Use a more descriptive user agent to avoid filtering
            let client = RedditClient::with_user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36 redrust/1.0 (by /u/Aggravating-Fix-3871)".to_string()
            );

            // Fetch posts from either a specific subreddit or the public frontpage
            let posts_result = match subreddit {
                Some(sub) => {
                    info!("Gathering new posts from r/{}", sub);
                    client.fetch_new_posts(sub, *count).await
                }
                None => {
                    info!("Gathering new posts from the public Reddit frontpage");
                    client.fetch_public_new_posts(*count).await
                }
            };

            match posts_result {
                Ok(response) => {
                    if response.data.children.is_empty() {
                        println!("No posts found.");
                        return;
                    }

                    println!("Found {} posts", response.data.children.len());

                    if *brief {
                        // Brief one-line format with simplified type indicators
                        for (i, post) in response.data.children.iter().enumerate() {
                            let local_time =
                                DateTime::from_timestamp(post.data.created_utc as i64, 0)
                                    .unwrap()
                                    .with_timezone(&Los_Angeles);
                            let timestamp_str = local_time.format("%H:%M").to_string();

                            // Determine post type indicator with a single character
                            let (post_type, _type_code) = if post.data.is_self {
                                ("T", "Text") // Text post
                            } else if post.data.is_video {
                                ("V", "Video") // Video
                            } else if post.data.url.contains("i.redd.it")
                                || post.data.url.contains("imgur.com")
                            {
                                ("I", "Image") // Image
                            } else if post.data.url.contains("reddit.com/gallery") {
                                ("G", "Gallery") // Gallery
                            } else {
                                ("L", "Link") // Link
                            };

                            // Truncate the title if necessary (30 chars), safely handling UTF-8
                            let title = if post.data.title.chars().count() > 30 {
                                let mut chars =
                                    post.data.title.chars().take(27).collect::<String>();
                                chars.push_str("...");
                                chars
                            } else {
                                post.data.title.clone()
                            };

                            // Get content excerpt or URL
                            let content = if post.data.is_self {
                                // For text posts, get a brief excerpt
                                let text = post.data.selftext.trim();
                                if text.is_empty() {
                                    "[No content]".to_string()
                                } else if text.chars().count() > 30 {
                                    let excerpt = text
                                        .chars()
                                        .take(27)
                                        .collect::<String>()
                                        .replace('\n', " ");
                                    format!("\"{}...\"", excerpt)
                                } else {
                                    format!("\"{}\"", text.replace('\n', " "))
                                }
                            } else {
                                // For non-text posts, get the URL (could be hyperlinked in some terminals)
                                // Note: Basic terminal support for hyperlinks uses this format: \x1B]8;;url\x07text\x1B]8;;\x07
                                // But not all terminals support this, so we'll just show a simplified URL
                                let url_display = if post.data.url.len() > 30 {
                                    let shortened_url = if post.data.url.starts_with("https://") {
                                        post.data.url[8..].to_string() // Remove https:// for display
                                    } else if post.data.url.starts_with("http://") {
                                        post.data.url[7..].to_string() // Remove http:// for display
                                    } else {
                                        post.data.url.clone()
                                    };

                                    if shortened_url.len() > 30 {
                                        format!("{:.27}...", shortened_url)
                                    } else {
                                        shortened_url
                                    }
                                } else {
                                    post.data.url.clone()
                                };

                                url_display
                            };

                            // Construct permalink URL
                            let permalink = format!("https://reddit.com{}", post.data.permalink);

                            println!(
                                "{:2}. [{}] [{}] {} ({}) r/{} | {}",
                                i + 1,
                                post_type,
                                timestamp_str,
                                title,
                                content,
                                post.data.subreddit,
                                permalink
                            );
                        }

                        // Print a legend for the post type indicators
                        println!("\nPost Type Legend:");
                        println!("[T] = Text post");
                        println!("[V] = Video");
                        println!("[I] = Image");
                        println!("[G] = Gallery");
                        println!("[L] = Link");
                    } else {
                        // Detailed format
                        for post in response.data.children {
                            let local_time =
                                DateTime::from_timestamp(post.data.created_utc as i64, 0)
                                    .unwrap()
                                    .with_timezone(&Los_Angeles);
                            let timestamp_str = local_time.format("%Y-%m-%d %H:%M:%S").to_string();

                            // Display post with more details
                            println!("\n============ POST =============");
                            println!("[{}] [Los Angeles time]", timestamp_str);
                            println!("{}", post.data.format_summary());
                            println!("================================\n");
                        }
                    }
                }
                Err(err) => error!("Error fetching posts: {:?}", err),
            }
        }
        Commands::Create {
            subreddit,
            title,
            text,
            client_id,
        } => {
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
        }
        Commands::BrowserCreate {
            subreddit,
            title,
            text,
            client_id,
            port,
        } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!(
                "Creating a new post in {} via browser authentication: '{}'",
                display_sub, title
            );

            // Use stored tokens if available
            let mut client = RedditClient::with_stored_tokens(client_id);

            // Try to authenticate with stored tokens first, falling back to browser OAuth
            info!("Checking for stored OAuth tokens...");

            match client
                .authenticate_with_stored_or_browser(client_id, *port, Some("identity submit read"))
                .await
            {
                Ok(_) => {
                    if client
                        .token_storage
                        .as_ref()
                        .map_or(false, |s| s.is_access_token_valid())
                    {
                        info!("Using existing OAuth token (no browser login required)");
                    } else if client
                        .token_storage
                        .as_ref()
                        .map_or(false, |s| s.has_refresh_token())
                    {
                        info!("Successfully refreshed OAuth token (no browser login required)");
                    } else {
                        info!("Successfully authenticated with Reddit API via browser");
                    }
                }
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
        }
        Commands::TokenCreate {
            subreddit,
            title,
            text,
            client_id,
            access_token,
            refresh_token,
            expires_in,
        } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!(
                "Creating a new post in {} using provided token: '{}'",
                display_sub, title
            );

            // Create a client and set the tokens
            let mut client = RedditClient::new();

            // Set the tokens directly
            match client.set_tokens(
                client_id,
                access_token,
                refresh_token.as_deref(),
                *expires_in,
            ) {
                Ok(_) => info!("Successfully set manual tokens"),
                Err(err) => {
                    error!("Failed to set tokens: {:?}", err);
                    return;
                }
            }

            // Now create the post
            info!("Using provided token to create post...");
            match client.create_post(subreddit, title, text).await {
                Ok(url) => info!("Post created successfully! URL: {}", url),
                Err(err) => error!("Error creating post: {:?}", err),
            }
        }
        Commands::UserCreate {
            subreddit,
            title,
            text,
            client_id,
            username,
            password,
        } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!(
                "Creating a new post in {} as user {}: '{}'",
                display_sub, username, title
            );

            let mut client = RedditClient::new();

            // Authenticate with username and password
            match client
                .authenticate_user(client_id, username, password)
                .await
            {
                Ok(_) => info!(
                    "Successfully authenticated with Reddit API as user {}",
                    username
                ),
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
        Commands::ApiCreate {
            subreddit,
            title,
            text,
            client_id,
            client_secret,
            username,
            password,
        } => {
            // Handle subreddit format - don't add r/ if it's already there
            let display_sub = if subreddit.starts_with("r/") {
                subreddit.to_string()
            } else {
                format!("r/{}", subreddit)
            };
            info!(
                "Creating a new post in {} as {} using script app credentials: '{}'",
                display_sub, username, title
            );

            let mut client = RedditClient::new();

            // Authenticate with API credentials
            match client
                .authenticate_with_api_credentials(client_id, client_secret, username, password)
                .await
            {
                Ok(_) => {
                    info!("Successfully authenticated with Reddit API using script app credentials")
                }
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
