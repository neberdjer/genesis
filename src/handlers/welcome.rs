use crate::db;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{error, warn};

pub async fn handle_member_join(
    ctx: &serenity::Context,
    member: &serenity::Member,
    pool: Option<&PgPool>,
) {
    let Some(pool) = pool else {
        return;
    };

    let guild_id = member.guild_id.to_string();

    match db::is_server_blacklisted(pool, &guild_id).await {
        Ok(true) => return,
        Err(e) => warn!("Failed to check server blacklist: {}", e),
        _ => {}
    }

    let settings = match db::get_welcome_settings(pool, &guild_id).await {
        Ok(settings) => settings,
        Err(e) => {
            error!(
                "Failed to get welcome settings for guild {}: {}",
                guild_id, e
            );
            return;
        }
    };

    if !settings.enabled {
        return;
    }

    // Assign welcome role if configured
    if let Some(role_id_str) = &settings.role_id
        && let Ok(role_id) = role_id_str.parse::<u64>()
        && let Err(e) = ctx
            .http
            .add_member_role(
                member.guild_id,
                member.user.id,
                serenity::RoleId::new(role_id),
                Some("Welcome role"),
            )
            .await
    {
        warn!(
            "Failed to assign welcome role {} to user {}: {}",
            role_id, member.user.id, e
        );
    }

    // Send welcome message if channel is configured
    let Some(channel_id_str) = settings.channel_id else {
        return;
    };

    let channel_id = match channel_id_str.parse::<u64>() {
        Ok(id) => serenity::ChannelId::new(id),
        Err(_) => return,
    };

    let guild_name = member
        .guild_id
        .name(&ctx.cache)
        .unwrap_or_else(|| "the server".to_string());

    let member_count = member
        .guild_id
        .to_guild_cached(&ctx.cache)
        .map(|g| g.member_count.to_string())
        .unwrap_or_else(|| "?".to_string());

    let message = settings
        .message
        .replace("{server_name}", &guild_name)
        .replace("{user}", &format!("<@{}>", member.user.id))
        .replace("{mention}", &format!("<@{}>", member.user.id))
        .replace("{username}", &member.user.name)
        .replace("{member_count}", &member_count);

    let allowed_mentions = if settings.message.contains("{user}") {
        serenity::CreateAllowedMentions::new().users(vec![member.user.id])
    } else {
        serenity::CreateAllowedMentions::new().empty_users()
    };

    let built_message = serenity::CreateMessage::new()
        .content(&message)
        .allowed_mentions(allowed_mentions);

    if let Err(e) = channel_id.send_message(&ctx.http, built_message).await {
        error!("Failed to send welcome message: {}", e);
    }
}
