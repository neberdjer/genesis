use super::shared::{self, SettingCheck};
use super::twitter_handler::TwitterPost;
use crate::constants::TWITTER_DOWNLOAD_UA;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

const TWITTER_ACCENT_COLOR: u32 = 0x1DA1F2;

fn is_twitter_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if lower.contains("t.co/") {
        return true;
    }

    let Some(scheme_end) = lower.find("://") else {
        return false;
    };
    let after_scheme = &lower[scheme_end + 3..];
    let host = after_scheme.split('/').next().unwrap_or("");
    let normalized = host
        .trim_start_matches("www.")
        .trim_start_matches("mobile.")
        .trim_start_matches("m.");
    if normalized != "twitter.com" && normalized != "x.com" {
        return false;
    }

    lower.contains("/status/")
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
    format!("twitter_{}.{}", index, ext)
}

pub async fn build_container(
    post: &TwitterPost,
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
    for (i, media_url) in post.media.iter().take(10).enumerate() {
        let filename = media_filename(i, media_url);
        if let Some(data) = shared::download_media(media_url, TWITTER_DOWNLOAD_UA).await {
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

    if !shared::pre_check(msg, pool, SettingCheck::Twitter).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "twitter") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for url in found_urls {
        debug!("Fetching tweet: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || TwitterPost::fetch(&url_owned)).await {
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
                    Err(e) => warn!("Failed to send tweet message: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch tweet {}: {}", url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
