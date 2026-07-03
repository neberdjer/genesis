use crate::constants::REMINDER_POLL_SECONDS;
use crate::{Data, db};
use poise::serenity_prelude as serenity;
use std::time::Duration;
use tracing::warn;

pub async fn reminder_poller(ctx: serenity::Context) {
    let data = ctx.data::<Data>();
    let mut interval = tokio::time::interval(Duration::from_secs(REMINDER_POLL_SECONDS));
    loop {
        interval.tick().await;
        let due = match db::due_reminders(&data.pool, chrono::Utc::now().timestamp()).await {
            Ok(due) => due,
            Err(e) => {
                warn!("Failed to fetch due reminders: {}", e);
                continue;
            }
        };

        if due.is_empty() {
            continue;
        }

        let ids: Vec<i32> = due.iter().map(|r| r.id).collect();
        for reminder in due {
            if let Err(e) = send_reminder(&ctx, &reminder).await {
                warn!("Failed to send reminder {}: {}", reminder.id, e);
            }
        }
        if let Err(e) = db::delete_reminders(&data.pool, &ids).await {
            warn!("Failed to delete fired reminders: {}", e);
        }
    }
}

async fn send_reminder(
    ctx: &serenity::Context,
    reminder: &db::Reminder,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel_id = serenity::GenericChannelId::new(reminder.channel_id.parse()?);
    let user_id = serenity::UserId::new(reminder.user_id.parse()?);

    let content = match &reminder.reminder {
        Some(text) => format!("<@{}> Reminder: {}", user_id, text),
        None => format!("<@{}> Reminder.", user_id),
    };
    let message = serenity::CreateMessage::new()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new().users(vec![user_id]));

    channel_id.send_message(&ctx.http, message).await?;
    Ok(())
}
