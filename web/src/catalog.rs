use std::sync::LazyLock;
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
                desc: "posts, reels, and full photo carousels",
            },
            Feature {
                icon: "tiktok",
                title: "tiktok",
                desc: "videos and photo slideshows",
            },
            Feature {
                icon: "bsky",
                title: "bluesky",
                desc: "posts, images, videos, quotes, and link cards",
            },
            Feature {
                icon: "github",
                title: "github / gitlab",
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
                desc: "add genesis to your account and use its commands in any dm or group chat",
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
        key: "bsky",
        label: "bluesky",
        media: true,
        domains: &[
            "bsky.app",
            "fxbsky.app",
            "bskx.app",
            "psky.app",
            "cbsky.app",
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
                usage: "/bsky <url>",
                desc: "post a bluesky link (text, images, video, quotes)",
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
                desc: "set a reminder (e.g. 10m, 1h30m, 2d)",
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
                usage: "/settings report_channel [channel]",
                desc: "send embed-failure reports (the link and the error) to a channel",
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
                desc: "configure welcome messages",
            },
        ],
    },
    Group {
        title: "moderation",
        note: "requires ban or kick permission",
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
    "bsky",
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

fn command_name(usage: &'static str) -> Option<&'static str> {
    usage.trim_start_matches('/').split_whitespace().next()
}

fn build_toggleable_command_groups() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut placed: Vec<&'static str> = Vec::new();
    let mut groups: Vec<(&'static str, Vec<&'static str>)> = Vec::new();

    for group in COMMAND_GROUPS {
        let mut names: Vec<&'static str> = Vec::new();
        for command in group.commands {
            let Some(name) = command_name(command.usage) else {
                continue;
            };
            if TOGGLEABLE_COMMANDS.contains(&name) && !names.contains(&name) {
                names.push(name);
                placed.push(name);
            }
        }
        if !names.is_empty() {
            groups.push((group.title, names));
        }
    }

    let rest: Vec<&'static str> = TOGGLEABLE_COMMANDS
        .iter()
        .copied()
        .filter(|c| !placed.contains(c))
        .collect();
    if !rest.is_empty() {
        groups.push(("other", rest));
    }

    groups
}

static TOGGLEABLE_COMMAND_GROUPS: LazyLock<Vec<(&'static str, Vec<&'static str>)>> =
    LazyLock::new(build_toggleable_command_groups);

pub fn toggleable_command_groups() -> &'static [(&'static str, Vec<&'static str>)] {
    &TOGGLEABLE_COMMAND_GROUPS
}

pub fn extra_mirrors<'a>(
    git_hosts: &'a [String],
    media_hosts: &'a [(String, String)],
) -> Vec<&'a str> {
    let mut known: std::collections::HashSet<&str> = DOMAIN_GROUPS
        .iter()
        .flat_map(|g| g.domains.iter().copied())
        .collect();
    git_hosts
        .iter()
        .map(String::as_str)
        .chain(media_hosts.iter().map(|(_, d)| d.as_str()))
        .filter(|d| known.insert(d))
        .collect()
}
