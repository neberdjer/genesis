use crate::db;
use crate::{Context, Error};

#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    subcommands("toggle")
)]
pub async fn settings(_ctx: Context<'_>) -> Result<(), Error> {
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

#[allow(dead_code)]
async fn autocomplete_service<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    ["git_diffs", "git_compares", "git_links", "twitter"]
        .iter()
        .filter(move |name| name.starts_with(partial))
        .map(|name| name.to_string())
}
