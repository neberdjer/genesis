use super::layout::{
    command_toggles, failure_list, layout_wide, pager, panel_head, panel_intro, toggle_row,
    toggle_state,
};
use crate::catalog::{self, DOMAIN_GROUPS};
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

fn services_panel(guild: &DashGuild, settings: &Settings, channels: &[GuildChannel]) -> Markup {
    let selected = settings.report_channel_id.as_deref();
    html! {
        section.group {
            (panel_intro("choose which links genesis re-embeds in this server."))
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

        section.group {
            (panel_head(
                "report channel",
                "where genesis posts embed failures as they happen. they're listed on the \
                 failure reports tab either way.",
            ))
            form.config-form.inline-form method="post"
                 action=(format!("/dashboard/{}/report-channel", guild.id)) {
                div.field {
                    label.visually-hidden for="report-channel" { "report channel" }
                    select #report-channel name="channel_id" {
                        option value="" selected[selected.is_none()] { "none (reports off)" }
                        @for c in channels {
                            option value=(c.id) selected[selected == Some(c.id.as_str())] {
                                "#" (c.name)
                            }
                        }
                    }
                }
                button.btn.primary type="submit" { "save" }
            }
            @if channels.is_empty() {
                p.field-hint { "no channels found, or the bot can't view them." }
            }
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
      section.group {
        (panel_intro("greet new members and optionally give them a role."))
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
}

fn commands_panel(guild: &DashGuild, disabled: &[String]) -> Markup {
    let disabled: HashSet<&str> = disabled.iter().map(String::as_str).collect();
    html! {
        section.group {
            (panel_intro("turn commands off in this server."))
            (command_toggles(&format!("/dashboard/{}/commands", guild.id), &disabled))
        }
    }
}

fn domain_group(label: &str, domains: &[&str], blocked: &HashSet<&str>) -> Markup {
    let items: Vec<(&str, bool)> = domains.iter().map(|d| (*d, blocked.contains(d))).collect();
    toggle_row(
        label,
        &format!("{label} domains to block"),
        "check block",
        "blocked",
        &items,
    )
}

fn domains_panel(
    guild: &DashGuild,
    blocked: &[String],
    git_hosts: &[String],
    media_hosts: &[(String, String)],
) -> Markup {
    let blocked_set: HashSet<&str> = blocked.iter().map(String::as_str).collect();
    let extra = catalog::extra_mirrors(git_hosts, media_hosts);

    let mirror_total = DOMAIN_GROUPS.iter().map(|g| g.domains.len()).sum::<usize>() + extra.len();
    let mirror_blocked = DOMAIN_GROUPS
        .iter()
        .flat_map(|g| g.domains.iter().copied())
        .chain(extra.iter().copied())
        .filter(|d| blocked_set.contains(d))
        .count();

    html! {
        section.group {
            (panel_intro(
                "block a domain and genesis stops re-embedding its links in this server. \
                 subdomains are blocked too.",
            ))

            form.add-row method="post" action=(format!("/dashboard/{}/domains/add", guild.id)) {
                input type="text" name="domain" placeholder="example.com"
                      aria-label="domain to block" required;
                button.btn.sm type="submit" { "block" }
            }

            @if blocked.is_empty() {
                p.muted { "nothing is blocked in this server." }
            } @else {
                ul.idlist {
                    @for d in blocked {
                        li {
                            span.idlist-id { (d) }
                            form.idlist-form method="post"
                                 action=(format!("/dashboard/{}/domains/remove", guild.id)) {
                                input type="hidden" name="domain" value=(d);
                                button.idlist-remove type="submit" aria-label=(format!("unblock {d}"))
                                        title="unblock" { "×" }
                            }
                        }
                    }
                }
            }
        }

        section.group {
            details.disclosure open[mirror_blocked > 0] {
                summary.disclosure-head {
                    span.disclosure-title { "platform mirrors" }
                    span.group-count data-toggle-summary data-toggle-word="blocked" {
                        (toggle_state(mirror_blocked, mirror_total, "blocked"))
                    }
                }
                div.disclosure-body {
                    p.field-hint { "tick a platform, or individual mirrors, instead of typing them out." }
                    form.config-form.row-form data-group-toggles method="post"
                         action=(format!("/dashboard/{}/domains", guild.id)) {
                        @for g in DOMAIN_GROUPS {
                            (domain_group(g.label, g.domains, &blocked_set))
                        }
                        @if !extra.is_empty() {
                            (domain_group("custom", &extra, &blocked_set))
                        }
                        div.row-form-actions { button.btn.primary type="submit" { "save mirrors" } }
                    }
                }
            }
        }
    }
}

fn audit_panel(entries: &[AuditEntry], guild_id: &str, page: usize, total_pages: usize) -> Markup {
    html! {
        section.group {
            (panel_intro("configuration changes made from the dashboard, newest first."))
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
                                span.audit-actor { (e.actor_name) }
                                span.audit-time { (e.at) }
                            }
                            p.audit-action title=(e.action) { (e.action) }
                        }
                    }
                }
                (pager(&format!("/dashboard/{}?tab=audit", guild_id), page, total_pages))
            }
        }
    }
}

fn failures_panel(
    guild: &DashGuild,
    settings: &Settings,
    entries: &[FailureEntry],
    code_counts: &[(String, i64)],
    active_code: Option<&str>,
    page: usize,
    total_pages: usize,
) -> Markup {
    let base = format!("/dashboard/{}?tab=failures", guild.id);
    html! {
        section.group {
            p.field-hint {
                "links genesis failed to embed in this server over the last 30 days, newest first. "
                @if settings.report_channel_id.is_some() {
                    "they're also posted to your report channel as they happen; change it on the "
                } @else {
                    "genesis can post them to a channel as they happen; pick one on the "
                }
                a href=(format!("/dashboard/{}?tab=services", guild.id)) { "services tab" } "."
            }
            (failure_list(&base, active_code, code_counts, entries, false, page, total_pages))
        }
    }
}

fn danger_panel(guild: &DashGuild) -> Markup {
    html! {
        section.group {
            (panel_intro(
                "permanently delete everything genesis stores for this server: services, \
                 blocked domains, command settings, welcome message, and change history. \
                 you'll be signed out once it's done.",
            ))
            form method="post" action=(format!("/dashboard/{}/delete", guild.id))
                onsubmit="return confirm('delete all stored data for this server? this cannot be undone.')" {
                button.btn.danger type="submit" { "delete all data" }
            }
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
    failure_codes: &[(String, i64)],
    failure_code: Option<&str>,
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
            div.success-message role="status" { "saved." }
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
                    failures,
                    failure_codes,
                    failure_code,
                    failure_page,
                    failure_total_pages,
                )),
                "danger" => (danger_panel(guild)),
                _ => (services_panel(guild, settings, channels)),
            }
        }
        script src="/static/js/autosave.js" defer {}
        script src="/static/js/group-toggles.js" defer {}
    };
    layout_wide(&guild.name, "dashboard", Some(user), body)
}
