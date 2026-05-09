pub mod general;
pub mod media;
pub mod moderation;
pub mod settings;
pub mod utility;
pub mod welcome;

use crate::Data;

pub fn all_commands() -> Vec<poise::Command<Data, crate::Error>> {
    let mut commands = vec![
        general::ping(),
        general::stats(),
        media::instagram(),
        media::twitter(),
        media::tiktok(),
        settings::settings(),
        welcome::welcome(),
    ];
    commands.extend(moderation::commands());
    commands.extend(utility::commands());
    commands
}

pub type Command = poise::Command<Data, crate::Error>;
