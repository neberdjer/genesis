use poise::serenity_prelude as serenity;

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
