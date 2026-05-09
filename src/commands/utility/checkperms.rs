use poise::serenity_prelude as serenity;

/// Show the bot's permissions in this channel
#[poise::command(slash_command, guild_only)]
pub async fn checkperms(ctx: crate::Context<'_>) -> Result<(), crate::Error> {
    let guild_id = ctx
        .guild_id()
        .ok_or("This command can only be used in a server.")?;
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
                .get(&serenity::ChannelId::new(channel_id.get()))
                .map(|channel| guild.user_permissions_in(channel, member))
        });

    if let Some(permissions) = permission_result {
        let yes_no = |b: bool| if b { "Yes" } else { "No" };
        let response = format!(
            "**Bot permissions in this channel:**\n\
             Send Messages: **{}**\n\
             Manage Messages: **{}**\n\
             Read Message History: **{}**\n\n\
             Channel ID: {}\n\
             Raw permissions: {}",
            yes_no(permissions.send_messages()),
            yes_no(permissions.manage_messages()),
            yes_no(permissions.read_message_history()),
            channel_id,
            permissions.bits()
        );

        ctx.say(response).await?;
    } else {
        ctx.say("Failed to retrieve permissions.").await?;
    }

    Ok(())
}
