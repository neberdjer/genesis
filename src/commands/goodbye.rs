use crate::db;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

/// Configure goodbye messages for members who leave
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn goodbye(
    ctx: Context<'_>,
    #[description = "Enable or disable goodbye messages"] enabled: Option<bool>,
    #[description = "Channel to send goodbye messages to"] channel: Option<serenity::GuildChannel>,
    #[description = "Message template ({server_name}, {username}, {member_count})"] message: Option<
        String,
    >,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    let pool = &ctx.data().pool;
    let guild_id_str = guild_id.to_string();

    let has_changes = enabled.is_some() || channel.is_some() || message.is_some();

    if let Some(enabled) = enabled {
        db::set_goodbye_enabled(pool, &guild_id_str, enabled).await?;
    }

    if let Some(channel) = &channel {
        db::set_goodbye_channel(pool, &guild_id_str, Some(&channel.id.to_string())).await?;
    }

    if let Some(message) = &message {
        db::set_goodbye_message(pool, &guild_id_str, message).await?;
    }

    let settings = db::get_goodbye_settings(pool, &guild_id_str).await?;

    let status = if settings.enabled {
        "Enabled"
    } else {
        "Disabled"
    };

    let channel_display = match &settings.channel_id {
        Some(id) => format!("<#{}>", id),
        None => "Not set".to_string(),
    };

    let response = if has_changes {
        format!(
            "**Goodbye Settings Updated**\nStatus: {}\nChannel: {}\nMessage: `{}`",
            status, channel_display, settings.message
        )
    } else {
        format!(
            "**Goodbye Settings**\nStatus: {}\nChannel: {}\nMessage: `{}`\n\nPlaceholders: `{{server_name}}`, `{{username}}`, `{{member_count}}`\nThe member who left is shown by name, never pinged (a ping would just show \"unknown user\").",
            status, channel_display, settings.message
        )
    };

    ctx.say(response).await?;

    Ok(())
}
