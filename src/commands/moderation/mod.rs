pub mod ban;
pub mod blacklist;
pub mod kick;
pub mod leave_server;

pub use ban::ban;
pub use blacklist::blacklist;
pub use kick::kick;
pub use leave_server::leave_server;

use crate::Data;

pub fn commands() -> Vec<poise::Command<Data, crate::Error>> {
    vec![ban(), kick(), blacklist(), leave_server()]
}
