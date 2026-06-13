use crate::{Context, Error, db};
use poise::ChoiceParameter;
use poise::serenity_prelude as serenity;
use tracing::info;

use super::check_owner;

#[derive(Debug, poise::ChoiceParameter)]
pub enum StatusKind {
    #[name = "playing"]
    Playing,
    #[name = "watching"]
    Watching,
    #[name = "listening"]
    Listening,
    #[name = "competing"]
    Competing,
    #[name = "custom"]
    Custom,
}

impl StatusKind {
    fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "playing" => Self::Playing,
            "listening" => Self::Listening,
            "competing" => Self::Competing,
            "custom" => Self::Custom,
            _ => Self::Watching,
        }
    }

    fn activity(&self, text: String) -> serenity::ActivityData {
        match self {
            Self::Playing => serenity::ActivityData::playing(text),
            Self::Watching => serenity::ActivityData::watching(text),
            Self::Listening => serenity::ActivityData::listening(text),
            Self::Competing => serenity::ActivityData::competing(text),
            Self::Custom => serenity::ActivityData::custom(text),
        }
    }
}

#[derive(Debug, poise::ChoiceParameter)]
pub enum OnlineKind {
    #[name = "online"]
    Online,
    #[name = "idle"]
    Idle,
    #[name = "dnd"]
    Dnd,
    #[name = "invisible"]
    Invisible,
}

impl OnlineKind {
    fn from_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "idle" => Self::Idle,
            "dnd" => Self::Dnd,
            "invisible" => Self::Invisible,
            _ => Self::Online,
        }
    }

    fn online_status(&self) -> serenity::OnlineStatus {
        match self {
            Self::Online => serenity::OnlineStatus::Online,
            Self::Idle => serenity::OnlineStatus::Idle,
            Self::Dnd => serenity::OnlineStatus::DoNotDisturb,
            Self::Invisible => serenity::OnlineStatus::Invisible,
        }
    }
}

pub fn presence_from_parts(
    status_type: &str,
    status_text: &str,
    online: &str,
) -> (serenity::ActivityData, serenity::OnlineStatus) {
    (
        StatusKind::from_name(status_type).activity(status_text.to_string()),
        OnlineKind::from_name(online).online_status(),
    )
}

/// Owner-only: set the bot's activity and online status
#[poise::command(slash_command, prefix_command, check = "check_owner")]
pub async fn status(
    ctx: Context<'_>,
    #[description = "Activity type"] kind: StatusKind,
    #[description = "Status text"] text: String,
    #[description = "Online status (defaults to online)"] online: Option<OnlineKind>,
) -> Result<(), Error> {
    let online = online.unwrap_or(OnlineKind::Online);

    db::set_bot_status(&ctx.data().pool, kind.name(), &text, online.name()).await?;

    ctx.serenity_context()
        .set_presence(Some(kind.activity(text.clone())), online.online_status());

    info!(
        "Bot status set by {}: {} {} ({})",
        ctx.author().name,
        kind.name(),
        text,
        online.name()
    );

    ctx.say(format!(
        "Status updated: **{}** {} ({}).",
        kind.name(),
        text,
        online.name()
    ))
    .await?;

    Ok(())
}
