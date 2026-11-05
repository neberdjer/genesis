use crate::constants::{
    FAILURE_DELETED, FAILURE_UNAVAILABLE, TWITTER_DESKTOP_UA, TWITTER_HOSTS, TWITTER_SYNDICATION_UA,
};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::debug;

static TWITTER_PATTERN: OnceLock<Regex> = OnceLock::new();

#[derive(Debug)]
pub enum TwitterError {
    Deleted,
    Unavailable,
}

impl TwitterError {
    pub fn code(&self) -> &'static str {
        match self {
            TwitterError::Deleted => FAILURE_DELETED,
            TwitterError::Unavailable => FAILURE_UNAVAILABLE,
        }
    }
}

impl std::fmt::Display for TwitterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TwitterError::Deleted => write!(f, "tweet has been deleted or removed"),
            TwitterError::Unavailable => write!(f, "tweet is unavailable (private or restricted)"),
        }
    }
}

impl std::error::Error for TwitterError {}

pub(crate) fn matches_twitter_host(url: &str) -> bool {
    super::shared::matches_host(url, TWITTER_HOSTS, "twitter")
}

pub struct TwitterMedia {
    pub url: String,
    pub is_gif: bool,
}

#[derive(Default)]
pub struct TwitterEntities {
    pub mentions: Vec<String>,
    pub hashtags: Vec<String>,
    pub cashtags: Vec<String>,
}

pub struct TwitterPoll {
    pub choices: Vec<(String, u64)>,
    pub ends_at: Option<i64>,
    pub is_final: bool,
}

pub struct TwitterPost {
    pub author: String,
    pub username: String,
    pub text: String,
    pub media: Vec<TwitterMedia>,
    pub replying_to: Vec<String>,
    pub entities: TwitterEntities,
    pub created_at: Option<i64>,
    pub sensitive: bool,
    pub poll: Option<TwitterPoll>,
    pub quote_author: Option<String>,
    pub quote_username: Option<String>,
    pub quote_text: Option<String>,
    pub quote_entities: TwitterEntities,
}

impl TwitterPost {
    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let tweet_id = if let Some(id) = Self::extract_tweet_id(url) {
            id
        } else if url.contains("t.co/") {
            let resolved = Self::resolve_tco_link(url).ok_or("Failed to resolve t.co link")?;
            Self::extract_tweet_id(&resolved)
                .ok_or("Resolved t.co link did not contain a tweet ID")?
        } else {
            return Err("Not a recognizable Twitter URL".into());
        };

