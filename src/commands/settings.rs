use crate::db;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    subcommands("toggle")
)]
pub async fn settings(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Usage: `/settings toggle <service> <enabled>`")
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
