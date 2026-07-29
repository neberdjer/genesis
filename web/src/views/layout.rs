use crate::discord::User;
use maud::{DOCTYPE, Markup, PreEscaped, html};

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

pub(super) fn failure_rows(entries: &[crate::db::FailureEntry], show_guild: bool) -> Markup {
    html! {
        ul.audit-log {
            @for e in entries {
                li.audit-row {
                    div.audit-head {
                        span.audit-cat { (e.service) }
                        span.audit-time {
                            (e.code)
                            @if show_guild {
                                @if let Some(gid) = &e.guild_id {
                                    " · server " (gid)
                                }
                            }
                            " · " (e.at)
                        }
                    }
                    div.audit-action {
                        @if let Some(url) = &e.url {
                            (link_or_code(url))
                            br;
                        }
                        (e.detail)
                    }
                }
            }
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
                    main.main #main tabindex="-1" { (body) }
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
