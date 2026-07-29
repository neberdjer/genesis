use super::deny;
use crate::handlers::bsky_handler::BskyPost;
use crate::handlers::instagram_handler::InstagramPost;
use crate::handlers::shared;
use crate::handlers::tiktok_handler::TikTokPost;
use crate::handlers::twitter_handler::TwitterPost;
use crate::handlers::{bsky, instagram, tiktok, twitter};
use crate::{Context, Error};
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use tracing::debug;

/// Fetch and post an Instagram link (post, reel, IGTV, photo carousel)
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn instagram(
    ctx: Context<'_>,
    #[description = "Instagram post, reel, or share URL"] url: String,
) -> Result<(), Error> {
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
            tracing::warn!("Failed to fetch Instagram post via slash command: {}", e);
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "instagram",
                crate::constants::FAILURE_FETCH,
                Some(&url),
                &e.to_string(),
            );
            ctx.send(
                CreateReply::default()
                    .content("Failed to fetch the Instagram post.")
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = instagram::build_container(&post, ctx.author().id).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;
    crate::db::record_embed(&ctx.data().pool, "instagram", true).await;

    Ok(())
}

/// Fetch and post a tweet (text, photos, videos, quote tweets)
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn twitter(
    ctx: Context<'_>,
    #[description = "Twitter or X post URL"] url: String,
) -> Result<(), Error> {
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
            tracing::warn!("Failed to fetch tweet via slash command: {}", e);
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "twitter",
                crate::constants::FAILURE_FETCH,
                Some(&url),
                &e.to_string(),
            );
            ctx.send(
                CreateReply::default()
                    .content("Failed to fetch the tweet.")
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = twitter::build_container(&post, ctx.author().id).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;
    crate::db::record_embed(&ctx.data().pool, "twitter", true).await;

    Ok(())
}

/// Fetch and post a Bluesky post (text, images, video, quotes)
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn bsky(
    ctx: Context<'_>,
    #[description = "Bluesky post URL"] url: String,
) -> Result<(), Error> {
    if !shared::check_rate_limit(ctx.author().id, "bsky") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching Bluesky post via slash command: url={}", url);

    let url_owned = url.clone();
    let post = match shared::spawn_blocking_fetch(move || BskyPost::fetch(&url_owned)).await {
        Ok(post) => post,
        Err(e) => {
            tracing::warn!("Failed to fetch Bluesky post via slash command: {}", e);
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "bsky",
                crate::constants::FAILURE_FETCH,
                Some(&url),
                &e.to_string(),
            );
            ctx.send(
                CreateReply::default()
                    .content("Failed to fetch the Bluesky post.")
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = bsky::build_container(&post, ctx.author().id).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;
    crate::db::record_embed(&ctx.data().pool, "bsky", true).await;

    Ok(())
}

/// Fetch and post a TikTok video or photo slideshow
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn tiktok(
    ctx: Context<'_>,
    #[description = "TikTok video, photo, or share URL"] url: String,
) -> Result<(), Error> {
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
            tracing::warn!("Failed to fetch TikTok via slash command: {}", e);
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "tiktok",
                crate::constants::FAILURE_FETCH,
                Some(&url),
                &e.to_string(),
            );
            ctx.send(
                CreateReply::default()
                    .content("Failed to fetch the TikTok post.")
                    .ephemeral(true),
            )
            .await?;
            return Ok(());
        }
    };

    let (attachments, container) = tiktok::build_container(&post, ctx.author().id).await;
    let mut reply = CreateReply::default()
        .components(vec![serenity::CreateComponent::Container(container)])
        .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));
    for attachment in attachments {
        reply = reply.attachment(attachment);
    }
    ctx.send(reply).await?;
    crate::db::record_embed(&ctx.data().pool, "tiktok", true).await;

    Ok(())
}
