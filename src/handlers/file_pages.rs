use super::git_handler::{FileResponse, GitFileLink};
use super::pagination;
use super::shared;
use crate::constants::{FILE_CACHE_MAX_ENTRIES, PAGE_CACHE_TTL_SECONDS};
use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static FILE_CACHE: OnceLock<Mutex<HashMap<String, CachedFile>>> = OnceLock::new();

struct CachedFile {
    link: GitFileLink,
    pages: Vec<String>,
    timestamp: Instant,
}

fn file_cache() -> &'static Mutex<HashMap<String, CachedFile>> {
    FILE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(raw_url: &str) -> String {
    let mut hasher = DefaultHasher::new();
    raw_url.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

pub fn cache_pages(link: &GitFileLink, pages: Vec<String>) -> String {
    let key = cache_key(&link.raw_url);
    if let Ok(mut cache) = file_cache().lock() {
        for entry in cache.values_mut() {
            if pagination::is_cache_expired(entry.timestamp) {
                entry.pages = Vec::new();
            }
        }
        shared::evict_for_insert(
            &mut cache,
            FILE_CACHE_MAX_ENTRIES,
            PAGE_CACHE_TTL_SECONDS,
            |e| e.timestamp,
        );
        cache.insert(
            key.clone(),
            CachedFile {
                link: link.clone(),
                pages,
                timestamp: Instant::now(),
            },
        );
    }
    key
}

async fn page_for(key: &str, page: usize) -> Option<(String, usize, usize)> {
    let link = {
        let cache = file_cache().lock().ok()?;
        let entry = cache.get(key)?;
        if !pagination::is_cache_expired(entry.timestamp) && !entry.pages.is_empty() {
            let total = entry.pages.len();
            let page = page.min(total - 1);
            return Some((entry.pages[page].clone(), page, total));
        }
        entry.link.clone()
    };

    let refreshed = shared::spawn_blocking_fetch(move || link.format_response())
        .await
        .ok()??;
    let pages = match refreshed {
        FileResponse::Paged(pages) if !pages.is_empty() => pages,
        FileResponse::Single(snippet) => vec![snippet],
        _ => return None,
    };

    let total = pages.len();
    let page = page.min(total - 1);
    let content = pages[page].clone();
    if let Ok(mut cache) = file_cache().lock()
        && let Some(entry) = cache.get_mut(key)
    {
        entry.pages = pages;
        entry.timestamp = Instant::now();
    }
    Some((content, page, total))
}

pub fn create_file_pagination_buttons(
    key: &str,
    current_page: usize,
    total_pages: usize,
    lock_user_id: Option<u64>,
) -> serenity::CreateActionRow<'static> {
    let lock_suffix = lock_user_id
        .map(|id| format!(":{}", id))
        .unwrap_or_default();
    let payload = format!("{}_{}{}", key, current_page, lock_suffix);

    pagination::pagination_row(
        format!("file:prev:{}", payload),
        format!("file:next:{}", payload),
        current_page,
        total_pages,
    )
}

pub async fn send_paginated_file(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    link: &GitFileLink,
    pages: Vec<String>,
) -> bool {
    if pages.is_empty() {
        return false;
    }

    let total_pages = pages.len();
    let first_page = pages[0].clone();
    let key = cache_pages(link, pages);
    let buttons =
        (total_pages > 1).then(|| create_file_pagination_buttons(&key, 0, total_pages, None));

    pagination::send_first_page(ctx, msg, "git_links", &first_page, buttons).await
}

pub async fn handle_file_pagination(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
) {
    let custom_id = &interaction.data.custom_id;

    if !custom_id.starts_with("file:prev:") && !custom_id.starts_with("file:next:") {
        return;
    }

    let parts: Vec<&str> = custom_id.splitn(3, ':').collect();
    if parts.len() < 3 {
        return;
    }

    let direction = parts[1];
    let (key_and_page, lock_user_id) = pagination::split_lock_suffix(parts[2]);

    if let Some(lock_uid) = lock_user_id
        && interaction.user.id.get() != lock_uid
    {
        pagination::respond_ephemeral(
            ctx,
            interaction,
            "Only the original poster can navigate this file.",
        )
        .await;
        return;
    }

    let (key, page_str) = match key_and_page.rsplit_once('_') {
        Some((k, p)) => (k, p),
        None => return,
    };

    let current_page: usize = match page_str.parse() {
        Ok(p) => p,
        Err(_) => return,
    };

    let new_page = match direction {
        "prev" => current_page.saturating_sub(1),
        "next" => current_page + 1,
        _ => return,
    };

    let Some((page_content, page, total_pages)) = page_for(key, new_page).await else {
        pagination::respond_ephemeral(
            ctx,
            interaction,
            "This file view has expired. Post the link again.",
        )
        .await;
        return;
    };

    let buttons = create_file_pagination_buttons(key, page, total_pages, lock_user_id);
    pagination::update_to_page(ctx, interaction, &page_content, buttons).await;
}
