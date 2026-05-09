use super::shared::{self, SettingCheck};
use super::tiktok_handler::TikTokPost;
use crate::constants::TIKTOK_DOWNLOAD_UA;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

const TIKTOK_ACCENT_COLOR: u32 = 0x000000;

fn is_tiktok_url(word: &str) -> bool {
    word.contains("tiktok.com")
}

fn clean_url(url: &str) -> &str {
    url.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

fn media_filename(index: usize, url: &str) -> String {
    let ext = if url.contains(".mp4") || url.contains("video") {
        "mp4"
    } else if url.contains(".webp") {
        "webp"
    } else {
        "jpg"
    };
    format!("tiktok_{}.{}", index, ext)
}

pub async fn build_container(
    post: &TikTokPost,
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
    for (i, media_url) in post.media.iter().enumerate() {
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

    let container = serenity::CreateContainer::new(components).accent_color(TIKTOK_ACCENT_COLOR);
    (attachments, container)
}

pub async fn handle_tiktok_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    if !shared::pre_check(msg, pool, SettingCheck::TikTok).await {
        return;
    }

    let found_videos: Vec<String> = msg
        .content
        .split_whitespace()
        .filter(|word| is_tiktok_url(word))
        .filter_map(|word| TikTokPost::parse(clean_url(word)))
        .collect();

    if found_videos.is_empty() {
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
                let (attachments, container) = build_container(&post).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                match msg.channel_id.send_message(&ctx.http, message).await {
                    Ok(_) => any_sent = true,
                    Err(e) => warn!("Failed to send TikTok message: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch TikTok {}: {}", video_url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
