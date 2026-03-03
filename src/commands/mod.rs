pub mod general;
pub mod moderation;
pub mod settings;
pub mod utility;
pub mod welcome;

use crate::Data;

pub fn all_commands() -> Vec<poise::Command<Data, crate::Error>> {
    let mut commands = Vec::new();

    commands.push(general::ping());
    commands.push(general::stats());
    commands.extend(moderation::commands());
    commands.extend(utility::commands());
    commands.push(settings::settings());
    commands.push(welcome::welcome());

    commands
}

pub type Command = poise::Command<Data, crate::Error>;
