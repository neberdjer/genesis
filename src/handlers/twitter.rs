use super::shared::{self, SettingCheck};
use super::twitter_handler::TwitterPost;
use poise::serenity_prelude as serenity;
use serde_json::json;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_twitter_url(word: &str) -> bool {
    (word.contains("twitter.com") || word.contains("x.com")) && word.contains("/status/")
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

    let found_tweets: Vec<(String, String)> = msg
        .content
        .split_whitespace()
        .filter(|word| is_twitter_url(word))
        .filter_map(|word| TwitterPost::parse(clean_url(word)))
        .collect();

    if found_tweets.is_empty() {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "twitter") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for (username, tweet_id) in found_tweets {
        debug!("Fetching tweet: username={}, id={}", username, tweet_id);
        let u = username.clone();
        let t = tweet_id.clone();
        match shared::spawn_blocking_fetch(move || TwitterPost::fetch(&u, &t)).await {
            Ok(post) => {
                let mut content = format!("**{}** (@{})", post.author, post.username);

                if let Some(replying_to) = &post.replying_to {
                    content.push_str(&format!("\nReplying to @{}", replying_to));
                }

                content.push_str(&format!("\n{}", post.text));

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
                if !post.images.is_empty() {
                    let mut media_items = Vec::new();
                    for (i, url) in post.images.iter().enumerate() {
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
            Err(e) => warn!("Failed to fetch tweet {}/{}: {}", username, tweet_id, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
