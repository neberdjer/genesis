use crate::discord::User;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use std::borrow::Cow;
use std::collections::HashSet;

const THEME_INIT: &str = r#"(function(){var m=null;try{m=localStorage.getItem("theme")}catch(_){}if(m!=="light"&&m!=="dark")m="system";var t=m==="system"?(matchMedia("(prefers-color-scheme: light)").matches?"light":"dark"):m;var r=document.documentElement;r.setAttribute("data-theme",t);r.setAttribute("data-theme-mode",m);})();"#;

fn inline_svg(svg: &str, class: &str) -> Markup {
    PreEscaped(svg.replacen(
        "<svg",
        &format!("<svg class=\"{class}\" aria-hidden=\"true\" focusable=\"false\""),
        1,
    ))
}

pub(super) fn ico(name: &str) -> Markup {
    let svg = match name {
        "discord" => include_str!("../../static/icons/discord.svg"),
        "user" => include_str!("../../static/icons/user.svg"),
        "code" => include_str!("../../static/icons/code.svg"),
        "chat" => include_str!("../../static/icons/chat.svg"),
        "video" => include_str!("../../static/icons/video.svg"),
        "camera" => include_str!("../../static/icons/camera.svg"),
        "clock" => include_str!("../../static/icons/clock.svg"),
        "terminal" => include_str!("../../static/icons/terminal.svg"),
        "twitter" => include_str!("../../static/icons/twitter.svg"),
        "instagram" => include_str!("../../static/icons/instagram.svg"),
        "tiktok" => include_str!("../../static/icons/tiktok.svg"),
        "bsky" => include_str!("../../static/icons/bsky.svg"),
        "github" => include_str!("../../static/icons/github.svg"),
        "gitlab" => include_str!("../../static/icons/gitlab.svg"),
        _ => return PreEscaped(String::new()),
    };
    inline_svg(svg, "ico")
}

fn is_http_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

/// Render an attacker-controlled url as a link only when it uses an http(s)
/// scheme; anything else (e.g. a `javascript:` uri) is shown as plain text.
pub(super) fn link_or_code(url: &str) -> Markup {
    html! {
        @if is_http_url(url) {
            a href=(url) rel="noopener noreferrer nofollow" { (url) }
        } @else {
            code { (url) }
        }
    }
}

const FAILURE_CODES: &[(&str, &str, &str)] = &[
    (
        "fetch_failed",
        "fetch failed",
        "genesis couldn't reach the service, or it returned an error",
    ),
    (
        "send_failed",
        "send failed",
        "the embed was built but discord refused the message, usually a missing permission",
    ),
    (
        "too_long",
        "too long",
        "the result didn't fit in a discord message",
    ),
];

fn failure_meta(code: &str) -> (&'static str, Cow<'_, str>, Cow<'_, str>) {
    match FAILURE_CODES.iter().find(|(k, _, _)| *k == code) {
        Some((key, label, desc)) => (key, Cow::Borrowed(*label), Cow::Borrowed(*desc)),
        None => (
            "other",
            Cow::Owned(code.replace('_', " ")),
            Cow::Owned(format!("failure code: {code}")),
        ),
    }
}

fn service_label(service: &str) -> Cow<'_, str> {
    if service.contains('_') {
        Cow::Owned(service.replace('_', " "))
    } else {
        Cow::Borrowed(service)
    }
}

