use super::shared;
use crate::constants::GOODBYE_RATE_LIMIT_SECONDS;
use crate::db;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tracing::{error, warn};

static GOODBYE_RATE_LIMITS: OnceLock<Mutex<HashMap<serenity::GuildId, Instant>>> = OnceLock::new();

pub async fn handle_member_leave(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    user: &serenity::User,
    pool: Option<&PgPool>,
) {
    if user.bot() {
        return;
    }

    if !shared::check_guild_rate_limit(&GOODBYE_RATE_LIMITS, guild_id, GOODBYE_RATE_LIMIT_SECONDS) {
        return;
    }

    let Some(pool) = pool else {
        return;
    };

    let guild_id_str = guild_id.to_string();

    match db::is_server_blacklisted(pool, &guild_id_str).await {
        Ok(true) => return,
        Err(e) => warn!("Failed to check server blacklist: {}", e),
        _ => {}
    }

    let settings = match db::get_goodbye_settings(pool, &guild_id_str).await {
        Ok(settings) => settings,
        Err(e) => {
            error!(
                "Failed to get goodbye settings for guild {}: {}",
                guild_id_str, e
            );
            return;
        }
    };

    if !settings.enabled || settings.message.trim().is_empty() {
        return;
    }

    let Some(channel_id_str) = settings.channel_id else {
        return;
    };

    let channel_id = match channel_id_str.parse::<u64>() {
        Ok(id) => serenity::ChannelId::new(id),
        Err(_) => return,
    };

    let guild_name = guild_id
        .name(&ctx.cache)
        .unwrap_or_else(|| "the server".to_string());

    let member_count = guild_id
        .to_guild_cached(&ctx.cache)
        .map(|g| g.member_count.to_string())
        .unwrap_or_else(|| "?".to_string());

    let message = settings
        .message
        .replace("{server_name}", &guild_name)
        .replace("{username}", &user.name)
        .replace("{member_count}", &member_count);

    let built_message = serenity::CreateMessage::new()
        .content(&message)
        .allowed_mentions(serenity::CreateAllowedMentions::new().empty_users());

    if let Err(e) = channel_id
        .widen()
        .send_message(&ctx.http, built_message)
        .await
    {
        error!("Failed to send goodbye message: {}", e);
    }
}
