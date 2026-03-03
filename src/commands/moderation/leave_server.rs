use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use tracing::info;

use super::check_owner;

#[poise::command(slash_command, prefix_command, check = "check_owner")]
pub async fn leave_server(
    ctx: Context<'_>,
    #[description = "Server ID to leave"] guild_id: String,
) -> Result<(), Error> {
    let guild_id_parsed = guild_id.parse::<u64>().map_err(|_| "Invalid server ID")?;
    let guild_id_obj = serenity::GuildId::new(guild_id_parsed);

    match ctx.serenity_context().http.get_guild(guild_id_obj).await {
        Ok(guild) => {
            let guild_name = guild.name.clone();
            guild_id_obj.leave(ctx.serenity_context()).await?;

            info!(
                "Left server {} ({}) by owner command from {}",
                guild_name,
                guild_id,
                ctx.author().name
            );

            ctx.say(format!(
                "Successfully left server **{}** ({})",
                guild_name, guild_id
            ))
            .await?;
        }
        Err(_) => {
            ctx.say(format!("Failed to find server with ID **{}**", guild_id))
                .await?;
        }
    }

    Ok(())
}
