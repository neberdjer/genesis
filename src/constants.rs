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

pub const RATE_LIMIT_SECONDS: u64 = 10;
pub const MAX_RATE_LIMIT_ENTRIES: usize = 10_000;
pub const WELCOME_RATE_LIMIT_SECONDS: u64 = 2;
pub const DIFF_CACHE_MAX_ENTRIES: usize = 1000;

pub const COMMAND_SCOPE_GLOBAL: &str = "global";

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
