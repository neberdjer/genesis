use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use tracing::{debug, warn};

static INSTAGRAM_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct InstagramPost {
    pub author: String,
    pub username: String,
    pub text: String,
    pub media: Vec<String>,
}

const GRAPHQL_DOC_IDS: &[&str] = &["8845758582119845", "25531498899829322", "10913595222663636"];

fn shortcode_to_media_id(shortcode: &str) -> u64 {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut id: u64 = 0;
    for ch in shortcode.bytes() {
        if let Some(pos) = ALPHABET.iter().position(|&c| c == ch) {
            id = id * 64 + pos as u64;
        }
    }
    id
}

impl InstagramPost {
    pub fn parse(url: &str) -> Option<(String, Option<usize>)> {
        let pattern = INSTAGRAM_PATTERN.get_or_init(|| {
            Regex::new(
                r"(?i)https?://(?:www\.)?instagram\.com/(?:[^/]+/)?(?:p|reel|tv)/([a-zA-Z0-9_-]+)",
            )
            .unwrap()
        });

        let captures = pattern.captures(url)?;
        let post_id = captures.get(1)?.as_str().to_string();

        let img_index = if let Some(query_start) = url.find('?') {
            let query = &url[query_start + 1..];
            for param in query.split('&') {
                if let Some((key, value)) = param.split_once('=')
                    && key == "img_index"
                    && let Ok(index) = value.parse::<usize>()
                {
                    return Some((post_id, Some(index)));
                }
            }
            None
        } else {
            None
        };

        Some((post_id, img_index))
    }

    pub fn fetch(
        post_id: &str,
        img_index: Option<usize>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(post) = Self::fetch_graphql(post_id, img_index) {
            return Ok(post);
        }
        debug!("GraphQL failed for {}, trying ?__a=1 endpoint", post_id);

        if let Some(post) = Self::fetch_a1(post_id, img_index) {
            return Ok(post);
        }
        debug!("?__a=1 failed for {}, trying mobile API", post_id);

        if let Some(post) = Self::fetch_mobile_api(post_id, img_index) {
            return Ok(post);
        }

        Err("All Instagram fetch methods failed".into())
    }

    fn fetch_graphql(post_id: &str, img_index: Option<usize>) -> Option<Self> {
        let variables = serde_json::json!({ "shortcode": post_id });

        for doc_id in GRAPHQL_DOC_IDS {
            let api_url = format!(
                "https://www.instagram.com/graphql/query?variables={}&doc_id={}&server_timestamps=true",
                urlencoding::encode(&variables.to_string()),
                doc_id
            );

            let result = ureq::get(&api_url)
                .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
                .set("X-Ig-App-Id", "936619743392459")
                .set("Accept", "*/*")
                .set("Accept-Language", "en-US,en;q=0.9")
                .set("Sec-Fetch-Site", "same-origin")
                .call();

            match result {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<Value>()
                        && let Ok(post) = Self::parse_graphql_response(&json, img_index)
                    {
                        return Some(post);
                    }
                }
                Err(e) => {
                    warn!("GraphQL doc_id {} failed: {}", doc_id, e);
                }
            }
        }

