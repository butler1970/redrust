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
    /// Command to manage posts.
    Posts {
        /// The name of the subreddit to manage.
        #[arg(help = "Subreddit name", required = true)]
        subreddit: String,

        #[arg(help = "Number of posts to retrieve", required = true)]
        count: i32,
    },
}

#[tokio::main]
async fn main() {
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
        }
    }
}