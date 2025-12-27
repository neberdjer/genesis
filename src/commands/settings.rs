use crate::db;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    subcommands("toggle", "welcome")
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
    [
        "git_diffs",
        "git_compares",
        "git_links",
        "twitter",
        "tiktok",
        "instagram",
    ]
    .iter()
    .filter(move |name| name.starts_with(partial))
    .map(|name| name.to_string())
}

#[poise::command(
    slash_command,
    guild_only,
    required_permissions = "ADMINISTRATOR",
    subcommands("welcome_enable", "welcome_channel", "welcome_message", "welcome_show")
)]
async fn welcome(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "enable")]
async fn welcome_enable(
    ctx: Context<'_>,
    #[description = "Enable or disable welcome messages"] enabled: bool,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;

    db::set_welcome_enabled(pool, &guild_id.to_string(), enabled).await?;

    let status = if enabled { "enabled" } else { "disabled" };
    ctx.say(format!("Welcome messages have been **{}**", status))
        .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "channel")]
async fn welcome_channel(
    ctx: Context<'_>,
    #[description = "Channel to send welcome messages to"] channel: serenity::Channel,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;

    db::set_welcome_channel(pool, &guild_id.to_string(), Some(&channel.id().to_string())).await?;

    ctx.say(format!("Welcome channel set to <#{}>", channel.id()))
        .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "message")]
async fn welcome_message(
    ctx: Context<'_>,
    #[description = "Welcome message template. Use {server_name}, {user}, {username}, {member_count}"]
    message: String,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;

    db::set_welcome_message(pool, &guild_id.to_string(), &message).await?;

    ctx.say(format!("Welcome message set to:\n> {}", message))
        .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR", rename = "show")]
async fn welcome_show(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;

    let settings = db::get_welcome_settings(pool, &guild_id.to_string()).await?;

    let channel_display = match &settings.channel_id {
        Some(id) => format!("<#{}>", id),
        None => "Not set".to_string(),
    };

    let status = if settings.enabled { "Enabled" } else { "Disabled" };

    ctx.say(format!(
        "**Welcome Settings**\nStatus: {}\nChannel: {}\nMessage: `{}`\n\nPlaceholders: `{{server_name}}`, `{{user}}`, `{{username}}`, `{{member_count}}`",
        status, channel_display, settings.message
    ))
    .await?;

    Ok(())
}
