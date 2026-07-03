use super::deny;
use crate::constants::{
    MAX_REMINDER_CHARS, MAX_REMINDERS_PER_USER, REMINDER_MAX_SECONDS, REMINDER_MIN_SECONDS,
};
use crate::handlers::shared;
use crate::{Context, Error, db};
use chrono::Utc;
use poise::serenity_prelude as serenity;

pub struct RestText(String);

#[poise::async_trait]
impl poise::SlashArgument for RestText {
    async fn extract(
        _ctx: &serenity::Context,
        _interaction: &serenity::CommandInteraction,
        value: &serenity::ResolvedValue<'_>,
    ) -> Result<Self, poise::SlashArgError> {
        match value {
            serenity::ResolvedValue::String(s) => Ok(Self(s.trim().to_string())),
            _ => Err(poise::SlashArgError::new_command_structure_mismatch(
                "expected string",
            )),
        }
    }

    fn create(builder: serenity::CreateCommandOption<'_>) -> serenity::CreateCommandOption<'_> {
        builder.kind(serenity::CommandOptionType::String)
    }
}

#[poise::async_trait]
impl<'a> poise::PopArgument<'a> for RestText {
    async fn pop_from(
        args: &'a str,
        attachment_index: usize,
        _ctx: &serenity::Context,
        _msg: &serenity::Message,
    ) -> Result<(&'a str, usize, Self), (Box<dyn std::error::Error + Send + Sync>, Option<String>)>
    {
        let input = args.trim();
        if input.is_empty() {
            return Err((poise::TooFewArguments::default().into(), None));
        }
        Ok(("", attachment_index, Self(input.to_string())))
    }
}

fn parse_duration_secs(input: &str) -> Option<u64> {
    let input = input.trim().to_ascii_lowercase();
    if input.is_empty() {
        return None;
    }

    if let Ok(minutes) = input.parse::<u64>() {
        return minutes.checked_mul(60);
    }

    let mut total: u64 = 0;
    let mut number = String::new();
    for ch in input.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
        } else {
            if number.is_empty() {
                return None;
            }
            let value: u64 = number.parse().ok()?;
            number.clear();
            let unit: u64 = match ch {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86400,
                'w' => 604800,
                _ => return None,
            };
            total = total.checked_add(value.checked_mul(unit)?)?;
        }
    }

    if !number.is_empty() || total == 0 {
        return None;
    }
    Some(total)
}

/// Manage reminders
#[poise::command(
    slash_command,
    prefix_command,
    aliases("remind"),
    subcommands("add", "list", "remove"),
    subcommand_required,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn reminder(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Set a reminder
#[poise::command(slash_command, prefix_command)]
pub async fn add(
    ctx: Context<'_>,
    #[description = "When to remind you (e.g. 10m, 1h30m, 2d)"] duration: String,
    #[description = "What to remind you about"] reminder: Option<RestText>,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;
    if !shared::check_rate_limit(ctx.author().id, "reminder") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    let Some(seconds) = parse_duration_secs(&duration) else {
        return deny(
            ctx,
            "Invalid duration. Use a number of minutes or units like `10m`, `1h30m`, `2d`.",
        )
        .await;
    };

    if !(REMINDER_MIN_SECONDS..=REMINDER_MAX_SECONDS).contains(&seconds) {
        return deny(ctx, "Duration must be between 10 seconds and 1 year.").await;
    }

    let reminder_text = reminder
        .map(|r| r.0)
        .filter(|r| !r.is_empty())
        .map(|mut r| {
            r.truncate(shared::floor_char_boundary(&r, MAX_REMINDER_CHARS));
            r
        });

    let user_id = ctx.author().id.to_string();
    let count = db::count_reminders(pool, &user_id).await?;
    if count >= MAX_REMINDERS_PER_USER {
        return deny(
            ctx,
            &format!(
                "You already have {} reminders. Remove one first with `reminder remove <id>`.",
                MAX_REMINDERS_PER_USER
            ),
        )
        .await;
    }

    let remind_at = Utc::now().timestamp() + seconds as i64;
    let id = db::add_reminder(
        pool,
        &user_id,
        &ctx.channel_id().to_string(),
        reminder_text.as_deref(),
        remind_at,
    )
    .await?;

    ctx.say(format!("Reminder `#{}` set for <t:{}:R>.", id, remind_at))
        .await?;

    Ok(())
}

/// List your pending reminders
#[poise::command(slash_command, prefix_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let pool = &ctx.data().pool;
    let reminders = db::list_reminders(pool, &ctx.author().id.to_string()).await?;
    if reminders.is_empty() {
        return deny(ctx, "You have no reminders.").await;
    }

    let mut lines = vec!["**Your reminders:**".to_string()];
    for reminder in reminders {
        let mut text = reminder.reminder.unwrap_or_default().replace('\n', " ");
        if text.is_empty() {
            text = "(no text)".to_string();
        }
        text.truncate(shared::floor_char_boundary(&text, 60));
        lines.push(format!(
            "`#{}` <t:{}:R> - {}",
            reminder.id, reminder.remind_at, text
        ));
    }

    ctx.send(
        poise::CreateReply::default()
            .content(lines.join("\n"))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Remove one of your reminders
#[poise::command(slash_command, prefix_command)]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Reminder id (see reminder list)"] id: i32,
) -> Result<(), Error> {
    let pool = &ctx.data().pool;
    let deleted = db::delete_user_reminder(pool, id, &ctx.author().id.to_string()).await?;
    if deleted {
        ctx.say(format!("Reminder `#{}` removed.", id)).await?;
    } else {
        return deny(
            ctx,
            &format!("No reminder `#{}` found that belongs to you.", id),
        )
        .await;
    }

    Ok(())
}
