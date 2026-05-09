use crate::constants::{REDDIT_MAX_GALLERY_ITEMS, REDDIT_USER_AGENT};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::debug;

static POST_PATTERN: OnceLock<Regex> = OnceLock::new();
static SHORT_PATTERN: OnceLock<Regex> = OnceLock::new();
static SHARE_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct RedditPost {
    pub author: String,
    pub subreddit: String,
    pub title: String,
    pub text: String,
    pub media: Vec<String>,
    pub nsfw: bool,
    pub spoiler: bool,
}

impl RedditPost {
    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let post_id = if let Some(id) = Self::extract_post_id(url) {
            id
        } else if Self::is_share_link(url) {
            let resolved =
                Self::resolve_share_link(url).ok_or("Failed to resolve Reddit share link")?;
            Self::extract_post_id(&resolved)
                .ok_or("Resolved Reddit share link did not contain a post ID")?
        } else {
            return Err("Not a recognizable Reddit post URL".into());
        };

        Self::fetch_by_id(&post_id)
    }

    fn fetch_by_id(post_id: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let api_url = format!(
            "https://www.reddit.com/comments/{}.json?raw_json=1",
            post_id
        );
        debug!("Fetching Reddit post: {}", api_url);

        let response = ureq::get(&api_url)
            .set("User-Agent", REDDIT_USER_AGENT)
            .set("Accept", "application/json")
            .timeout(Duration::from_secs(15))
            .call()?;

        let json: Value = response.into_json()?;

        let post = json
            .get(0)
            .and_then(|listing| listing.get("data"))
            .and_then(|d| d.get("children"))
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("data"))
            .ok_or("Unexpected Reddit JSON shape")?;

        Self::parse_post(post)
    }

    fn parse_post(post: &Value) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let title = post
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let author = post
            .get("author")
            .and_then(|v| v.as_str())
            .unwrap_or("[deleted]")
            .to_string();

        let subreddit = post
            .get("subreddit_name_prefixed")
            .and_then(|v| v.as_str())
            .or_else(|| post.get("subreddit").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        let removed_by = post
            .get("removed_by_category")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !removed_by.is_empty() {
            return Err(format!("Post was removed ({})", removed_by).into());
        }

        let selftext_raw = post.get("selftext").and_then(|v| v.as_str()).unwrap_or("");
        if selftext_raw == "[deleted]" || selftext_raw == "[removed]" {
            return Err("Post has been deleted or removed".into());
        }

        let nsfw = post
            .get("over_18")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let spoiler = post
            .get("spoiler")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let text = if selftext_raw.chars().count() > 1500 {
            let truncated: String = selftext_raw.chars().take(1500).collect();
            format!("{}…", truncated)
        } else {
            selftext_raw.to_string()
        };

        let media_source = post
            .get("crosspost_parent_list")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .unwrap_or(post);

        let media = Self::extract_media(media_source);

        Ok(Self {
            author,
            subreddit,
            title,
            text,
            media,
            nsfw,
            spoiler,
        })
    }

    fn extract_media(post: &Value) -> Vec<String> {
        if post
            .get("is_gallery")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            && let Some(items) = post
                .get("gallery_data")
                .and_then(|g| g.get("items"))
                .and_then(|i| i.as_array())
        {
            let metadata = post.get("media_metadata");
            let mut urls = Vec::new();
            for item in items.iter().take(REDDIT_MAX_GALLERY_ITEMS) {
                let Some(media_id) = item.get("media_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(meta) = metadata.and_then(|m| m.get(media_id)) else {
                    continue;
                };
                let url = meta
                    .get("s")
                    .and_then(|s| s.get("u").or_else(|| s.get("gif")).or_else(|| s.get("mp4")))
                    .and_then(|v| v.as_str());
                if let Some(u) = url {
                    urls.push(u.to_string());
                }
            }
            return urls;
        }

        if post
            .get("is_video")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let video_url = post
                .get("secure_media")
                .and_then(|m| m.get("reddit_video"))
                .and_then(|v| v.get("fallback_url"))
                .and_then(|v| v.as_str())
                .or_else(|| {
                    post.get("media")
                        .and_then(|m| m.get("reddit_video"))
                        .and_then(|v| v.get("fallback_url"))
                        .and_then(|v| v.as_str())
                });
            if let Some(url) = video_url {
                return vec![url.to_string()];
            }
        }

        let post_hint = post.get("post_hint").and_then(|v| v.as_str()).unwrap_or("");
        if post_hint == "image"
            && let Some(url) = post.get("url").and_then(|v| v.as_str())
        {
            return vec![url.to_string()];
        }

        if let Some(preview_url) = post
            .get("preview")
            .and_then(|p| p.get("images"))
            .and_then(|i| i.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("source"))
            .and_then(|s| s.get("url"))
            .and_then(|v| v.as_str())
        {
            return vec![preview_url.to_string()];
        }

        Vec::new()
    }

    fn extract_post_id(url: &str) -> Option<String> {
        let post_re = POST_PATTERN.get_or_init(|| {
            Regex::new(
                r"(?i)https?://(?:[a-z0-9-]+\.)?reddit(?:media)?\.com/(?:(?:r|u|user)/[^/?#]+/)?comments/([a-z0-9]+)",
            )
            .unwrap()
        });
        if let Some(caps) = post_re.captures(url) {
            return caps.get(1).map(|m| m.as_str().to_string());
        }

        let short_re =
            SHORT_PATTERN.get_or_init(|| Regex::new(r"(?i)https?://redd\.it/([a-z0-9]+)").unwrap());
        if let Some(caps) = short_re.captures(url) {
            return caps.get(1).map(|m| m.as_str().to_string());
        }

        None
    }

    fn is_share_link(url: &str) -> bool {
        let re = SHARE_PATTERN.get_or_init(|| {
            Regex::new(
                r"(?i)https?://(?:[a-z0-9-]+\.)?reddit\.com/(?:r|u|user)/[^/?#]+/s/[a-zA-Z0-9]+",
            )
            .unwrap()
        });
        re.is_match(url)
    }

    fn resolve_share_link(url: &str) -> Option<String> {
        debug!("Resolving Reddit share link: {}", url);
        let response = ureq::head(url)
            .set("User-Agent", REDDIT_USER_AGENT)
            .timeout(Duration::from_secs(10))
            .call()
            .ok()?;
        let final_url = response.get_url().to_string();
        if final_url == url {
            let response = ureq::get(url)
                .set("User-Agent", REDDIT_USER_AGENT)
                .timeout(Duration::from_secs(10))
                .call()
                .ok()?;
            return Some(response.get_url().to_string());
        }
        Some(final_url)
    }
}
