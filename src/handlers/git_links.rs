use super::git_handler::GitFileLink;
use super::shared::{self, SettingCheck};
use crate::constants::{DISCORD_MESSAGE_LIMIT, TRUNCATED_MESSAGE_LIMIT};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::warn;

fn is_git_platform_url(word: &str) -> bool {
    (word.contains("github.com") && word.contains("/blob/"))
        || word.contains("/-/blob/")
        || word.contains("/src/branch/")
        || word.contains("/src/commit/")
        || (word.contains("/src/") && word.contains(".rs.html"))
}

fn clean_url(url: &str) -> &str {
    url.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

async fn send_code_snippet(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    response: String,
) -> bool {
    let footer = format!("\n-# Sent by <@{}>", msg.author.id);
    let max_body = DISCORD_MESSAGE_LIMIT - footer.len();
    let body = if response.len() <= max_body {
        response
    } else {
        let truncate_at = TRUNCATED_MESSAGE_LIMIT.min(max_body.saturating_sub(30));
        format!("{}...\n```\n(Content too long)", &response[..truncate_at])
    };
    let content = format!("{}{}", body, footer);

    let reply = serenity::CreateMessage::new()
        .content(content)
        .reference_message(msg)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    shared::send_reply(ctx, msg, "git_links", reply).await
}

pub async fn handle_git_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_links: Vec<GitFileLink> = msg
        .content
        .split_whitespace()
        .filter(|word| is_git_platform_url(word))
        .filter_map(|word| GitFileLink::parse(clean_url(word)))
        .collect();

    if found_links.is_empty() {
        return;
    }

    let blocklist = shared::fetch_blocklist(pool, msg.guild_id).await;
    found_links.retain(|link| !shared::is_url_in_blocklist(&link.original_url, &blocklist));
    if found_links.is_empty() {
        return;
    }

    if !shared::pre_check(msg, pool, SettingCheck::GitLinks).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "git_links") {
        return;
    }

    let mut any_sent = false;
    let mut any_failed = false;
    for link in found_links {
        match shared::spawn_blocking_fetch(move || link.format_response()).await {
            Ok(response) => {
                if send_code_snippet(ctx, msg, response).await {
                    any_sent = true;
                } else {
                    any_failed = true;
                }
            }
            Err(e) => {
                warn!("Failed to fetch git content: {}", e);
                shared::record_embed(ctx, "git_links", false).await;
                any_failed = true;
            }
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
    if any_failed {
        shared::react_failure(ctx, msg).await;
    }
}
