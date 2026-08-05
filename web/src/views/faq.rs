use super::layout::layout;
use crate::config::Config;
use crate::discord::User;
use maud::{Markup, html};

pub fn faq(config: &Config, user: Option<&User>) -> Markup {
    let body = html! {
        div.page-intro {
            h1 { "faq" }
            p.muted { "what genesis does, how to use it, and how to set it up." }
        }

        h2.faq-section { "getting started" }
        div.faq-list {
            details.faq-item open[true] {
                summary.faq-q { "what does genesis do?" }
                div.faq-a {
                    p { "discord's built-in previews for twitter / x, instagram, tiktok, and bluesky are often broken: the video won't play, carousel images go missing, or the text is cut off." }
                    p { "genesis spots those links and replaces the preview with a clean embed that shows the video, every image, and the full post." }
                }
            }
            details.faq-item {
                summary.faq-q { "which sites are supported?" }
                div.faq-a {
                    p { "genesis fixes embeds for:" }
                    ul {
                        li { "twitter / x, including fxtwitter and vxtwitter links" }
                        li { "instagram posts, reels, and full photo carousels" }
                        li { "tiktok videos and photo slideshows" }
                        li { "bluesky posts, including images, video, and quotes" }
                        li { "github and gitlab file snippets and commit diffs" }
                    }
                    p { "it also has timezone tools and reminders. see the " a href="/features" { "features" } " page for the full list." }
                }
            }
            details.faq-item {
                summary.faq-q { "how do i add genesis?" }
                div.faq-a {
                    p { "use the buttons on the home page to add it to a server, or to your own account so it works in any dm or group chat." }
                }
            }
            details.faq-item {
                summary.faq-q { "does it cost anything?" }
                div.faq-a {
                    p { "no, genesis is free to add and use." }
                }
            }
        }

        h2.faq-section { "using genesis" }
        div.faq-list {
            details.faq-item {
                summary.faq-q { "do i have to run a command every time?" }
                div.faq-a {
                    p { "no. once a service is enabled, post a supported link and genesis replaces the broken preview." }
                    p {
                        "the slash commands ("
                        code { "/twitter" } ", " code { "/instagram" } ", " code { "/tiktok" } ", "
                        code { "/bsky" } ", " code { "/git" }
                        ") post one on demand, and work in dms and group chats."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "can i use it in dms?" }
                div.faq-a {
                    p {
                        "yes. the slash commands ("
                        code { "/twitter" } ", " code { "/instagram" } ", " code { "/tiktok" } ", "
                        code { "/bsky" } ", " code { "/git" } ", " code { "/timezone" }
                        ") work in dms and group chats if you added genesis to your account."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "can i post an embed just for myself?" }
                div.faq-a {
                    p {
                        "only for git links. " code { "/git" } " takes an " code { "only_me" }
                        " option that shows the result to you only. see the "
                        a href="/commands" { "commands" } " page for every command and its options."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "can i stop genesis from touching my messages?" }
                div.faq-a {
                    p {
                        "yes. run " code { "/optout" } " and genesis will skip your messages everywhere "
                        "it's used: it won't auto-embed your links or reply to you. run "
                        code { "/optin" } " to turn it back on."
                    }
                    p {
                        "slash commands you run yourself still work while opted out, so you stay in "
                        "control of what gets posted."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "what are the timezone tools?" }
                div.faq-a {
                    p {
                        code { "/timezone" } " looks up a user's timezone and current local time, and "
                        code { "/time" } " converts a time between time zones."
                    }
                }
            }
        }

        h2.faq-section { "configuration" }
        div.faq-list {
            details.faq-item {
                summary.faq-q { "how do i turn a service on or off?" }
                div.faq-a {
                    p {
                        "sign in, open the dashboard, pick your server, and toggle the services you want. "
                        "in the server itself, admins can also use " code { "/settings toggle" } "."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "how do i stop it embedding a specific site?" }
                div.faq-a {
                    p {
                        "block the domain by adding it on the dashboard, or with "
                        code { "/settings block_domain" } " in the server. links to that domain and "
                        "its subdomains are then ignored there."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "what permissions does it need?" }
                div.faq-a {
                    p {
                        "it needs to read messages and to send messages with embeds in the channel. run "
                        code { "/checkperms" } " to see anything it's missing."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "why isn't it responding to links?" }
                div.faq-a {
                    p { "a few things to check:" }
                    ul {
                        li { "the service is enabled for your server" }
                        li {
                            "the bot can read and send messages in that channel (run "
                            code { "/checkperms" } ")"
                        }
                        li { "the link's domain isn't on your blocked list" }
                    }
                }
            }
        }

        h2.faq-section { "privacy & support" }
        div.faq-list {
            details.faq-item {
                summary.faq-q { "what data does it store?" }
                div.faq-a {
                    p {
                        "your server's settings (which services are enabled, any domains you block), any "
                        "reminders you've set until they fire, a 30-day record of links that failed "
                        "to embed, and your user id if you " code { "/optout" } "."
                    }
                    p {
                        "it does not store message content or any media; embeds are built on demand. see "
                        "the " a href="/privacy" { "privacy policy" } " for details."
                    }
                }
            }
            details.faq-item {
                summary.faq-q { "how do i remove it?" }
                div.faq-a {
                    p { "remove the bot from your server like any other. that leaves its settings in place; \
                         clear them from the danger zone tab first if you want them gone." }
                }
            }
            details.faq-item {
                summary.faq-q { "how do i get in touch?" }
                div.faq-a {
                    @if let Some(url) = config.support_invite_url() {
                        p { "join the support server: " a href=(url) { (url) } }
                    }
                    p { "or email " a href=(format!("mailto:{}", config.contact_email)) { (config.contact_email) } "." }
                }
            }
        }
    };
    layout("faq", "faq", user, body)
}
