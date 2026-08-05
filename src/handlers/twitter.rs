use super::shared::{self, SettingCheck};
use super::twitter_handler::{self, TwitterError, TwitterPost};
use crate::constants::{FAILURE_FETCH, FAILURE_SEND, TWITTER_ACCENT_COLOR, TWITTER_DOWNLOAD_UA};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

fn is_twitter_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if let Some(host) = shared::extract_host(&lower)
        && shared::host_under_domain(host, "t.co")
    {
        return true;
    }
    twitter_handler::matches_twitter_host(word) && lower.contains("/status/")
}

pub async fn build_container(
    post: &TwitterPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut content = format!("**{}** (@{})", post.author, post.username);

    if let Some(replying_to) = &post.replying_to {
        content.push_str(&format!("\nReplying to @{}", replying_to));
    }

    if !post.text.is_empty() {
        content.push_str(&format!("\n{}", post.text));
    }

    if let Some(quote_author) = &post.quote_author
        && let (Some(quote_username), Some(quote_text)) = (&post.quote_username, &post.quote_text)
    {
        let quoted_lines = quote_text.replace('\n', "\n> ");
        content.push_str(&format!(
            "\n\n> **{}** (@{})\n> {}",
            quote_author, quote_username, quoted_lines
        ));
    }

    let mut components: Vec<serenity::CreateContainerComponent<'static>> =
        vec![serenity::CreateContainerComponent::TextDisplay(
            serenity::CreateTextDisplay::new(content),
        )];

    let mut attachments = Vec::new();
    let mut gallery_items: Vec<serenity::CreateMediaGalleryItem<'static>> = Vec::new();
    for (i, item) in post.media.iter().take(10).enumerate() {
        let Some(data) = shared::download_media(&item.url, TWITTER_DOWNLOAD_UA).await else {
            warn!("Failed to download media: {}", item.url);
            continue;
        };

        let (data, ext) = if item.is_gif {
            match shared::mp4_to_gif(data.clone()).await {
                Some(gif) => (gif, Some("gif")),
                None => (data, Some("mp4")),
            }
        } else {
            (data, None)
        };

        let filename = shared::media_filename("twitter", i, &item.url, ext);
        let attachment_url = format!("attachment://{}", filename);
        attachments.push(serenity::CreateAttachment::bytes(data, filename));
        gallery_items.push(serenity::CreateMediaGalleryItem::new(
            serenity::CreateUnfurledMediaItem::new(attachment_url),
        ));
    }

    if !gallery_items.is_empty() {
        components.push(serenity::CreateContainerComponent::MediaGallery(
            serenity::CreateMediaGallery::new(gallery_items),
        ));
    }

    components.push(serenity::CreateContainerComponent::TextDisplay(
        serenity::CreateTextDisplay::new(format!("-# Sent by <@{}>", user_id)),
    ));

    let container = serenity::CreateContainer::new(components).accent_color(TWITTER_ACCENT_COLOR);
    (attachments, container)
}

pub async fn handle_twitter_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_twitter_url(word) {
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

    if !shared::pre_check(msg, pool, SettingCheck::Twitter).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "twitter") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    let mut failure: Option<&'static str> = None;
    for url in found_urls {
        debug!("Fetching tweet: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || TwitterPost::fetch(&url_owned)).await {
            Ok(post) => {
                let (attachments, container) = build_container(&post, msg.author.id).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                if shared::send_reply(ctx, msg, "twitter", message).await {
                    any_sent = true;
                } else {
                    failure = Some(FAILURE_SEND);
                }
            }
            Err(e) => {
                warn!("Failed to fetch tweet {}: {}", url, e);
                let code = e
                    .downcast_ref::<TwitterError>()
                    .map_or(FAILURE_FETCH, TwitterError::code);
                shared::report_failure(
                    ctx,
                    msg.guild_id,
                    "twitter",
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
        shared::notify_failure(ctx, msg, "twitter", code).await;
    }
}
