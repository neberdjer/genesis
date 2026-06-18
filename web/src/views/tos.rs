use super::layout::layout;
use crate::config::Config;
use crate::discord::User;
use maud::{Markup, html};

pub fn tos(config: &Config, user: Option<&User>) -> Markup {
    let body = html! {
        div.legal {
            div.page-intro {
                h1 { "terms of service" }
                p.muted { "last updated 18 june 2026" }
            }

            p {
                "these terms cover your use of genesis: the discord bot and its web dashboard. by adding "
                "genesis to a server or your account, or by using its commands, you agree to them."
            }

            h2.faq-section { "what genesis does" }
            p {
                "when a service is enabled, genesis watches for supported links and, when it finds one, "
                "replies with a clean embed in its place. it does this automatically; you don't run a "
                "command each time."
            }
            ul {
                li {
                    "twitter / x, instagram, tiktok, and reddit: it downloads the post's media and "
                    "re-uploads it to discord, then hides the preview on your original message. it does "
                    "not delete your message."
                }
                li { "github, gitlab, and self-hosted git hosts: it posts file snippets and commit or compare diffs." }
                li {
                    "utility commands such as " code { "/git" } ", " code { "/timezone" } ", "
                    code { "/time" } ", " code { "/stats" } ", and " code { "/checkperms" } "."
                }
                li { "optional welcome messages and an auto-assigned role for new members." }
                li {
                    "moderation commands (" code { "/ban" } ", " code { "/kick" } ") for users who already "
                    "hold the matching discord permissions."
                }
            }

            h2.faq-section { "re-hosted content" }
            p {
                "embeds are built from content other people posted on third-party platforms. that content "
                "belongs to its creators and platforms and may be copyrighted or age-restricted. genesis "
                "re-flags reddit media marked nsfw as a spoiler, but does not otherwise rate or moderate "
                "what it re-embeds. we are not responsible for content that originates elsewhere; report "
                "anything that shouldn't be there and we'll act on it."
            }

            h2.faq-section { "acceptable use" }
            p {
                "you agree not to use genesis to break "
                a href="https://discord.com/terms" { "discord's terms" }
                ", to harass anyone, or to share illegal content. server admins choose which services and "
                "commands run in their server and are responsible for how it's used there."
            }

            h2.faq-section { "permissions" }
            p {
                "genesis needs to read and send messages, embed links, and attach files. it uses “manage "
                "messages” to hide the preview on your original link and “manage roles” for welcome roles. "
                "without a permission, the related feature simply doesn't run."
            }

            h2.faq-section { "enforcement" }
            p {
                "we may stop genesis from working for a user or a server by blacklisting the relevant id, "
                "whether for abuse, to comply with discord, or at our discretion. server admins can also "
                "disable any service or command with " code { "/settings" } "."
            }

            h2.faq-section { "availability" }
            p {
                "genesis is provided as-is and free of charge, with no guarantee of uptime. features may "
                "change or be removed at any time, and we may update these terms as it evolves."
            }

            h2.faq-section { "liability" }
            p {
                "to the extent the law allows, genesis and its operators are not liable for any damages "
                "arising from using it, including anything related to third-party content it re-embeds."
            }

            h2.faq-section { "contact" }
            @if let Some(url) = config.support_invite_url() {
                p { "join the support server: " a href=(url) { (url) } }
            }
            p { "or email " a href=(format!("mailto:{}", config.contact_email)) { (config.contact_email) } "." }
        }
    };
    layout("terms", "", user, body)
}