        None
    }

    fn fetch_a1(post_id: &str, img_index: Option<usize>) -> Option<Self> {
        let url = format!("https://www.instagram.com/p/{}/?__a=1&__d=1", post_id);

        let response = ureq::get(&url)
            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .set("X-Ig-App-Id", "936619743392459")
            .set("Accept", "*/*")
            .call()
            .ok()?;

        let json: Value = response.into_json().ok()?;

        let item = json
            .get("graphql")
            .and_then(|g| g.get("shortcode_media"))
            .or_else(|| json.get("items").and_then(|i| i.get(0)))?;

        Self::parse_media_item(item, img_index).ok()
    }

    fn fetch_mobile_api(post_id: &str, img_index: Option<usize>) -> Option<Self> {
        let media_id = shortcode_to_media_id(post_id);
        let url = format!("https://i.instagram.com/api/v1/media/{}/info/", media_id);

        let response = ureq::get(&url)
            .set("User-Agent", "Instagram 275.0.0.27.98 Android (33/13; 420dpi; 1080x2400; samsung; SM-G991B; o1s; exynos2100)")
            .set("X-Ig-App-Id", "567067343352427")
            .call()
            .ok()?;

        let json: Value = response.into_json().ok()?;
        let item = json.get("items")?.get(0)?;

        Self::parse_mobile_response(item, img_index).ok()
    }

    fn parse_graphql_response(
        json: &Value,
        img_index: Option<usize>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = json.get("data").ok_or("No data in GraphQL response")?;

        let item = data
            .get("xdt_shortcode_media")
            .or_else(|| data.get("shortcode_media"))
            .ok_or("No media data in GraphQL response")?;

        Self::parse_media_item(item, img_index)
    }

    fn parse_media_item(
        item: &Value,
        img_index: Option<usize>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let username = item
            .get("owner")
            .and_then(|o| o.get("username"))
            .or_else(|| item.get("user").and_then(|u| u.get("username")))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();

        let caption = item
            .get("edge_media_to_caption")
            .and_then(|c| c.get("edges"))
            .and_then(|e| e.get(0))
            .and_then(|e| e.get("node"))
            .and_then(|n| n.get("text"))
            .and_then(|t| t.as_str())
            .or_else(|| {
                item.get("caption")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
            })
            .unwrap_or("")
            .to_string();

        let mut media = Vec::new();

        if let Some(children) = item
            .get("edge_sidecar_to_children")
            .and_then(|c| c.get("edges"))
        {
            if let Some(edges) = children.as_array() {
                for edge in edges {
                    if let Some(node) = edge.get("node")
                        && let Some(url) = node
                            .get("video_url")
                            .or_else(|| node.get("display_url"))
                            .and_then(|u| u.as_str())
                    {
                        media.push(url.to_string());
                    }
                }
            }
        } else if let Some(carousel) = item.get("carousel_media").and_then(|c| c.as_array()) {
            for child in carousel {
                if let Some(url) = Self::best_media_url(child) {
                    media.push(url);
                }
            }
        } else if let Some(url) = item
            .get("video_url")
            .or_else(|| item.get("display_url"))
            .and_then(|u| u.as_str())
        {
            media.push(url.to_string());
        } else if let Some(url) = Self::best_media_url(item) {
            media.push(url);
        }

        if media.is_empty() {
            return Err("No media found in post".into());
        }

        let media = if let Some(index) = img_index {
            if index > 0 && index <= media.len() {
                vec![media[index - 1].clone()]
            } else {
                media
            }
        } else {
            media
        };

        Ok(Self {
            author: username.clone(),
            username,
            text: caption,
            media,
        })
    }

    fn parse_mobile_response(
        item: &Value,
        img_index: Option<usize>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let username = item
            .get("user")
            .and_then(|u| u.get("username"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();

        let full_name = item
            .get("user")
            .and_then(|u| u.get("full_name"))
            .and_then(|n| n.as_str())
            .unwrap_or(&username)
            .to_string();

        let caption = item
            .get("caption")
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let mut media = Vec::new();

        if let Some(carousel) = item.get("carousel_media").and_then(|c| c.as_array()) {
            for child in carousel {
                if let Some(url) = Self::best_media_url(child) {
                    media.push(url);
                }
            }
        } else if let Some(url) = Self::best_media_url(item) {
            media.push(url);
        }

        if media.is_empty() {
            return Err("No media found in post".into());
        }

        let media = if let Some(index) = img_index {
            if index > 0 && index <= media.len() {
                vec![media[index - 1].clone()]
            } else {
                media
            }
        } else {
            media
        };

        Ok(Self {
            author: full_name,
            username,
            text: caption,
            media,
        })
    }

    fn best_media_url(item: &Value) -> Option<String> {
        if let Some(url) = item.get("video_url").and_then(|u| u.as_str()) {
            return Some(url.to_string());
        }

        if let Some(candidates) = item
            .get("image_versions2")
            .and_then(|iv| iv.get("candidates"))
            .and_then(|c| c.as_array())
            && let Some(best) = candidates.first()
            && let Some(url) = best.get("url").and_then(|u| u.as_str())
        {
            return Some(url.to_string());
        }

        if let Some(url) = item.get("display_url").and_then(|u| u.as_str()) {
            return Some(url.to_string());
        }

        None
    }
}
