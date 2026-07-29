use crate::constants::{
    EMBED_SUPPRESS_DELAY_MS, EMBED_SUPPRESS_RETRY_DELAY_MS, FAILED_EMBED_REACTION,
    HANDLED_MESSAGE_TTL_SECONDS, MAX_FAILURE_DETAIL_CHARS, MAX_HANDLED_MESSAGE_ENTRIES,
    MAX_RATE_LIMIT_ENTRIES, MAX_REPORT_DEDUP_ENTRIES, META_REPORT_CHANNEL, RATE_LIMIT_SECONDS,
    REPORT_DEDUP_SECONDS,
};
use crate::db;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

pub fn evict_for_insert<K: Clone + Eq + std::hash::Hash, V>(
    map: &mut HashMap<K, V>,
    max_entries: usize,
    ttl_seconds: u64,
    timestamp: impl Fn(&V) -> Instant,
) {
    if map.len() < max_entries {
        return;
    }
    map.retain(|_, v| timestamp(v).elapsed().as_secs() <= ttl_seconds);
    if map.len() >= max_entries
        && let Some(oldest_key) = map
            .iter()
            .min_by_key(|(_, v)| timestamp(v))
            .map(|(k, _)| k.clone())
    {
        map.remove(&oldest_key);
    }
}

static RATE_LIMITS: OnceLock<Mutex<HashMap<(serenity::UserId, &'static str), Instant>>> =
    OnceLock::new();

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

    evict_for_insert(&mut map, MAX_RATE_LIMIT_ENTRIES, RATE_LIMIT_SECONDS, |t| *t);
    map.insert(key, Instant::now());
    true
}

