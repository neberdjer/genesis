use super::instagram_handler::InstagramPost;
use poise::serenity_prelude as serenity;
use serde_json::json;
use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

use crate::db;
use sqlx::PgPool;

const EMBED_SUPPRESS_DELAY_MS: u64 = 500;

fn download_media(url: &str) -> Option<Vec<u8>> {
    let response = ureq::get(url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes).ok()?;
    Some(bytes)
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
static RATE_LIMIT: OnceLock<Mutex<HashMap<serenity::UserId, Instant>>> = OnceLock::new();

const RATE_LIMIT_SECONDS: u64 = 10;

pub fn is_instagram_url(word: &str) -> bool {
    word.contains("instagram.com")
        && (word.contains("/p/") || word.contains("/reel/") || word.contains("/tv/"))
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

pub async fn handle_instagram_links(
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
                Ok(settings) if !settings.instagram_enabled => return,
                Err(e) => warn!("Failed to fetch server settings: {}", e),
                _ => {}
            }
        }
    }

    handle_instagram_links_impl(ctx, msg).await;
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

async fn handle_instagram_links_impl(ctx: &serenity::Context, msg: &serenity::Message) {
    debug!("Checking message for Instagram URLs: {}", msg.content);

    let words: Vec<&str> = msg.content.split_whitespace().collect();
    debug!("Split into {} words", words.len());

    let found_posts: Vec<(String, Option<usize>)> = words
        .iter()
        .filter(|word| {
            let is_instagram = is_instagram_url(word);
            debug!("Word '{}' is Instagram URL: {}", word, is_instagram);
            is_instagram
        })
        .filter_map(|word| {
            let cleaned = clean_url(word);
            debug!("Cleaned URL: '{}'", cleaned);
            let parsed = InstagramPost::parse(cleaned);
            debug!("Parsed result: {:?}", parsed);
            parsed
        })
        .collect();

    debug!("Found {} Instagram posts", found_posts.len());

    if found_posts.is_empty() {
        return;
    }

    if !check_rate_limit(msg.author.id) {
        debug!("Rate limited, skipping");
        return;
    }

    debug!("Processing {} Instagram posts", found_posts.len());
    let mut any_sent = false;
    for (post_id, img_index) in found_posts {
        debug!(
            "Fetching Instagram post: id={}, img_index={:?}",
            post_id, img_index
        );
        match InstagramPost::fetch(&post_id, img_index) {
            Ok(post) => {
                debug!("Successfully fetched Instagram from @{}", post.username);
                debug!("Instagram has {} media items", post.media.len());

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
                    debug!("Downloading {} media items", post.media.len());
                    let mut media_items = Vec::new();
                    for (i, url) in post.media.iter().enumerate() {
                        let filename = media_filename(i, url);
                        if let Some(data) = download_media(url) {
                            debug!("Downloaded {} ({} bytes)", filename, data.len());
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

                debug!("Sending Instagram message to channel {}", msg.channel_id);
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
        debug!("Suppressing embeds");
        suppress_embeds(ctx, msg).await;
    }
}
