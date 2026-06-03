use super::reddit_handler::RedditPost;
use super::shared::{self, SettingCheck};
use crate::constants::REDDIT_DOWNLOAD_UA;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use tracing::{debug, warn};

const REDDIT_ACCENT_COLOR: u32 = 0xFF4500;

fn is_reddit_url(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    let Some(scheme_end) = lower.find("://") else {
        return false;
    };
    let after_scheme = &lower[scheme_end + 3..];
    let host = after_scheme.split('/').next().unwrap_or("");

    if host == "redd.it" {
        return after_scheme
            .split_once('/')
            .map(|(_, p)| !p.is_empty())
            .unwrap_or(false);
    }

    if host == "i.redd.it" || host == "v.redd.it" || host == "preview.redd.it" {
        return false;
    }

    let normalized = host
        .trim_start_matches("www.")
        .trim_start_matches("old.")
        .trim_start_matches("new.")
        .trim_start_matches("m.")
        .trim_start_matches("np.")
        .trim_start_matches("amp.")
        .trim_start_matches("i.");

    if normalized != "reddit.com" && normalized != "redditmedia.com" {
        return false;
    }

    lower.contains("/comments/") || lower.contains("/s/")
}

fn clean_url(url: &str) -> &str {
    let trimmed = url.trim_start_matches(|c: char| !c.is_alphanumeric());
    trimmed.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

fn media_filename(index: usize, url: &str) -> String {
    let lower = url.to_ascii_lowercase();
    let ext = if lower.contains(".mp4") || lower.contains("/dash_") {
        "mp4"
    } else if lower.contains(".gif") {
        "gif"
    } else if lower.contains(".webp") {
        "webp"
    } else if lower.contains(".png") {
        "png"
    } else {
        "jpg"
    };
    format!("reddit_{}.{}", index, ext)
}

pub async fn build_container(
    post: &RedditPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut content = if post.subreddit.is_empty() {
        format!("**{}** by u/{}", post.title, post.author)
    } else {
        format!("**{}**\n{} · u/{}", post.title, post.subreddit, post.author)
    };

    let mut tags = Vec::new();
    if post.nsfw {
        tags.push("NSFW");
    }
    if post.spoiler {
        tags.push("Spoiler");
    }
    if !tags.is_empty() {
        content.push_str(&format!(" · {}", tags.join(" · ")));
    }

    if !post.text.is_empty() {
        content.push_str(&format!("\n\n{}", post.text));
    }

    let mut components: Vec<serenity::CreateContainerComponent<'static>> =
        vec![serenity::CreateContainerComponent::TextDisplay(
            serenity::CreateTextDisplay::new(content),
        )];

    let mut attachments = Vec::new();
    let mut gallery_items: Vec<serenity::CreateMediaGalleryItem<'static>> = Vec::new();
    let spoiler_media = post.nsfw || post.spoiler;
    for (i, media_url) in post.media.iter().take(10).enumerate() {
        let filename = media_filename(i, media_url);
        if let Some(data) = shared::download_media(media_url, REDDIT_DOWNLOAD_UA).await {
            let attachment_url = format!("attachment://{}", filename);
            attachments.push(serenity::CreateAttachment::bytes(data, filename));
            let mut item = serenity::CreateMediaGalleryItem::new(
                serenity::CreateUnfurledMediaItem::new(attachment_url),
            );
            if spoiler_media {
                item = item.spoiler(true);
            }
            gallery_items.push(item);
        } else {
            warn!("Failed to download Reddit media: {}", media_url);
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

    let container = serenity::CreateContainer::new(components).accent_color(REDDIT_ACCENT_COLOR);
    (attachments, container)
}

pub async fn handle_reddit_links(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    pool: Option<&PgPool>,
) {
    let mut found_urls: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for word in msg.content.split_whitespace() {
        if !is_reddit_url(word) {
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

    if !shared::pre_check(msg, pool, SettingCheck::Reddit).await {
        return;
    }

    if !shared::check_rate_limit(msg.author.id, "reddit") {
        debug!("Rate limited, skipping");
        return;
    }

    let mut any_sent = false;
    for url in found_urls {
        debug!("Fetching Reddit post: url={}", url);
        let url_owned = url.clone();
        match shared::spawn_blocking_fetch(move || RedditPost::fetch(&url_owned)).await {
            Ok(post) => {
                let (attachments, container) = build_container(&post, msg.author.id).await;
                let message = serenity::CreateMessage::new()
                    .components(vec![serenity::CreateComponent::Container(container)])
                    .flags(serenity::MessageFlags::IS_COMPONENTS_V2)
                    .reference_message(msg)
                    .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
                    .files(attachments);

                match msg.channel_id.send_message(&ctx.http, message).await {
                    Ok(_) => any_sent = true,
                    Err(e) => warn!("Failed to send Reddit message: {}", e),
                }
            }
            Err(e) => warn!("Failed to fetch Reddit {}: {}", url, e),
        }
    }

    if any_sent {
        shared::suppress_embeds(ctx, msg).await;
    }
}
