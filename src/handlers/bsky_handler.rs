use crate::constants::{BSKY_API_BASE, BSKY_HOSTS, BSKY_PLC_DIRECTORY};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;
use std::time::Duration;

static BSKY_PATTERN: OnceLock<Regex> = OnceLock::new();

pub(crate) fn matches_bsky_host(url: &str) -> bool {
    super::shared::matches_host(url, BSKY_HOSTS, "bsky")
}

pub struct BskyMedia {
    pub url: String,
    pub is_video: bool,
}

pub struct BskyPost {
    pub author: String,
    pub handle: String,
    pub text: String,
    pub media: Vec<BskyMedia>,
    pub external: Option<(String, String)>,
    pub quote_author: Option<String>,
    pub quote_text: Option<String>,
}

impl BskyPost {
    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (actor, rkey) = Self::parse_url(url).ok_or("Not a recognizable Bluesky post URL")?;

        let uri = format!("at://{}/app.bsky.feed.post/{}", actor, rkey);
        let thread_url = format!(
            "{}/xrpc/app.bsky.feed.getPostThread?uri={}",
            BSKY_API_BASE,
            urlencoding::encode(&uri)
        );
        let json: Value = ureq::get(&thread_url)
            .timeout(Duration::from_secs(15))
            .call()?
            .into_json()?;

        let post = json
            .get("thread")
            .and_then(|t| t.get("post"))
            .ok_or("Post not found")?;

        Self::parse_post(post)
    }

    fn parse_url(url: &str) -> Option<(String, String)> {
        let pattern = BSKY_PATTERN
            .get_or_init(|| Regex::new(r"(?i)/profile/([^/?#]+)/post/([^/?#]+)").unwrap());
        if !matches_bsky_host(url) {
            return None;
        }
        let caps = pattern.captures(url)?;
        Some((
            caps.get(1)?.as_str().to_string(),
            caps.get(2)?.as_str().to_string(),
        ))
    }

    fn parse_post(post: &Value) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let author_obj = post.get("author");
        let handle = author_obj
            .and_then(|a| a.get("handle"))
            .and_then(|h| h.as_str())
            .unwrap_or("")
            .to_string();
        let author = author_obj
            .and_then(|a| a.get("displayName"))
            .and_then(|n| n.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&handle)
            .to_string();
        let did = author_obj
            .and_then(|a| a.get("did"))
            .and_then(|d| d.as_str())
            .unwrap_or("");

        let text = post
            .get("record")
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let embed = post.get("embed");
        let (media, external) = Self::parse_embed(embed, did);
        let (quote_author, quote_text) = Self::parse_quote(embed);

        Ok(Self {
            author,
            handle,
            text,
            media,
            external,
            quote_author,
            quote_text,
        })
    }

    fn parse_embed(embed: Option<&Value>, did: &str) -> (Vec<BskyMedia>, Option<(String, String)>) {
        let Some(embed) = embed else {
            return (Vec::new(), None);
        };
        let kind = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");

        let media_view = if kind == "app.bsky.embed.recordWithMedia#view" {
            embed.get("media")
        } else {
            Some(embed)
        };
        let Some(media_view) = media_view else {
            return (Vec::new(), None);
        };

        let mkind = media_view
            .get("$type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        match mkind {
            "app.bsky.embed.images#view" => (Self::images(media_view), None),
            "app.bsky.embed.video#view" => {
                (Self::video(media_view, did).into_iter().collect(), None)
            }
            "app.bsky.embed.external#view" => (Vec::new(), Self::external(media_view)),
            _ => (Vec::new(), None),
        }
    }

    fn images(view: &Value) -> Vec<BskyMedia> {
        view.get("images")
            .and_then(|i| i.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|img| img.get("fullsize").and_then(|u| u.as_str()))
                    .map(|url| BskyMedia {
                        url: url.to_string(),
                        is_video: false,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn video(view: &Value, did: &str) -> Option<BskyMedia> {
        let cid = view.get("cid").and_then(|c| c.as_str())?;
        if did.is_empty() {
            return None;
        }
        let pds = Self::resolve_pds(did)?;
        Some(BskyMedia {
            url: format!(
                "{}/xrpc/com.atproto.sync.getBlob?did={}&cid={}",
                pds.trim_end_matches('/'),
                urlencoding::encode(did),
                urlencoding::encode(cid)
            ),
            is_video: true,
        })
    }

    fn resolve_pds(did: &str) -> Option<String> {
        let doc_url = if let Some(host) = did.strip_prefix("did:web:") {
            format!("https://{}/.well-known/did.json", host)
        } else {
            format!("{}/{}", BSKY_PLC_DIRECTORY, did)
        };
        let doc: Value = ureq::get(&doc_url)
            .timeout(Duration::from_secs(10))
            .call()
            .ok()?
            .into_json()
            .ok()?;
        doc.get("service")
            .and_then(|s| s.as_array())
            .and_then(|services| {
                services.iter().find_map(|svc| {
                    let id = svc.get("id").and_then(|i| i.as_str())?;
                    if id.ends_with("atproto_pds") {
                        svc.get("serviceEndpoint")
                            .and_then(|e| e.as_str())
                            .map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
    }

    fn external(view: &Value) -> Option<(String, String)> {
        let ext = view.get("external")?;
        let uri = ext.get("uri").and_then(|u| u.as_str())?.to_string();
        let title = ext
            .get("title")
            .and_then(|t| t.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&uri)
            .to_string();
        Some((title, uri))
    }

    fn parse_quote(embed: Option<&Value>) -> (Option<String>, Option<String>) {
        let Some(embed) = embed else {
            return (None, None);
        };
        let kind = embed.get("$type").and_then(|t| t.as_str()).unwrap_or("");
        let record = match kind {
            "app.bsky.embed.record#view" => embed.get("record"),
            "app.bsky.embed.recordWithMedia#view" => {
                embed.get("record").and_then(|r| r.get("record"))
            }
            _ => None,
        };
        let Some(record) = record else {
            return (None, None);
        };

        let qauthor = record
            .get("author")
            .and_then(|a| {
                a.get("displayName")
                    .and_then(|n| n.as_str())
                    .filter(|s| !s.is_empty())
                    .or_else(|| a.get("handle").and_then(|h| h.as_str()))
            })
            .map(|s| s.to_string());
        let qtext = record
            .get("value")
            .and_then(|v| v.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string());

        if qauthor.is_none() && qtext.is_none() {
            (None, None)
        } else {
            (qauthor, qtext)
        }
    }
}
