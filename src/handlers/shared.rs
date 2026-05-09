use crate::constants::EMBED_SUPPRESS_DELAY_MS;
use crate::db;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

static RATE_LIMITS: OnceLock<Mutex<HashMap<(serenity::UserId, &'static str), Instant>>> =
    OnceLock::new();

const RATE_LIMIT_SECONDS: u64 = 10;
const MAX_RATE_LIMIT_ENTRIES: usize = 10_000;

pub fn check_rate_limit(user_id: serenity::UserId, handler: &'static str) -> bool {
    let rate_limits = RATE_LIMITS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = match rate_limits.lock() {
        Ok(guard) => guard,
        Err(_) => return true,
    };

    let key = (user_id, handler);
    if let Some(last_time) = map.get(&key)
        && last_time.elapsed().as_secs() < RATE_LIMIT_SECONDS
    {
        return false;
    }

    if map.len() >= MAX_RATE_LIMIT_ENTRIES {
        map.retain(|_, instant| instant.elapsed().as_secs() < RATE_LIMIT_SECONDS);
    }

    map.insert(key, Instant::now());
    true
}

pub async fn suppress_embeds(ctx: &serenity::Context, msg: &serenity::Message) {
    tokio::time::sleep(Duration::from_millis(EMBED_SUPPRESS_DELAY_MS)).await;

    if let Err(e) = msg
        .channel_id
        .edit_message(
            &ctx.http,
            msg.id,
            serenity::EditMessage::new().suppress_embeds(true),
        )
        .await
    {
        warn!(
            "Failed to suppress embed in channel {} (needs Manage Messages permission): {}",
            msg.channel_id, e
        );
    }
}

pub async fn download_media(url: &str, user_agent: &str) -> Option<Vec<u8>> {
    let url = url.to_string();
    let user_agent = user_agent.to_string();
    tokio::task::spawn_blocking(move || {
        let response = ureq::get(&url).set("User-Agent", &user_agent).call().ok()?;
        let mut bytes = Vec::new();
        response.into_reader().read_to_end(&mut bytes).ok()?;
        Some(bytes)
    })
    .await
    .ok()?
}

pub async fn spawn_blocking_fetch<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
}

pub enum SettingCheck {
    TikTok,
    Instagram,
    Twitter,
    GitLinks,
    GitDiffs,
}

pub fn is_safe_host(host: &str) -> bool {
    let host = host.split(':').next().unwrap_or(host);

    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return !ip.is_loopback()
            && !ip.is_private()
            && !ip.is_link_local()
            && !ip.is_unspecified()
            && !ip.is_broadcast();
    }
    if let Ok(ip) = host.parse::<std::net::Ipv6Addr>() {
        return !ip.is_loopback() && !ip.is_unspecified();
    }

    let lower = host.to_lowercase();
    if lower == "localhost"
        || lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower.ends_with(".lan")
        || lower.ends_with(".localdomain")
    {
        return false;
    }

    true
}

pub async fn pre_check_user(user_id: serenity::UserId, pool: Option<&PgPool>) -> bool {
    let Some(pool) = pool else {
        return true;
    };

    match db::is_user_blacklisted(pool, &user_id.to_string()).await {
        Ok(true) => false,
        Err(e) => {
            warn!("Failed to check user blacklist: {}", e);
            true
        }
        _ => true,
    }
}

pub async fn pre_check(
    msg: &serenity::Message,
    pool: Option<&PgPool>,
    setting: SettingCheck,
) -> bool {
    if msg.author.bot() {
        return false;
    }

    let Some(pool) = pool else {
        return true;
    };

    match db::is_user_blacklisted(pool, &msg.author.id.to_string()).await {
        Ok(true) => return false,
        Err(e) => warn!("Failed to check user blacklist: {}", e),
        _ => {}
    }

    if let Some(guild_id) = msg.guild_id {
        match db::is_server_blacklisted(pool, &guild_id.to_string()).await {
            Ok(true) => return false,
            Err(e) => warn!("Failed to check server blacklist: {}", e),
            _ => {}
        }

        match db::get_server_settings(pool, &guild_id.to_string()).await {
            Ok(settings) => {
                let enabled = match setting {
                    SettingCheck::TikTok => settings.tiktok_enabled,
                    SettingCheck::Instagram => settings.instagram_enabled,
                    SettingCheck::Twitter => settings.twitter_enabled,
                    SettingCheck::GitLinks => settings.git_links_enabled,
                    SettingCheck::GitDiffs => settings.git_diffs_enabled,
                };
                if !enabled {
                    return false;
                }
            }
            Err(e) => warn!("Failed to fetch server settings: {}", e),
        }
    }

    true
}
