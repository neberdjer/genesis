use super::git_diff_handler::CommitDiff;
use super::shared::{self, SettingCheck};
use crate::constants::{DISCORD_MESSAGE_LIMIT, FILES_PER_PAGE};
use crate::db;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use tracing::{error, warn};

static DIFF_CACHE: OnceLock<Mutex<HashMap<String, CachedDiff>>> = OnceLock::new();

const MAX_CACHE_ENTRIES: usize = 1000;

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
    let kind = if commit.is_compare { "cmp" } else { "cmt" };
    let base = match &commit.host {
        Some(host) => format!(
            "{}:{}:{}:{}:{}",
            kind, host, commit.owner, commit.repo, commit.commit
        ),
        None => format!(
            "{}:github.com:{}:{}:{}",
            kind, commit.owner, commit.repo, commit.commit
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
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.retain(|_, v| !v.is_expired());
            if cache.len() >= MAX_CACHE_ENTRIES
                && let Some(oldest_key) = cache
                    .iter()
                    .min_by_key(|(_, v)| v.timestamp)
                    .map(|(k, _)| k.clone())
            {
                cache.remove(&oldest_key);
            }
        }
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
        .trim_end_matches([',', ')', ']', '}', ';'])
}

fn extract_sent_by_footer(content: &str) -> Option<String> {
    let marker = "\n-# Sent by <@";
    let start = content.rfind(marker)?;
    let rest = &content[start..];
    let end = rest.find('>')?;
    Some(rest[..=end].to_string())
}

pub fn create_pagination_buttons(
    current_page: usize,
    total_pages: usize,
    commit: &CommitDiff,
    lock_user_id: Option<u64>,
) -> serenity::CreateActionRow<'_> {
    let platform_prefix = match (commit.platform, commit.is_compare) {
        (super::git_diff_handler::GitPlatform::GitHub, false) => "gh",
        (super::git_diff_handler::GitPlatform::GitHub, true) => "gH",
        (super::git_diff_handler::GitPlatform::GitLab, false) => "gl",
        (super::git_diff_handler::GitPlatform::GitLab, true) => "gL",
    };

    let host_part = commit.host.as_deref().unwrap_or("");
    let separator = if host_part.is_empty() { "" } else { "|" };

    let file_filter_part = commit.file_filter.as_deref().unwrap_or("");
    let file_separator = if file_filter_part.is_empty() { "" } else { "|" };

    let lock_suffix = lock_user_id
        .map(|id| format!(":{}", id))
        .unwrap_or_default();

    let prev_button = serenity::CreateButton::new(format!(
        "diff:prev:{}{}{}:{}:{}:{}{}{}_{}{}",
        platform_prefix,
        separator,
        host_part,
        commit.owner,
        commit.repo,
        commit.commit,
        file_separator,
        file_filter_part,
        current_page,
        lock_suffix
    ))
    .label("Previous")
    .style(serenity::ButtonStyle::Primary)
    .disabled(current_page == 0);

    let page_info = serenity::CreateButton::new("page_info")
        .label(format!("{}/{}", current_page + 1, total_pages))
        .style(serenity::ButtonStyle::Secondary)
        .disabled(true);

    let next_button = serenity::CreateButton::new(format!(
        "diff:next:{}{}{}:{}:{}:{}{}{}_{}{}",
        platform_prefix,
        separator,
        host_part,
        commit.owner,
        commit.repo,
        commit.commit,
        file_separator,
        file_filter_part,
        current_page,
        lock_suffix
    ))
    .label("Next")
    .style(serenity::ButtonStyle::Primary)
    .disabled(current_page >= total_pages - 1);

    serenity::CreateActionRow::Buttons(vec![prev_button, page_info, next_button].into())
}

