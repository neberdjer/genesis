pub mod general;
pub mod moderation;
pub mod utility;

pub fn all_commands() -> Vec<poise::Command<(), crate::Error>> {
    let mut commands = Vec::new();

    commands.push(general::ping());
    commands.extend(moderation::commands());
    commands.extend(utility::commands());
    commands
}

pub type Command = poise::Command<(), crate::Error>;