async fn fire_flags_only_suppress(
    ctx: &serenity::Context,
    msg: &serenity::Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use serenity::http::{LightMethod, Request, Route};

    let request = Request::new(
        Route::ChannelMessage {
            channel_id: msg.channel_id,
            message_id: msg.id,
        },
        LightMethod::Patch,
    )
    .body(Some(br#"{"flags":4}"#.to_vec()));

    ctx.http.request(request).await?;
    Ok(())
}

pub async fn suppress_embeds(ctx: &serenity::Context, msg: &serenity::Message) {
    tokio::time::sleep(Duration::from_millis(EMBED_SUPPRESS_DELAY_MS)).await;

    if let Err(e) = fire_flags_only_suppress(ctx, msg).await {
        warn!(
            "Failed to suppress embed in channel {} (needs Manage Messages permission): {}",
            msg.channel_id, e
        );
        return;
    }

    tokio::time::sleep(Duration::from_millis(EMBED_SUPPRESS_RETRY_DELAY_MS)).await;
    let _ = fire_flags_only_suppress(ctx, msg).await;
}

pub async fn record_embed(ctx: &serenity::Context, service: &str, success: bool) {
    db::record_embed(&ctx.data::<crate::Data>().pool, service, success).await;
}

static CUSTOM_MEDIA_HOSTS: OnceLock<RwLock<HashMap<String, Vec<String>>>> = OnceLock::new();

fn custom_media_hosts() -> &'static RwLock<HashMap<String, Vec<String>>> {
    CUSTOM_MEDIA_HOSTS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub async fn refresh_custom_media_hosts(pool: &PgPool) {
    match db::list_media_hosts(pool).await {
        Ok(rows) => {
            let mut map: HashMap<String, Vec<String>> = HashMap::new();
            for (service, domain) in rows {
                map.entry(service)
                    .or_default()
                    .push(domain.to_ascii_lowercase());
            }
            if let Ok(mut cache) = custom_media_hosts().write() {
                *cache = map;
            }
        }
        Err(e) => warn!("Failed to refresh custom media hosts: {}", e),
    }
}

pub fn host_under_domain(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

pub fn matches_host(url: &str, hosts: &[&str], service: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let Some(host) = extract_host(&lower) else {
        return false;
    };
    hosts.iter().any(|&d| host_under_domain(host, d)) || custom_host_matches(service, host)
}

pub fn custom_host_matches(service: &str, host: &str) -> bool {
    let Ok(cache) = custom_media_hosts().read() else {
        return false;
    };
    cache
        .get(service)
        .is_some_and(|domains| domains.iter().any(|d| host_under_domain(host, d)))
}

pub async fn react_failure(ctx: &serenity::Context, msg: &serenity::Message) {
    if let Err(e) = msg.react(&ctx.http, FAILED_EMBED_REACTION).await {
        debug!("Failed to add failure reaction: {}", e);
    }
}

static HANDLED_MESSAGES: OnceLock<Mutex<HashMap<serenity::MessageId, Instant>>> = OnceLock::new();

fn handled_messages() -> &'static Mutex<HashMap<serenity::MessageId, Instant>> {
    HANDLED_MESSAGES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn mark_message_handled(id: serenity::MessageId) {
    if let Ok(mut map) = handled_messages().lock() {
        evict_for_insert(
            &mut map,
            MAX_HANDLED_MESSAGE_ENTRIES,
            HANDLED_MESSAGE_TTL_SECONDS,
            |t| *t,
        );
        map.insert(id, Instant::now());
    }
}

pub fn was_message_handled(id: serenity::MessageId) -> bool {
    handled_messages().lock().is_ok_and(|map| {
        map.get(&id)
            .is_some_and(|t| t.elapsed().as_secs() < HANDLED_MESSAGE_TTL_SECONDS)
    })
}

static REPORT_DEDUP: OnceLock<Mutex<HashMap<String, Instant>>> = OnceLock::new();

fn should_report(scope: &str, service: &str, code: &str) -> bool {
    let dedup = REPORT_DEDUP.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut map) = dedup.lock() else {
        return false;
    };

    let key = format!("{}:{}:{}", scope, service, code);
    if let Some(last) = map.get(&key)
        && last.elapsed().as_secs() < REPORT_DEDUP_SECONDS
    {
        return false;
    }

    evict_for_insert(
        &mut map,
        MAX_REPORT_DEDUP_ENTRIES,
        REPORT_DEDUP_SECONDS,
        |t| *t,
    );
    map.insert(key, Instant::now());
    true
}

async fn send_report(ctx: &serenity::Context, channel: &str, content: &str) {
    let Ok(channel_id) = channel.parse::<u64>() else {
        return;
    };
    let message = serenity::CreateMessage::new()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new());
    if let Err(e) = serenity::GenericChannelId::new(channel_id)
        .send_message(&ctx.http, message)
        .await
    {
        warn!(
            "Failed to send failure report to channel {}: {}",
            channel_id, e
        );
    }
}

pub fn report_failure(
    ctx: &serenity::Context,
    guild_id: Option<serenity::GuildId>,
    service: &str,
    code: &str,
    url: Option<&str>,
    detail: &str,
) {
    let ctx = ctx.clone();
    let guild_id = guild_id.map(|g| g.to_string());
    let service = service.to_string();
    let code = code.to_string();
    let url = url.map(str::to_string);
    let mut detail = detail.replace('\n', " ");
    detail.truncate(floor_char_boundary(&detail, MAX_FAILURE_DETAIL_CHARS));

    tokio::spawn(async move {
        let data = ctx.data::<crate::Data>();
        db::record_embed(&data.pool, &service, false).await;
        db::record_failure(
            &data.pool,
            &service,
            &code,
            url.as_deref(),
            guild_id.as_deref(),
            &detail,
        )
        .await;

        let guild_channel = match &guild_id {
            Some(gid) if should_report(gid, &service, &code) => {
                db::get_server_settings(&data.pool, gid)
                    .await
                    .ok()
                    .and_then(|s| s.report_channel_id)
            }
            _ => None,
        };
        let global_channel = if should_report("global", &service, &code) {
            db::get_meta(&data.pool, META_REPORT_CHANNEL)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        if guild_channel.is_none() && global_channel.is_none() {
            return;
        }

        let content = match &url {
            Some(url) => format!(
                "**{}** failed with `{}`\n<{}>\n-# {}",
                service, code, url, detail
            ),
            None => format!("**{}** failed with `{}`\n-# {}", service, code, detail),
        };

        for channel in [guild_channel, global_channel].into_iter().flatten() {
            send_report(&ctx, &channel, &content).await;
        }
    });
}

pub async fn send_reply(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    service: &str,
    reply: serenity::CreateMessage<'_>,
) -> bool {
    match msg.channel_id.send_message(&ctx.http, reply).await {
        Ok(sent) => {
            super::reply_watch::watch(msg.id, sent.id, msg.channel_id, msg.author.id);
            mark_message_handled(msg.id);
            record_embed(ctx, service, true).await;
            true
        }
        Err(e) => {
            warn!("Failed to send reply in channel {}: {}", msg.channel_id, e);
            report_failure(
                ctx,
                msg.guild_id,
                service,
                crate::constants::FAILURE_SEND,
                None,
                &e.to_string(),
            );
            false
        }
    }
}

pub fn clean_media_url(url: &str) -> &str {
    let trimmed = url.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

pub fn media_filename(prefix: &str, index: usize, url: &str, ext_override: Option<&str>) -> String {
    let lower = url.to_ascii_lowercase();
    let ext = ext_override.unwrap_or_else(|| {
        if lower.contains(".jpeg") || lower.contains(".jpg") {
            "jpg"
        } else if lower.contains(".webp") {
            "webp"
        } else if lower.contains(".png") {
            "png"
        } else if lower.contains(".mp4")
            || lower.contains("video")
            || lower.contains("/play/")
            || lower.contains("playwm")
        {
            "mp4"
        } else {
            "jpg"
        }
    });
    format!("{}_{}.{}", prefix, index, ext)
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

pub async fn mp4_to_gif(data: Vec<u8>) -> Option<Vec<u8>> {
    use crate::constants::{GIF_FPS, GIF_MAX_UPLOAD_BYTES, GIF_MAX_WIDTH};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut input = std::env::temp_dir();
    input.push(format!("genesis_gif_{}_{}.mp4", std::process::id(), seq));

    tokio::task::spawn_blocking(move || {
        std::fs::write(&input, &data).ok()?;
        let filter = format!(
            "fps={},scale='min({},iw)':-2:flags=lanczos,split[s0][s1];[s0]palettegen=stats_mode=diff[p];[s1][p]paletteuse=dither=bayer",
            GIF_FPS, GIF_MAX_WIDTH
        );
        let output = std::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&input)
            .args(["-vf", &filter, "-f", "gif", "-"])
            .output();
        let _ = std::fs::remove_file(&input);

        let output = output.ok()?;
        if !output.status.success() || output.stdout.is_empty() {
            return None;
        }
        if output.stdout.len() > GIF_MAX_UPLOAD_BYTES {
            return None;
        }
        Some(output.stdout)
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
    Bsky,
    GitLinks,
    GitDiffs,
}

pub fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub fn normalize_domain(domain: &str) -> String {
    let lower = domain.trim().to_ascii_lowercase();
    let stripped = lower
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    stripped
        .split('/')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_string()
}

pub fn extract_host(url: &str) -> Option<&str> {
    let scheme_end = url.find("://")?;
    let after_scheme = &url[scheme_end + 3..];
    let host = after_scheme.split('/').next()?.split('?').next()?;
    let host = host.split(':').next()?;
    if host.is_empty() { None } else { Some(host) }
}

pub fn host_matches_blocked(host: &str, blocked: &str) -> bool {
    let host = host.trim_start_matches("www.").to_ascii_lowercase();
    let blocked = blocked.trim_start_matches("www.").to_ascii_lowercase();
    host_under_domain(&host, &blocked)
}

pub async fn fetch_blocklist(
    pool: Option<&PgPool>,
    guild_id: Option<serenity::GuildId>,
) -> Vec<String> {
    let Some(pool) = pool else {
        return Vec::new();
    };
    let guild_id_str = guild_id.map(|g| g.to_string());
    match db::fetch_blocked_domains(pool, guild_id_str.as_deref()).await {
        Ok(b) => b,
        Err(e) => {
            warn!("Failed to fetch blocked domains: {}", e);
            Vec::new()
        }
    }
}

pub fn is_url_in_blocklist(url: &str, blocklist: &[String]) -> bool {
    if blocklist.is_empty() {
        return false;
    }
    let Some(host) = extract_host(url) else {
        return false;
    };
    blocklist.iter().any(|b| host_matches_blocked(host, b))
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
                    SettingCheck::Bsky => settings.bsky_enabled,
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
