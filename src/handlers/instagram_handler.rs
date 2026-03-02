use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

static INSTAGRAM_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct InstagramPost {
    pub author: String,
    pub username: String,
    pub text: String,
    pub media: Vec<String>,
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
        let doc_ids = [
            "8845758582119845",  // current
            "25531498899829322", // alternative
            "10913595222663636", // fallback
        ];

        let variables = serde_json::json!({
            "shortcode": post_id
        });

        let mut last_error = None;

        for doc_id in &doc_ids {
            let api_url = format!(
                "https://www.instagram.com/graphql/query?variables={}&doc_id={}&server_timestamps=true",
                urlencoding::encode(&variables.to_string()),
                doc_id
            );

            let result = ureq::get(&api_url)
                .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .set("X-Ig-App-Id", "936619743392459")
                .call();

            match result {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<Value>()
                        && let Ok(post) = Self::parse_graphql_response(&json, img_index)
                    {
                        return Ok(post);
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .map(|e| e.to_string())
            .unwrap_or_else(|| "All doc_ids failed".to_string())
            .into())
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

        let username = item
            .get("owner")
            .and_then(|o| o.get("username"))
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
            .unwrap_or("")
            .to_string();

        let mut media = Vec::new();

        if let Some(children) = item
            .get("edge_sidecar_to_children")
            .and_then(|c| c.get("edges"))
        {
            if let Some(edges) = children.as_array() {
                for edge in edges {
                    if let Some(node) = edge.get("node") {
                        let media_url = node
                            .get("video_url")
                            .or_else(|| node.get("display_url"))
                            .and_then(|u| u.as_str())
                            .map(|s| s.to_string());
                        if let Some(url) = media_url {
                            media.push(url);
                        }
                    }
                }
            }
        } else {
            let media_url = item
                .get("video_url")
                .or_else(|| item.get("display_url"))
                .and_then(|u| u.as_str())
                .ok_or("No media URL found")?
                .to_string();
            media.push(media_url);
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
}
