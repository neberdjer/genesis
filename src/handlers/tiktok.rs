use super::shared::{self, SettingCheck};
use super::tiktok_handler::TikTokPost;
use crate::constants::{TIKTOK_ACCENT_COLOR, TIKTOK_DOWNLOAD_UA};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_tiktok_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let Some(scheme_end) = lower.find("://") else {
        return false;
    };
    let after_scheme = &lower[scheme_end + 3..];
    let host = after_scheme.split('/').next().unwrap_or("");
    let normalized = host
        .trim_start_matches("www.")
        .trim_start_matches("vm.")
        .trim_start_matches("vt.")
        .trim_start_matches("m.");
    normalized == "tiktok.com"
}

fn clean_url(url: &str) -> &str {
    let trimmed = url.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

fn media_filename(index: usize, url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    let ext = if lower.contains(".jpeg") || lower.contains(".jpg") {
        "jpg"
    } else if lower.contains(".webp") {
        "webp"
    } else if lower.contains(".png") {
        "png"
    } else if lower.contains(".mp4") || lower.contains("/play/") || lower.contains("playwm") {
        "mp4"
    } else {
        "jpg"
    };
    format!("tiktok_{}.{}", index, ext)
}

pub async fn build_container(
    post: &TikTokPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut content = format!("**{}** (@{})", post.author, post.username);
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
        if let Some(data) = shared::download_media(media_url, TIKTOK_DOWNLOAD_UA).await {
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

    let container = serenity::CreateContainer::new(components).accent_color(TIKTOK_ACCENT_COLOR);
    (attachments, container)
}

pub async fn handle_tiktok_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_videos: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_tiktok_url(word) {
            continue;
        }
        if let Some(parsed) = TikTokPost::parse(clean_url(word))
            && seen.insert(parsed.clone())
        {
            found_videos.push(parsed);
        }
    }

    if found_videos.is_empty() {
        return;
    }

    let blocklist = shared::fetch_blocklist(pool, msg.guild_id).await;
    found_videos.retain(|url| !shared::is_url_in_blocklist(url, &blocklist));
    if found_videos.is_empty() {
        return;
    }

    if !shared::pre_check(msg, pool, SettingCheck::TikTok).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "tiktok") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for video_url in found_videos {
        debug!("Fetching TikTok: url={}", video_url);
        let url = video_url.clone();
        match shared::spawn_blocking_fetch(move || TikTokPost::fetch(&url)).await {
            Ok(post) => {
                let (attachments, container) = build_container(&post, msg.author.id).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                if shared::send_reply(ctx, msg, message).await {
                    any_sent = true;
                }
            }
            Err(e) => warn!("Failed to fetch TikTok {}: {}", video_url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
