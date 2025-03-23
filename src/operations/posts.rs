use crate::client::RedditClient;
use crate::models::RedditRNewResponse;
use chrono::DateTime;
use chrono_tz::America::Los_Angeles;
use log::{error, info};

/// Configuration options for fetching posts
#[derive(Debug, Clone)]
pub struct PostsOptions {
    /// The number of posts to retrieve
    pub count: i32,
    /// The name of the subreddit to fetch posts from (None for public frontpage)
    pub subreddit: Option<String>,
    /// Display posts in a brief, one-line format
    pub brief: bool,
    /// Custom user agent for the Reddit client (optional)
    pub user_agent: Option<String>,
}

impl Default for PostsOptions {
    fn default() -> Self {
        Self {
            count: 10,
            subreddit: None,
            brief: false,
            user_agent: None,
        }
    }
}

/// Result of a posts fetch operation
#[derive(Debug)]
pub struct PostsResult {
    /// The number of posts found
    pub post_count: usize,
    /// Formatted output (for CLI display)
    pub formatted_output: String,
    /// The raw API response data
    pub raw_response: RedditRNewResponse,
}

/// Operation for fetching posts from Reddit
pub struct PostsOperation {
    /// Configuration options for the operation
    options: PostsOptions,
    /// Reddit client for API interactions
    client: RedditClient,
}

impl PostsOperation {
    /// Create a new posts operation with the provided options
    pub fn new(options: PostsOptions) -> Self {
        // Create a client with user agent if provided, otherwise use a default
        let client = match &options.user_agent {
            Some(user_agent) => RedditClient::with_user_agent(user_agent.clone()),
            None => RedditClient::with_user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36 redrust/1.0 (by /u/Aggravating-Fix-3871)".to_string()
            ),
        };

        Self { options, client }
    }

    /// Create a new posts operation with a custom Reddit client
    pub fn with_client(options: PostsOptions, client: RedditClient) -> Self {
        Self { options, client }
    }

    /// Execute the posts operation
    pub async fn execute(&self) -> Result<PostsResult, crate::client::RedditClientError> {
        // Fetch posts from either a specific subreddit or the public frontpage
        info!(
            "Fetching {} posts from {}",
            self.options.count,
            self.options
                .subreddit
                .as_deref()
                .unwrap_or("public frontpage")
        );

        let posts_result = match &self.options.subreddit {
            Some(sub) => self.client.fetch_new_posts(sub, self.options.count).await,
            None => self.client.fetch_public_new_posts(self.options.count).await,
        }?;

        // Generate formatted output for display
        let mut output = String::new();

        if posts_result.data.children.is_empty() {
            output.push_str("No posts found.\n");
        } else {
            output.push_str(&format!(
                "Found {} posts\n",
                posts_result.data.children.len()
            ));

            if self.options.brief {
                // Implementation of brief format output
                self.format_brief_output(&posts_result, &mut output);

                // Print a legend for the post type indicators
                output.push_str("\nPost Type Legend:\n");
                output.push_str("[T] = Text post\n");
                output.push_str("[V] = Video\n");
                output.push_str("[I] = Image\n");
                output.push_str("[G] = Gallery\n");
                output.push_str("[L] = Link\n");
            } else {
                // Implementation of detailed format output
                self.format_detailed_output(&posts_result, &mut output);
            }
        }

        Ok(PostsResult {
            post_count: posts_result.data.children.len(),
            formatted_output: output,
            raw_response: posts_result,
        })
    }

    // Internal helper method for brief output format
    fn format_brief_output(&self, response: &RedditRNewResponse, output: &mut String) {
        for (i, post) in response.data.children.iter().enumerate() {
            let local_time = DateTime::from_timestamp(post.data.created_utc as i64, 0)
                .unwrap()
                .with_timezone(&Los_Angeles);
            let timestamp_str = local_time.format("%H:%M").to_string();

            // Create the API thing_id (t3_ prefix for posts)
            let thing_id = format!("t3_{}", post.data.id);

            // Determine post type indicator with a single character
            let (post_type, _type_code) = if post.data.is_self {
                ("T", "Text") // Text post
            } else if post.data.is_video {
                ("V", "Video") // Video
            } else if post.data.url.contains("i.redd.it") || post.data.url.contains("imgur.com") {
                ("I", "Image") // Image
            } else if post.data.url.contains("reddit.com/gallery") {
                ("G", "Gallery") // Gallery
            } else {
                ("L", "Link") // Link
            };

            // Truncate the title if necessary (30 chars), safely handling UTF-8
            let title = if post.data.title.chars().count() > 30 {
                let mut chars = post.data.title.chars().take(27).collect::<String>();
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
                    let excerpt = text.chars().take(27).collect::<String>().replace('\n', " ");
                    format!("\"{}...\"", excerpt)
                } else {
                    format!("\"{}\"", text.replace('\n', " "))
                }
            } else {
                // For non-text posts, get shortened URL
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

            output.push_str(&format!(
                "{:2}. [{}] [{}] {} ({}) r/{} | ID: {} | {}\n",
                i + 1,
                post_type,
                timestamp_str,
                title,
                content,
                post.data.subreddit,
                thing_id,
                permalink
            ));
        }
    }

    // Internal helper method for detailed output format
    fn format_detailed_output(&self, response: &RedditRNewResponse, output: &mut String) {
        for post in &response.data.children {
            let local_time = DateTime::from_timestamp(post.data.created_utc as i64, 0)
                .unwrap()
                .with_timezone(&Los_Angeles);
            let timestamp_str = local_time.format("%Y-%m-%d %H:%M:%S").to_string();

            // Create the API thing_id (t3_ prefix for posts)
            let thing_id = format!("t3_{}", post.data.id);

            // Display post with more details
            output.push_str("\n============ POST =============\n");
            output.push_str(&format!("[{}] [Los Angeles time]\n", timestamp_str));
            output.push_str(&format!(
                "Thing ID: {} (use this for commenting)\n",
                thing_id
            ));
            output.push_str(&post.data.format_summary());
            output.push_str("\n================================\n\n");
        }
    }
}

/// CLI handler function for posts command
pub async fn handle_posts_command(
    count: i32,
    subreddit: Option<String>,
    brief: bool,
) -> Result<(), crate::client::RedditClientError> {
    let options = PostsOptions {
        count,
        subreddit,
        brief,
        user_agent: None,
    };

    // Create a new operation with the default client
    let operation = PostsOperation::new(options);
    match operation.execute().await {
        Ok(result) => {
            // Print the formatted output to the console
            print!("{}", result.formatted_output);
            Ok(())
        }
        Err(err) => {
            error!("Error fetching posts: {:?}", err);
            Err(err)
        }
    }
}

/// CLI handler function for posts command that accepts a preconfigured client
pub async fn handle_posts_command_with_client(
    count: i32,
    subreddit: Option<String>,
    brief: bool,
    client: RedditClient,
) -> Result<(), crate::client::RedditClientError> {
    let options = PostsOptions {
        count,
        subreddit,
        brief,
        user_agent: None,
    };

    // Create a new operation with the provided client
    let operation = PostsOperation::with_client(options, client);
    match operation.execute().await {
        Ok(result) => {
            // Print the formatted output to the console
            print!("{}", result.formatted_output);
            Ok(())
        }
        Err(err) => {
            error!("Error fetching posts: {:?}", err);
            Err(err)
        }
    }
}
