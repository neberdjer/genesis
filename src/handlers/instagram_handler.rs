use crate::constants::{
    INSTAGRAM_APP_ID, INSTAGRAM_DESKTOP_UA, INSTAGRAM_DOC_IDS, INSTAGRAM_MAX_CAROUSEL_SLIDES,
    INSTAGRAM_MIRROR_UA, INSTAGRAM_MIRRORS, INSTAGRAM_MOBILE_UA,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, warn};

static INSTAGRAM_PATTERN: OnceLock<Regex> = OnceLock::new();
static LSD_PATTERN: OnceLock<Regex> = OnceLock::new();
static CSRF_PATTERN: OnceLock<Regex> = OnceLock::new();
static BLOKS_PATTERN: OnceLock<Regex> = OnceLock::new();
static DEVICE_ID_PATTERN: OnceLock<Regex> = OnceLock::new();
static MACHINE_ID_PATTERN: OnceLock<Regex> = OnceLock::new();
static OG_IMAGE_PATTERN: OnceLock<Regex> = OnceLock::new();
static OG_VIDEO_PATTERN: OnceLock<Regex> = OnceLock::new();
static OG_TITLE_PATTERN: OnceLock<Regex> = OnceLock::new();
static OG_DESC_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct InstagramPost {
    pub author: String,
    pub username: String,
    pub text: String,
    pub media: Vec<String>,
}

struct AnonTokens {
    lsd: String,
    csrf: String,
    bloks_version: String,
    cookie: String,
}

impl InstagramPost {
    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let post_id = if let Some(id) = Self::extract_shortcode(url) {
            id
        } else if url.contains("/share/") {
            let resolved = Self::resolve_share_link(url).ok_or("Failed to resolve share link")?;
            Self::extract_shortcode(&resolved)
                .ok_or("Resolved share link did not contain a post shortcode")?
        } else {
            return Err("Not a recognizable Instagram post URL".into());
        };

