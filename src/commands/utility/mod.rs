pub mod checkperms;

pub fn commands() -> Vec<crate::commands::Command> {
    vec![checkperms::checkperms()]
}
