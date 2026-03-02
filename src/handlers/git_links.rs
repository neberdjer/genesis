use super::git_handler::GitFileLink;
use crate::constants::{DISCORD_MESSAGE_LIMIT, EMBED_SUPPRESS_DELAY_MS, TRUNCATED_MESSAGE_LIMIT};
use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{error, warn};

use crate::db;
use sqlx::PgPool;

static RATE_LIMIT: OnceLock<Mutex<HashMap<serenity::UserId, Instant>>> = OnceLock::new();

const RATE_LIMIT_SECONDS: u64 = 10;

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

async fn send_code_snippet(ctx: &serenity::Context, msg: &serenity::Message, response: String) {
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

pub async fn handle_git_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if msg.author.bot {
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
                Ok(settings) if !settings.git_links_enabled => return,
                Err(e) => warn!("Failed to fetch server settings: {}", e),
                _ => {}
            }
        }
    }

    handle_git_links_impl(ctx, msg).await;
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

async fn handle_git_links_impl(ctx: &serenity::Context, msg: &serenity::Message) {
    let found_links: Vec<GitFileLink> = msg
        .content
        .split_whitespace()
        .filter(|word| is_git_platform_url(word))
        .filter_map(|word| GitFileLink::parse(clean_url(word)))
        .collect();

    if found_links.is_empty() {
        return;
    }

    if !check_rate_limit(msg.author.id) {
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
