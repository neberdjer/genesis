use super::instagram_handler::InstagramPost;
use super::shared::{self, SettingCheck};
use crate::constants::{
    INSTAGRAM_ACCENT_COLOR, INSTAGRAM_DESKTOP_UA, INSTAGRAM_MIRROR_UA, INSTAGRAM_MIRRORS,
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
    let lower = word.to_ascii_lowercase();
    if !lower.contains("instagram.com") {
        return false;
    }
    lower.contains("/p/")
        || lower.contains("/reel/")
        || lower.contains("/reels/")
        || lower.contains("/tv/")
        || lower.contains("/share/")
}

fn clean_url(url: &str) -> &str {
    let trimmed = url.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

fn media_filename(index: usize, url: &str) -> String {
    let ext = if url.contains(".mp4") || url.contains("video") {
        "mp4"
    } else if url.contains(".webp") {
        "webp"
    } else {
        "jpg"
    };
    format!("instagram_{}.{}", index, ext)
}

pub async fn build_container(
    post: &InstagramPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut content = if post.username.is_empty() {
        if post.author.is_empty() {
            "**Instagram**".to_string()
        } else {
            format!("**{}**", post.author)
        }
    } else if post.author.is_empty() || post.author == post.username {
        format!("**@{}**", post.username)
    } else {
        format!("**{}** (@{})", post.author, post.username)
    };
    if !post.text.is_empty() {
        content.push_str(&format!("\n{}", post.text));
    }

    let mut components: Vec<serenity::CreateContainerComponent<'static>> =
        vec![serenity::CreateContainerComponent::TextDisplay(
            serenity::CreateTextDisplay::new(content),
        )];

    let mut attachments = Vec::new();
    let mut gallery_items: Vec<serenity::CreateMediaGalleryItem<'static>> = Vec::new();
    for (i, media_url) in post.media.iter().take(10).enumerate() {
        let filename = media_filename(i, media_url);
        if let Some(data) = shared::download_media(media_url, ua_for_media_url(media_url)).await {
            let attachment_url = format!("attachment://{}", filename);
            attachments.push(serenity::CreateAttachment::bytes(data, filename));
            gallery_items.push(serenity::CreateMediaGalleryItem::new(
                serenity::CreateUnfurledMediaItem::new(attachment_url),
            ));
        } else {
            warn!("Failed to download media: {}", media_url);
        }
    }

    if !gallery_items.is_empty() {
        components.push(serenity::CreateContainerComponent::MediaGallery(
            serenity::CreateMediaGallery::new(gallery_items),
        ));
    }

    components.push(serenity::CreateContainerComponent::TextDisplay(
        serenity::CreateTextDisplay::new(format!("-# Sent by <@{}>", user_id)),
    ));

    let container = serenity::CreateContainer::new(components).accent_color(INSTAGRAM_ACCENT_COLOR);
    (attachments, container)
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
        let cleaned = clean_url(word).to_string();
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
    let mut any_failed = false;
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

                if shared::send_reply(ctx, msg, "instagram", message).await {
                    any_sent = true;
                } else {
                    any_failed = true;
                }
            }
            Err(e) => {
                warn!("Failed to fetch Instagram {}: {}", url, e);
                shared::record_embed(ctx, "instagram", false).await;
                any_failed = true;
            }
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
    if any_failed {
        shared::react_failure(ctx, msg).await;
    }
}