async fn send_paginated_diff(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    commit: &CommitDiff,
    responses: Vec<String>,
) -> bool {
    if responses.is_empty() {
        return false;
    }

    let total_pages = responses.len();
    let footer = format!("\n-# Sent by <@{}>", msg.author.id);
    let first_page = &responses[0];

    if first_page.len() + footer.len() > DISCORD_MESSAGE_LIMIT {
        warn!("Diff response too long, skipping");
        return false;
    }

    let content = format!("{}{}", first_page, footer);
    let mut message_builder = serenity::CreateMessage::new()
        .content(content)
        .reference_message(msg)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));

    if total_pages > 1 {
        let buttons = create_pagination_buttons(0, total_pages, commit, None);
        message_builder =
            message_builder.components(vec![serenity::CreateComponent::ActionRow(buttons)]);
    }

    match msg
        .channel_id
        .send_message(&ctx.http, message_builder)
        .await
    {
        Ok(_) => true,
        Err(e) => {
            error!("Failed to send diff message: {}", e);
            false
        }
    }
}

pub async fn handle_commit_diffs(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if !msg.content.split_whitespace().any(is_commit_url) {
        return;
    }

    if !shared::pre_check(msg, pool, SettingCheck::GitDiffs).await {
        return;
    }

    let git_compares_enabled = if let Some(pool) = pool
        && let Some(guild_id) = msg.guild_id
    {
        db::get_server_settings(pool, &guild_id.to_string())
            .await
            .map(|s| s.git_compares_enabled)
            .unwrap_or(true)
    } else {
        true
    };

    let blocklist = shared::fetch_blocklist(pool, msg.guild_id).await;

    handle_commit_diffs_impl(ctx, msg, git_compares_enabled, &blocklist).await;
}

