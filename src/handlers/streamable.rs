use super::shared::{self, SettingCheck};
use super::streamable_handler::{self, StreamablePost};
use crate::constants::{
    FAILURE_FETCH, FAILURE_SEND, STREAMABLE_ACCENT_COLOR, STREAMABLE_DOWNLOAD_UA,
};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_streamable_url(word: &str) -> bool {
    streamable_handler::matches_streamable_host(word)
}

pub async fn build_container(
    post: &StreamablePost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut components: Vec<serenity::CreateContainerComponent<'static>> = Vec::new();
    if !post.title.is_empty() {
        components.push(serenity::CreateContainerComponent::TextDisplay(
            serenity::CreateTextDisplay::new(format!("**{}**", post.title)),
        ));
    }

    let mut attachments = Vec::new();
    let gallery_url = if post.attach {
        match shared::download_media(&post.video_url, STREAMABLE_DOWNLOAD_UA).await {
            Some(data) => {
                let filename =
                    shared::media_filename("streamable", 0, &post.video_url, Some("mp4"));
                let attachment_url = format!("attachment://{}", filename);
                attachments.push(serenity::CreateAttachment::bytes(data, filename));
                Some(attachment_url)
            }
            None => {
                warn!("Failed to download Streamable video: {}", post.video_url);
                None
            }
        }
    } else {
        Some(post.video_url.clone())
    };

    if let Some(url) = gallery_url {
        components.push(serenity::CreateContainerComponent::MediaGallery(
            serenity::CreateMediaGallery::new(vec![serenity::CreateMediaGalleryItem::new(
                serenity::CreateUnfurledMediaItem::new(url),
            )]),
        ));
    }

    components.push(serenity::CreateContainerComponent::TextDisplay(
        serenity::CreateTextDisplay::new(format!("-# Sent by <@{}>", user_id)),
    ));

    let container =
        serenity::CreateContainer::new(components).accent_color(STREAMABLE_ACCENT_COLOR);
    (attachments, container)
}

pub async fn handle_streamable_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_streamable_url(word) {
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

    if !shared::pre_check(msg, pool, SettingCheck::Streamable).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "streamable") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    let mut failure: Option<&'static str> = None;
    for url in found_urls {
        debug!("Fetching Streamable: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || StreamablePost::fetch(&url_owned)).await {
            Ok(post) => {
                let (attachments, container) = build_container(&post, msg.author.id).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                if shared::send_reply(ctx, msg, "streamable", message).await {
                    any_sent = true;
                } else {
                    failure = Some(FAILURE_SEND);
                }
            }
            Err(e) => {
                warn!("Failed to fetch Streamable {}: {}", url, e);
                shared::report_failure(
                    ctx,
                    msg.guild_id,
                    "streamable",
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
        shared::notify_failure(ctx, msg, "streamable", code).await;
    }
}
