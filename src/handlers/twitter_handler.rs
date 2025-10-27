use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

static TWITTER_PATTERN: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct FxTwitterResponse {
    tweet: TweetData,
}

#[derive(Debug, Deserialize)]
struct TweetData {
    author: AuthorData,
    text: String,
    #[serde(default)]
    media: Option<MediaData>,
    #[serde(default)]
    replying_to: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    replying_to_status: Option<String>,
    #[serde(default)]
    quote: Option<Box<TweetData>>,
}

#[derive(Debug, Deserialize)]
struct AuthorData {
    name: String,
    screen_name: String,
}

#[derive(Debug, Deserialize)]
struct MediaData {
    #[serde(default)]
    photos: Vec<PhotoItem>,
    #[serde(default)]
    videos: Vec<VideoItem>,
}

#[derive(Debug, Deserialize)]
struct PhotoItem {
    url: String,
}

#[derive(Debug, Deserialize)]
struct VideoItem {
    url: String,
}

pub struct TwitterPost {
    pub author: String,
    pub username: String,
    pub text: String,
    pub images: Vec<String>,
    pub replying_to: Option<String>,
    pub quote_author: Option<String>,
    pub quote_username: Option<String>,
    pub quote_text: Option<String>,
}

impl TwitterPost {
    pub fn parse(url: &str) -> Option<(String, String)> {
        let pattern = TWITTER_PATTERN.get_or_init(|| {
            Regex::new(r"(?i)https?://(?:www\.)?(twitter\.com|x\.com)/([a-zA-Z0-9_]+)/status/(\d+)")
                .unwrap()
        });

        let captures = pattern.captures(url)?;
        let username = captures.get(2)?.as_str().to_string();
        let tweet_id = captures.get(3)?.as_str().to_string();

        Some((username, tweet_id))
    }

    pub fn fetch(
        username: &str,
        tweet_id: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let api_url = format!("https://api.fxtwitter.com/{}/status/{}", username, tweet_id);

        let response = ureq::get(&api_url)
            .set("User-Agent", "Mozilla/5.0 (compatible; GenesisBot/1.0)")
            .call()?;

        let data: FxTwitterResponse = response.into_json()?;
        let tweet = data.tweet;

        let mut images = tweet
            .media
            .as_ref()
            .map(|m| {
                let mut all_media = m.photos.iter().map(|p| p.url.clone()).collect::<Vec<_>>();
                all_media.extend(m.videos.iter().map(|v| v.url.clone()));
                all_media
            })
            .unwrap_or_default();

        let (quote_author, quote_username, quote_text) = if let Some(quoted) = &tweet.quote {
            if let Some(quoted_media) = &quoted.media {
                let mut quoted_images = quoted_media
                    .photos
                    .iter()
                    .map(|p| p.url.clone())
                    .collect::<Vec<_>>();
                quoted_images.extend(quoted_media.videos.iter().map(|v| v.url.clone()));
                images.extend(quoted_images);
            }
            (
                Some(quoted.author.name.clone()),
                Some(quoted.author.screen_name.clone()),
                Some(quoted.text.clone()),
            )
        } else {
            (None, None, None)
        };

        Ok(Self {
            author: tweet.author.name,
            username: tweet.author.screen_name,
            text: tweet.text,
            images,
            replying_to: tweet.replying_to,
            quote_author,
            quote_username,
            quote_text,
        })
    }
}
