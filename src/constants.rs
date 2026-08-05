pub const BUILD_SHA: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(sha) => sha,
    None => "dev",
};
pub const BUILD_DATE: &str = match option_env!("VERGEN_GIT_COMMIT_DATE") {
    Some(date) => date,
    None => "",
};

pub const EMBED_SUPPRESS_DELAY_MS: u64 = 500;
pub const EMBED_SUPPRESS_RETRY_DELAY_MS: u64 = 3000;
pub const DISCORD_MESSAGE_LIMIT: usize = 2000;
pub const TRUNCATED_MESSAGE_LIMIT: usize = 1950;
pub const DEFAULT_PREFIX: &str = "!";
pub const DEFAULT_ENVIRONMENT: &str = "prod";
pub const STATUS_POLL_SECONDS: u64 = 20;
pub const MEDIA_HOSTS_POLL_SECONDS: u64 = 60;
pub const REPLY_WATCH_SECONDS: u64 = 60;
pub const FAILED_EMBED_REACTION: char = '❌';
pub const DEFAULT_STATUS_TYPE: &str = "watching";
pub const DEFAULT_STATUS_TEXT: &str = "you";
pub const DEFAULT_ONLINE_STATUS: &str = "online";
pub const MAX_DIFF_CHARS_PER_FILE: usize = 1800;
pub const FILES_PER_PAGE: usize = 1;
pub const MAX_FILE_FETCH_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_DIFF_FETCH_BYTES: usize = 10 * 1024 * 1024;
pub const SPACES_PER_TAB: &str = "    ";
pub const MAX_FILE_CHARS: usize = 1700;

pub const GIF_MAX_UPLOAD_BYTES: usize = 9 * 1024 * 1024;
pub const GIF_MAX_WIDTH: u32 = 480;
pub const GIF_FPS: u32 = 15;

pub const INSTAGRAM_DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
pub const INSTAGRAM_MOBILE_UA: &str = "Instagram 275.0.0.27.98 Android (33/13; 420dpi; 1080x2400; samsung; SM-G991B; o1s; exynos2100)";
pub const INSTAGRAM_EMBED_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
pub const INSTAGRAM_MIRROR_UA: &str = "TelegramBot (like TwitterBot)";
pub const INSTAGRAM_APP_ID: &str = "936619743392459";
pub const INSTAGRAM_DOC_IDS: &[&str] = &["8845758582119845", "25981206651899035"];
pub const INSTAGRAM_MIRRORS: &[&str] = &[
    "kkinstagram.com",
    "vxinstagram.com",
    "eeinstagram.com",
    "uuinstagram.com",
];
pub const INSTAGRAM_HOSTS: &[&str] = &[
    "instagram.com",
    "ddinstagram.com",
    "kkinstagram.com",
    "vxinstagram.com",
    "eeinstagram.com",
    "uuinstagram.com",
    "zzinstagram.com",
    "fxstagram.com",
];
pub const INSTAGRAM_MAX_CAROUSEL_SLIDES: usize = 20;

pub const TWITTER_SYNDICATION_UA: &str = "Googlebot";
pub const TWITTER_DOWNLOAD_UA: &str = "Mozilla/5.0 (compatible; GenesisBot/1.0)";
pub const TWITTER_DESKTOP_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

pub const TIKTOK_DOWNLOAD_UA: &str = "Mozilla/5.0 (compatible; GenesisBot/1.0)";
pub const TIKTOK_HOSTS: &[&str] = &[
    "tiktok.com",
    "vxtiktok.com",
    "tiktxk.com",
    "tnktok.com",
    "tfxktok.com",
    "kktiktok.com",
    "fixtiktok.com",
];

pub const TIMEZONE_API: &str = "https://timezone.creations.works/v1/get";
pub const TIMEZONE_REGISTER_URL: &str = "https://timezone.creations.works/";

pub const INSTAGRAM_ACCENT_COLOR: u32 = 0xE4405F;
pub const TWITTER_ACCENT_COLOR: u32 = 0x1DA1F2;
pub const TIKTOK_ACCENT_COLOR: u32 = 0x000000;
pub const BSKY_ACCENT_COLOR: u32 = 0x1185FE;

pub const TWITTER_HOSTS: &[&str] = &[
    "twitter.com",
    "x.com",
    "vxtwitter.com",
    "fxtwitter.com",
    "fixupx.com",
    "xfixup.com",
    "fixvx.com",
    "twittpr.com",
    "twitterez.com",
];

pub const BSKY_DOWNLOAD_UA: &str = "Mozilla/5.0 (compatible; GenesisBot/1.0)";
pub const BSKY_API_BASE: &str = "https://public.api.bsky.app";
pub const BSKY_PLC_DIRECTORY: &str = "https://plc.directory";
pub const BSKY_HOSTS: &[&str] = &[
    "bsky.app",
    "fxbsky.app",
    "bskx.app",
    "psky.app",
    "cbsky.app",
];

pub const RATE_LIMIT_SECONDS: u64 = 10;
pub const MAX_RATE_LIMIT_ENTRIES: usize = 10_000;
pub const HANDLED_MESSAGE_TTL_SECONDS: u64 = 3600;
pub const MAX_HANDLED_MESSAGE_ENTRIES: usize = 10_000;

pub const FAILURE_FETCH: &str = "fetch_failed";
pub const FAILURE_SEND: &str = "send_failed";
pub const FAILURE_TOO_LONG: &str = "too_long";
pub const FAILURE_NOT_TEXT: &str = "not_text";
pub const FAILURE_OUT_OF_RANGE: &str = "out_of_range";
pub const FAILURE_DELETED: &str = "deleted";
pub const FAILURE_UNAVAILABLE: &str = "unavailable";
pub const META_REPORT_CHANNEL: &str = "report_channel";
pub const REPORT_DEDUP_SECONDS: u64 = 600;
pub const MAX_REPORT_DEDUP_ENTRIES: usize = 1000;
pub const MAX_FAILURE_DETAIL_CHARS: usize = 300;
pub const WELCOME_RATE_LIMIT_SECONDS: u64 = 2;
pub const DIFF_CACHE_MAX_ENTRIES: usize = 1000;
pub const FILE_CACHE_MAX_ENTRIES: usize = 50;
pub const PAGE_CACHE_TTL_SECONDS: u64 = 600;
pub const REPLY_WATCH_PRUNE_THRESHOLD: usize = 256;

pub const TOGGLEABLE_SERVICES: &[&str] = &[
    "git_diffs",
    "git_compares",
    "git_links",
    "twitter",
    "tiktok",
    "instagram",
    "bsky",
];

pub const REMINDER_POLL_SECONDS: u64 = 15;
pub const REMINDER_MIN_SECONDS: u64 = 10;
pub const REMINDER_MAX_SECONDS: u64 = 365 * 86400;
pub const MAX_REMINDERS_PER_USER: i64 = 25;
pub const MAX_REMINDER_CHARS: usize = 1000;
pub const REMINDER_PREFIX: &str = "Reminder: ";
pub const SNOOZE_CHOICES: &[(u64, &str)] = &[
    (600, "Snooze 10m"),
    (3600, "Snooze 1h"),
    (86400, "Snooze 1d"),
];

pub const COMMAND_SCOPE_GLOBAL: &str = "global";

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
