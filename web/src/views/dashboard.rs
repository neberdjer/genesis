use super::layout::layout;
use crate::config::Config;
use crate::discord::{DashGuild, GuildState, User};
use maud::{Markup, html};

pub fn dashboard(user: &User, guilds: &[DashGuild], config: &Config, is_owner: bool) -> Markup {
    let body = html! {
        div.page-head-row {
            h1 { "your servers" }
            @if is_owner {
                a.btn.sm href="/owner" { "global settings" }
            }
        }
        p.muted { "configure genesis where you manage it, and add it where you can." }
        div.servers {
            @for g in guilds {
                @match g.state() {
                    GuildState::Configure => {
                        a.server href=(format!("/dashboard/{}", g.id)) {
                            img.icon src=(g.icon_url()) alt="";
                            span.name { (g.name) }
                            span.tag.on { "configure" }
                        }
                    }
                    GuildState::AddBot => {
                        a.server href=(config.invite_url_for_guild(&g.id)) {
                            img.icon src=(g.icon_url()) alt="";
                            span.name { (g.name) }
                            span.tag.add { "add bot" }
                        }
                    }
                    GuildState::Active => {
                        div.server.is-muted title="genesis is in this server, but you can't manage it" {
                            img.icon src=(g.icon_url()) alt="";
                            span.name { (g.name) }
                            span.tag { "in server" }
                        }
                    }
                    GuildState::CantAdd => {
                        div.server.is-muted title="you need the manage server permission to add genesis here" {
                            img.icon src=(g.icon_url()) alt="";
                            span.name { (g.name) }
                            span.tag { "can't add" }
                        }
                    }
                }
            }
        }
        @if guilds.is_empty() {
            p.muted { "no servers found." }
        }

        section.danger-zone {
            h2 { "your data" }
            p.muted {
                "delete your pending reminders and your dashboard change history across every "
                "server. this signs you out and cannot be undone. it does not change your "
                "servers' settings."
            }
            form method="post" action="/account/delete"
                onsubmit="return confirm('delete all data about you and sign out? this cannot be undone.')" {
                button.btn.danger type="submit" { "delete my data" }
            }
        }
    };
    layout("dashboard", "dashboard", Some(user), body)
}
