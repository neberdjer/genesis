use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::env;
use tracing::info;

async fn check_owner(ctx: Context<'_>) -> Result<bool, Error> {
    let owner_id = env::var("OWNER_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok())
        .map(serenity::UserId::new)
        .ok_or("OWNER_ID not configured")?;
    Ok(ctx.author().id == owner_id)
}

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
            ctx.say(format!("Could not find server with ID **{}**", guild_id))
                .await?;
        }
    }

    Ok(())
}
