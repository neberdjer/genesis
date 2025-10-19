#[poise::command(slash_command, guild_only)]
pub async fn checkperms(ctx: crate::Context<'_>) -> Result<(), crate::Error> {
    let guild_id = ctx.guild_id().ok_or("Not in a guild")?;
    let channel_id = ctx.channel_id();

    let permission_result = ctx.serenity_context().cache.guild(guild_id).map(|guild| {
        let member = guild.members.get(&ctx.serenity_context().cache.current_user().id);
        if let Some(member) = member {
            if let Some(channel) = guild.channels.get(&channel_id) {
                Some(guild.user_permissions_in(channel, member))
            } else {
                None
            }
        } else {
            None
        }
    }).flatten();

    if let Some(permissions) = permission_result {
        let response = format!(
            "**Bot Permissions in this channel:**\n\
             Send Messages: {}\n\
             Manage Messages: {}\n\
             Read Message History: {}\n\n\
             Channel ID: {}\n\
             Raw Permissions: {}",
            if permissions.send_messages() { "✅" } else { "❌" },
            if permissions.manage_messages() { "✅" } else { "❌" },
            if permissions.read_message_history() { "✅" } else { "❌" },
            channel_id,
            permissions.bits()
        );

        ctx.say(response).await?;
    } else {
        ctx.say("Could not retrieve permissions").await?;
    }

    Ok(())
}
