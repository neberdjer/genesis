pub mod ban;
pub mod blacklist;
pub mod kick;
pub mod leave_server;

pub use ban::ban;
pub use blacklist::blacklist;
pub use kick::kick;
pub use leave_server::leave_server;

use crate::{Context, Data, Error};
use poise::serenity_prelude as serenity;

pub fn commands() -> Vec<poise::Command<Data, crate::Error>> {
    vec![ban(), kick(), blacklist(), leave_server()]
}

pub(super) async fn check_owner(ctx: Context<'_>) -> Result<bool, Error> {
    let owner_id = std::env::var("OWNER_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok())
        .map(serenity::UserId::new)
        .ok_or("OWNER_ID not configured")?;
    Ok(ctx.author().id == owner_id)
}

pub(super) async fn get_highest_role(
    ctx: Context<'_>,
    guild_id: serenity::GuildId,
    member: &serenity::Member,
) -> u16 {
    let mut highest = 0;
    for role_id in &member.roles {
        if let Ok(role) = guild_id.role(ctx, *role_id).await {
            highest = highest.max(role.position);
        }
    }
    highest
}
