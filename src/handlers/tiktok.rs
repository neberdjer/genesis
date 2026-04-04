use super::shared::{self, SettingCheck};
use super::tiktok_handler::TikTokPost;
use poise::serenity_prelude as serenity;
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_tiktok_url(word: &str) -> bool {
    word.contains("tiktok.com")
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
    format!("tiktok_{}.{}", index, ext)
}

pub async fn handle_tiktok_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if !shared::pre_check(msg, pool, SettingCheck::TikTok).await {
        return;
    }

    let found_videos: Vec<String> = msg
        .content
        .split_whitespace()
        .filter(|word| is_tiktok_url(word))
        .filter_map(|word| TikTokPost::parse(clean_url(word)))
        .collect();

    if found_videos.is_empty() {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "tiktok") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for video_url in found_videos {
        debug!("Fetching TikTok: url={}", video_url);
        let url = video_url.clone();
        match shared::spawn_blocking_fetch(move || TikTokPost::fetch(&url)).await {
            Ok(post) => {
                let mut content = format!("**{}** (@{})", post.author, post.username);
                content.push_str(&format!("\n{}", post.text));

                let mut container_components = vec![json!({
                    "type": 10,
                    "content": content
                })];

                let mut attachments = Vec::new();
                if !post.media.is_empty() {
                    let mut media_items = Vec::new();
                    for (i, url) in post.media.iter().enumerate() {
                        let filename = media_filename(i, url);
                        if let Some(data) =
                            shared::download_media(url, "Mozilla/5.0 (compatible; GenesisBot/1.0)")
                                .await
                        {
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
                        "accent_color": 0x000000,
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
                    Err(e) => warn!("Failed to send TikTok message: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch TikTok {}: {}", video_url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
