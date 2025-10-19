use crate::twitter_handler::TwitterPost;
use poise::serenity_prelude as serenity;
use serde_json::json;
use std::time::Duration;
use tracing::warn;

const EMBED_SUPPRESS_DELAY_MS: u64 = 500;

pub fn is_twitter_url(word: &str) -> bool {
    (word.contains("twitter.com") || word.contains("x.com")) && word.contains("/status/")
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

pub async fn handle_twitter_links(ctx: &serenity::Context, msg: &serenity::Message) {
    if msg.author.bot {
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

    suppress_embeds(ctx, msg).await;

    for (username, tweet_id) in found_tweets {
        match TwitterPost::fetch(&username, &tweet_id) {
            Ok(post) => {
                let mut container_components = vec![
                    json!({
                        "type": 10,
                        "content": format!("**{}** (@{})\n{}", post.author, post.username, post.text)
                    })
                ];

                if !post.images.is_empty() {
                    let media_items: Vec<_> = post.images.iter().map(|url| {
                        json!({
                            "media": {
                                "url": url
                            }
                        })
                    }).collect();

                    container_components.push(json!({
                        "type": 12,
                        "items": media_items
                    }));
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

                let _ = ctx.http.send_message(msg.channel_id, vec![], &payload).await;
            }
            Err(e) => warn!("Failed to fetch tweet {}/{}: {}", username, tweet_id, e),
        }
    }
}
