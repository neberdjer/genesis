use super::layout::layout;
use crate::catalog::COMMAND_GROUPS;
use crate::discord::User;
use maud::{Markup, html};

pub fn commands(user: Option<&User>) -> Markup {
    let body = html! {
        div.page-intro {
            h1 { "commands" }
            p.muted { "every command genesis supports." }
        }
        div.search {
            input #cmd-search type="search" placeholder="search commands" aria-label="search commands" autocomplete="off";
        }
        @for group in COMMAND_GROUPS {
            section.group.cmd-group {
                h2.group-title {
                    span.group-label { (group.title) }
                    span.group-count aria-hidden="true" { (group.commands.len()) }
                    @if !group.note.is_empty() {
                        span.group-note aria-hidden="true" { (group.note) }
                    }
                }
                div.cmd-list {
                    @for cmd in group.commands {
                        div.cmd {
                            code.cmd-usage { (cmd.usage) }
                            span.cmd-desc { (cmd.desc) }
                        }
                    }
                }
            }
        }
        p #cmd-empty .cmd-empty .muted aria-live="polite" style="display:none" { "no commands match your search." }
        script src="/static/js/commands.js" defer {}
    };
    layout("commands", "commands", user, body)
}
