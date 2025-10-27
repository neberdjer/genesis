pub mod checkperms;
pub mod time;

pub fn commands() -> Vec<crate::commands::Command> {
    vec![checkperms::checkperms(), time::time()]
}
