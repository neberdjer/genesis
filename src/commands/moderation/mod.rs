pub mod ban;
pub mod kick;

pub use ban::ban;
pub use kick::kick;

pub fn commands() -> Vec<poise::Command<(), crate::Error>> {
    vec![ban(), kick()]
}
