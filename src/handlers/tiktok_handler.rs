use crate::constants::{TIKTOK_DOWNLOAD_UA, TIKTOK_HOSTS};
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

static TIKTOK_POST_PATTERN: OnceLock<Regex> = OnceLock::new();
static TIKTOK_SHORT_PATTERN: OnceLock<Regex> = OnceLock::new();

pub(crate) fn matches_tiktok_host(url: &str) -> bool {
    super::shared::matches_host(url, TIKTOK_HOSTS, "tiktok")
}

fn is_short_link_host(url: &str) -> bool {
    super::shared::extract_host(url).is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        host.starts_with("vm.") || host.starts_with("vt.")
    })
}

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
        if !matches_tiktok_host(url) {
            return None;
        }

        let post_pattern = TIKTOK_POST_PATTERN.get_or_init(|| {
            Regex::new(
                r"(?i)https?://[^/]+/(?:@[\w.-]*/(?:video|photo)/(\d+)|(?:embed(?:/v\d+)?|player/v1)/(\d+)|v/(\d+)\.html|t/([A-Za-z0-9]+))",
            )
            .unwrap()
        });
        if let Some(captures) = post_pattern.captures(url) {
            let id = (1..=4).find_map(|i| captures.get(i))?.as_str();
            return Some(format!("https://vm.tiktok.com/{}", id));
        }

        if is_short_link_host(url) {
            let short_pattern = TIKTOK_SHORT_PATTERN
                .get_or_init(|| Regex::new(r"(?i)https?://[^/]+/([A-Za-z0-9]+)").unwrap());
            if let Some(captures) = short_pattern.captures(url) {
                return Some(format!(
                    "https://vm.tiktok.com/{}",
                    captures.get(1)?.as_str()
                ));
            }
        }

        None
    }

    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let api_url = format!(
            "https://www.tikwm.com/api/?url={}",
            urlencoding::encode(url)
        );

        let response = ureq::get(&api_url)
            .set("User-Agent", TIKTOK_DOWNLOAD_UA)
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
