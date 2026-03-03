#[poise::command(slash_command, guild_only)]
pub async fn checkperms(ctx: crate::Context<'_>) -> Result<(), crate::Error> {
    let guild_id = ctx.guild_id().ok_or("Not in a guild")?;
    let channel_id = ctx.channel_id();

    let permission_result = ctx
        .serenity_context()
        .cache
        .guild(guild_id)
        .and_then(|guild| {
            let member = guild
                .members
                .get(&ctx.serenity_context().cache.current_user().id)?;
            guild
                .channels
                .get(&channel_id)
                .map(|channel| guild.user_permissions_in(channel, member))
        });

    if let Some(permissions) = permission_result {
        let response = format!(
            "**Bot Permissions in this channel:**\n\
             Send Messages: {}\n\
             Manage Messages: {}\n\
             Read Message History: {}\n\n\
             Channel ID: {}\n\
             Raw Permissions: {}",
            if permissions.send_messages() {
                "✅"
            } else {
                "❌"
            },
            if permissions.manage_messages() {
                "✅"
            } else {
                "❌"
            },
            if permissions.read_message_history() {
                "✅"
            } else {
                "❌"
            },
            channel_id,
            permissions.bits()
        );

        ctx.say(response).await?;
    } else {
        ctx.say("Failed to retrieve permissions").await?;
    }

    Ok(())
}
