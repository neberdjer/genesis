use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use tracing::info;

/// Kick a user from the server
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    required_permissions = "KICK_MEMBERS"
)]
pub async fn kick(
    ctx: Context<'_>,
    #[description = "User to kick"] user: serenity::User,
    #[description = "Reason for kick"] reason: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    if user.id == ctx.author().id {
        ctx.say("You cannot kick yourself").await?;
        return Ok(());
    }

    if user.id == ctx.framework().bot_id {
        ctx.say("Fine, I'll leave...").await?;

        if let Err(e) = guild_id.leave(ctx).await {
            ctx.say(format!("Failed to leave server: {}", e)).await?;
        } else {
            info!(
                "Bot was kicked from server {} by {}",
                guild_id,
                ctx.author().name
            );
        }

        return Ok(());
    }

    if let Ok(target_member) = guild_id.member(ctx, user.id).await {
        let author_member = guild_id.member(ctx, ctx.author().id).await?;
        let bot_member = guild_id.member(ctx, ctx.framework().bot_id).await?;

        let guild = guild_id.to_partial_guild(ctx).await?;
        if target_member.user.id == guild.owner_id {
            ctx.say("Cannot kick the server owner").await?;
            return Ok(());
        }

        let target_highest = super::get_highest_role(ctx, guild_id, &target_member).await;
        let author_highest = super::get_highest_role(ctx, guild_id, &author_member).await;
        let bot_highest = super::get_highest_role(ctx, guild_id, &bot_member).await;

        if target_highest >= author_highest {
            ctx.say("Cannot kick this user - they have equal or higher role than you")
                .await?;
            return Ok(());
        }

        if target_highest >= bot_highest {
            ctx.say("Cannot kick this user - they have equal or higher role than me")
                .await?;
            return Ok(());
        }
    } else {
        ctx.say("User is not in this server").await?;
        return Ok(());
    }

    let kick_reason = reason.as_deref().unwrap_or("No reason provided");

    match guild_id.kick_with_reason(ctx, user.id, kick_reason).await {
        Ok(_) => {
            info!(
                "KICK performed by {} on user {} (ID: {}). Reason: {}",
                ctx.author().name,
                user.name,
                user.id,
                kick_reason
            );

            ctx.say(format!(
                "**Kicked** {} ({})\n**Reason:** {}",
                user.name, user.id, kick_reason
            ))
            .await?;
        }
        Err(e) => {
            ctx.say(format!("Failed to kick {}: {}", user.name, e))
                .await?;
        }
    }

    Ok(())
}
