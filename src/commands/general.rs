use crate::{Context, Error, db};
use poise::serenity_prelude::EditMessage;
use tracing::info;

#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let start = std::time::Instant::now();
    let msg = ctx.say("Pong!").await?;
    let latency = start.elapsed();

    info!(
        "Ping command by {} - response time: {:.2}ms",
        ctx.author().name,
        latency.as_millis()
    );

    if let Ok(mut msg) = msg.into_message().await {
        let _ = msg
            .edit(
                ctx,
                EditMessage::new().content(format!(
                    "Pong! (Response time: {:.2}ms)",
                    latency.as_millis()
                )),
            )
            .await;
    }

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn stats(ctx: Context<'_>) -> Result<(), Error> {
    let pool = &ctx.data().pool;

    let cache = ctx.cache();
    let guilds = cache.guilds();
    let guild_count = guilds.len();
    let (user_count, channel_count) = {
        let mut users = 0u64;
        let mut channels = 0u64;
        for guild_id in &guilds {
            if let Some(guild) = cache.guild(*guild_id) {
                users += u64::from(u32::from(guild.member_count));
                channels += guild.channels.len() as u64;
            }
        }
        (users, channels)
    };

    let uptime = ctx.data().start_time.elapsed();
    let days = uptime.as_secs() / 86400;
    let hours = (uptime.as_secs() % 86400) / 3600;
    let minutes = (uptime.as_secs() % 3600) / 60;
    let seconds = uptime.as_secs() % 60;
    let uptime_str = if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    };

    let blacklisted_users = db::count_blacklisted_users(pool).await.unwrap_or(0);
    let blacklisted_servers = db::count_blacklisted_servers(pool).await.unwrap_or(0);
    let configured_servers = db::count_configured_servers(pool).await.unwrap_or(0);

    let response = format!(
        "**Genesis Stats**\n\
         Servers: **{}** | Users: **{}** | Channels: **{}**\n\
         Uptime: **{}**\n\
         Configured Servers: **{}**\n\
         Blacklisted Users: **{}** | Blacklisted Servers: **{}**\n\
         Version: **{}**",
        guild_count,
        user_count,
        channel_count,
        uptime_str,
        configured_servers,
        blacklisted_users,
        blacklisted_servers,
        env!("CARGO_PKG_VERSION"),
    );

    ctx.say(response).await?;

    info!(
        "Stats command by {} - {} servers, {} users",
        ctx.author().name,
        guild_count,
        user_count
    );

    Ok(())
}
