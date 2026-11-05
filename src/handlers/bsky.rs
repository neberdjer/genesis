use super::bsky_handler::{self, BskyFacet, BskyPost, BskySpan};
use super::shared::{self, MediaGallery, SettingCheck};
use crate::constants::{
    BSKY_ACCENT_COLOR, BSKY_DOWNLOAD_UA, BSKY_MAX_LINK_TITLE_CHARS, FAILURE_FETCH,
};
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use std::fmt::Write as _;
use tracing::{debug, warn};

fn render_text(text: &str, spans: &[BskySpan]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for span in spans {
        if span.start < last
            || span.start >= span.end
            || span.end > text.len()
            || !text.is_char_boundary(span.start)
            || !text.is_char_boundary(span.end)
        {
            continue;
        }
        let label: String = text[span.start..span.end]
            .chars()
            .filter(|c| !matches!(c, '[' | ']' | '`'))
            .collect();
        let url = match &span.facet {
            BskyFacet::Link(uri) if uri.starts_with("http") => uri.clone(),
            BskyFacet::Link(_) => continue,
            BskyFacet::Mention(did) => format!("https://bsky.app/profile/{}", did),
            BskyFacet::Tag(tag) => {
                format!("https://bsky.app/hashtag/{}", urlencoding::encode(tag))
            }
        };
        if label.trim().is_empty() {
            continue;
        }
        out.push_str(&shared::escape_text(&text[last..span.start]));
        let _ = write!(
            out,
            "[{}](<{}>)",
            label,
            url.replace('>', "%3E").replace(' ', "%20")
        );
        last = span.end;
    }
    out.push_str(&shared::escape_text(&text[last..]));
    out
}

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
    let mut trailer = String::new();
    if let Some((title, uri)) = &post.external {
        let title = shared::truncate_text(title, BSKY_MAX_LINK_TITLE_CHARS);
        let _ = write!(
            trailer,
            "\n\n[{}](<{}>)",
            shared::escape_markdown(&title),
            uri
        );
    }
    if let Some(quote_author) = &post.quote_author {
        let quote = render_text(post.quote_text.as_deref().unwrap_or(""), &post.quote_spans);
        let _ = write!(
            trailer,
            "\n\n> **{}**\n> {}",
            shared::escape_markdown(quote_author),
            quote.replace('\n', "\n> ")
        );
    }

    let items: Vec<_> = post
        .media
        .iter()
        .map(|item| {
            (
                item.url.as_str(),
                BSKY_DOWNLOAD_UA,
                item.is_video.then_some("mp4"),
            )
        })
        .collect();
    let mut gallery = MediaGallery::default().spoiler(post.sensitive);
    gallery.add_all("bsky", &items).await;

    shared::post_container(
        shared::with_timestamp(
            shared::author_header(&post.author, &post.handle),
            post.created_at,
        ),
        &render_text(&post.text, &post.spans),
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
