pub mod general;
pub mod moderation;

pub fn all_commands() -> Vec<poise::Command<(), crate::Error>> {
    let mut commands = Vec::new();

    commands.push(general::ping());
    commands.extend(moderation::commands());
    commands
}
