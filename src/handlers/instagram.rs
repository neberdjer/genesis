use super::instagram_handler::InstagramPost;
use super::shared::{self, SettingCheck};
use poise::serenity_prelude as serenity;
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, warn};

const INSTAGRAM_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

fn is_instagram_url(word: &str) -> bool {
    word.contains("instagram.com")
        && (word.contains("/p/") || word.contains("/reel/") || word.contains("/tv/"))
}

fn clean_url(url: &str) -> &str {
    url.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
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

    let found_posts: Vec<(String, Option<usize>)> = msg
        .content
        .split_whitespace()
        .filter(|word| is_instagram_url(word))
        .filter_map(|word| InstagramPost::parse(clean_url(word)))
        .collect();

    if found_posts.is_empty() {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "instagram") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for (post_id, img_index) in found_posts {
        debug!(
            "Fetching Instagram post: id={}, img_index={:?}",
            post_id, img_index
        );
        let pid = post_id.clone();
        match shared::spawn_blocking_fetch(move || InstagramPost::fetch(&pid, img_index)).await {
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
                    for (i, url) in post.media.iter().enumerate() {
                        let filename = media_filename(i, url);
                        if let Some(data) = shared::download_media(url, INSTAGRAM_UA).await {
                            attachments
                                .push(serenity::CreateAttachment::bytes(data, filename.clone()));
                            media_items.push(json!({
                                "media": {
                                    "url": format!("attachment://{}", filename)
                                }
                            }));
                        } else {
                            warn!("Failed to download media: {}", url);
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
            Err(e) => warn!("Failed to fetch Instagram {}: {}", post_id, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
