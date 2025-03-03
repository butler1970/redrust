pub mod client;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct RedditRNewResponse {
    pub kind: String,
    pub data: RedditPostCollection,
}

#[derive(Deserialize)]
pub struct RedditPostCollection {
    pub after: Option<String>,
    pub dist: i32,
    pub modhash: String,
    pub geo_filter: String,
    pub children: Vec<RedditPostEntity>,
    pub before: Option<String>,
}

#[derive(Deserialize)]
pub struct RedditPostEntity {
    pub kind: String,
    pub data: RedditPostData,
}

#[derive(Deserialize)]
pub struct RedditPostData {
    pub id: String,
    pub title: String,
    pub author: String,
    pub subreddit: String,
    pub permalink: String,
    pub thumbnail: String,
    pub url: String,
    pub created_utc: f64,
}
