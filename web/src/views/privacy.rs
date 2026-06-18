use super::layout::layout;
use crate::config::Config;
use crate::discord::User;
use maud::{Markup, html};

pub fn privacy(config: &Config, user: Option<&User>) -> Markup {
    let body = html! {
        div.legal {
            div.page-intro {
                h1 { "privacy policy" }
                p.muted { "last updated 18 june 2026" }
            }

            p {
                "genesis is a discord bot that re-embeds links and offers a few utility commands, "
                "with an optional web dashboard for configuration. this explains what it reads, what "
                "it stores, and who it talks to."
            }

            h2.faq-section { "what genesis reads" }
            p {
                "to find supported links, genesis reads the text of messages in servers it's in "
                "(discord's “message content” permission). this happens in memory, as messages arrive, "
                "purely to spot links worth re-embedding. message text is never written to a database "
                "or a log."
            }

            h2.faq-section { "what we store" }
            p {
                "genesis keeps a small amount of configuration in its database, all keyed to discord ids "
                "(server, channel, role, and user ids) rather than names:"
            }
            ul {
                li { "per-server settings: which services are enabled, and your welcome message, channel, and auto-role" }
                li { "domains you've blocked, per server and globally" }
                li { "which commands are turned off, per server or globally" }
                li { "self-hosted git hosts an operator has added, and the bot's configured status" }
                li { "blacklisted user and server ids, with an optional reason" }
            }
            p {
                "when a setting is changed, we also store the discord id of whoever changed it, so it "
                "can be audited. that is the only place a personal id is recorded."
            }

            h2.faq-section { "what we don't store" }
            p {
                "we do not store message content, the links you post, the media in them, your username, "
                "or your message history. media is downloaded only long enough to re-upload it to discord "
                "and is then discarded; the only short-lived cache is a git diff kept in memory for a few "
                "minutes."
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
                "twitter / x, instagram, tiktok (through tikwm.com), reddit, and github / gitlab / "
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
                "settings stay until they're removed. removing the bot from a server does not by itself "
                "erase that server's stored settings; to have them deleted, clear them from the dashboard "
                "or ask us. per-server blocked domains can be removed any time with "
                code { "/settings unblock_domain" } "."
            }

            h2.faq-section { "contact" }
            @if let Some(url) = config.support_invite_url() {
                p { "join the support server: " a href=(url) { (url) } }
            }
            p { "or email " a href=(format!("mailto:{}", config.contact_email)) { (config.contact_email) } "." }

            h2.faq-section { "changes" }
            p { "we may update this policy as genesis changes; the date above is the latest version." }
        }
    };
    layout("privacy", "", user, body)
}
