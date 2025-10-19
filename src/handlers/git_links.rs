use crate::constants::{
    DISCORD_MESSAGE_LIMIT, EMBED_SUPPRESS_DELAY_MS, TRUNCATED_MESSAGE_LIMIT,
};
use crate::git_handler::GitFileLink;
use poise::serenity_prelude as serenity;
use std::time::Duration;
use tracing::{error, warn};

pub fn is_git_platform_url(word: &str) -> bool {
    (word.contains("github.com") && word.contains("/blob/"))
        || word.contains("/-/blob/")
        || word.contains("/src/branch/")
        || word.contains("/src/commit/")
        || (word.contains("/src/") && word.contains(".rs.html"))
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

async fn send_code_snippet(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    response: String,
) {
    let content = if response.len() <= DISCORD_MESSAGE_LIMIT {
        response
    } else {
        format!(
            "{}...\n```\n(Content too long)",
            &response[..TRUNCATED_MESSAGE_LIMIT]
        )
    };

    if let Err(e) = msg
        .channel_id
        .send_message(
            &ctx.http,
            serenity::CreateMessage::new()
                .content(content)
                .reference_message(msg),
        )
        .await
    {
        error!("Failed to send message: {}", e);
    }
}

pub async fn handle_git_links(ctx: &serenity::Context, msg: &serenity::Message) {
    if msg.author.bot {
        return;
    }

    let found_links: Vec<GitFileLink> = msg
        .content
        .split_whitespace()
        .filter(|word| is_git_platform_url(word))
        .filter_map(|word| GitFileLink::parse(clean_url(word)))
        .collect();

    if found_links.is_empty() {
        return;
    }

    suppress_embeds(ctx, msg).await;

    for link in found_links {
        match link.format_response() {
            Ok(response) => send_code_snippet(ctx, msg, response).await,
            Err(e) => warn!("Failed to fetch git content: {}", e),
        }
    }
}
