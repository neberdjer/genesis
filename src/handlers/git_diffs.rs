use super::git_diff_handler::CommitDiff;
use crate::constants::{DISCORD_MESSAGE_LIMIT, EMBED_SUPPRESS_DELAY_MS, FILES_PER_PAGE};
use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{error, warn};

#[cfg(feature = "database")]
use crate::db;
#[cfg(feature = "database")]
use sqlx::PgPool;

static DIFF_CACHE: OnceLock<Mutex<HashMap<String, CachedDiff>>> = OnceLock::new();
static RATE_LIMIT: OnceLock<Mutex<HashMap<serenity::UserId, Instant>>> = OnceLock::new();

const RATE_LIMIT_SECONDS: u64 = 10;

struct CachedDiff {
    responses: Vec<String>,
    timestamp: Instant,
}

impl CachedDiff {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            timestamp: Instant::now(),
        }
    }

    fn is_expired(&self) -> bool {
        self.timestamp.elapsed().as_secs() > 600
    }
}

fn get_cache_key(commit: &CommitDiff) -> String {
    let base = match &commit.host {
        Some(host) => format!(
            "{}:{}:{}:{}",
            host, commit.owner, commit.repo, commit.commit
        ),
        None => format!(
            "github.com:{}:{}:{}",
            commit.owner, commit.repo, commit.commit
        ),
    };

    match &commit.file_filter {
        Some(filter) => format!("{}:{}", base, filter),
        None => base,
    }
}

fn get_cached_responses(commit: &CommitDiff) -> Option<Vec<String>> {
    let cache = DIFF_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut cache = cache.lock().ok()?;
    let key = get_cache_key(commit);

    if let Some(cached) = cache.get(&key) {
        if !cached.is_expired() {
            return Some(cached.responses.clone());
        }
        cache.remove(&key);
    }
    None
}

fn cache_responses(commit: &CommitDiff, responses: Vec<String>) {
    let cache = DIFF_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache) = cache.lock() {
        let key = get_cache_key(commit);
        cache.insert(key, CachedDiff::new(responses));
    }
}

pub fn is_commit_url(word: &str) -> bool {
    is_github_commit_url(word) || is_gitlab_commit_url(word)
}

pub fn is_github_commit_url(word: &str) -> bool {
    word.contains("github.com") && (word.contains("/commit/") || word.contains("/compare/"))
}

pub fn is_gitlab_commit_url(word: &str) -> bool {
    word.contains("/-/commit/") || word.contains("/-/compare/")
}

pub fn clean_url(url: &str) -> &str {
    url.split('?')
        .next()
        .unwrap_or(url)
        .trim_end_matches(|c: char| matches!(c, ',' | ')' | ']' | '}' | ';'))
}

