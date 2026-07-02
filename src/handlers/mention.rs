use super::shared;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::warn;

fn is_bare_bot_mention(content: &str, bot_id: serenity::UserId) -> bool {
    let trimmed = content.trim();
    trimmed == format!("<@{}>", bot_id) || trimmed == format!("<@!{}>", bot_id)
}

pub async fn handle_bot_mention(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
    prefix: &str,
) {
    let bot_id = ctx.cache.current_user().id;
    if !is_bare_bot_mention(&msg.content, bot_id) {
        return;
    }

    if !shared::pre_check_user(msg.author.id, pool).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "mention") {
        return;
    }

    let content = format!(
        "My prefix is `{}`, use `{}help` to see commands.",
        prefix, prefix
    );
    let reply = serenity::CreateMessage::new()
        .content(content)
        .reference_message(msg)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    if let Err(e) = msg.channel_id.send_message(&ctx.http, reply).await {
        warn!(
            "Failed to send mention reply in channel {}: {}",
            msg.channel_id, e
        );
    }
}
