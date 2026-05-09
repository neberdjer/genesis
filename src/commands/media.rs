use crate::handlers::instagram_handler::InstagramPost;
use crate::handlers::shared;
use crate::handlers::tiktok_handler::TikTokPost;
use crate::handlers::twitter_handler::TwitterPost;
use crate::handlers::{instagram, tiktok, twitter};
use crate::{Context, Error};
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use tracing::debug;

async fn deny(ctx: Context<'_>, message: &str) -> Result<(), Error> {
    ctx.send(CreateReply::default().content(message).ephemeral(true))
        .await?;
    Ok(())
}

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn instagram(
    ctx: Context<'_>,
    #[description = "Instagram post, reel, or share URL"] url: String,
) -> Result<(), Error> {
    let data = ctx.data();
    let pool = Some(data.pool.as_ref());
    if !shared::pre_check_user(ctx.author().id, pool).await {
        return deny(ctx, "You are blacklisted from using this bot.").await;
    }

    if !shared::check_rate_limit(ctx.author().id, "instagram") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching Instagram post via slash command: url={}", url);

    let url_owned = url.clone();
    let post = match shared::spawn_blocking_fetch(move || InstagramPost::fetch(&url_owned)).await {
        Ok(post) => post,
        Err(e) => {
            ctx.send(
                CreateReply::default()
                    .content(format!("Failed to fetch Instagram post: {}", e))
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = instagram::build_container(&post).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;

    Ok(())
}

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn twitter(
    ctx: Context<'_>,
    #[description = "Twitter or X post URL"] url: String,
) -> Result<(), Error> {
    let data = ctx.data();
    let pool = Some(data.pool.as_ref());
    if !shared::pre_check_user(ctx.author().id, pool).await {
        return deny(ctx, "You are blacklisted from using this bot.").await;
    }

    if !shared::check_rate_limit(ctx.author().id, "twitter") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching tweet via slash command: url={}", url);

    let url_owned = url.clone();
    let post = match shared::spawn_blocking_fetch(move || TwitterPost::fetch(&url_owned)).await {
        Ok(post) => post,
        Err(e) => {
            ctx.send(
                CreateReply::default()
                    .content(format!("Failed to fetch tweet: {}", e))
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = twitter::build_container(&post).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;

    Ok(())
}

#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn tiktok(
    ctx: Context<'_>,
    #[description = "TikTok video, photo, or share URL"] url: String,
) -> Result<(), Error> {
    let data = ctx.data();
    let pool = Some(data.pool.as_ref());
    if !shared::pre_check_user(ctx.author().id, pool).await {
        return deny(ctx, "You are blacklisted from using this bot.").await;
    }

    if !shared::check_rate_limit(ctx.author().id, "tiktok") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching TikTok via slash command: url={}", url);

    let resolved_url = TikTokPost::parse(&url).unwrap_or(url.clone());
    let post = match shared::spawn_blocking_fetch(move || TikTokPost::fetch(&resolved_url)).await {
        Ok(post) => post,
        Err(e) => {
            ctx.send(
                CreateReply::default()
                    .content(format!("Failed to fetch TikTok: {}", e))
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = tiktok::build_container(&post).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;

    Ok(())
}
