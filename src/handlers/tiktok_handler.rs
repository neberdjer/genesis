use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

static TIKTOK_PATTERN: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct TikTokApiResponse {
    code: i32,
    msg: String,
    data: Option<TikTokData>,
}

#[derive(Debug, Deserialize)]
struct TikTokData {
    title: String,
    author: AuthorData,
    #[serde(default)]
    images: Vec<String>,
    #[serde(default)]
    play: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorData {
    nickname: String,
    unique_id: String,
}

pub struct TikTokPost {
    pub author: String,
    pub username: String,
    pub text: String,
    pub media: Vec<String>,
}

impl TikTokPost {
    pub fn parse(url: &str) -> Option<String> {
        let pattern = TIKTOK_PATTERN.get_or_init(|| {
            Regex::new(r"(?i)https?://(?:www\.|vm\.|vt\.)?tiktok\.com/(?:@[\w.-]+/video/|t/)?(\w+)")
                .unwrap()
        });

        pattern.captures(url)?;
        Some(url.to_string())
    }

    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let api_url = format!("https://www.tikwm.com/api/?url={}", url);

        let response = ureq::get(&api_url)
            .set("User-Agent", "Mozilla/5.0 (compatible; GenesisBot/1.0)")
            .call()?;

        let data: TikTokApiResponse = response.into_json()?;

        if data.code != 0 {
            return Err(format!("API error: {}", data.msg).into());
        }

        let tiktok_data = data.data.ok_or("No data returned from TikTok API")?;

        let media = if !tiktok_data.images.is_empty() {
            tiktok_data.images
        } else if let Some(video_url) = tiktok_data.play {
            vec![video_url]
        } else {
            return Err("No media found in TikTok post".into());
        };

        Ok(Self {
            author: tiktok_data.author.nickname,
            username: tiktok_data.author.unique_id,
            text: tiktok_data.title,
            media,
        })
    }
}
