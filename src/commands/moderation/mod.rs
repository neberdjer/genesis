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
    let owner_id = ctx.data().owner_id.ok_or("OWNER_ID not configured")?;
    Ok(ctx.author().id == owner_id)
}

pub(super) fn get_highest_role(
    ctx: Context<'_>,
    guild_id: serenity::GuildId,
    member: &serenity::Member,
) -> i16 {
    let Some(guild) = ctx.cache().guild(guild_id) else {
        return 0;
    };
    member
        .roles
        .iter()
        .filter_map(|role_id| guild.roles.get(role_id))
        .map(|role| role.position)
        .max()
        .unwrap_or(0)
}
