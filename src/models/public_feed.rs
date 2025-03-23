use serde::Deserialize;
use std::collections::HashMap;

/// A simpler model for the public feed response
/// This handles more cases with less required fields
#[derive(Deserialize, Debug)]
pub struct PublicFeedResponse {
    pub kind: String,
    pub data: PublicFeedData,
}

#[derive(Deserialize, Debug)]
pub struct PublicFeedData {
    pub after: Option<String>,
    #[serde(default)]
    pub dist: i32,
    #[serde(default)]
    pub modhash: String,
    // geo_filter can be null in some responses
    pub geo_filter: Option<String>,
    pub children: Vec<PublicFeedPostEntity>,
    pub before: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct PublicFeedPostEntity {
    pub kind: String,
    pub data: PublicFeedPostData,
}

/// A more forgiving post data model that handles public feed posts
#[derive(Deserialize, Debug)]
pub struct PublicFeedPostData {
    // Required core fields
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub subreddit: String,
    #[serde(default)]
    pub permalink: String,
    #[serde(default)]
    pub url: String,
    pub created_utc: f64,

    // Optional fields with defaults
    #[serde(default)]
    pub name: String,

    // Post type and content
    #[serde(default)]
    pub is_self: bool,
    #[serde(default)]
    pub selftext: String,
    pub selftext_html: Option<String>,
    #[serde(default)]
    pub is_video: bool,
    #[serde(default)]
    pub is_original_content: bool,
    #[serde(default)]
    pub is_reddit_media_domain: bool,
    #[serde(default)]
    pub is_meta: bool,
    #[serde(default)]
    pub is_crosspostable: bool,

    // Media-related fields
    #[serde(default)]
    pub thumbnail: String,
    pub thumbnail_width: Option<i32>,
    pub thumbnail_height: Option<i32>,
    pub secure_media: Option<serde_json::Value>,
    #[serde(default)]
    pub secure_media_embed: HashMap<String, serde_json::Value>,
    pub media: Option<serde_json::Value>,
    #[serde(default)]
    pub media_embed: HashMap<String, serde_json::Value>,
    pub preview: Option<serde_json::Value>,
    pub gallery_data: Option<serde_json::Value>,
    pub media_metadata: Option<HashMap<String, serde_json::Value>>,

    // Post metrics
    #[serde(default)]
    pub score: i32,
    #[serde(default)]
    pub upvote_ratio: f32,
    #[serde(default)]
    pub ups: i32,
    #[serde(default)]
    pub downs: i32,
    #[serde(default)]
    pub num_comments: i32,
    #[serde(default)]
    pub num_crossposts: i32,
    #[serde(default)]
    pub total_awards_received: i32,

    // Subreddit information
    #[serde(default)]
    pub subreddit_id: String,
    #[serde(default)]
    pub subreddit_subscribers: i32,
    #[serde(default)]
    pub subreddit_type: String,
    #[serde(default)]
    pub subreddit_name_prefixed: String,

    // Post status and moderation
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub hidden: bool,
    pub removed_by_category: Option<String>,
    pub removed_by: Option<String>,
    #[serde(default)]
    pub stickied: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub spoiler: bool,
    #[serde(default)]
    pub over_18: bool,

    // Flags and display options
    #[serde(default)]
    pub hide_score: bool,
    #[serde(default)]
    pub contest_mode: bool,
    #[serde(default = "default_edited_value")]
    pub edited: serde_json::Value,
    pub distinguished: Option<String>,

    // Flair information
    pub link_flair_text: Option<String>,
    pub link_flair_type: Option<String>,
    pub link_flair_background_color: Option<String>,
    pub link_flair_text_color: Option<String>,
    pub author_flair_text: Option<String>,
    pub author_flair_type: Option<String>,
    pub author_flair_background_color: Option<String>,
    pub author_flair_text_color: Option<String>,

    // Additional fields we don't explicitly model
    #[serde(flatten)]
    pub additional_fields: HashMap<String, serde_json::Value>,
}

fn default_edited_value() -> serde_json::Value {
    serde_json::Value::Bool(false)
}

impl PublicFeedPostData {
    /// Format a post for display with important metadata
    pub fn format_summary(&self) -> String {
        let mut content = format!(
            "Title: {}\nAuthor: u/{}\nSubreddit: r/{}\nScore: {} ({}% upvoted) | Comments: {}\n",
            self.title,
            self.author,
            self.subreddit,
            self.score,
            (self.upvote_ratio * 100.0) as i32,
            self.num_comments,
        );

        // Add post type indicators
        let mut flags = Vec::new();
        if self.is_self {
            flags.push("Self Post");
        }
        if self.over_18 {
            flags.push("NSFW");
        }
        if self.spoiler {
            flags.push("Spoiler");
        }
        if self.is_video {
            flags.push("Video");
        }
        if self.is_original_content {
            flags.push("OC");
        }
        if self.stickied {
            flags.push("Stickied");
        }
        if self.locked {
            flags.push("Locked");
        }
        if !flags.is_empty() {
            content.push_str(&format!("Flags: [{}]\n", flags.join(", ")));
        }

        // Add flair if available
        if let Some(flair) = &self.link_flair_text {
            if !flair.is_empty() {
                content.push_str(&format!("Flair: {}\n", flair));
            }
        }

        // For text posts, include the text (truncated if long)
        if self.is_self && !self.selftext.is_empty() {
            let text = if self.selftext.len() > 500 {
                format!("{}...", &self.selftext[..500])
            } else {
                self.selftext.clone()
            };
            content.push_str("\nContent:\n---------\n");
            content.push_str(&text);
            content.push_str("\n---------\n");
        }

        // Add permalink and external links if different
        content.push_str(&format!(
            "\nPermalink: https://reddit.com{}",
            self.permalink
        ));
        if !self.is_self && self.url != format!("https://reddit.com{}", self.permalink) {
            content.push_str(&format!("\nExternal URL: {}", self.url));
        }

        content
    }

    /// Format timestamp as a human-readable string
    pub fn format_timestamp(&self) -> String {
        use chrono::{TimeZone, Utc};

        let timestamp = Utc
            .timestamp_opt(self.created_utc as i64, 0)
            .single()
            .unwrap_or_else(|| Utc::now());

        timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    }
}
