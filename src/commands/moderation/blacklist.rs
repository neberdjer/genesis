use crate::handlers::shared::normalize_domain;
use crate::{Context, Error, db};
use poise::serenity_prelude as serenity;
use tracing::info;

use super::check_owner;

/// Owner-only blacklist controls for users, servers, and domains
#[poise::command(
    slash_command,
    prefix_command,
    check = "check_owner",
    subcommands(
        "add",
        "remove",
        "add_server",
        "remove_server",
        "add_domain",
        "remove_domain",
        "domains"
    )
)]
pub async fn blacklist(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(
        "Available subcommands:\n\
         - `/blacklist add` — blacklist a user\n\
         - `/blacklist remove` — remove a user from the blacklist\n\
         - `/blacklist add_server` — blacklist a server\n\
         - `/blacklist remove_server` — remove a server from the blacklist\n\
         - `/blacklist add_domain` — globally block a domain\n\
         - `/blacklist remove_domain` — remove a domain from the global blocklist\n\
         - `/blacklist domains` — list globally blocked domains",
    )
    .await?;
    Ok(())
}

/// Globally block a domain across every server (subdomains too)
#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn add_domain(
    ctx: Context<'_>,
    #[description = "Domain to globally block (e.g. tiktok.com)"] domain: String,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;

    let normalized = normalize_domain(&domain);
    if normalized.is_empty() || !normalized.contains('.') {
        ctx.say("Invalid domain. Provide something like `tiktok.com`.")
            .await?;
        return Ok(());
    }

    let already = db::list_global_blocked_domains(pool)
        .await
        .map(|list| list.iter().any(|d| d == &normalized))
        .unwrap_or(false);

    if already {
        ctx.say(format!("**{}** is already globally blocked.", normalized))
            .await?;
        return Ok(());
    }

    db::add_global_blocked_domain(pool, &normalized, &ctx.author().id.to_string()).await?;

    info!(
        "Domain {} globally blocked by {}",
        normalized,
        ctx.author().name
    );

    ctx.say(format!(
        "**{}** has been globally blocked. Subdomains are also blocked.",
        normalized
    ))
    .await?;
    Ok(())
}

/// Remove a domain from the global blocklist
#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn remove_domain(
    ctx: Context<'_>,
    #[description = "Domain to remove from the global blocklist"] domain: String,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;
    let normalized = normalize_domain(&domain);

    let removed = db::remove_global_blocked_domain(pool, &normalized).await?;
    if removed {
        info!(
            "Domain {} unblocked globally by {}",
            normalized,
            ctx.author().name
        );
        ctx.say(format!(
            "**{}** has been removed from the global blocklist.",
            normalized
        ))
        .await?;
    } else {
        ctx.say(format!(
            "**{}** is not on the global blocklist.",
            normalized
        ))
        .await?;
    }
    Ok(())
}

/// List all globally blocked domains
#[poise::command(slash_command, prefix_command, check = "check_owner")]
async fn domains(ctx: Context<'_>) -> Result<(), Error> {
    let pool = &ctx.data().pool;
    let list = db::list_global_blocked_domains(pool).await?;

    if list.is_empty() {
        ctx.say("No domains are globally blocked.").await?;
    } else {
        let body = list
            .iter()
            .map(|d| format!("- {}", d))
            .collect::<Vec<_>>()
            .join("\n");
        ctx.say(format!("**Globally blocked domains:**\n{}", body))
            .await?;
    }
    Ok(())
}

/// Blacklist a user from using the bot
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

    let reason_part = reason
        .map(|r| format!("\n**Reason:** {}", r))
        .unwrap_or_default();

    ctx.say(format!(
        "**{}** has been blacklisted.{}",
        user.tag(),
        reason_part
    ))
    .await?;

    Ok(())
}

/// Remove a user from the blacklist
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
            "**{}** has been removed from the blacklist.",
            user.tag()
        ))
        .await?;
    } else {
        ctx.say(format!("**{}** is not on the blacklist.", user.tag()))
            .await?;
    }

    Ok(())
}

/// Blacklist a server (the bot will refuse to operate there)
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

    let reason_part = reason
        .map(|r| format!("\n**Reason:** {}", r))
        .unwrap_or_default();

    ctx.say(format!(
        "Server **{}** has been blacklisted.{}",
        guild_id, reason_part
    ))
    .await?;

    Ok(())
}

/// Remove a server from the blacklist
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
            "Server **{}** has been removed from the blacklist.",
            guild_id
        ))
        .await?;
    } else {
        ctx.say(format!("Server **{}** is not on the blacklist.", guild_id))
            .await?;
    }

    Ok(())
}
