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
                icon: "chat",
                title: "reminders",
                desc: "set reminders with durations like 10m or 2d and get pinged when they're due",
            },
            Feature {
                icon: "user",
                title: "works anywhere",
                desc: "add genesis to your own account and use the slash commands in any dm or group chat, not just servers",
            },
        ],
    },
];

pub struct DomainGroup {
    pub key: &'static str,
    pub label: &'static str,
    pub domains: &'static [&'static str],
    pub media: bool,
}

pub const DOMAIN_GROUPS: &[DomainGroup] = &[
    DomainGroup {
        key: "twitter",
        label: "twitter / x",
        media: true,
        domains: &[
            "twitter.com",
            "x.com",
            "vxtwitter.com",
            "fxtwitter.com",
            "fixupx.com",
            "xfixup.com",
            "fixvx.com",
            "twittpr.com",
            "twitterez.com",
        ],
    },
    DomainGroup {
        key: "instagram",
        label: "instagram",
        media: true,
        domains: &[
            "instagram.com",
            "ddinstagram.com",
            "kkinstagram.com",
            "vxinstagram.com",
            "eeinstagram.com",
            "uuinstagram.com",
            "zzinstagram.com",
            "fxstagram.com",
        ],
    },
    DomainGroup {
        key: "tiktok",
        label: "tiktok",
        media: true,
        domains: &[
            "tiktok.com",
            "vxtiktok.com",
            "tiktxk.com",
            "tnktok.com",
            "tfxktok.com",
            "kktiktok.com",
            "fixtiktok.com",
        ],
    },
    DomainGroup {
        key: "git",
        label: "github / gitlab",
        media: false,
        domains: &["github.com", "gitlab.com"],
    },
];

pub fn media_services() -> impl Iterator<Item = &'static DomainGroup> {
    DOMAIN_GROUPS.iter().filter(|g| g.media)
}

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
                usage: "/reminder add <duration> [reminder]",
                desc: "set a reminder (e.g. 10m, 1h30m, 2d), pings you when it's due",
            },
            Command {
                usage: "/reminder list",
                desc: "list your pending reminders",
            },
            Command {
                usage: "/reminder remove <id>",
                desc: "remove one of your reminders",
            },
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
                usage: "/settings command <command> <enabled>",
                desc: "enable or disable a command in this server",
            },
            Command {
                usage: "/settings commands",
                desc: "list commands disabled in this server",
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
