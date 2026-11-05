use crate::db;
use crate::handlers::shared;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

/// Configure goodbye messages for members who leave
#[poise::command(slash_command, guild_only, required_permissions = "ADMINISTRATOR")]
pub async fn goodbye(
    ctx: Context<'_>,
    #[description = "Enable or disable goodbye messages"] enabled: Option<bool>,
    #[description = "Channel to send goodbye messages to"] channel: Option<serenity::GuildChannel>,
    #[description = "Message template ({server_name}, {username}, {member_count})"]
    #[max_length = 1500]
    message: Option<String>,
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

    let (title, help) = if has_changes {
        ("Goodbye Settings Updated", "")
    } else {
        (
            "Goodbye Settings",
            "\n\nPlaceholders: `{server_name}`, `{username}`, `{member_count}`\nI show the member's name instead of pinging them, since a ping would just show \"unknown user\".",
        )
    };
    let response = format!(
        "**{title}**\nStatus: {status}\nChannel: {channel_display}\nMessage: {}{help}",
        shared::template_block(&settings.message)
    );

    ctx.say(response).await?;

    Ok(())
}
