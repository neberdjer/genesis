use super::layout::{failure_rows, layout, pager};
use crate::catalog::{DOMAIN_GROUPS, toggleable_commands};
use crate::db::{AuditEntry, FailureEntry, SERVICES, Settings};
use crate::discord::{DashGuild, GuildChannel, GuildRole, User};
use maud::{Markup, html};
use std::collections::HashSet;

const TABS: &[(&str, &str)] = &[
    ("services", "services"),
    ("commands", "commands"),
    ("welcome", "welcome"),
    ("domains", "blocked domains"),
    ("audit", "audit log"),
    ("failures", "failure reports"),
    ("danger", "danger zone"),
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

fn domains_panel(
    guild: &DashGuild,
    blocked: &[String],
    git_hosts: &[String],
    media_hosts: &[(String, String)],
) -> Markup {
    let blocked_set: HashSet<&str> = blocked.iter().map(String::as_str).collect();

    let mut known: HashSet<&str> = DOMAIN_GROUPS
        .iter()
        .flat_map(|g| g.domains.iter().copied())
        .collect();
    let custom: Vec<&str> = git_hosts
        .iter()
        .map(String::as_str)
        .chain(media_hosts.iter().map(|(_, d)| d.as_str()))
        .chain(blocked.iter().map(String::as_str))
        .filter(|d| known.insert(d))
        .collect();

    html! {
        p.muted { "tick a domain to stop genesis embedding its links in this server. each platform and its mirror domains can be blocked separately." }
        form.config-form.autosave method="post" action=(format!("/dashboard/{}/domains", guild.id)) {
            @for g in DOMAIN_GROUPS {
                h3 { (g.label) }
                (domain_checks(g.domains, &blocked_set))
            }
            @if !custom.is_empty() {
                h3 { "custom domains" }
                p.field-hint { "extra git and mirror domains added by the bot owner." }
                (domain_checks(&custom, &blocked_set))
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

fn failures_panel(
    guild: &DashGuild,
    settings: &Settings,
    channels: &[GuildChannel],
    entries: &[FailureEntry],
    page: usize,
    total_pages: usize,
) -> Markup {
    let selected = settings.report_channel_id.as_deref();
    html! {
        p.muted {
            "links genesis failed to embed in this server over the last 30 days, and an optional "
            "channel where new failures are posted as they happen."
        }
        form.config-form method="post" action=(format!("/dashboard/{}/report-channel", guild.id)) {
            div.field {
                label for="report-channel" { "report channel" }
                select #report-channel name="channel_id" {
                    option value="" selected[selected.is_none()] { "none (reports off)" }
                    @for c in channels {
                        option value=(c.id) selected[selected == Some(c.id.as_str())] {
                            "#" (c.name)
                        }
                    }
                }
                @if channels.is_empty() {
                    p.field-hint { "no channels found, or the bot can't view them." }
                }
            }
            div { button.btn.primary type="submit" { "save report channel" } }
        }

        @if entries.is_empty() {
            p.muted { "no failures recorded." }
        } @else {
            (failure_rows(entries, false))
            (pager(&format!("/dashboard/{}?tab=failures", guild.id), page, total_pages))
        }
    }
}

fn danger_panel(guild: &DashGuild) -> Markup {
    html! {
        p.muted {
            "permanently delete everything genesis stores for this server: its services, blocked "
            "domains, command settings, welcome message, and change history. this cannot be undone."
        }
        p.muted { "you'll be signed out once it's done." }
        form method="post" action=(format!("/dashboard/{}/delete", guild.id))
            onsubmit="return confirm('delete all stored data for this server? this cannot be undone.')" {
            button.btn.danger type="submit" { "delete all data for this server" }
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
    media_hosts: &[(String, String)],
    disabled_commands: &[String],
    channels: &[GuildChannel],
    roles: &[GuildRole],
    audit: &[AuditEntry],
    audit_page: usize,
    audit_total_pages: usize,
    failures: &[FailureEntry],
    failure_page: usize,
    failure_total_pages: usize,
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
                "domains" => (domains_panel(guild, domains, git_hosts, media_hosts)),
                "audit" => (audit_panel(audit, &guild.id, audit_page, audit_total_pages)),
                "failures" => (failures_panel(
                    guild,
                    settings,
                    channels,
                    failures,
                    failure_page,
                    failure_total_pages,
                )),
                "danger" => (danger_panel(guild)),
                _ => (services_panel(guild, settings)),
            }
        }
        script src="/static/js/autosave.js" defer {}
    };
    layout(&guild.name, "dashboard", Some(user), body)
}
