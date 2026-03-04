use super::tiktok_handler::TikTokPost;
use poise::serenity_prelude as serenity;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::db;
use sqlx::PgPool;

const EMBED_SUPPRESS_DELAY_MS: u64 = 500;
static RATE_LIMIT: OnceLock<Mutex<HashMap<serenity::UserId, Instant>>> = OnceLock::new();

const RATE_LIMIT_SECONDS: u64 = 10;

pub fn is_tiktok_url(word: &str) -> bool {
    word.contains("tiktok.com")
}

pub fn clean_url(url: &str) -> &str {
    url.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

async fn suppress_embeds(ctx: &serenity::Context, msg: &serenity::Message) {
    tokio::time::sleep(Duration::from_millis(EMBED_SUPPRESS_DELAY_MS)).await;

    if let Err(e) = msg
        .channel_id
        .edit_message(
            &ctx.http,
            msg.id,
            serenity::EditMessage::new().suppress_embeds(true),
        )
        .await
    {
        warn!(
            "Failed to suppress embed in channel {} (needs Manage Messages permission): {}",
            msg.channel_id, e
        );
    }
}

pub async fn handle_tiktok_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if msg.author.bot() {
        return;
    }

    if let Some(pool) = pool {
        match db::is_user_blacklisted(pool, &msg.author.id.to_string()).await {
            Ok(true) => return,
            Err(e) => warn!("Failed to check user blacklist: {}", e),
            _ => {}
        }

        if let Some(guild_id) = msg.guild_id {
            match db::is_server_blacklisted(pool, &guild_id.to_string()).await {
                Ok(true) => return,
                Err(e) => warn!("Failed to check server blacklist: {}", e),
                _ => {}
            }

            match db::get_server_settings(pool, &guild_id.to_string()).await {
                Ok(settings) if !settings.tiktok_enabled => return,
                Err(e) => warn!("Failed to fetch server settings: {}", e),
                _ => {}
            }
        }
    }

    handle_tiktok_links_impl(ctx, msg).await;
}

fn check_rate_limit(user_id: serenity::UserId) -> bool {
    let rate_limit = RATE_LIMIT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut rate_limit = match rate_limit.lock() {
        Ok(guard) => guard,
        Err(_) => return true,
    };

    if let Some(last_time) = rate_limit.get(&user_id)
        && last_time.elapsed().as_secs() < RATE_LIMIT_SECONDS
    {
        return false;
    }

    rate_limit.insert(user_id, Instant::now());
    true
}

async fn handle_tiktok_links_impl(ctx: &serenity::Context, msg: &serenity::Message) {
    debug!("Checking message for TikTok URLs: {}", msg.content);

    let words: Vec<&str> = msg.content.split_whitespace().collect();
    debug!("Split into {} words", words.len());

    let found_videos: Vec<String> = words
        .iter()
        .filter(|word| {
            let is_tiktok = is_tiktok_url(word);
            debug!("Word '{}' is TikTok URL: {}", word, is_tiktok);
            is_tiktok
        })
        .filter_map(|word| {
            let cleaned = clean_url(word);
            debug!("Cleaned URL: '{}'", cleaned);
            let parsed = TikTokPost::parse(cleaned);
            debug!("Parsed result: {:?}", parsed);
            parsed
        })
        .collect();

    debug!("Found {} TikTok videos", found_videos.len());

    if found_videos.is_empty() {
        return;
    }

    if !check_rate_limit(msg.author.id) {
        debug!("Rate limited, skipping");
        return;
    }

    debug!("Suppressing embeds");
    suppress_embeds(ctx, msg).await;

    debug!("Processing {} TikTok videos", found_videos.len());
    for video_url in found_videos {
        debug!("Fetching TikTok: url={}", video_url);
        match TikTokPost::fetch(&video_url) {
            Ok(post) => {
                debug!("Successfully fetched TikTok from @{}", post.username);
                debug!("TikTok has {} media items", post.media.len());

                let mut content = format!("**{}** (@{})", post.author, post.username);
                content.push_str(&format!("\n{}", post.text));

                let mut container_components = vec![json!({
                    "type": 10,
                    "content": content
                })];

                if !post.media.is_empty() {
                    debug!("Adding {} media items to payload", post.media.len());
                    let media_items: Vec<_> = post
                        .media
                        .iter()
                        .map(|url| {
                            debug!("Media URL: {}", url);
                            json!({
                                "media": {
                                    "url": url
                                }
                            })
                        })
                        .collect();

                    container_components.push(json!({
                        "type": 12,
                        "items": media_items
                    }));
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

                debug!("Sending TikTok message to channel {}", msg.channel_id);
                let _ = ctx
                    .http
                    .send_message(msg.channel_id, vec![], &payload)
                    .await;
            }
            Err(e) => warn!("Failed to fetch TikTok {}: {}", video_url, e),
        }
    }
}
