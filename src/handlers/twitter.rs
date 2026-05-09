use super::shared::{self, SettingCheck};
use super::twitter_handler::TwitterPost;
use crate::constants::TWITTER_DOWNLOAD_UA;
use poise::serenity_prelude as serenity;
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_twitter_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if lower.contains("t.co/") {
        return true;
    }

    let Some(scheme_end) = lower.find("://") else {
        return false;
    };
    let after_scheme = &lower[scheme_end + 3..];
    let host = after_scheme.split('/').next().unwrap_or("");
    let normalized = host
        .trim_start_matches("www.")
        .trim_start_matches("mobile.")
        .trim_start_matches("m.");
    if normalized != "twitter.com" && normalized != "x.com" {
        return false;
    }

    lower.contains("/status/")
}

fn clean_url(url: &str) -> &str {
    let trimmed = url.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

fn media_filename(index: usize, url: &str) -> String {
    let ext = if url.contains(".mp4") || url.contains("video") {
        "mp4"
    } else if url.contains(".webp") {
        "webp"
    } else {
        "jpg"
    };
    format!("twitter_{}.{}", index, ext)
}

pub async fn handle_twitter_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if !shared::pre_check(msg, pool, SettingCheck::Twitter).await {
        return;
    }

    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_twitter_url(word) {
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

    if !shared::check_rate_limit(msg.author.id, "twitter") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for url in found_urls {
        debug!("Fetching tweet: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || TwitterPost::fetch(&url_owned)).await {
            Ok(post) => {
                let mut content = format!("**{}** (@{})", post.author, post.username);

                if let Some(replying_to) = &post.replying_to {
                    content.push_str(&format!("\nReplying to @{}", replying_to));
                }

                if !post.text.is_empty() {
                    content.push_str(&format!("\n{}", post.text));
                }

                if let Some(quote_author) = &post.quote_author
                    && let (Some(quote_username), Some(quote_text)) =
                        (&post.quote_username, &post.quote_text)
                {
                    content.push_str(&format!(
                        "\n\n> **{}** (@{})\n> {}",
                        quote_author, quote_username, quote_text
                    ));
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
                            shared::download_media(media_url, TWITTER_DOWNLOAD_UA).await
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
                        "accent_color": 0x1DA1F2,
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
                    Err(e) => warn!("Failed to send tweet message: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch tweet {}: {}", url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
