use super::instagram_handler::InstagramPost;
use super::shared::{self, SettingCheck};
use crate::constants::{INSTAGRAM_DESKTOP_UA, INSTAGRAM_MIRROR_UA, INSTAGRAM_MIRRORS};
use poise::serenity_prelude as serenity;
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_instagram_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if !lower.contains("instagram.com") {
        return false;
    }
    lower.contains("/p/")
        || lower.contains("/reel/")
        || lower.contains("/reels/")
        || lower.contains("/tv/")
        || lower.contains("/share/")
}

fn clean_url(url: &str) -> &str {
    let trimmed = url.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

fn ua_for_media_url(url: &str) -> &'static str {
    if INSTAGRAM_MIRRORS.iter().any(|m| url.contains(m)) {
        INSTAGRAM_MIRROR_UA
    } else {
        INSTAGRAM_DESKTOP_UA
    }
}

fn media_filename(index: usize, url: &str) -> String {
    let ext = if url.contains(".mp4") || url.contains("video") {
        "mp4"
    } else if url.contains(".webp") {
        "webp"
    } else {
        "jpg"
    };
    format!("instagram_{}.{}", index, ext)
}

pub async fn handle_instagram_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if !shared::pre_check(msg, pool, SettingCheck::Instagram).await {
        return;
    }

    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_instagram_url(word) {
            continue;
        }
        let cleaned = clean_url(word).to_string();
        if seen.insert(cleaned.clone()) {
            found_urls.push(cleaned);
        }
    }

    if found_urls.is_empty() {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "instagram") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for url in found_urls {
        debug!("Fetching Instagram post: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || InstagramPost::fetch(&url_owned)).await {
            Ok(post) => {
                let mut content = format!("**{}** (@{})", post.author, post.username);
                if !post.text.is_empty() {
                    content.push_str(&format!("\n{}", post.text));
                }

                let mut container_components = vec![json!({
                    "type": 10,
                    "content": content
                })];

                let mut attachments = Vec::new();
                if !post.media.is_empty() {
                    let mut media_items = Vec::new();
                    for (i, media_url) in post.media.iter().enumerate() {
                        let filename = media_filename(i, media_url);
                        if let Some(data) =
                            shared::download_media(media_url, ua_for_media_url(media_url)).await
                        {
                            attachments
                                .push(serenity::CreateAttachment::bytes(data, filename.clone()));
                            media_items.push(json!({
                                "media": {
                                    "url": format!("attachment://{}", filename)
                                }
                            }));
                        } else {
                            warn!("Failed to download media: {}", media_url);
                        }
                    }

                    if !media_items.is_empty() {
                        container_components.push(json!({
                            "type": 12,
                            "items": media_items
                        }));
                    }
                }

                let payload = json!({
                    "components": [{
                        "type": 17,
                        "accent_color": 0xE4405F,
                        "components": container_components
                    }],
                    "message_reference": {
                        "message_id": msg.id.to_string()
                    },
                    "allowed_mentions": {
                        "parse": [],
                        "replied_user": false
                    },
                    "flags": 1 << 15
                });

                match ctx
                    .http
                    .send_message(msg.channel_id, attachments, &payload)
                    .await
                {
                    Ok(_) => any_sent = true,
                    Err(e) => warn!("Failed to send Instagram message: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch Instagram {}: {}", url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
