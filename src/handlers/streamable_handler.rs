use crate::constants::{STREAMABLE_HOSTS, STREAMABLE_MAX_UPLOAD_BYTES};
use regex::Regex;
use serde::Deserialize;
use std::sync::OnceLock;

static STREAMABLE_PATTERN: OnceLock<Regex> = OnceLock::new();

pub(crate) fn matches_streamable_host(url: &str) -> bool {
    super::shared::matches_host(url, STREAMABLE_HOSTS, "streamable")
}

#[derive(Deserialize)]
struct StreamableResponse {
    status: i32,
    title: Option<String>,
    #[serde(default)]
    files: Files,
}

#[derive(Deserialize, Default)]
struct Files {
    mp4: Option<StreamableFile>,
    #[serde(rename = "mp4-mobile")]
    mp4_mobile: Option<StreamableFile>,
}

#[derive(Deserialize)]
struct StreamableFile {
    url: Option<String>,
    size: Option<u64>,
}

pub enum Media {
    Attach(String),
    Stream(String),
}

pub struct StreamablePost {
    pub title: String,
    pub media: Media,
}

impl StreamablePost {
    pub fn extract_shortcode(url: &str) -> Option<String> {
        if !matches_streamable_host(url) {
            return None;
        }
        let pattern = STREAMABLE_PATTERN
            .get_or_init(|| Regex::new(r"(?i)streamable\.com/(?:e/|o/|s/)?([a-z0-9]+)").unwrap());
        pattern
            .captures(url)?
            .get(1)
            .map(|m| m.as_str().to_string())
    }

    pub fn fetch(url: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let shortcode = Self::extract_shortcode(url).ok_or("Not a recognizable Streamable URL")?;
        let api_url = format!("https://api.streamable.com/videos/{}", shortcode);

        let response = match ureq::get(&api_url).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) => return Err("Streamable video not found".into()),
            Err(e) => return Err(e.into()),
        };

        let data: StreamableResponse = response.into_json()?;
        if data.status != 2 {
            return Err("Streamable video is not ready".into());
        }

        let media = Self::pick_file(&data.files).ok_or("No playable Streamable video")?;

        Ok(Self {
            title: data.title.unwrap_or_default(),
            media,
        })
    }

    fn pick_file(files: &Files) -> Option<Media> {
        let cap = STREAMABLE_MAX_UPLOAD_BYTES as u64;
        let fits = |file: &StreamableFile| file.size.is_none_or(|size| size <= cap);

        let mut fallback = None;
        for file in [&files.mp4, &files.mp4_mobile].into_iter().flatten() {
            let Some(url) = &file.url else {
                continue;
            };
            if fits(file) {
                return Some(Media::Attach(url.clone()));
            }
            fallback.get_or_insert_with(|| url.clone());
        }
        fallback.map(Media::Stream)
    }
}
