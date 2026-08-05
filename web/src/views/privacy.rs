use super::layout::layout;
use crate::config::Config;
use crate::discord::User;
use maud::{Markup, html};

pub fn privacy(config: &Config, user: Option<&User>) -> Markup {
    let body = html! {
        div.legal {
            div.page-intro {
                h1 { "privacy policy" }
                p.muted { "last updated 8 july 2026" }
            }

            p {
                "genesis is a discord bot that re-embeds links and offers a few utility commands, "
                "with an optional web dashboard for configuration. this explains what it reads, what "
                "it stores, and who it talks to."
            }

            h2.faq-section { "what genesis reads" }
            p {
                "to find supported links, genesis reads the text of messages in servers it's in "
                "(discord's \"message content\" permission). this happens in memory, as messages arrive "
                "or are edited, purely to spot links worth re-embedding. message text is never written "
                "to a database or a log."
            }
            p {
                "you can turn this off for yourself with " code { "/optout" } ": genesis will then skip your "
                "messages entirely and stop auto-embedding your links. slash commands you run yourself "
                "still work, and " code { "/optin" } " reverses it."
            }
            p {
                "if a server turns on reply cleanup, genesis also reads that server's audit log when a "
                "message it replied to is deleted, only to check whether a moderator rather than the "
                "original author removed it. this is read on demand and not stored."
            }

            h2.faq-section { "what we store" }
            p {
                "genesis keeps a small amount of configuration in its database, all keyed to discord ids "
                "(server, channel, role, and user ids) rather than names:"
            }
            ul {
                li { "per-server settings: which services are enabled, your welcome message, channel, and auto-role, and an optional channel for embed-failure reports" }
                li { "domains you've blocked, per server and globally" }
                li { "which commands are turned off, per server or globally" }
                li { "self-hosted git hosts an operator has added, and the bot's configured status" }
                li { "blacklisted user and server ids, with an optional reason" }
                li { "reminders you set: your user id, the channel to ping you in, when to fire, and the reminder text you typed. a reminder is deleted as soon as it fires (snoozing reschedules it)" }
                li { "failed embeds: details of links that failed to embed, kept for up to 30 days so failures can be diagnosed (see failure reports below)" }
                li { "opt-outs: if you run " code { "/optout" } ", we store your user id so genesis knows to leave your messages alone; it's removed when you run " code { "/optin" } }
            }
            p {
                "when a setting is changed, we also store the discord id of whoever changed it, so it "
                "can be audited. beyond that, the personal ids we keep are yours on reminders you've "
                "set, an opt-out you've chosen, and any blacklisted user ids."
            }

            h2.faq-section { "usage stats" }
            p {
                "genesis keeps anonymous, aggregate counters: how many times each command has been run, "
                "and how many embeds per platform succeeded or failed. the counters are plain totals, "
                "not tied to any user, server, message, or url. failed embeds are also "
                "recorded individually; see below."
            }

            h2.faq-section { "failure reports" }
            p {
                "when a link fails to embed, genesis records the service, an error code, the link, the "
                "server id, and a short error message, and deletes the record after 30 days. server "
                "admins can set a report channel (" code { "/settings report_channel" } " or the "
                "dashboard) that receives that server's failures. the bot's operator also has a global "
                "report channel and a dashboard view that receive these reports, including the failed "
                "link, from every server, so recurring breakage can be spotted and fixed. successful "
                "embeds are never recorded this way."
            }

            h2.faq-section { "what we don't store" }
            p {
                "we do not store message content, your username, or your message history, and links "
                "that embed successfully are never kept. the two exceptions are things described above: "
                "the text of a reminder you set (stored until it fires), and links that failed to embed "
                "(kept for 30 days). media is downloaded only long enough to re-upload it to discord "
                "and is then discarded. the only short-lived in-memory state is a git diff cache kept for "
                "a few minutes and, where reply cleanup is on, a brief list of recent replies (ids only, "
                "about a minute) so they can be removed if a moderator deletes the original."
            }

            h2.faq-section { "the dashboard" }
            p {
                "signing in uses discord oauth with the " code { "identify" } " and " code { "guilds" }
                " scopes, so we can show your name and the servers you manage. your session lives in an "
                "encrypted cookie, and we do not keep your discord access token after the session ends."
            }

            h2.faq-section { "third parties" }
            p {
                "to build an embed, genesis fetches the post from the source platform or a mirror of it: "
                "twitter / x, instagram, tiktok (through tikwm.com), bluesky (through its public atproto api), and github / gitlab / "
                "self-hosted git hosts. the " code { "/timezone" } " command sends a discord user id to our "
                "timezone service to look up their zone. each service's own privacy policy applies to the "
                "data it receives."
            }

            h2.faq-section { "logging" }
            p {
                "genesis writes operational logs to its own server console. moderation actions (ban, kick) "
                "are logged with the moderator, the target, and the reason. message content is not logged, "
                "and logs are not shared."
            }

            h2.faq-section { "keeping and removing data" }
            p {
                "settings stay until they're removed. reminders delete themselves when they fire and "
                "can be removed early with " code { "/reminder remove" } "; embed-failure records expire "
                "after 30 days on their own. from the dashboard you can wipe everything stored for a "
                "server including its failure records (its danger zone tab), or delete all data about "
                "yourself including your pending reminders (the dashboard's your-data section); both "
                "sign you out when done. removing the bot from a server does not "
                "by itself erase its settings. per-server blocked domains can also be cleared with "
                code { "/settings unblock_domain" } ", or you can ask us."
            }

            h2.faq-section { "contact" }
            @if let Some(url) = config.support_invite_url() {
                p { "join the support server: " a href=(url) { (url) } }
            }
            p { "or email " a href=(format!("mailto:{}", config.contact_email)) { (config.contact_email) } "." }

            h2.faq-section { "changes" }
            p { "we may update this policy as genesis changes; the date above is when it last changed." }
        }
    };
    layout("privacy", "", user, body)
}
