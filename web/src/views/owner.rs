use super::layout::{command_toggles, failure_list, layout_wide, pager, panel_head, panel_intro};
use crate::catalog;
use crate::config;
use crate::db::{BlacklistEntry, BotStatus, FailureEntry};
use crate::discord::{self, User};
use maud::{Markup, html};
use std::collections::HashSet;

const TABS: &[(&str, &str)] = &[
    ("blacklist", "blacklisted users"),
    ("servers", "servers"),
    ("domains", "global domains"),
    ("commands", "commands"),
    ("status", "bot status"),
    ("failures", "failures"),
];

fn tabs(active: &str) -> Markup {
    html! {
        nav.tabs aria-label="owner sections" {
            @for (key, label) in TABS {
                a class=(if *key == active { "tab active" } else { "tab" })
                  href=(format!("/owner?tab={}", key))
                  aria-current=[(*key == active).then_some("page")] {
                    (label)
                }
            }
        }
    }
}

fn id_list(entries: &[BlacklistEntry], remove_action: &str) -> Markup {
    html! {
        ul.idlist {
            @for e in entries {
                li {
                    span.idlist-id { (e.id) }
                    @if let Some(r) = &e.reason {
                        span.idlist-reason { (r) }
                    }
                    form.idlist-form method="post" action=(remove_action) {
                        input type="hidden" name="id" value=(e.id);
                        button.idlist-remove type="submit" aria-label=(format!("remove {}", e.id)) title="remove" { "×" }
                    }
                }
            }
            @if entries.is_empty() {
                li.muted { "none" }
            }
        }
    }
}

fn domain_list(domains: &[String], remove_action: &str) -> Markup {
    html! {
        ul.idlist {
            @for d in domains {
                li {
                    span.idlist-id { (d) }
                    form.idlist-form method="post" action=(remove_action) {
                        input type="hidden" name="domain" value=(d);
                        button.idlist-remove type="submit" aria-label=(format!("remove {d}")) title="remove" { "×" }
                    }
                }
            }
            @if domains.is_empty() {
                li.muted { "none" }
            }
        }
    }
}

fn blacklist_panel(users: &[BlacklistEntry]) -> Markup {
    html! {
        section.group {
            (panel_intro("genesis ignores everything from these users."))
            (id_list(users, "/owner/blacklist/user/remove"))
            form.add-row method="post" action="/owner/blacklist/user/add" {
                input type="text" name="id" placeholder="user id" aria-label="user id" required;
                input type="text" name="reason" placeholder="reason (optional)" aria-label="reason";
                button.btn.sm type="submit" { "add" }
            }
        }
    }
}

fn servers_panel(
    guilds: &[discord::BotGuild],
    blacklist: &[BlacklistEntry],
    query: &str,
    page: usize,
    total_pages: usize,
    total: usize,
) -> Markup {
    let blocked: HashSet<&str> = blacklist.iter().map(|e| e.id.as_str()).collect();
    html! {
      section.group {
        (panel_intro("servers genesis is in, most members first. blacklist one to make genesis ignore it."))
        form.search-row method="get" action="/owner" {
            input type="hidden" name="tab" value="servers";
            input type="search" name="q" value=(query) placeholder="search servers" aria-label="search servers";
            button.btn.sm type="submit" { "search" }
        }
        p.muted.results-count {
            (total) " server" @if total != 1 { "s" }
            @if !query.is_empty() { " matching " (query) }
        }
        ul.server-list {
            @for g in guilds {
                li.server-row {
                    img.icon src=(g.icon_url()) alt="";
                    span.server-row-name { (g.name) }
                    span.server-row-count { (g.member_count) " member" @if g.member_count != 1 { "s" } }
                    @if blocked.contains(g.id.as_str()) {
                        form method="post" action="/owner/blacklist/server/remove" {
                            input type="hidden" name="id" value=(g.id);
                            button.btn.sm.danger type="submit" { "unblacklist" }
                        }
                    } @else {
                        form method="post" action="/owner/blacklist/server/add" {
                            input type="hidden" name="id" value=(g.id);
                            button.btn.sm type="submit" { "blacklist" }
                        }
                    }
                }
            }
            @if guilds.is_empty() {
                li.muted { "no servers found." }
            }
        }
        (pager(&format!("/owner?tab=servers&q={}", config::urlencode(query)), page, total_pages))
      }

      section.group {
        (panel_head("all blacklisted servers", "includes servers genesis isn't currently in."))
        (id_list(blacklist, "/owner/blacklist/server/remove"))
        form.add-row method="post" action="/owner/blacklist/server/add" {
            input type="text" name="id" placeholder="server id" aria-label="server id" required;
            input type="text" name="reason" placeholder="reason (optional)" aria-label="reason";
            button.btn.sm type="submit" { "add" }
        }
      }
    }
}

fn media_host_list(media_hosts: &[(String, String)]) -> Markup {
    html! {
        ul.idlist {
            @for (service, domain) in media_hosts {
                li {
                    span.idlist-id {
                        span.tag.sm { (service) }
                        " " (domain)
                    }
                    form.idlist-form method="post" action="/owner/media-hosts/remove" {
                        input type="hidden" name="service" value=(service);
                        input type="hidden" name="domain" value=(domain);
                        button.idlist-remove type="submit" aria-label=(format!("remove {domain}")) title="remove" { "×" }
                    }
                }
            }
            @if media_hosts.is_empty() {
                li.muted { "none" }
            }
        }
    }
}

