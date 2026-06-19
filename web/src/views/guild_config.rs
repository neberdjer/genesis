use super::layout::{layout, pager};
use crate::catalog::{SUPPORTED_DOMAINS, toggleable_commands};
use crate::db::{AuditEntry, SERVICES, Settings};
use crate::discord::{DashGuild, GuildChannel, GuildRole, User};
use maud::{Markup, html};
use std::collections::HashSet;

const TABS: &[(&str, &str)] = &[
    ("services", "services"),
    ("commands", "commands"),
    ("welcome", "welcome"),
    ("domains", "blocked domains"),
    ("audit", "audit log"),
];

fn tabs(guild_id: &str, active: &str) -> Markup {
    html! {
        nav.tabs aria-label="settings sections" {
            @for (key, label) in TABS {
                a class=(if *key == active { "tab active" } else { "tab" })
                  href=(format!("/dashboard/{}?tab={}", guild_id, key))
                  aria-current=[(*key == active).then_some("page")] {
                    (label)
                }
            }
        }
    }
}

fn services_panel(guild: &DashGuild, settings: &Settings) -> Markup {
    html! {
        p.muted { "choose which links genesis re-embeds in this server." }
        form.config-form.autosave method="post" action=(format!("/dashboard/{}/settings", guild.id)) {
            fieldset.check-grid {
                legend.visually-hidden { "services to enable" }
                @for (key, label) in SERVICES {
                    label.check {
                        input type="checkbox" name=(*key) checked[settings.enabled(key)];
                        span { (label) }
                    }
                }
            }

            label.toggle {
                input type="checkbox" name="reply_cleanup" checked[settings.reply_cleanup];
                span { "clean up replies on mod-delete" }
            }
            p.field-hint { "if a moderator deletes the original message within a minute, genesis deletes its reply too. requires the bot's view audit log permission." }

            span.save-status aria-live="polite" {}
        }
    }
}

fn welcome_panel(
    guild: &DashGuild,
    settings: &Settings,
    channels: &[GuildChannel],
    roles: &[GuildRole],
) -> Markup {
    let selected_channel = settings.welcome_channel_id.as_deref();
    let selected_role = settings.welcome_role_id.as_deref();
    html! {
        p.muted { "greet new members with a message, and optionally give them a role." }
        form.config-form method="post" action=(format!("/dashboard/{}/welcome", guild.id)) {
            label.toggle {
                input type="checkbox" name="enabled" checked[settings.welcome_enabled];
                span { "send a welcome message when someone joins" }
            }

            div.field {
                label for="welcome-channel" { "channel" }
                select #welcome-channel name="channel_id" {
                    option value="" selected[selected_channel.is_none()] { "none" }
                    @for c in channels {
                        option value=(c.id) selected[selected_channel == Some(c.id.as_str())] {
                            "#" (c.name)
                        }
                    }
                }
                @if channels.is_empty() {
                    p.field-hint { "no channels found, or the bot can't view them." }
                }
            }

            div.field {
                label for="welcome-role" { "auto-assign role" }
                select #welcome-role name="role_id" {
                    option value="" selected[selected_role.is_none()] { "none" }
                    @for r in roles {
                        option value=(r.id) selected[selected_role == Some(r.id.as_str())] {
                            (r.name)
                        }
                    }
                }
            }

            div.field {
                label for="welcome-message" { "message" }
                textarea #welcome-message name="message" rows="3" { (settings.welcome_message) }
                p.field-hint { "placeholders: {user} or {mention} (pings them), {username}, {server_name}, {member_count}" }
            }

            div { button.btn.primary type="submit" { "save welcome" } }
        }
    }
}

fn commands_panel(guild: &DashGuild, disabled: &[String]) -> Markup {
    let disabled: HashSet<&str> = disabled.iter().map(String::as_str).collect();
    html! {
        p.muted { "turn commands off in this server. enabled commands are highlighted." }
        form.config-form.autosave method="post" action=(format!("/dashboard/{}/commands", guild.id)) {
            fieldset.check-grid {
                legend.visually-hidden { "enabled commands" }
                @for name in toggleable_commands() {
                    label.check {
                        input type="checkbox" name=(name) checked[!disabled.contains(name)];
                        span { (name) }
                    }
                }
            }
            span.save-status aria-live="polite" {}
        }
    }
}

