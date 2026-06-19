pub struct Feature {
    pub icon: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
}

pub struct FeatureGroup {
    pub title: &'static str,
    pub features: &'static [Feature],
}

pub const FEATURE_GROUPS: &[FeatureGroup] = &[
    FeatureGroup {
        title: "link embeds",
        features: &[
            Feature {
                icon: "twitter",
                title: "twitter / x",
                desc: "posts, quote tweets, videos, and photos, including fxtwitter and vxtwitter links",
            },
            Feature {
                icon: "instagram",
                title: "instagram",
                desc: "posts, reels, and full photo carousels with no missing slides",
            },
            Feature {
                icon: "tiktok",
                title: "tiktok",
                desc: "videos and photo slideshows",
            },
            Feature {
                icon: "github",
                title: "github and gitlab",
                desc: "file snippets and commit diffs with pagination",
            },
        ],
    },
    FeatureGroup {
        title: "more",
        features: &[
            Feature {
                icon: "clock",
                title: "timezone tools",
                desc: "look up a user's local time and convert times between zones",
            },
            Feature {
                icon: "user",
                title: "works anywhere",
                desc: "add genesis to your own account and use the slash commands in any dm or group chat, not just servers",
            },
        ],
    },
];

pub const SUPPORTED_DOMAINS: &[&str] = &[
    "twitter.com",
    "x.com",
    "instagram.com",
    "tiktok.com",
    "github.com",
    "gitlab.com",
];

pub const ACTIVITY_TYPES: &[&str] = &["playing", "listening", "watching", "competing"];
pub const ONLINE_STATES: &[&str] = &["online", "idle", "dnd", "invisible"];

pub struct Command {
    pub usage: &'static str,
    pub desc: &'static str,
}

pub struct Group {
    pub title: &'static str,
    pub note: &'static str,
    pub commands: &'static [Command],
}

pub const COMMAND_GROUPS: &[Group] = &[
    Group {
        title: "media",
        note: "usable in servers, dms, and group chats",
        commands: &[
            Command {
                usage: "/instagram <url>",
                desc: "post an instagram link (post, reel, carousel)",
            },
            Command {
                usage: "/twitter <url>",
                desc: "post a tweet (text, photos, videos, quotes)",
            },
            Command {
                usage: "/tiktok <url>",
                desc: "post a tiktok video or photo slideshow",
            },
            Command {
                usage: "/git <url> [only_me]",
                desc: "post a git file snippet or commit diff",
            },
            Command {
                usage: "/timezone [user]",
                desc: "look up a user's timezone and local time",
            },
        ],
    },
    Group {
        title: "utility",
        note: "",
        commands: &[
            Command {
                usage: "/time <time> [from_tz] [to_tz]",
                desc: "convert a time between timezones",
            },
            Command {
                usage: "/checkperms",
                desc: "show the bot's permissions in this channel",
            },
            Command {
                usage: "/ping",
                desc: "check the bot's response time",
            },
            Command {
                usage: "/stats",
                desc: "show servers, users, uptime, and version",
            },
            Command {
                usage: "/help [command]",
                desc: "show help for all commands or one command",
            },
        ],
    },
    Group {
        title: "server settings",
        note: "requires manage server",
        commands: &[
            Command {
                usage: "/settings toggle <service> <enabled>",
                desc: "enable or disable a service in this server",
            },
            Command {
                usage: "/settings reply_cleanup <enabled>",
                desc: "delete the bot's reply if a mod removes the original message within a minute",
            },
            Command {
                usage: "/settings block_domain <domain>",
                desc: "block a domain in this server",
            },
            Command {
                usage: "/settings unblock_domain <domain>",
                desc: "unblock a domain in this server",
            },
            Command {
                usage: "/settings blocked_domains",
                desc: "list domains blocked in this server",
            },
            Command {
                usage: "/welcome",
                desc: "configure welcome messages for new members",
            },
        ],
    },
    Group {
        title: "moderation",
        note: "requires the matching permission",
        commands: &[
            Command {
                usage: "/ban <user> [delete_days] [softban] [reason]",
                desc: "ban a user, with optional message cleanup",
            },
            Command {
                usage: "/kick <user> [reason]",
                desc: "kick a user from the server",
            },
        ],
    },
];

pub const TOGGLEABLE_COMMANDS: &[&str] = &[
    "instagram",
    "twitter",
    "tiktok",
    "git",
    "timezone",
    "time",
    "checkperms",
    "ping",
    "stats",
    "ban",
    "kick",
];

pub fn toggleable_commands() -> Vec<&'static str> {
    TOGGLEABLE_COMMANDS.to_vec()
}
