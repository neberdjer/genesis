use super::deny;
use crate::constants::{TIMEZONE_API, TIMEZONE_REGISTER_URL};
use crate::handlers::shared;
use crate::{Context, Error};
use chrono::{Offset, Utc};
use chrono_tz::Tz;
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct TimezoneResponse {
    timezone: String,
}

enum LookupResult {
    Found(TimezoneResponse),
    NotRegistered,
    Error,
}

fn fetch_timezone(user_id: u64) -> LookupResult {
    let url = format!("{}?id={}", TIMEZONE_API, user_id);
    match ureq::get(&url).timeout(Duration::from_secs(10)).call() {
        Ok(response) => match response.into_json::<TimezoneResponse>() {
            Ok(data) => LookupResult::Found(data),
            Err(_) => LookupResult::Error,
        },
        Err(ureq::Error::Status(404, _)) => LookupResult::NotRegistered,
        Err(_) => LookupResult::Error,
    }
}

async fn lookup(user_id: u64) -> LookupResult {
    tokio::task::spawn_blocking(move || fetch_timezone(user_id))
        .await
        .unwrap_or(LookupResult::Error)
}

fn format_diff(target_offset_secs: i32, caller_offset_secs: i32) -> String {
    let diff_secs = target_offset_secs - caller_offset_secs;
    if diff_secs == 0 {
        return "the same time as you".to_string();
    }
    let abs = diff_secs.abs();
    let hours = abs / 3600;
    let minutes = (abs % 3600) / 60;
    let direction = if diff_secs > 0 { "ahead of" } else { "behind" };
    let mut parts: Vec<String> = Vec::new();
    if hours > 0 {
        parts.push(format!(
            "{} hour{}",
            hours,
            if hours == 1 { "" } else { "s" }
        ));
    }
    if minutes > 0 {
        parts.push(format!(
            "{} minute{}",
            minutes,
            if minutes == 1 { "" } else { "s" }
        ));
    }
    format!("{} {} you", parts.join(" "), direction)
}

/// Look up a user's timezone and current local time
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn timezone(
    ctx: Context<'_>,
    #[description = "User to look up (default: yourself)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let data = ctx.data();
    let pool = Some(data.pool.as_ref());
    if !shared::pre_check_user(ctx.author().id, pool).await {
        return deny(ctx, "You are blacklisted from using this bot.").await;
    }

    if !shared::check_rate_limit(ctx.author().id, "timezone") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    let target = user.as_ref().unwrap_or_else(|| ctx.author());
    let target_id = target.id;
    let target_name = target.display_name().to_string();
    let caller_id = ctx.author().id;

    ctx.defer().await?;

    let target_data = match lookup(target_id.get()).await {
        LookupResult::Found(d) => d,
        LookupResult::NotRegistered => {
            return deny(
                ctx,
                &format!(
                    "**{}** has not registered a timezone. They can set one at <{}>.",
                    target_name, TIMEZONE_REGISTER_URL
                ),
            )
            .await;
        }
        LookupResult::Error => {
            return deny(ctx, "Failed to reach the timezone service.").await;
        }
    };

    let target_tz: Tz = match target_data.timezone.parse() {
        Ok(t) => t,
        Err(_) => {
            return deny(
                ctx,
                &format!(
                    "Invalid timezone returned for **{}**: `{}`",
                    target_name, target_data.timezone
                ),
            )
            .await;
        }
    };

    let now_utc = Utc::now();
    let target_dt = now_utc.with_timezone(&target_tz);
    let time_24 = target_dt
        .format("%a, %b %d %Y %H:%M:%S %Z (%:z)")
        .to_string();
    let time_12 = target_dt.format("%a, %b %d %Y %I:%M:%S %p").to_string();

    let mut content = format!(
        "**{}**\nTimezone: `{}`\n24h: {}\n12h: {}",
        target_name, target_data.timezone, time_24, time_12
    );

    if target_id != caller_id
        && let LookupResult::Found(caller_data) = lookup(caller_id.get()).await
        && let Ok(caller_tz) = caller_data.timezone.parse::<Tz>()
    {
        let caller_dt = now_utc.with_timezone(&caller_tz);
        let target_offset = target_dt.offset().fix().local_minus_utc();
        let caller_offset = caller_dt.offset().fix().local_minus_utc();
        content.push_str(&format!(
            "\n\n-# {} is {}.",
            target_name,
            format_diff(target_offset, caller_offset)
        ));
    }

    ctx.send(CreateReply::default().content(content)).await?;

    Ok(())
}