async fn handle_commit_diffs_impl(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    git_compares_enabled: bool,
    blocklist: &[String],
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

    if !shared::check_rate_limit(msg.author.id, "git_diffs") {
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
            if let Some(&next_word) = words.get(idx + 1)
                && looks_like_filename(next_word)
                && (idx + 1 == words.len() || commit_url_indices.contains(&(idx + 2)))
            {
                commit.file_filter = Some(next_word.to_string());
            }
            found_commits.push(commit);
        }
    }

    if found_commits.is_empty() {
        return;
    }

    if !blocklist.is_empty() {
        found_commits.retain(|commit| {
            let host = commit.host.as_deref().unwrap_or("github.com");
            !blocklist
                .iter()
                .any(|b| shared::host_matches_blocked(host, b))
        });
        if found_commits.is_empty() {
            return;
        }
    }

    if !git_compares_enabled {
        found_commits.retain(|commit| !commit.is_compare);
        if found_commits.is_empty() {
            return;
        }
    }

    let mut any_sent = false;
    for commit in found_commits {
        let chunked = match fetch_or_cached(&commit).await {
            Ok(chunked) => chunked,
            Err(e) => {
                warn!("Failed to fetch commit diff: {}", e);
                continue;
            }
        };

        if send_paginated_diff(ctx, msg, &commit, chunked).await {
            any_sent = true;
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}

pub async fn fetch_or_cached(
    commit: &CommitDiff,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    if let Some(cached) = get_cached_responses(commit) {
        return Ok(cached);
    }
    let chunked = fetch_and_chunk(commit).await?;
    cache_responses(commit, chunked.clone());
    Ok(chunked)
}

async fn fetch_and_chunk(
    commit: &CommitDiff,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let platform = commit.platform;
    let owner = commit.owner.clone();
    let repo = commit.repo.clone();
    let commit_str = commit.commit.clone();
    let host = commit.host.clone();
    let file_filter = commit.file_filter.clone();
    let is_compare = commit.is_compare;

    tokio::task::spawn_blocking(move || {
        let c = CommitDiff {
            platform,
            owner,
            repo,
            commit: commit_str,
            diff_hash: None,
            host,
            file_filter,
            is_compare,
        };
        let responses = c.format_diff_response()?;
        let chunked: Vec<String> = responses
            .chunks(FILES_PER_PAGE)
            .map(|chunk| chunk.join("\n\n"))
            .collect();
        Ok(chunked)
    })
    .await
    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
}

fn looks_like_filename(word: &str) -> bool {
    word.contains('.') && !word.starts_with("http")
}

pub async fn handle_diff_pagination(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
) {
    let custom_id = &interaction.data.custom_id;

    if !custom_id.starts_with("diff:prev:") && !custom_id.starts_with("diff:next:") {
        return;
    }

    let parts: Vec<&str> = custom_id.splitn(3, ':').collect();
    if parts.len() < 3 {
        return;
    }

    let direction = parts[1];
    let rest = parts[2];

    // {platform}{|host}:{owner}:{repo}:{commit}{|filter}_{page}
    let colon_parts: Vec<&str> = rest.splitn(4, ':').collect();
    if colon_parts.len() < 4 {
        return;
    }

    let platform_and_host = colon_parts[0];
    let owner = colon_parts[1];
    let repo = colon_parts[2];

    let (commit_filter_page, lock_user_id) = match colon_parts[3].rsplit_once(':') {
        Some((cfp, uid)) if uid.parse::<u64>().is_ok() => (cfp, uid.parse::<u64>().ok()),
        _ => (colon_parts[3], None),
    };

    if let Some(lock_uid) = lock_user_id
        && interaction.user.id.get() != lock_uid
    {
        let response = serenity::CreateInteractionResponse::Message(
            serenity::CreateInteractionResponseMessage::new()
                .content("Only the original poster can navigate this diff.")
                .ephemeral(true),
        );
        let _ = interaction.create_response(&ctx.http, response).await;
        return;
    }

    let (platform, is_compare, host) = if let Some(rest) = platform_and_host.strip_prefix("gh") {
        (
            super::git_diff_handler::GitPlatform::GitHub,
            false,
            rest.strip_prefix('|').map(|s| s.to_string()),
        )
    } else if let Some(rest) = platform_and_host.strip_prefix("gH") {
        (
            super::git_diff_handler::GitPlatform::GitHub,
            true,
            rest.strip_prefix('|').map(|s| s.to_string()),
        )
    } else if let Some(rest) = platform_and_host.strip_prefix("gl") {
        let host = rest
            .strip_prefix('|')
            .map(|s| s.to_string())
            .or_else(|| Some("gitlab.com".to_string()));
        (super::git_diff_handler::GitPlatform::GitLab, false, host)
    } else if let Some(rest) = platform_and_host.strip_prefix("gL") {
        let host = rest
            .strip_prefix('|')
            .map(|s| s.to_string())
            .or_else(|| Some("gitlab.com".to_string()));
        (super::git_diff_handler::GitPlatform::GitLab, true, host)
    } else {
        return;
    };

    // {commit}{|filter}_{page}
    let (commit_and_filter, page_str) = match commit_filter_page.rsplit_once('_') {
        Some((cf, p)) => (cf, p),
        None => return,
    };

    let current_page: usize = match page_str.parse() {
        Ok(p) => p,
        Err(_) => return,
    };

    let (commit_sha, file_filter) = if let Some((c, f)) = commit_and_filter.split_once('|') {
        (c, Some(f.to_string()))
    } else {
        (commit_and_filter, None)
    };

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
        is_compare,
    };

    let chunked = match fetch_or_cached(&commit).await {
        Ok(chunked) => chunked,
        Err(e) => {
            warn!("Failed to fetch commit diff for pagination: {}", e);
            return;
        }
    };

    if new_page >= chunked.len() {
        return;
    }

    let page_content = &chunked[new_page];
    let total_pages = chunked.len();

    let buttons = create_pagination_buttons(new_page, total_pages, &commit, lock_user_id);

    let footer = extract_sent_by_footer(&interaction.message.content);
    let content = match footer {
        Some(f) => format!("{}{}", page_content, f),
        None => page_content.to_string(),
    };

    let response = serenity::CreateInteractionResponse::UpdateMessage(
        serenity::CreateInteractionResponseMessage::new()
            .content(content)
            .components(vec![serenity::CreateComponent::ActionRow(buttons)]),
    );

    if let Err(e) = interaction.create_response(&ctx.http, response).await {
        error!("Failed to update message: {}", e);
    }
}
