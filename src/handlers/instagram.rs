use super::instagram_handler::{InstagramError, InstagramPost};
use super::shared::{self, MediaGallery, SettingCheck};
use crate::constants::{
    FAILURE_FETCH, INSTAGRAM_ACCENT_COLOR, INSTAGRAM_DESKTOP_UA, INSTAGRAM_MIRROR_UA,
    INSTAGRAM_MIRRORS,
};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

fn ua_for_media_url(url: &str) -> &'static str {
    if INSTAGRAM_MIRRORS.iter().any(|m| url.contains(m)) {
        INSTAGRAM_MIRROR_UA
    } else {
        INSTAGRAM_DESKTOP_UA
    }
}

fn is_instagram_url(word: &str) -> bool {
    if !super::instagram_handler::matches_instagram_host(word) {
        return false;
    }
    let lower = word.to_ascii_lowercase();
    lower.contains("/p/")
        || lower.contains("/reel/")
        || lower.contains("/reels/")
        || lower.contains("/tv/")
        || lower.contains("/share/")
}

pub async fn build_container(
    post: &InstagramPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let header = match (post.author.is_empty(), post.username.is_empty()) {
        (true, true) => "**Instagram**".to_string(),
        (false, true) => format!("**{}**", shared::escape_markdown(&post.author)),
        _ => shared::author_header(&post.author, &post.username),
    };

    let mut gallery = MediaGallery::default();
    for (i, media_url) in post.media.iter().enumerate() {
        let filename = shared::media_filename("instagram", i, media_url, None);
        gallery
            .add(media_url, ua_for_media_url(media_url), filename)
            .await;
    }

    shared::post_container(
        header,
        &shared::escape_text(&post.text),
        "",
        gallery,
        user_id,
        INSTAGRAM_ACCENT_COLOR,
    )
}

pub async fn handle_instagram_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_instagram_url(word) {
            continue;
        }
        let cleaned = shared::clean_media_url(word).to_string();
        if seen.insert(cleaned.clone()) {
            found_urls.push(cleaned);
        }
    }

    if found_urls.is_empty() {
        return;
    }

    let blocklist = shared::fetch_blocklist(pool, msg.guild_id).await;
    found_urls.retain(|url| !shared::is_url_in_blocklist(url, &blocklist));
    if found_urls.is_empty() {
        return;
    }

    if !shared::pre_check(msg, pool, SettingCheck::Instagram).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "instagram") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    let mut failure: Option<&'static str> = None;
    for url in found_urls {
        debug!("Fetching Instagram post: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || InstagramPost::fetch(&url_owned)).await {
            Ok(post) => {
                let (attachments, container) = build_container(&post, msg.author.id).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                match shared::send_reply(ctx, msg, "instagram", message).await {
                    None => any_sent = true,
                    Some(code) => failure = Some(code),
                }
            }
            Err(e) => {
                warn!("Failed to fetch Instagram {}: {}", url, e);
                let code = e
                    .downcast_ref::<InstagramError>()
                    .map_or(FAILURE_FETCH, InstagramError::code);
                shared::report_failure(
                    ctx,
                    msg.guild_id,
                    "instagram",
                    code,
                    Some(&url),
                    &e.to_string(),
                );
                failure = Some(code);
            }
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
    if let Some(code) = failure {
        shared::notify_failure(ctx, msg, "instagram", code).await;
    }
}
