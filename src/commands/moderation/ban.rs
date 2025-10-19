use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use tracing::info;

/// Ban a user from the server with optional message deletion and softban
#[poise::command(slash_command, prefix_command, required_permissions = "BAN_MEMBERS")]
pub async fn ban(
    ctx: Context<'_>,
    #[description = "User to ban"] user: serenity::User,
    #[description = "Reason for ban"] reason: Option<String>,
    #[description = "Delete message history (days, 0-7)"] delete_days: Option<u8>,
    #[description = "Soft ban (ban then immediately unban)"] softban: Option<bool>,
) -> Result<(), Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server")?;

    // check if trying to ban yourself
    if user.id == ctx.author().id {
        ctx.say("You cannot ban yourself").await?;
        return Ok(());
    }

    // check if trying to ban the bot
    if user.id == ctx.framework().bot_id {
        ctx.say("I cannot ban myself").await?;
        return Ok(());
    }

    // check if target user can be banned (role hierarchy check)
    if let Ok(target_member) = guild_id.member(ctx, user.id).await {
        let author_member = guild_id.member(ctx, ctx.author().id).await?;
        let bot_member = guild_id.member(ctx, ctx.framework().bot_id).await?;

        // check if target is server owner first
        let guild = guild_id.to_partial_guild(ctx).await?;
        if target_member.user.id == guild.owner_id {
            ctx.say("Cannot ban the server owner").await?;
            return Ok(());
        }

        // get highest role positions using simple comparison
        let mut target_highest = 0;
        for role_id in &target_member.roles {
            if let Ok(role) = guild_id.role(ctx, *role_id).await {
                target_highest = target_highest.max(role.position);
            }
        }

        let mut author_highest = 0;
        for role_id in &author_member.roles {
            if let Ok(role) = guild_id.role(ctx, *role_id).await {
                author_highest = author_highest.max(role.position);
            }
        }

        let mut bot_highest = 0;
        for role_id in &bot_member.roles {
            if let Ok(role) = guild_id.role(ctx, *role_id).await {
                bot_highest = bot_highest.max(role.position);
            }
        }

        // check if target has higher role than command author
        if target_highest >= author_highest {
            ctx.say("Cannot ban this user - they have equal or higher role than you")
                .await?;
            return Ok(());
        }

        // check if target has higher role than bot
        if target_highest >= bot_highest {
            ctx.say("Cannot ban this user - they have equal or higher role than me")
                .await?;
            return Ok(());
        }
    }

    let delete_days = delete_days.unwrap_or(0);
    if delete_days > 7 {
        ctx.say("Delete days must be between 0 and 7").await?;
        return Ok(());
    }

    let ban_reason = reason.as_deref().unwrap_or("No reason provided");
    let is_softban = softban.unwrap_or(false);

    match guild_id.member(ctx, user.id).await {
        Ok(_) => {
            // user is still in the server, so they arent banned
        }
        Err(_) => {
            // user is not in server could be banned or just left, check ban list to be sure
            if let Ok(bans) = guild_id.bans(ctx, None, None).await {
                if bans.iter().any(|ban| ban.user.id == user.id) {
                    if is_softban {
                        ctx.say("User is already banned, cannot perform softban")
                            .await?;
                        return Ok(());
                    } else {
                        ctx.say("User is already banned").await?;
                        return Ok(());
                    }
                }
            }
        }
    }

    match guild_id
        .ban_with_reason(ctx, user.id, delete_days, ban_reason)
        .await
    {
        Ok(_) => {
            let action_type = if is_softban { "softban" } else { "ban" };

            info!(
                "{} performed by {} on user {} (ID: {}). Reason: {}. Delete days: {}",
                action_type.to_uppercase(),
                ctx.author().name,
                user.name,
                user.id,
                ban_reason,
                delete_days
            );

            if is_softban {
                match guild_id.unban(ctx, user.id).await {
                    Ok(_) => {
                        ctx.say(format!(
                            "**Softbanned** {} ({})\n**Reason:** {}\n**Messages deleted:** {} days",
                            user.name, user.id, ban_reason, delete_days
                        ))
                        .await?;

                        info!("Softban completed - user {} has been unbanned", user.name);
                    }
                    Err(e) => {
                        ctx.say(format!(
                            "**Banned** {} ({}) but failed to unban for softban: {}\n**Reason:** {}",
                            user.name, user.id, e, ban_reason
                        )).await?;
                    }
                }
            } else {
                ctx.say(format!(
                    "**Banned** {} ({})\n**Reason:** {}\n**Messages deleted:** {} days",
                    user.name, user.id, ban_reason, delete_days
                ))
                .await?;
            }
        }
        Err(e) => {
            ctx.say(format!("Failed to ban {}: {}", user.name, e))
                .await?;
        }
    }

    Ok(())
}