fn domains_panel(
    global_domains: &[String],
    git_hosts: &[String],
    media_hosts: &[(String, String)],
) -> Markup {
    html! {
        section.group {
            (panel_intro("blocked in every server, on top of per-server blocks."))
            (domain_list(global_domains, "/owner/domains/remove"))
            form.add-row method="post" action="/owner/domains/add" {
                input type="text" name="domain" placeholder="example.com" aria-label="domain to block" required;
                button.btn.sm type="submit" { "add" }
            }
        }

        section.group {
            (panel_head("git hosts", "self-hosted gitea / forgejo domains genesis should embed, alongside github.com and gitlab.com."))
            (domain_list(git_hosts, "/owner/git-hosts/remove"))
            form.add-row method="post" action="/owner/git-hosts/add" {
                input type="text" name="domain" placeholder="git.example.com" aria-label="git host domain" required;
                button.btn.sm type="submit" { "add" }
            }
        }

        section.group {
            (panel_head("custom media domains", "extra mirror domains genesis should treat as a platform, alongside the built-in ones. each server can block them too."))
            (media_host_list(media_hosts))
            form.add-row method="post" action="/owner/media-hosts/add" {
            select name="service" aria-label="service" required {
                @for g in catalog::media_services() {
                    option value=(g.key) { (g.label) }
                }
            }
                input type="text" name="domain" placeholder="example.com" aria-label="media host domain" required;
                button.btn.sm type="submit" { "add" }
            }
        }
    }
}

fn commands_panel(disabled: &[String]) -> Markup {
    let disabled: HashSet<&str> = disabled.iter().map(String::as_str).collect();
    html! {
      section.group {
        (panel_intro("turn commands off everywhere."))
        (command_toggles("/owner/commands", &disabled))
      }
    }
}

fn status_panel(status: Option<&BotStatus>) -> Markup {
    let cur_type = status.map(|s| s.status_type.as_str()).unwrap_or("playing");
    let cur_text = status.map(|s| s.status_text.as_str()).unwrap_or("");
    let cur_online = status.map(|s| s.online_status.as_str()).unwrap_or("online");
    html! {
      section.group {
        (panel_intro("what genesis shows as its activity and presence in discord."))
        form.config-form method="post" action="/owner/status" {
            div.field {
                label for="status-type" { "activity" }
                select #status-type name="status_type" {
                    @for t in catalog::ACTIVITY_TYPES {
                        option value=(t) selected[cur_type == *t] { (t) }
                    }
                }
            }
            div.field {
                label for="status-text" { "text" }
                input #status-text type="text" name="status_text" value=(cur_text)
                    placeholder="with your links" aria-label="status text";
            }
            div.field {
                label for="status-online" { "presence" }
                select #status-online name="online_status" {
                    @for o in catalog::ONLINE_STATES {
                        option value=(o) selected[cur_online == *o] { (o) }
                    }
                }
            }
            div { button.btn.primary type="submit" { "save status" } }
        }
      }
    }
}

fn failures_panel(
    entries: &[FailureEntry],
    code_counts: &[(String, i64)],
    active_code: Option<&str>,
    page: usize,
    total_pages: usize,
    total: usize,
) -> Markup {
    html! {
        section.group {
            p.field-hint {
                "embed failures from every server over the last 30 days, newest first. "
                (total) " shown."
            }
            (failure_list(
                "/owner?tab=failures",
                active_code,
                code_counts,
                entries,
                true,
                page,
                total_pages,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn owner(
    user: &User,
    tab: &str,
    users: &[BlacklistEntry],
    servers: &[BlacklistEntry],
    global_domains: &[String],
    git_hosts: &[String],
    media_hosts: &[(String, String)],
    disabled_commands: &[String],
    status: Option<&BotStatus>,
    bot_servers: &[discord::BotGuild],
    query: &str,
    page: usize,
    total_pages: usize,
    total: usize,
    failures: &[FailureEntry],
    failure_codes: &[(String, i64)],
    failure_code: Option<&str>,
    saved: bool,
) -> Markup {
    let body = html! {
        div.page-head-row {
            h1 { "global settings" }
            a.btn.sm href="/dashboard" { "back to servers" }
        }

        (tabs(tab))

        @if saved {
            div.success-message role="status" { "saved." }
        }

        div.owner-body {
            @match tab {
                "servers" => (servers_panel(bot_servers, servers, query, page, total_pages, total)),
                "domains" => (domains_panel(global_domains, git_hosts, media_hosts)),
                "commands" => (commands_panel(disabled_commands)),
                "status" => (status_panel(status)),
                "failures" => (failures_panel(
                    failures,
                    failure_codes,
                    failure_code,
                    page,
                    total_pages,
                    total,
                )),
                _ => (blacklist_panel(users)),
            }
        }
        script src="/static/js/autosave.js" defer {}
        script src="/static/js/group-toggles.js" defer {}
    };
    layout_wide("global settings", "", Some(user), body)
}