async fn suppress_embeds(ctx: &serenity::Context, msg: &serenity::Message) {
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

fn create_pagination_buttons(
    current_page: usize,
    total_pages: usize,
    commit: &CommitDiff,
) -> serenity::CreateActionRow {
    let platform_prefix = match commit.platform {
        super::git_diff_handler::GitPlatform::GitHub => "gh",
        super::git_diff_handler::GitPlatform::GitLab => "gl",
    };

    let host_part = commit.host.as_deref().unwrap_or("");
    let separator = if host_part.is_empty() { "" } else { "|" };

    let file_filter_part = commit.file_filter.as_deref().unwrap_or("");
    let file_separator = if file_filter_part.is_empty() { "" } else { "|" };

    let prev_button = serenity::CreateButton::new(format!(
        "diff_prev_{}{}{}_{}_{}_{}{}{}_{}",
        platform_prefix,
        separator,
        host_part,
        commit.owner,
        commit.repo,
        commit.commit,
        file_separator,
        file_filter_part,
        current_page
    ))
    .label("Previous")
    .style(serenity::ButtonStyle::Primary)
    .disabled(current_page == 0);

    let page_info = serenity::CreateButton::new("page_info")
        .label(format!("{}/{}", current_page + 1, total_pages))
        .style(serenity::ButtonStyle::Secondary)
        .disabled(true);

    let next_button = serenity::CreateButton::new(format!(
        "diff_next_{}{}{}_{}_{}_{}{}{}_{}",
        platform_prefix,
        separator,
        host_part,
        commit.owner,
        commit.repo,
        commit.commit,
        file_separator,
        file_filter_part,
        current_page
    ))
    .label("Next")
    .style(serenity::ButtonStyle::Primary)
    .disabled(current_page >= total_pages - 1);

    serenity::CreateActionRow::Buttons(vec![prev_button, page_info, next_button])
}

async fn send_paginated_diff(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    commit: &CommitDiff,
    responses: Vec<String>,
) {
    if responses.is_empty() {
        return;
    }

    let total_pages = responses.len();
    let first_page = &responses[0];

    if first_page.len() > DISCORD_MESSAGE_LIMIT {
        warn!("Diff response too long, skipping");
        return;
    }

    let mut message_builder = serenity::CreateMessage::new()
        .content(first_page)
        .reference_message(msg);

    if total_pages > 1 {
        let buttons = create_pagination_buttons(0, total_pages, commit);
        message_builder = message_builder.components(vec![buttons]);
    }

    if let Err(e) = msg
        .channel_id
        .send_message(&ctx.http, message_builder)
        .await
    {
        error!("Failed to send diff message: {}", e);
    }
}

#[cfg(feature = "database")]
pub async fn handle_commit_diffs(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if msg.author.bot {
        return;
    }

    let mut settings = None;
    if let Some(pool) = pool {
        match db::is_user_blacklisted(pool, &msg.author.id.to_string()).await {
            Ok(true) => return,
            Err(e) => warn!("Failed to check user blacklist: {}", e),
            _ => {}
        }

        if let Some(guild_id) = msg.guild_id {
            match db::is_server_blacklisted(pool, &guild_id.to_string()).await {
                Ok(true) => return,
                Err(e) => warn!("Failed to check server blacklist: {}", e),
                _ => {}
            }

            match db::get_server_settings(pool, &guild_id.to_string()).await {
                Ok(s) => {
                    if !s.git_diffs_enabled {
                        return;
                    }
                    settings = Some(s);
                }
                Err(e) => warn!("Failed to fetch server settings: {}", e),
            }
        }
    }

    handle_commit_diffs_impl(ctx, msg, settings.as_ref()).await;
}

#[cfg(not(feature = "database"))]
pub async fn handle_commit_diffs(ctx: &serenity::Context, msg: &serenity::Message) {
    if msg.author.bot {
        return;
    }

    handle_commit_diffs_impl(ctx, msg, None).await;
}

fn check_rate_limit(user_id: serenity::UserId) -> bool {
    let rate_limit = RATE_LIMIT.get_or_init(|| Mutex::new(HashMap::new()));
    let mut rate_limit = match rate_limit.lock() {
        Ok(guard) => guard,
        Err(_) => return true,
    };

    if let Some(last_time) = rate_limit.get(&user_id) {
        if last_time.elapsed().as_secs() < RATE_LIMIT_SECONDS {
            return false;
        }
    }

    rate_limit.insert(user_id, Instant::now());
    true
}

async fn handle_commit_diffs_impl(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    settings: Option<&db::ServerSettings>,
) {
    let words: Vec<&str> = msg.content.split_whitespace().collect();

    if words.is_empty() {
        return;
    }

    let commit_url_indices: Vec<usize> = words
        .iter()
        .enumerate()
        .filter(|(_, word)| is_commit_url(word))
        .map(|(i, _)| i)
        .collect();

    if commit_url_indices.is_empty() {
        return;
    }

    if !check_rate_limit(msg.author.id) {
        return;
    }

    let mut has_non_link_non_filename = false;
    for (i, word) in words.iter().enumerate() {
        if commit_url_indices.contains(&i) {
            continue;
        }

        if i > 0 && commit_url_indices.contains(&(i - 1)) && looks_like_filename(word) {
            continue;
        }

        has_non_link_non_filename = true;
        break;
    }

    if has_non_link_non_filename {
        return;
    }

    let mut found_commits: Vec<CommitDiff> = Vec::new();

    for &idx in &commit_url_indices {
        if let Some(mut commit) = CommitDiff::parse(clean_url(words[idx])) {
            if let Some(&next_word) = words.get(idx + 1) {
                if looks_like_filename(next_word)
                    && (idx + 1 == words.len() || commit_url_indices.contains(&(idx + 2)))
                {
                    commit.file_filter = Some(next_word.to_string());
                }
            }
            found_commits.push(commit);
        }
    }

    if found_commits.is_empty() {
        return;
    }

    #[cfg(feature = "database")]
    if let Some(settings) = settings {
        if !settings.git_compares_enabled {
            found_commits.retain(|commit| !commit.is_compare);
            if found_commits.is_empty() {
                return;
            }
        }
    }

    suppress_embeds(ctx, msg).await;

    for commit in found_commits {
        let chunked = if let Some(cached) = get_cached_responses(&commit) {
            cached
        } else {
            match commit.format_diff_response() {
                Ok(responses) => {
                    let chunked: Vec<String> = responses
                        .chunks(FILES_PER_PAGE)
                        .map(|chunk| chunk.join("\n\n"))
                        .collect();

                    cache_responses(&commit, chunked.clone());
                    chunked
                }
                Err(e) => {
                    warn!("Failed to fetch commit diff: {}", e);
                    continue;
                }
            }
        };

        send_paginated_diff(ctx, msg, &commit, chunked).await;
    }
}

fn looks_like_filename(word: &str) -> bool {
    word.contains('.') && !word.starts_with("http")
}

pub async fn handle_diff_pagination(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
) {
    let custom_id = &interaction.data.custom_id;

    if !custom_id.starts_with("diff_prev_") && !custom_id.starts_with("diff_next_") {
        return;
    }

    let parts: Vec<&str> = custom_id.split('_').collect();
    if parts.len() < 6 {
        return;
    }

    let direction = parts[1];
    let platform_and_host = parts[2];

    let (platform, host) = if platform_and_host.starts_with("gh") {
        (super::git_diff_handler::GitPlatform::GitHub, None)
    } else if platform_and_host.starts_with("gl") {
        if platform_and_host.contains('|') {
            let host_part = platform_and_host.strip_prefix("gl|").unwrap_or("");
            (
                super::git_diff_handler::GitPlatform::GitLab,
                Some(host_part.to_string()),
            )
        } else {
            (
                super::git_diff_handler::GitPlatform::GitLab,
                Some("gitlab.com".to_string()),
            )
        }
    } else {
        return;
    };

    let owner = parts[3];
    let repo = parts[4];

    let commit_and_filter = parts[5];
    let (commit_sha, file_filter) = if commit_and_filter.contains('|') {
        let mut split = commit_and_filter.splitn(2, '|');
        let commit = split.next().unwrap_or("");
        let filter = split.next().map(|s| s.to_string());
        (commit, filter)
    } else {
        (commit_and_filter, None)
    };

    let current_page: usize = parts[6].parse().unwrap_or(0);

    let new_page = match direction {
        "prev" => current_page.saturating_sub(1),
        "next" => current_page + 1,
        _ => return,
    };

    let commit = CommitDiff {
        platform,
        owner: owner.to_string(),
        repo: repo.to_string(),
        commit: commit_sha.to_string(),
        diff_hash: None,
        host,
        file_filter,
        is_compare: false,
    };

    let chunked = if let Some(cached) = get_cached_responses(&commit) {
        cached
    } else {
        match commit.format_diff_response() {
            Ok(responses) => {
                let chunked: Vec<String> = responses
                    .chunks(FILES_PER_PAGE)
                    .map(|chunk| chunk.join("\n\n"))
                    .collect();

                cache_responses(&commit, chunked.clone());
                chunked
            }
            Err(e) => {
                warn!("Failed to fetch commit diff for pagination: {}", e);
                return;
            }
        }
    };

    if new_page >= chunked.len() {
        return;
    }

    let page_content = &chunked[new_page];
    let total_pages = chunked.len();

    let buttons = create_pagination_buttons(new_page, total_pages, &commit);

    let response = serenity::CreateInteractionResponse::UpdateMessage(
        serenity::CreateInteractionResponseMessage::new()
            .content(page_content)
            .components(vec![buttons]),
    );

    if let Err(e) = interaction.create_response(&ctx.http, response).await {
        error!("Failed to update message: {}", e);
    }
}
