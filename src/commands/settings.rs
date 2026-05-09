use crate::handlers::shared::normalize_domain;
use crate::{Context, Error, db};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    subcommands("toggle", "block_domain", "unblock_domain", "blocked_domains")
)]
pub async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Usage: `/settings toggle <service> <enabled>` or `/settings block_domain`, `/settings unblock_domain`, `/settings blocked_domains`")
        .await?;
    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn toggle(
    ctx: Context<'_>,
    #[description = "Service to toggle"]
    #[autocomplete = "autocomplete_service"]
    service: String,
    #[description = "Enable or disable"] enabled: bool,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;

    db::update_server_setting(pool, &guild_id.to_string(), &service, enabled).await?;

    let status = if enabled { "enabled" } else { "disabled" };
    ctx.say(format!("**{}** has been {}", service, status))
        .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn block_domain(
    ctx: Context<'_>,
    #[description = "Domain to block (e.g. tiktok.com); subdomains are also blocked"]
    domain: String,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let normalized = normalize_domain(&domain);
    if normalized.is_empty() || !normalized.contains('.') {
        ctx.say("Invalid domain. Provide something like `tiktok.com` or `github.com`.")
            .await?;
        return Ok(());
    }

    let pool = &ctx.data().pool;
    db::add_guild_blocked_domain(
        pool,
        &guild_id.to_string(),
        &normalized,
        &ctx.author().id.to_string(),
    )
    .await?;

    ctx.say(format!(
        "Blocked **{}** in this server (also blocks subdomains)",
        normalized
    ))
    .await?;
    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn unblock_domain(
    ctx: Context<'_>,
    #[description = "Domain to unblock"] domain: String,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let normalized = normalize_domain(&domain);
    let pool = &ctx.data().pool;
    let removed = db::remove_guild_blocked_domain(pool, &guild_id.to_string(), &normalized).await?;

    if removed {
        ctx.say(format!("Unblocked **{}** in this server", normalized))
            .await?;
    } else {
        ctx.say(format!(
            "**{}** was not in this server's blocklist",
            normalized
        ))
        .await?;
    }
    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
async fn blocked_domains(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

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
    let services = [
        "git_diffs",
        "git_compares",
        "git_links",
        "twitter",
        "tiktok",
        "instagram",
    ];

    let choices: Vec<_> = services
        .iter()
        .filter(|name| name.starts_with(partial))
        .map(|name| serenity::AutocompleteChoice::from(*name))
        .collect();

    serenity::CreateAutocompleteResponse::new().set_choices(choices)
}