        Self::fetch_by_id(&tweet_id)
    }

    fn fetch_by_id(tweet_id: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let token = Self::syndication_token(tweet_id);
        let api_url = format!(
            "https://cdn.syndication.twimg.com/tweet-result?id={}&token={}&lang=en",
            tweet_id, token
        );

        let mut last_err: String = "Unknown error".to_string();
        for attempt in 0..2 {
            match ureq::get(&api_url)
                .set("User-Agent", TWITTER_SYNDICATION_UA)
                .set("Accept", "*/*")
                .set("Accept-Language", "en-US,en;q=0.9")
                .set("Origin", "https://platform.twitter.com")
                .set("Referer", "https://platform.twitter.com/")
                .timeout(Duration::from_secs(15))
                .call()
            {
                Ok(response) => match response.into_json::<Value>() {
                    Ok(json) => {
                        let typename = json
                            .get("__typename")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");

                        if typename == "TweetTombstone" {
                            return Err(TwitterError::Deleted.into());
                        }

                        if typename == "TweetUnavailable" {
                            return Err(TwitterError::Unavailable.into());
                        }

                        let is_empty = json.as_object().is_some_and(|obj| obj.is_empty());
                        if is_empty {
                            last_err = "Empty syndication response".to_string();
                        } else {
                            return Self::parse_tweet(&json);
                        }
                    }
                    Err(e) => last_err = format!("Failed to parse syndication JSON: {}", e),
                },
                Err(e) => last_err = format!("Syndication HTTP error: {}", e),
            }

            if attempt == 0 {
                debug!("Syndication attempt 1 failed for {}, retrying", tweet_id);
                std::thread::sleep(Duration::from_millis(500));
            }
        }

        Err(last_err.into())
    }

    fn parse_tweet(item: &Value) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let user = item.get("user");
        let author = user
            .and_then(|u| u.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let username = user
            .and_then(|u| u.get("screen_name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();

        let start = Self::display_start(item);
        let text = Self::tweet_text(item, start);
        let text = Self::substitute_tco_urls(&text, item);

        let replying_to = Self::reply_mentions(item, start, &username);
        let quoted = item.get("quoted_tweet");
        let entities = Self::entities(item);
        let quote_entities = quoted.map(Self::entities).unwrap_or_default();
        let created_at =
            Self::str_field(item, "created_at").and_then(super::shared::parse_timestamp);
        let sensitive = Self::is_sensitive(item) || quoted.is_some_and(Self::is_sensitive);
        let poll = Self::poll(item);

        let mut media = Self::extract_media(item);

        let (quote_author, quote_username, quote_text) = if let Some(quoted) = quoted {
            let qauthor = quoted
                .get("user")
                .and_then(|u| u.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let qusername = quoted
                .get("user")
                .and_then(|u| u.get("screen_name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let qtext = Self::tweet_text(quoted, Self::display_start(quoted));
            let qtext = Self::substitute_tco_urls(&qtext, quoted);

            media.extend(Self::extract_media(quoted));

            if qauthor.is_empty() && qusername.is_empty() && qtext.is_empty() {
                (None, None, None)
            } else {
                (Some(qauthor), Some(qusername), Some(qtext))
            }
        } else {
            (None, None, None)
        };

        Ok(Self {
            author,
            username,
            text,
            media,
            replying_to,
            entities,
            created_at,
            sensitive,
            poll,
            quote_author,
            quote_username,
            quote_text,
            quote_entities,
        })
    }

    fn tweet_text(item: &Value, start: usize) -> String {
        let text = item.get("text").and_then(|t| t.as_str()).unwrap_or("");
        let prefix = &text[..text
            .char_indices()
            .nth(start)
            .map_or(text.len(), |(i, _)| i)];
        let body = item
            .get("note_tweet")
            .and_then(|n| n.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or(text);
        body.strip_prefix(prefix)
            .unwrap_or(body)
            .trim_start()
            .to_string()
    }

    fn display_start(item: &Value) -> usize {
        item.get("display_text_range")
            .and_then(|r| r.get(0))
            .and_then(|s| s.as_u64())
            .unwrap_or(0) as usize
    }

    fn entity_list<'a>(entities: Option<&'a Value>, key: &str) -> impl Iterator<Item = &'a Value> {
        entities
            .and_then(|e| e.get(key))
            .and_then(|m| m.as_array())
            .into_iter()
            .flatten()
    }

    fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
        value.get(key).and_then(|s| s.as_str())
    }

    fn card_binding<'a>(item: &'a Value, key: &str, kind: &str) -> Option<&'a Value> {
        item.get("card")?.get("binding_values")?.get(key)?.get(kind)
    }

    fn reply_mentions(item: &Value, start: usize, author: &str) -> Vec<String> {
        let mut names: Vec<String> = Self::entity_list(item.get("entities"), "user_mentions")
            .filter(|m| {
                m.get("indices")
                    .and_then(|i| i.get(1))
                    .and_then(|e| e.as_u64())
                    .is_some_and(|end| end as usize <= start)
            })
            .filter_map(|m| Self::str_field(m, "screen_name").map(String::from))
            .collect();

        if names.is_empty() {
            names.extend(
                Self::str_field(item, "in_reply_to_screen_name")
                    .filter(|name| !name.eq_ignore_ascii_case(author))
                    .map(String::from),
            );
        }

        names
    }

    fn entities(item: &Value) -> TwitterEntities {
        let sets = [
            item.get("entities"),
            item.get("note_tweet").and_then(|n| n.get("entity_set")),
        ];
        let collect = |keys: &[&str], field: &str| -> Vec<String> {
            sets.iter()
                .flat_map(|set| keys.iter().flat_map(|key| Self::entity_list(*set, key)))
                .filter_map(|e| Self::str_field(e, field).map(String::from))
                .collect()
        };
        TwitterEntities {
            mentions: collect(&["user_mentions"], "screen_name"),
            hashtags: collect(&["hashtags"], "text"),
            cashtags: collect(&["symbols", "cashtags"], "text"),
        }
    }

    fn is_sensitive(item: &Value) -> bool {
        item.get("possibly_sensitive")
            .and_then(|s| s.as_bool())
            .unwrap_or(false)
    }

    fn poll(item: &Value) -> Option<TwitterPoll> {
        if !Self::str_field(item.get("card")?, "name")?.starts_with("poll") {
            return None;
        }
        let string = |key: &str| Self::card_binding(item, key, "string_value")?.as_str();
        let choices: Vec<(String, u64)> = (1..=4)
            .map_while(|i| {
                let label = string(&format!("choice{i}_label"))?;
                let count = string(&format!("choice{i}_count"))
                    .and_then(|c| c.parse().ok())
                    .unwrap_or(0);
                Some((label.to_string(), count))
            })
            .collect();
        if choices.is_empty() {
            return None;
        }
        Some(TwitterPoll {
            choices,
            ends_at: string("end_datetime_utc").and_then(super::shared::parse_timestamp),
            is_final: Self::card_binding(item, "counts_are_final", "boolean_value")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        })
    }

    fn extract_media(item: &Value) -> Vec<TwitterMedia> {
        let mut media = Vec::new();
        if let Some(details) = item.get("mediaDetails").and_then(|m| m.as_array()) {
            for d in details {
                let kind = d.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match kind {
                    "video" | "animated_gif" => {
                        let url = Self::pick_best_video_variant(d).or_else(|| {
                            d.get("media_url_https")
                                .and_then(|u| u.as_str())
                                .map(str::to_string)
                        });
                        if let Some(url) = url {
                            media.push(TwitterMedia {
                                url,
                                is_gif: kind == "animated_gif",
                            });
                        }
                    }
                    _ => {
                        if let Some(url) = d.get("media_url_https").and_then(|u| u.as_str()) {
                            media.push(TwitterMedia {
                                url: format!("{}?name=orig", url),
                                is_gif: false,
                            });
                        }
                    }
                }
            }
        }

        if media.is_empty()
            && let Some(url) = Self::extract_card_image(item)
        {
            media.push(TwitterMedia { url, is_gif: false });
        }
        media
    }

    fn extract_card_image(item: &Value) -> Option<String> {
        const KEYS: &[&str] = &[
            "photo_image_full_size_original",
            "summary_photo_image_original",
            "photo_image_full_size_large",
            "summary_photo_image_large",
            "thumbnail_image_original",
            "thumbnail_image_large",
        ];
        KEYS.iter()
            .find_map(|key| Self::str_field(Self::card_binding(item, key, "image_value")?, "url"))
            .map(String::from)
    }

    fn pick_best_video_variant(media_detail: &Value) -> Option<String> {
        let variants = media_detail
            .get("video_info")
            .and_then(|v| v.get("variants"))
            .and_then(|v| v.as_array())?;

        let mut best_bitrate: i64 = -1;
        let mut best_url: Option<String> = None;

        for variant in variants {
            let content_type = variant
                .get("content_type")
                .and_then(|c| c.as_str())
                .unwrap_or("");
            if content_type != "video/mp4" {
                continue;
            }
            let url = variant.get("url").and_then(|u| u.as_str()).unwrap_or("");
            if url.is_empty() || url.contains("/hevc/") {
                continue;
            }
            let bitrate = variant.get("bitrate").and_then(|b| b.as_i64()).unwrap_or(0);
            if bitrate > best_bitrate {
                best_bitrate = bitrate;
                best_url = Some(url.to_string());
            }
        }

        best_url.map(|u| {
            if let Some(idx) = u.find("?tag=") {
                u[..idx].to_string()
            } else {
                u
            }
        })
    }

    fn substitute_tco_urls(text: &str, item: &Value) -> String {
        let mut result = text.to_string();

        if let Some(urls) = item
            .get("entities")
            .and_then(|e| e.get("urls"))
            .and_then(|u| u.as_array())
        {
            for url_obj in urls {
                let tco = url_obj.get("url").and_then(|u| u.as_str());
                let expanded = url_obj.get("expanded_url").and_then(|u| u.as_str());
                if let (Some(t), Some(e)) = (tco, expanded) {
                    result = result.replace(t, e);
                }
            }
        }

        if let Some(media_entities) = item
            .get("entities")
            .and_then(|e| e.get("media"))
            .and_then(|m| m.as_array())
        {
            for media_obj in media_entities {
                if let Some(t) = media_obj.get("url").and_then(|u| u.as_str()) {
                    result = result.replace(t, "");
                }
            }
        }

        Self::html_unescape(result.trim())
    }

    fn html_unescape(s: &str) -> String {
        s.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&#x27;", "'")
    }

    fn extract_tweet_id(url: &str) -> Option<String> {
        if !matches_twitter_host(url) {
            return None;
        }
        let pattern = TWITTER_PATTERN
            .get_or_init(|| Regex::new(r"(?i)/(?:i/web/|[^/?#]+/)?status/(\d+)").unwrap());
        pattern
            .captures(url)?
            .get(1)
            .map(|m| m.as_str().to_string())
    }

    fn resolve_tco_link(url: &str) -> Option<String> {
        debug!("Resolving t.co link: {}", url);
        let response = ureq::head(url)
            .set("User-Agent", TWITTER_DESKTOP_UA)
            .timeout(Duration::from_secs(10))
            .call()
            .ok()?;
        let final_url = response.get_url().to_string();
        if final_url == url {
            let response = ureq::get(url)
                .set("User-Agent", TWITTER_DESKTOP_UA)
                .timeout(Duration::from_secs(10))
                .call()
                .ok()?;
            return Some(response.get_url().to_string());
        }
        Some(final_url)
    }

    fn syndication_token(id_str: &str) -> String {
        let id: f64 = id_str.parse::<f64>().unwrap_or(0.0);
        let v = (id / 1e15) * std::f64::consts::PI;

        let int_part = v.trunc() as u64;
        let mut s = if int_part == 0 {
            "0".to_string()
        } else {
            let mut tmp = String::new();
            let mut x = int_part;
            while x > 0 {
                tmp.push(char::from_digit((x % 36) as u32, 36).unwrap());
                x /= 36;
            }
            tmp.chars().rev().collect()
        };

        let mut frac = v.fract().abs();
        if frac > 0.0 {
            s.push('.');
            for _ in 0..30 {
                frac *= 36.0;
                let d = frac.trunc() as u32;
                s.push(char::from_digit(d, 36).unwrap());
                frac -= d as f64;
                if frac == 0.0 {
                    break;
                }
            }
        }

        let result: String = s.chars().filter(|c| *c != '0' && *c != '.').collect();
        if result.is_empty() {
            "0".to_string()
        } else {
            result
        }
    }
}
