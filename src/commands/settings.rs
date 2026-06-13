use crate::handlers::shared::normalize_domain;
use crate::{Context, Error, db};
use poise::serenity_prelude as serenity;

const SERVICES: &[&str] = &[
    "git_diffs",
    "git_compares",
    "git_links",
    "twitter",
    "tiktok",
    "instagram",
    "reddit",
];

/// Configure server-wide bot settings (admin only)
#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    subcommands("toggle", "block_domain", "unblock_domain", "blocked_domains")
)]
pub async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(
        "Available subcommands:\n\
         - `/settings toggle` — enable or disable a service\n\
         - `/settings block_domain` — block a domain in this server\n\
         - `/settings unblock_domain` — unblock a domain in this server\n\
         - `/settings blocked_domains` — list blocked domains in this server",
    )
    .await?;
    Ok(())
}

/// Enable or disable a service in this server
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn toggle(
    ctx: Context<'_>,
    #[description = "Service to toggle"]
    #[autocomplete = "autocomplete_service"]
    service: String,
    #[description = "True to enable, False to disable"] enabled: bool,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server.")?;

    let service = service.to_ascii_lowercase();
    if !SERVICES.contains(&service.as_str()) {
        ctx.send(
            poise::CreateReply::default()
                .content(format!(
                    "Unknown service `{}`. Valid options: {}.",
                    service,
                    SERVICES
                        .iter()
                        .map(|s| format!("`{}`", s))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let pool = &ctx.data().pool;

    db::update_server_setting(pool, &guild_id.to_string(), &service, enabled).await?;

    let status = if enabled { "enabled" } else { "disabled" };
    ctx.say(format!("**{}** has been {}.", service, status))
        .await?;

    Ok(())
}

/// Block a domain in this server (subdomains are also blocked)
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn block_domain(
    ctx: Context<'_>,
    #[description = "Domain to block (e.g. tiktok.com)"] domain: String,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server.")?;

    let normalized = normalize_domain(&domain);
    if normalized.is_empty() || !normalized.contains('.') {
        ctx.say("Invalid domain. Provide something like `tiktok.com`.")
            .await?;
        return Ok(());
    }

    let pool = &ctx.data().pool;
    let already = db::list_guild_blocked_domains(pool, &guild_id.to_string())
        .await
        .map(|list| list.iter().any(|d| d == &normalized))
        .unwrap_or(false);

    if already {
        ctx.say(format!(
            "**{}** is already blocked in this server.",
            normalized
        ))
        .await?;
        return Ok(());
    }

    db::add_guild_blocked_domain(
        pool,
        &guild_id.to_string(),
        &normalized,
        &ctx.author().id.to_string(),
    )
    .await?;

    ctx.say(format!(
        "**{}** has been blocked in this server. Subdomains are also blocked.",
        normalized
    ))
    .await?;
    Ok(())
}

/// Unblock a domain in this server
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn unblock_domain(
    ctx: Context<'_>,
    #[description = "Domain to unblock"] domain: String,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server.")?;

    let normalized = normalize_domain(&domain);
    let pool = &ctx.data().pool;
    let removed = db::remove_guild_blocked_domain(pool, &guild_id.to_string(), &normalized).await?;

    if removed {
        ctx.say(format!(
            "**{}** has been unblocked in this server.",
            normalized
        ))
        .await?;
    } else {
        ctx.say(format!(
            "**{}** is not on this server's blocklist.",
            normalized
        ))
        .await?;
    }
    Ok(())
}

/// List all domains blocked in this server
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn blocked_domains(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server.")?;

    let pool = &ctx.data().pool;
    let domains = db::list_guild_blocked_domains(pool, &guild_id.to_string()).await?;

    if domains.is_empty() {
        ctx.say("No domains are blocked in this server.").await?;
    } else {
        let list = domains
            .iter()
            .map(|d| format!("- {}", d))
            .collect::<Vec<_>>()
            .join("\n");
        ctx.say(format!("**Blocked domains in this server:**\n{}", list))
            .await?;
    }
    Ok(())
}

#[allow(clippy::unused_async)]
async fn autocomplete_service<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> serenity::CreateAutocompleteResponse<'a> {
    let partial_lower = partial.to_ascii_lowercase();
    let choices: Vec<_> = SERVICES
        .iter()
        .filter(|name| name.starts_with(&partial_lower))
        .map(|name| serenity::AutocompleteChoice::from(*name))
        .collect();

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}
