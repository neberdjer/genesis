use crate::{Context, Error};
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
                poise::serenity_prelude::EditMessage::new().content(format!(
                    "Pong! (Response time: {:.2}ms)",
                    latency.as_millis()
                )),
            )
            .await;
    }

    Ok(())
}