pub(super) fn failure_filters(
    base: &str,
    active: Option<&str>,
    counts: &[(String, i64)],
) -> Markup {
    let total: i64 = counts.iter().map(|(_, n)| n).sum();
    let mut ordered: Vec<(&str, Cow<'_, str>, i64)> = counts
        .iter()
        .map(|(code, n)| (code.as_str(), failure_meta(code).1, *n))
        .collect();
    ordered.sort_by_key(|(code, ..)| {
        FAILURE_CODES
            .iter()
            .position(|(k, ..)| k == code)
            .unwrap_or(usize::MAX)
    });

    html! {
        @if !counts.is_empty() {
            nav.filter-bar aria-label="filter by failure type" {
                a class=(if active.is_none() { "filter-chip active" } else { "filter-chip" })
                  href=(base)
                  aria-current=[active.is_none().then_some("true")] {
                    "all" span.filter-count { (total) }
                }
                @for (code, label, n) in &ordered {
                    @let is_active = active == Some(*code);
                    a class=(if is_active { "filter-chip active" } else { "filter-chip" })
                      href=(format!("{base}&type={code}"))
                      aria-current=[is_active.then_some("true")] {
                        (label) span.filter-count { (n) }
                    }
                }
            }
        }
    }
}

pub(super) fn failure_rows(entries: &[crate::db::FailureEntry], show_guild: bool) -> Markup {
    html! {
        ul.failure-log {
            @for e in entries {
                @let (kind, label, desc) = failure_meta(&e.code);
                li.failure-row.{ "is-" (kind) } {
                    div.failure-head {
                        span.failure-type title=(desc) { (label) }
                        span.failure-service { (service_label(&e.service)) }
                        span.failure-time { (e.at) }
                    }
                    @if let Some(url) = &e.url {
                        div.failure-url title=(url) { (link_or_code(url)) }
                    }
                    p.failure-detail title=(e.detail) { (e.detail) }
                    @if show_guild {
                        @if let Some(gid) = &e.guild_id {
                            div.failure-meta { "server " span.failure-guild { (gid) } }
                        }
                    }
                }
            }
        }
    }
}

pub(super) fn toggle_state(on: usize, total: usize, word: &str) -> String {
    if total > 0 && on == total {
        format!("all {word}")
    } else if on == 0 {
        format!("none {word}")
    } else {
        format!("{on} of {total} {word}")
    }
}

pub(super) fn toggle_row(
    label: &str,
    legend: &str,
    chip_class: &str,
    word: &str,
    items: &[(&str, bool)],
) -> Markup {
    let on = items.iter().filter(|(_, checked)| *checked).count();
    html! {
        div.opt-row data-toggle-group {
            label.opt-row-head {
                input type="checkbox" data-toggle-parent checked[on == items.len()];
                span.checkbox aria-hidden="true" {}
                span.opt-row-title { (label) }
                span.opt-row-count data-toggle-count data-toggle-word=(word) {
                    (toggle_state(on, items.len(), word))
                }
            }
            fieldset.check-grid {
                legend.visually-hidden { (legend) }
                @for (name, checked) in items {
                    label class=(chip_class) {
                        input type="checkbox" name=(*name) checked[*checked];
                        span { (name) }
                    }
                }
            }
        }
    }
}

pub(super) fn panel_intro(hint: &str) -> Markup {
    html! {
        p.field-hint { (hint) }
    }
}

pub(super) fn panel_head(title: &str, hint: &str) -> Markup {
    html! {
        h2.group-title { span.group-label { (title) } }
        p.field-hint { (hint) }
    }
}

pub(super) fn failure_list(
    base: &str,
    active: Option<&str>,
    counts: &[(String, i64)],
    entries: &[crate::db::FailureEntry],
    show_guild: bool,
    page: usize,
    total_pages: usize,
) -> Markup {
    let paged_base = match active {
        Some(code) => Cow::Owned(format!("{base}&type={code}")),
        None => Cow::Borrowed(base),
    };
    html! {
        (failure_filters(base, active, counts))
        @if entries.is_empty() {
            @if active.is_some() {
                p.muted { "no failures of this type." }
            } @else {
                p.muted { "no failures recorded." }
            }
        } @else {
            (failure_rows(entries, show_guild))
            (pager(&paged_base, page, total_pages))
        }
    }
}

pub(super) fn command_toggles(action: &str, disabled: &HashSet<&str>) -> Markup {
    html! {
        form.config-form.autosave.row-form data-group-toggles method="post" action=(action) {
            @for (title, names) in crate::catalog::toggleable_command_groups() {
                @let items: Vec<(&str, bool)> = names
                    .iter()
                    .map(|n| (*n, !disabled.contains(n)))
                    .collect();
                (toggle_row(title, &format!("{title} commands"), "check", "enabled", &items))
            }
            div.row-form-actions { span.save-status aria-live="polite" {} }
        }
    }
}

pub(super) fn pager(base: &str, page: usize, total_pages: usize) -> Markup {
    html! {
        @if total_pages > 1 {
            nav.pager aria-label="pages" {
                @if page > 1 {
                    a.btn.sm href=(format!("{base}&page={}", page - 1)) { "prev" }
                }
                span.pager-info { "page " (page) " of " (total_pages) }
                @if page < total_pages {
                    a.btn.sm href=(format!("{base}&page={}", page + 1)) { "next" }
                }
            }
        }
    }
}

fn nav_link(href: &str, label: &str, active: &str) -> Markup {
    let is_active = label == active;
    html! {
        a class=(if is_active { "nav-link active" } else { "nav-link" })
          href=(href)
          aria-current=[is_active.then_some("page")] { (label) }
    }
}

pub(super) fn layout(title: &str, active: &str, user: Option<&User>, body: Markup) -> Markup {
    layout_inner(title, active, user, body, false)
}

pub(super) fn layout_wide(title: &str, active: &str, user: Option<&User>, body: Markup) -> Markup {
    layout_inner(title, active, user, body, true)
}

fn layout_inner(
    title: &str,
    active: &str,
    user: Option<&User>,
    body: Markup,
    wide: bool,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " | genesis" }
                link rel="icon" href="/static/logo.svg";
                link rel="stylesheet" href="/static/css/base.css";
                link rel="stylesheet" href="/static/css/layout.css";
                link rel="stylesheet" href="/static/css/components.css";
                link rel="stylesheet" href="/static/css/pages.css";
                script { (PreEscaped(THEME_INIT)) }
                script src="/static/js/theme.js" defer {}
            }
            body {
                a.skip-link href="#main" { "skip to content" }
                div.container {
                    header.header {
                      div.header-inner {
                        div.header-left {
                            a.brand-link href="/" {
                                img.site-icon src="/static/logo.svg" alt="";
                                span { "genesis" }
                            }
                            nav.nav {
                                (nav_link("/features", "features", active))
                                (nav_link("/commands", "commands", active))
                                (nav_link("/faq", "faq", active))
                            }
                        }
                        div.header-actions {
                            button.icon-btn type="button" onclick="toggleTheme()" aria-label="switch theme" data-theme-toggle {
                                (inline_svg(include_str!("../../static/icons/monitor.svg"), "ico theme-ico-system"))
                                (inline_svg(include_str!("../../static/icons/sun.svg"), "ico theme-ico-light"))
                                (inline_svg(include_str!("../../static/icons/moon.svg"), "ico theme-ico-dark"))
                            }
                            @if let Some(u) = user {
                                span.actions-divider {}
                                details.user-menu {
                                    summary.user-chip {
                                        img.avatar src=(u.avatar_url()) alt="";
                                        span.user-name { (u.username) }
                                        span.user-caret aria-hidden="true" {}
                                    }
                                    div.user-dropdown {
                                        a href="/dashboard" { "dashboard" }
                                        a href="/logout" { "log out" }
                                    }
                                }
                            } @else {
                                a.btn.sm href="/login" { "sign in" }
                            }
                        }
                      }
                    }
                    main #main class=(if wide { "main wide" } else { "main" }) tabindex="-1" { (body) }
                    footer.foot {
                        nav.foot-links {
                            a href="/tos" { "terms" }
                            a href="/privacy" { "privacy" }
                            a href="/faq" { "faq" }
                            a href="https://heliopolis.live/atums/genesis" target="_blank" rel="noopener" { "source" }
                        }
                        div.foot-brand {
                            img.foot-logo src="/static/logo.svg" alt="";
                            span { "a part of atums world" }
                        }
                    }
                }
            }
        }
    }
}
