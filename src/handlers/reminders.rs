use super::pagination;
use crate::constants::{
    MAX_REMINDERS_PER_USER, REMINDER_POLL_SECONDS, REMINDER_PREFIX, SNOOZE_CHOICES,
};
use crate::{Data, db};
use chrono::Utc;
use poise::serenity_prelude as serenity;
use std::time::Duration;
use tracing::warn;

pub async fn reminder_poller(ctx: serenity::Context) {
    let data = ctx.data::<Data>();
    let mut interval = tokio::time::interval(Duration::from_secs(REMINDER_POLL_SECONDS));
    loop {
        interval.tick().await;
        let due = match db::due_reminders(&data.pool, Utc::now().timestamp()).await {
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

fn snooze_buttons(user_id: serenity::UserId) -> serenity::CreateActionRow<'static> {
    let buttons = SNOOZE_CHOICES
        .iter()
        .map(|(secs, label)| {
            serenity::CreateButton::new(format!("rem:snooze:{}:{}", secs, user_id))
                .label(*label)
                .style(serenity::ButtonStyle::Secondary)
        })
        .collect::<Vec<_>>();
    serenity::CreateActionRow::Buttons(buttons.into())
}

async fn send_reminder(
    ctx: &serenity::Context,
    reminder: &db::Reminder,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel_id = serenity::GenericChannelId::new(reminder.channel_id.parse()?);
    let user_id = serenity::UserId::new(reminder.user_id.parse()?);

    let content = match &reminder.reminder {
        Some(text) => format!("<@{}> {}{}", user_id, REMINDER_PREFIX, text),
        None => format!("<@{}> Reminder.", user_id),
    };
    let message = serenity::CreateMessage::new()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new().users(vec![user_id]))
        .components(vec![serenity::CreateComponent::ActionRow(snooze_buttons(
            user_id,
        ))]);

    if let Err(channel_err) = channel_id.send_message(&ctx.http, message.clone()).await {
        warn!(
            "Failed to send reminder {} in channel {}, falling back to dm: {}",
            reminder.id, channel_id, channel_err
        );
        user_id.dm(&ctx.http, message).await?;
    }
    Ok(())
}

pub async fn handle_reminder_buttons(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
) {
    let custom_id = &interaction.data.custom_id;

    let Some(rest) = custom_id.strip_prefix("rem:snooze:") else {
        return;
    };
    let Some((secs, owner)) = rest.split_once(':') else {
        return;
    };
    let Ok(secs) = secs.parse::<u64>() else {
        return;
    };

    if interaction.user.id.to_string() != owner {
        pagination::respond_ephemeral(ctx, interaction, "Only the reminder owner can snooze this.")
            .await;
        return;
    }

    let data = ctx.data::<Data>();
    let count = db::count_reminders(&data.pool, owner).await.unwrap_or(0);
    if count >= MAX_REMINDERS_PER_USER {
        pagination::respond_ephemeral(
            ctx,
            interaction,
            &format!(
                "You already have {} reminders. Remove one first with `reminder remove <id>`.",
                MAX_REMINDERS_PER_USER
            ),
        )
        .await;
        return;
    }

    let content = &interaction.message.content;
    let text = content
        .split_once(REMINDER_PREFIX)
        .map(|(_, text)| text.to_string());

    let remind_at = Utc::now().timestamp() + secs as i64;
    if let Err(e) = db::add_reminder(
        &data.pool,
        owner,
        &interaction.channel_id.to_string(),
        text.as_deref(),
        remind_at,
    )
    .await
    {
        warn!("Failed to snooze reminder: {}", e);
        pagination::respond_ephemeral(ctx, interaction, "Failed to snooze that reminder.").await;
        return;
    }

    let updated = format!("{}\n-# Snoozed, next reminder <t:{}:R>", content, remind_at);
    let response = serenity::CreateInteractionResponse::UpdateMessage(
        serenity::CreateInteractionResponseMessage::new()
            .content(updated)
            .components(vec![]),
    );
    if let Err(e) = interaction.create_response(&ctx.http, response).await {
        warn!("Failed to update snoozed reminder message: {}", e);
    }
}
