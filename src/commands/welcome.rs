use crate::db;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

/// Configure welcome messages for new members
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn welcome(
    ctx: Context<'_>,
    #[description = "Enable or disable welcome messages"] enabled: Option<bool>,
    #[description = "Channel to send welcome messages to"] channel: Option<serenity::GuildChannel>,
    #[description = "Message template ({server_name}, {user}, {username}, {member_count})"]
    message: Option<String>,
    #[description = "Role to assign to new members"] role: Option<serenity::Role>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;
    let guild_id_str = guild_id.to_string();

    let has_changes = enabled.is_some() || channel.is_some() || message.is_some() || role.is_some();

    if let Some(enabled) = enabled {
        db::set_welcome_enabled(pool, &guild_id_str, enabled).await?;
    }

    if let Some(channel) = &channel {
        db::set_welcome_channel(pool, &guild_id_str, Some(&channel.id.to_string())).await?;
    }

    if let Some(message) = &message {
        db::set_welcome_message(pool, &guild_id_str, message).await?;
    }

    if let Some(role) = &role {
        db::set_welcome_role(pool, &guild_id_str, Some(&role.id.to_string())).await?;
    }

    let settings = db::get_welcome_settings(pool, &guild_id_str).await?;

    let status = if settings.enabled {
        "Enabled"
    } else {
        "Disabled"
    };

    let channel_display = match &settings.channel_id {
        Some(id) => format!("<#{}>", id),
        None => "Not set".to_string(),
    };

    let role_display = match &settings.role_id {
        Some(id) => format!("<@&{}>", id),
        None => "None".to_string(),
    };

    let response = if has_changes {
        format!(
            "**Welcome Settings Updated**\nStatus: {}\nChannel: {}\nRole: {}\nMessage: `{}`",
            status, channel_display, role_display, settings.message
        )
    } else {
        format!(
            "**Welcome Settings**\nStatus: {}\nChannel: {}\nRole: {}\nMessage: `{}`\n\nPlaceholders: `{{server_name}}`, `{{user}}`, `{{username}}`, `{{member_count}}`",
            status, channel_display, role_display, settings.message
        )
    };

    ctx.say(response).await?;

    Ok(())
}
