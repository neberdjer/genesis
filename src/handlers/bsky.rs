use super::bsky_handler::{self, BskyPost};
use super::shared::{self, MediaGallery, SettingCheck};
use crate::constants::{BSKY_ACCENT_COLOR, BSKY_DOWNLOAD_UA, FAILURE_FETCH};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_bsky_url(word: &str) -> bool {
    bsky_handler::matches_bsky_host(word) && word.contains("/post/")
}

pub async fn build_container(
    post: &BskyPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let header = if post.author == post.handle {
        format!("**@{}**", post.handle)
    } else {
        format!("**{}** (@{})", post.author, post.handle)
    };

    let mut trailer = String::new();
    if let Some((title, uri)) = &post.external {
        trailer.push_str(&format!("\n\n[{}]({})", title, uri));
    }
    if let Some(quote_author) = &post.quote_author {
        let quoted = post
            .quote_text
            .as_deref()
            .unwrap_or("")
            .replace('\n', "\n> ");
        trailer.push_str(&format!("\n\n> **{}**\n> {}", quote_author, quoted));
    }

    let mut gallery = MediaGallery::default();
    for (i, item) in post.media.iter().enumerate() {
        let ext = item.is_video.then_some("mp4");
        let filename = shared::media_filename("bsky", i, &item.url, ext);
        gallery.add(&item.url, BSKY_DOWNLOAD_UA, filename).await;
    }

    shared::post_container(
        header,
        &post.text,
        &trailer,
        gallery,
        user_id,
        BSKY_ACCENT_COLOR,
    )
}

pub async fn handle_bsky_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_bsky_url(word) {
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

    if !shared::pre_check(msg, pool, SettingCheck::Bsky).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "bsky") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    let mut failure: Option<&'static str> = None;
    for url in found_urls {
        debug!("Fetching Bluesky post: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || BskyPost::fetch(&url_owned)).await {
            Ok(post) => {
                let (attachments, container) = build_container(&post, msg.author.id).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                match shared::send_reply(ctx, msg, "bsky", message).await {
                    None => any_sent = true,
                    Some(code) => failure = Some(code),
                }
            }
            Err(e) => {
                warn!("Failed to fetch Bluesky post {}: {}", url, e);
                shared::report_failure(
                    ctx,
                    msg.guild_id,
                    "bsky",
                    FAILURE_FETCH,
                    Some(&url),
                    &e.to_string(),
                );
                failure = Some(FAILURE_FETCH);
            }
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
    if let Some(code) = failure {
        shared::notify_failure(ctx, msg, "bsky", code).await;
    }
}
