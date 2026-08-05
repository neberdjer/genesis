pub mod general;
pub mod git;
pub mod media;
pub mod moderation;
pub mod optout;
pub mod remind;
pub mod settings;
pub mod timezone;
pub mod utility;
pub mod welcome;

use crate::{Data, Error};
use poise::CreateReply;

pub async fn deny(ctx: poise::Context<'_, Data, Error>, message: &str) -> Result<(), Error> {
    ctx.send(CreateReply::default().content(message).ephemeral(true))
        .await?;
    Ok(())
}

pub fn all_commands() -> Vec<poise::Command<Data, crate::Error>> {
    let mut commands = vec![
        general::ping(),
        general::stats(),
        media::instagram(),
        media::twitter(),
        media::bsky(),
        media::tiktok(),
        git::git(),
        remind::reminder(),
        timezone::timezone(),
        settings::settings(),
        welcome::welcome(),
        optout::optout(),
        optout::optin(),
    ];
    commands.extend(moderation::commands());
    commands.extend(utility::commands());
    commands
}

pub type Command = poise::Command<Data, crate::Error>;