        Self::fetch_by_id(&post_id)
    }

    fn fetch_by_id(post_id: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(post) = Self::fetch_graphql_simple(post_id) {
            return Ok(post);
        }
        debug!(
            "Simple GraphQL failed for {}, trying authed GraphQL",
            post_id
        );

        if let Some(post) = Self::fetch_graphql_authed(post_id) {
            return Ok(post);
        }
        debug!("Authed GraphQL failed for {}, trying mobile API", post_id);

        if let Some(post) = Self::fetch_mobile_api(post_id) {
            return Ok(post);
        }
        debug!("Mobile API failed for {}, trying mirror fallback", post_id);

        if let Some(post) = Self::fetch_mirror(post_id) {
            return Ok(post);
        }

        Err("All Instagram fetch methods failed".into())
    }

    fn fetch_graphql_simple(post_id: &str) -> Option<Self> {
        let variables = serde_json::json!({ "shortcode": post_id });

        for doc_id in INSTAGRAM_DOC_IDS {
            let api_url = format!(
                "https://www.instagram.com/graphql/query?variables={}&doc_id={}&server_timestamps=true",
                urlencoding::encode(&variables.to_string()),
                doc_id
            );

            let result = ureq::get(&api_url)
                .set("User-Agent", INSTAGRAM_DESKTOP_UA)
                .set("X-Ig-App-Id", INSTAGRAM_APP_ID)
                .set("Accept", "*/*")
                .set("Accept-Language", "en-US,en;q=0.9")
                .set("Sec-Fetch-Site", "same-origin")
                .timeout(Duration::from_secs(15))
                .call();

            match result {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<Value>()
                        && let Ok(post) = Self::parse_graphql_response(&json)
                    {
                        return Some(post);
                    }
                }
                Err(e) => debug!("Simple GraphQL doc_id {} failed: {}", doc_id, e),
            }
        }

        None
    }

    fn harvest_tokens(post_id: &str) -> Option<AnonTokens> {
        let url = format!("https://www.instagram.com/p/{}/", post_id);
        let response = ureq::get(&url)
            .set("User-Agent", INSTAGRAM_DESKTOP_UA)
            .set(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .set("Accept-Language", "en-US,en;q=0.9")
            .set("Sec-Fetch-Dest", "document")
            .set("Sec-Fetch-Mode", "navigate")
            .set("Sec-Fetch-Site", "none")
            .set("Sec-Fetch-User", "?1")
            .set("Upgrade-Insecure-Requests", "1")
            .timeout(Duration::from_secs(15))
            .call()
            .ok()?;

        let mut html = String::new();
        use std::io::Read as _;
        response
            .into_reader()
            .take(8 * 1024 * 1024)
            .read_to_string(&mut html)
            .ok()?;

        let lsd = Self::extract_lsd(&html)?;
        let csrf = Self::extract_csrf(&html).unwrap_or_default();
        let bloks_version = Self::extract_bloks(&html).unwrap_or_default();
        let device_id = Self::extract_device_id(&html);
        let machine_id = Self::extract_machine_id(&html);

        let mut cookie_parts = Vec::new();
        if !csrf.is_empty() {
            cookie_parts.push(format!("csrftoken={}", csrf));
        }
        if let Some(did) = &device_id {
            cookie_parts.push(format!("ig_did={}", did));
        }
        if let Some(mid) = &machine_id {
            cookie_parts.push(format!("mid={}", mid));
        }
        cookie_parts.push("wd=1280x720".into());
        cookie_parts.push("dpr=2".into());
        cookie_parts.push("ig_nrcb=1".into());
        let cookie = cookie_parts.join("; ");

        Some(AnonTokens {
            lsd,
            csrf,
            bloks_version,
            cookie,
        })
    }

    fn fetch_graphql_authed(post_id: &str) -> Option<Self> {
        let tokens = Self::harvest_tokens(post_id)?;

        for doc_id in INSTAGRAM_DOC_IDS {
            let variables = serde_json::json!({ "shortcode": post_id }).to_string();
            let body_pairs: Vec<(&str, &str)> = vec![
                ("__d", "www"),
                ("__a", "1"),
                ("__req", "b"),
                ("__user", "0"),
                ("__comet_req", "7"),
                ("av", "0"),
                ("dpr", "2"),
                ("lsd", &tokens.lsd),
                ("fb_api_caller_class", "RelayModern"),
                (
                    "fb_api_req_friendly_name",
                    "PolarisPostActionLoadPostQueryQuery",
                ),
                ("variables", &variables),
                ("server_timestamps", "true"),
                ("doc_id", doc_id),
            ];
            let body = body_pairs
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");

            let mut req = ureq::post("https://www.instagram.com/graphql/query")
                .set("User-Agent", INSTAGRAM_DESKTOP_UA)
                .set("Accept", "*/*")
                .set("Accept-Language", "en-US,en;q=0.9")
                .set("Content-Type", "application/x-www-form-urlencoded")
                .set("Origin", "https://www.instagram.com")
                .set(
                    "Referer",
                    &format!("https://www.instagram.com/p/{}/", post_id),
                )
                .set("X-Ig-App-Id", INSTAGRAM_APP_ID)
                .set("X-FB-LSD", &tokens.lsd)
                .set("X-Asbd-Id", "129477")
                .set("X-Ig-Www-Claim", "0")
                .set("X-FB-Friendly-Name", "PolarisPostActionLoadPostQueryQuery")
                .set("Sec-Fetch-Dest", "empty")
                .set("Sec-Fetch-Mode", "cors")
                .set("Sec-Fetch-Site", "same-origin")
                .set("Cookie", &tokens.cookie);

            if !tokens.csrf.is_empty() {
                req = req.set("X-CSRFToken", &tokens.csrf);
            }
            if !tokens.bloks_version.is_empty() {
                req = req.set("X-Bloks-Version-Id", &tokens.bloks_version);
            }

            let req = req.timeout(Duration::from_secs(15));

            match req.send_string(&body) {
                Ok(response) => {
                    if let Ok(json) = response.into_json::<Value>()
                        && let Ok(post) = Self::parse_graphql_response(&json)
                    {
                        return Some(post);
                    }
                }
                Err(e) => debug!("Authed GraphQL doc_id {} failed: {}", doc_id, e),
            }
        }

        None
    }

    fn fetch_mobile_api(post_id: &str) -> Option<Self> {
        let media_id = Self::shortcode_to_media_id(post_id);
        let url = format!("https://i.instagram.com/api/v1/media/{}/info/", media_id);

        let response = ureq::get(&url)
            .set("User-Agent", INSTAGRAM_MOBILE_UA)
            .set("X-Ig-App-Id", INSTAGRAM_APP_ID)
            .set("Accept", "*/*")
            .set("Accept-Language", "en-US,en;q=0.9")
            .timeout(Duration::from_secs(15))
            .call()
            .ok()?;

        let json: Value = response.into_json().ok()?;
        let item = json.get("items")?.get(0)?;

        Self::parse_mobile_response(item).ok()
    }

    fn fetch_mirror(post_id: &str) -> Option<Self> {
        for mirror in INSTAGRAM_MIRRORS {
            if let Some(post) = Self::fetch_mirror_one(mirror, post_id) {
                return Some(post);
            }
        }
        None
    }

    fn fetch_mirror_one(mirror: &str, post_id: &str) -> Option<Self> {
        let title_re = OG_TITLE_PATTERN.get_or_init(|| {
            Regex::new(
                r#"<meta\s+(?:property|name)="(?:og:title|twitter:title)"\s+content="([^"]*)""#,
            )
            .unwrap()
        });
        let desc_re = OG_DESC_PATTERN.get_or_init(|| {
            Regex::new(r#"<meta\s+(?:property|name)="og:description"\s+content="([^"]*)""#).unwrap()
        });
        let img_re = OG_IMAGE_PATTERN.get_or_init(|| {
            Regex::new(r#"<meta\s+property="og:image"\s+content="([^"]*)""#).unwrap()
        });
        let vid_re = OG_VIDEO_PATTERN.get_or_init(|| {
            Regex::new(r#"<meta\s+property="og:video"\s+content="([^"]*)""#).unwrap()
        });

        let mut media = Vec::new();
        let mut username = String::new();
        let mut caption = String::new();
        let mut last_media_url: Option<String> = None;

        for n in 1..=INSTAGRAM_MAX_CAROUSEL_SLIDES {
            let url = format!("https://{}/p/{}/{}", mirror, post_id, n);
            let Ok(response) = ureq::get(&url)
                .set("User-Agent", INSTAGRAM_MIRROR_UA)
                .set("Accept", "text/html,*/*;q=0.8")
                .timeout(Duration::from_secs(15))
                .call()
            else {
                break;
            };

            let Ok(html) = response.into_string() else {
                break;
            };
            if html.trim().is_empty() {
                break;
            }

            if n == 1 {
                if let Some(t) = Self::capture_first(title_re, &html) {
                    username = Self::html_unescape(&t)
                        .trim_start_matches('@')
                        .split(" (")
                        .next()
                        .unwrap_or("")
                        .to_string();
                }
                if let Some(d) = Self::capture_first(desc_re, &html) {
                    caption = Self::html_unescape(&d);
                }
            }

            let slide_url =
                Self::capture_first(vid_re, &html).or_else(|| Self::capture_first(img_re, &html));

            let Some(slide) = slide_url else {
                break;
            };
            let slide = Self::resolve_mirror_url(mirror, &slide);

            if last_media_url.as_deref() == Some(slide.as_str()) {
                break;
            }
            last_media_url = Some(slide.clone());
            media.push(slide);
        }

        if media.is_empty() {
            return None;
        }

        let display_name = if username.is_empty() {
            "instagram".to_string()
        } else {
            username.clone()
        };

        Some(Self {
            author: display_name,
            username,
            text: caption,
            media,
        })
    }

    fn parse_graphql_response(
        json: &Value,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = json.get("data").ok_or("No data in GraphQL response")?;

        let item = data
            .get("xdt_shortcode_media")
            .or_else(|| data.get("shortcode_media"))
            .ok_or("No media data in GraphQL response")?;

        Self::parse_media_item(item)
    }

    fn parse_media_item(item: &Value) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
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

        let typename = item
            .get("__typename")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let has_sidecar_field =
            item.get("edge_sidecar_to_children").is_some() || item.get("carousel_media").is_some();
        let is_carousel =
            matches!(typename, "GraphSidecar" | "XDTGraphSidecar") || has_sidecar_field;

        let mut media = Vec::new();

        if let Some(edges) = item
            .get("edge_sidecar_to_children")
            .and_then(|c| c.get("edges"))
            .and_then(|e| e.as_array())
        {
            for edge in edges {
                if let Some(node) = edge.get("node")
                    && let Some(url) = Self::best_media_url(node)
                {
                    media.push(url);
                } else {
                    warn!("Sidecar child produced no URL");
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

        if is_carousel && media.len() < 2 {
            return Err(format!(
                "Carousel parse incomplete: typename={}, got {} item(s)",
                typename,
                media.len()
            )
            .into());
        }

        Ok(Self {
            author: username.clone(),
            username,
            text: caption,
            media,
        })
    }

    fn parse_mobile_response(
        item: &Value,
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

        let product_type = item
            .get("product_type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        let has_carousel = item.get("carousel_media").is_some();
        let is_carousel = product_type == "carousel_container" || has_carousel;

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

        if is_carousel && media.len() < 2 {
            return Err(format!(
                "Mobile carousel parse incomplete: got {} item(s)",
                media.len()
            )
            .into());
        }

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

        if let Some(versions) = item.get("video_versions").and_then(|v| v.as_array())
            && let Some(best) = versions.first()
            && let Some(url) = best.get("url").and_then(|u| u.as_str())
        {
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

    fn extract_shortcode(url: &str) -> Option<String> {
        let pattern = INSTAGRAM_PATTERN.get_or_init(|| {
            Regex::new(
                r"(?i)https?://(?:[a-z0-9-]+\.)?instagram\.com/(?:[^/?#]+/)?(?:p|reels?|tv)/([a-zA-Z0-9_-]+)",
            )
            .unwrap()
        });
        pattern
            .captures(url)?
            .get(1)
            .map(|m| m.as_str().to_string())
    }

    fn resolve_share_link(url: &str) -> Option<String> {
        debug!("Resolving Instagram share link: {}", url);
        let response = ureq::head(url)
            .set("User-Agent", INSTAGRAM_DESKTOP_UA)
            .timeout(Duration::from_secs(10))
            .call()
            .ok()?;
        let final_url = response.get_url().to_string();
        if final_url == url {
            let response = ureq::get(url)
                .set("User-Agent", INSTAGRAM_DESKTOP_UA)
                .timeout(Duration::from_secs(10))
                .call()
                .ok()?;
            return Some(response.get_url().to_string());
        }
        Some(final_url)
    }

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

    fn capture_first(re: &Regex, html: &str) -> Option<String> {
        re.captures(html)?.get(1).map(|m| m.as_str().to_string())
    }

    fn extract_lsd(html: &str) -> Option<String> {
        let re = LSD_PATTERN
            .get_or_init(|| Regex::new(r#""LSD"[\s\S]{0,200}?"token":"([^"]+)""#).unwrap());
        Self::capture_first(re, html)
    }

    fn extract_csrf(html: &str) -> Option<String> {
        let re = CSRF_PATTERN.get_or_init(|| Regex::new(r#""csrf_token":"([^"]+)""#).unwrap());
        Self::capture_first(re, html)
    }

    fn extract_bloks(html: &str) -> Option<String> {
        let re = BLOKS_PATTERN.get_or_init(|| Regex::new(r#""versioningID":"([^"]+)""#).unwrap());
        Self::capture_first(re, html)
    }

    fn extract_device_id(html: &str) -> Option<String> {
        let re = DEVICE_ID_PATTERN.get_or_init(|| Regex::new(r#""device_id":"([^"]+)""#).unwrap());
        Self::capture_first(re, html)
    }

    fn extract_machine_id(html: &str) -> Option<String> {
        let re =
            MACHINE_ID_PATTERN.get_or_init(|| Regex::new(r#""machine_id":"([^"]+)""#).unwrap());
        Self::capture_first(re, html)
    }

    fn html_unescape(s: &str) -> String {
        s.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&#x27;", "'")
    }

    fn resolve_mirror_url(host: &str, value: &str) -> String {
        if value.starts_with("http://") || value.starts_with("https://") {
            value.to_string()
        } else if let Some(stripped) = value.strip_prefix("//") {
            format!("https://{}", stripped)
        } else if value.starts_with('/') {
            format!("https://{}{}", host, value)
        } else {
            format!("https://{}/{}", host, value)
        }
    }
}
