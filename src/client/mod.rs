use reqwest::{Client, Error};
use crate::RedditRNewResponse;

#[derive(Clone)]
pub struct RedditClient {
    pub client: Client,
}

impl RedditClient {
    pub fn new() -> Self {
        Self {
            client: Self::get_client().unwrap(),
        }
    }

    fn get_client() -> Result<Client, Error> {
        Ok(Client::builder()
            .user_agent("redrust/1.0 (by /u/Aggravating-Fix-3871)")
            .build()?)
    }

    pub async fn get_access_token(&self, client_id: &str) -> Result<String, Error> {
        let params = [
            ("grant_type", "https://oauth.reddit.com/grants/installed_client"),
            ("device_id", "DO_NOT_TRACK_THIS_DEVICE")
        ];

        // Note: Since there is no client secret, the authorization is created using your client_id followed by a colon.
        let auth = base64::encode(format!("{}:", client_id));

        let res = self.client
            .post("https://www.reddit.com/api/v1/access_token")
            .header("Authorization", format!("Basic {}", auth))
            .form(&params)
            .send()
            .await?;

        let json: serde_json::Value = res.json().await?;
        Ok(json["access_token"].as_str().unwrap().to_string())
    }

    pub async fn fetch_new_posts(&self, subreddit: &str, limit: i32) -> Result<RedditRNewResponse, Error> {
        let url = format!("https://www.reddit.com/r/{}/new.json?limit={}", subreddit, limit);
        let response = self.client.get(&url).send().await?;
        let body = response.text().await?;

        let response: RedditRNewResponse = serde_json::from_str(&body).unwrap();
        Ok(response)
    }
}