fn domain_checks(domains: &[&str], blocked: &HashSet<&str>) -> Markup {
    html! {
        fieldset.check-grid {
            legend.visually-hidden { "domains to block" }
            @for d in domains {
                label.check {
                    input type="checkbox" name=(*d) checked[blocked.contains(d)];
                    span { (d) }
                }
            }
        }
    }
}

fn domains_panel(guild: &DashGuild, blocked: &[String], git_hosts: &[String]) -> Markup {
    let blocked_set: HashSet<&str> = blocked.iter().map(String::as_str).collect();
    let git_set: HashSet<&str> = git_hosts.iter().map(String::as_str).collect();

    let mut seen: HashSet<&str> = HashSet::new();
    let mut platforms: Vec<&str> = Vec::new();
    for d in SUPPORTED_DOMAINS
        .iter()
        .copied()
        .chain(blocked.iter().map(String::as_str))
    {
        if !git_set.contains(d) && seen.insert(d) {
            platforms.push(d);
        }
    }

    html! {
        p.muted { "tick a domain to stop genesis embedding its links in this server." }
        form.config-form.autosave method="post" action=(format!("/dashboard/{}/domains", guild.id)) {
            h3 { "platforms" }
            (domain_checks(&platforms, &blocked_set))

            @if !git_hosts.is_empty() {
                h3 { "git hosts" }
                p.field-hint { "self-hosted git domains added by the bot owner." }
                (domain_checks(&git_hosts.iter().map(String::as_str).collect::<Vec<_>>(), &blocked_set))
            }

            span.save-status aria-live="polite" {}
        }
    }
}

fn audit_panel(entries: &[AuditEntry], guild_id: &str, page: usize, total_pages: usize) -> Markup {
    html! {
        p.muted { "configuration changes made from the dashboard, newest first." }
        @if entries.is_empty() {
            p.muted { "no changes recorded yet." }
        } @else {
            ul.audit-log {
                @for e in entries {
                    li.audit-row {
                        div.audit-head {
                            @if !e.category.is_empty() {
                                span.audit-cat { (e.category) }
                            }
                            span.audit-time {
                                span.audit-actor { (e.actor_name) }
                                " · " (e.at)
                            }
                        }
                        div.audit-action { (e.action) }
                    }
                }
            }
            (pager(&format!("/dashboard/{}?tab=audit", guild_id), page, total_pages))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn guild_config(
    user: &User,
    guild: &DashGuild,
    tab: &str,
    settings: &Settings,
    domains: &[String],
    git_hosts: &[String],
    disabled_commands: &[String],
    channels: &[GuildChannel],
    roles: &[GuildRole],
    audit: &[AuditEntry],
    audit_page: usize,
    audit_total_pages: usize,
    saved: bool,
) -> Markup {
    let body = html! {
        div.page-head-row {
            div.page-head {
                img.icon src=(guild.icon_url()) alt="";
                h1 { (guild.name) }
            }
            a.btn.sm href="/dashboard" { "back to servers" }
        }

        (tabs(&guild.id, tab))

        @if saved {
            div.success-message role="status" { "settings saved." }
        }

        section.section style="border:none;padding:0" {
            @match tab {
                "commands" => (commands_panel(guild, disabled_commands)),
                "welcome" => (welcome_panel(guild, settings, channels, roles)),
                "domains" => (domains_panel(guild, domains, git_hosts)),
                "audit" => (audit_panel(audit, &guild.id, audit_page, audit_total_pages)),
                _ => (services_panel(guild, settings)),
            }
        }
        script src="/static/js/autosave.js" defer {}
    };
    layout(&guild.name, "dashboard", Some(user), body)
}
