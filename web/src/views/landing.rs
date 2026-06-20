use super::layout::layout;
use crate::catalog::COMMAND_GROUPS;
use crate::config::Config;
use crate::discord::{AppStats, User};
use maud::{Markup, html};

pub(super) fn install_buttons(config: &Config) -> Markup {
    html! {
        a.btn.primary href=(config.invite_url()) {
            "add to server"
        }
        a.btn href=(config.user_install_url()) {
            "add to your account"
        }
    }
}

fn fmt_count(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn stat(value: String, label: &str) -> Markup {
    html! {
        div.stat {
            span.stat-num { (value) }
            span.stat-label { (label) }
        }
    }
}

pub fn landing(
    config: &Config,
    stats: Option<AppStats>,
    users: Option<u64>,
    commands_run: Option<u64>,
    user: Option<&User>,
    deleted: Option<&str>,
) -> Markup {
    let commands: usize = COMMAND_GROUPS.iter().map(|g| g.commands.len()).sum();
    let num = |n: Option<u64>| n.map(fmt_count).unwrap_or_else(|| "…".to_string());

    let body = html! {
        @if let Some(kind) = deleted {
            div.success-message role="status" {
                @if kind == "server" {
                    "the server's data has been deleted, and you've been signed out."
                } @else {
                    "your data has been deleted, and you've been signed out."
                }
            }
        }
        section.hero {
            img.hero-logo src="/static/logo.svg" alt="";
            h1 { "fixes broken social media embeds in discord" }
            p { "paste a link. genesis replaces the broken preview with one that actually works, showing the video, every image, and the full text." }
            div.cta {
                (install_buttons(config))
                @if user.is_some() {
                    a.btn href="/dashboard" { "open dashboard" }
                }
            }
        }
        section.section {
            div.stat-grid {
                (stat(num(stats.map(|s| s.guild_count)), "servers"))
                (stat(num(users), "users"))
                (stat(num(stats.map(|s| s.user_install_count)), "user installs"))
                (stat(fmt_count(commands as u64), "commands"))
                (stat(num(commands_run), "commands run"))
            }
        }
    };
    layout("home", "", user, body)
}
