use super::deny;
use crate::{Context, Error, db};

/// Stop genesis from automatically embedding links in your messages
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn optout(ctx: Context<'_>) -> Result<(), Error> {
    db::set_user_opted_out(&ctx.data().pool, &ctx.author().id.to_string(), true).await?;
    deny(
        ctx,
        "**Auto-embedding** has been disabled for you. Genesis will no longer process your messages; \
         slash commands you run yourself still work. Use `/optin` to reverse this.",
    )
    .await
}

/// Let genesis automatically embed links in your messages again
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn optin(ctx: Context<'_>) -> Result<(), Error> {
    db::set_user_opted_out(&ctx.data().pool, &ctx.author().id.to_string(), false).await?;
    deny(
        ctx,
        "**Auto-embedding** has been enabled for you. Genesis will process your messages again.",
    )
    .await
}
