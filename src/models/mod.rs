use serde::Deserialize;
use std::collections::HashMap;

pub mod public_feed;
pub mod subreddit_posts;

// Common data types - to be gradually migrated to specialized modules

/// Top-level response for Reddit listings
#[derive(Deserialize, Debug)]
pub struct RedditRNewResponse {
    pub kind: String,
    pub data: RedditPostCollection,
}

/// Collection of posts in a listing
#[derive(Deserialize, Debug)]
pub struct RedditPostCollection {
    pub after: Option<String>,
    pub dist: i32,
    #[serde(default)]
    pub modhash: String,
    #[serde(default)]
    pub geo_filter: String,
    pub children: Vec<RedditPostEntity>,
    pub before: Option<String>,
}

/// Reddit post entity with kind and data fields
#[derive(Deserialize, Debug)]
pub struct RedditPostEntity {
    pub kind: String,
    pub data: RedditPostData,
}

/// Preview images in post
#[derive(Deserialize, Debug)]
pub struct RedditPreview {
    pub images: Vec<RedditImage>,
    pub enabled: bool,
}

/// Image data in post preview
#[derive(Deserialize, Debug)]
pub struct RedditImage {
    pub source: RedditImageSource,
    pub resolutions: Vec<RedditImageSource>,
    pub variants: HashMap<String, RedditImageVariant>,
    pub id: String,
}

/// Image variant data
#[derive(Deserialize, Debug)]
pub struct RedditImageVariant {
    pub source: RedditImageSource,
    pub resolutions: Vec<RedditImageSource>,
}

/// Image source data with dimensions and URL
#[derive(Deserialize, Debug)]
pub struct RedditImageSource {
    pub url: String,
    pub width: i32,
    pub height: i32,
}

/// Media embed data
#[derive(Deserialize, Debug)]
pub struct RedditMediaEmbed {
    pub content: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

/// Reddit media data
#[derive(Deserialize, Debug)]
pub struct RedditMedia {
    pub reddit_video: Option<RedditVideo>,
    #[serde(flatten)]
    pub other_fields: HashMap<String, serde_json::Value>,
}

/// Reddit video data
#[derive(Deserialize, Debug)]
pub struct RedditVideo {
    pub bitrate_kbps: i32,
    pub fallback_url: String,
    pub height: i32,
    pub width: i32,
    pub scrubber_media_url: String,
    pub dash_url: String,
    pub duration: i32,
    pub hls_url: String,
    pub is_gif: bool,
    pub transcoding_status: String,
}

/// Gallery data in post
#[derive(Deserialize, Debug)]
pub struct RedditGalleryData {
    pub items: Vec<RedditGalleryItem>,
}

/// Gallery item in post
#[derive(Deserialize, Debug)]
pub struct RedditGalleryItem {
    pub media_id: String,
    pub id: i32,
}

/// Flair data
#[derive(Deserialize, Debug)]
pub struct RedditFlair {
    pub text: String,
    pub background_color: String,
    pub text_color: String,
    pub type_: String,
}

/// Award data
#[derive(Deserialize, Debug)]
pub struct RedditAward {
    pub name: String,
    pub description: String,
    pub icon_url: String,
    pub count: i32,
}

/// Comprehensive Reddit post data
#[derive(Deserialize, Debug)]
pub struct RedditPostData {
    // Basic post information
    pub id: String,
    pub name: String,
    pub title: String,
    pub author: String,
    pub author_fullname: Option<String>,
    pub permalink: String,
    pub url: String,
    pub created_utc: f64,

    // Post type and content
    pub is_self: bool,
    pub selftext: String,
    pub selftext_html: Option<String>,
    pub is_video: bool,
    pub is_original_content: bool,
    pub is_reddit_media_domain: bool,
    pub is_meta: bool,
    pub is_crosspostable: bool,

    // Media-related fields
    pub thumbnail: String,
    pub thumbnail_width: Option<i32>,
    pub thumbnail_height: Option<i32>,
    pub secure_media: Option<RedditMedia>,
    pub secure_media_embed: RedditMediaEmbed,
    pub media: Option<RedditMedia>,
    pub media_embed: RedditMediaEmbed,
    pub preview: Option<RedditPreview>,
    pub gallery_data: Option<RedditGalleryData>,
    pub media_metadata: Option<HashMap<String, serde_json::Value>>,

    // Post metrics
    pub score: i32,
    pub upvote_ratio: f32,
    pub ups: i32,
    pub downs: i32,
    pub num_comments: i32,
    pub num_crossposts: i32,
    pub total_awards_received: i32,

    // Subreddit information
    pub subreddit: String,
    pub subreddit_id: String,
    pub subreddit_subscribers: i32,
    pub subreddit_type: String,
    pub subreddit_name_prefixed: String,

    // Post status and moderation
    pub archived: bool,
    pub locked: bool,
    pub hidden: bool,
    pub removed_by_category: Option<String>,
    pub removed_by: Option<String>,
    pub stickied: bool,
    pub pinned: bool,
    pub spoiler: bool,
    pub over_18: bool,

    // Flags and display options
    pub hide_score: bool,
    pub contest_mode: bool,
    pub edited: serde_json::Value, // Can be boolean or timestamp
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

impl RedditPostData {
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

    /// Get a short summary for the post (title, author, score)
    pub fn format_short_summary(&self) -> String {
        format!(
            "[{} | {} pts] {} - by u/{}",
            self.subreddit_name_prefixed, self.score, self.title, self.author
        )
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
