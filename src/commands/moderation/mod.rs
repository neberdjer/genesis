pub mod ban;
pub mod kick;
pub mod status;

pub use ban::ban;
pub use kick::kick;

use crate::{Context, Data};
use poise::serenity_prelude as serenity;

pub fn commands() -> Vec<poise::Command<Data, crate::Error>> {
    vec![ban(), kick()]
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
