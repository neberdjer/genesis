use crate::db;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::env;
use tracing::info;

async fn check_owner(ctx: Context<'_>) -> Result<bool, Error> {
    let owner_id = env::var("OWNER_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok())
        .map(serenity::UserId::new)
        .ok_or("OWNER_ID not configured")?;
    Ok(ctx.author().id == owner_id)
}

#[poise::command(
    slash_command,
    prefix_command,
    check = "check_owner",
    subcommands("add", "remove", "add_server", "remove_server")
)]
pub async fn blacklist(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn add(
    ctx: Context<'_>,
    #[description = "User to blacklist"] user: serenity::User,
    #[description = "Reason for blacklist"] reason: Option<String>,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;

    db::add_user_to_blacklist(
        pool,
        &user.id.to_string(),
        reason.as_deref(),
        &ctx.author().id.to_string(),
    )
    .await?;

    info!(
        "User {} blacklisted by {} - Reason: {}",
        user.tag(),
        ctx.author().name,
        reason.as_deref().unwrap_or("No reason provided")
    );

    ctx.say(format!(
        "**{}** has been blacklisted{}",
        user.tag(),
        reason
            .map(|r| format!(" - Reason: {}", r))
            .unwrap_or_default()
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn remove(
    ctx: Context<'_>,
    #[description = "User to remove from blacklist"] user: serenity::User,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;

    let removed = db::remove_user_from_blacklist(pool, &user.id.to_string()).await?;

    if removed {
        info!(
            "User {} removed from blacklist by {}",
            user.tag(),
            ctx.author().name
        );

        ctx.say(format!(
            "**{}** has been removed from the blacklist",
            user.tag()
        ))
        .await?;
    } else {
        ctx.say(format!("**{}** was not in the blacklist", user.tag()))
            .await?;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn add_server(
    ctx: Context<'_>,
    #[description = "Server ID to blacklist"] guild_id: String,
    #[description = "Reason for blacklist"] reason: Option<String>,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;

    db::add_server_to_blacklist(
        pool,
        &guild_id,
        reason.as_deref(),
        &ctx.author().id.to_string(),
    )
    .await?;

    info!(
        "Server {} blacklisted by {} - Reason: {}",
        guild_id,
        ctx.author().name,
        reason.as_deref().unwrap_or("No reason provided")
    );

    ctx.say(format!(
        "Server **{}** has been blacklisted{}",
        guild_id,
        reason
            .map(|r| format!(" - Reason: {}", r))
            .unwrap_or_default()
    ))
    .await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn remove_server(
    ctx: Context<'_>,
    #[description = "Server ID to remove from blacklist"] guild_id: String,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;

    let removed = db::remove_server_from_blacklist(pool, &guild_id).await?;

    if removed {
        info!(
            "Server {} removed from blacklist by {}",
            guild_id,
            ctx.author().name
        );

        ctx.say(format!(
            "Server **{}** has been removed from the blacklist",
            guild_id
        ))
        .await?;
    } else {
        ctx.say(format!("Server **{}** was not in the blacklist", guild_id))
            .await?;
    }

    Ok(())
}
