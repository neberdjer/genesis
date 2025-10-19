pub mod ban;
pub mod kick;

pub use ban::ban;
pub use kick::kick;

use crate::Data;

pub fn commands() -> Vec<poise::Command<Data, crate::Error>> {
    vec![ban(), kick()]
}
