use super::shared::{self, MediaGallery, SettingCheck};
use super::twitter_handler::{self, TwitterError, TwitterPost};
use crate::constants::{
    FAILURE_FETCH, TWITTER_ACCENT_COLOR, TWITTER_DOWNLOAD_UA, TWITTER_MAX_QUOTE_CHARS,
};
use poise::serenity_prelude as serenity;
use regex::{Captures, Regex};
use sqlx::PgPool;
use std::sync::OnceLock;
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

const MARKDOWN_CHARS: &str = "\\*_~`|<>[]()#-";

fn mention_link(username: &str) -> String {
    format!(
        "[@{}](<https://x.com/{}>)",
        escape_markdown(username),
        username
    )
}

fn escape_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if MARKDOWN_CHARS.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn author_line(author: &str, username: &str) -> String {
    format!(
        "**{}** ({})",
        escape_markdown(author),
        mention_link(username)
    )
}

fn render_text(text: &str, mentions: &[String]) -> String {
    static TOKEN_PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = TOKEN_PATTERN.get_or_init(|| {
        Regex::new(&format!(
            r"(?<url>https?://\S+)|\B@(?<name>[A-Za-z0-9_]{{1,15}})\b|[{}]",
            regex::escape(MARKDOWN_CHARS)
        ))
        .unwrap()
    });

    pattern
        .replace_all(text, |caps: &Captures| {
            if caps.name("url").is_some() {
                return caps[0].to_string();
            }
            match caps.name("name") {
                Some(name)
                    if mentions
                        .iter()
                        .any(|m| m.eq_ignore_ascii_case(name.as_str())) =>
                {
                    mention_link(name.as_str())
                }
                _ => escape_markdown(&caps[0]),
            }
        })
        .into_owned()
}

pub async fn build_container(
    post: &TwitterPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut header = author_line(&post.author, &post.username);
    if !post.replying_to.is_empty() {
        let names = post
            .replying_to
            .iter()
            .map(|n| mention_link(n))
            .collect::<Vec<_>>()
            .join(" ");
        header.push_str(&format!("\nReplying to {}", names));
    }

    let quote_block = match (&post.quote_author, &post.quote_username, &post.quote_text) {
        (Some(quote_author), Some(quote_username), Some(quote_text)) => {
            let quoted = render_text(quote_text, &post.quote_mentions).replace('\n', "\n> ");
            format!(
                "\n\n> {}\n> {}",
                author_line(quote_author, quote_username),
                shared::truncate_text(&quoted, TWITTER_MAX_QUOTE_CHARS)
            )
        }
        _ => String::new(),
    };

    let mut gallery = MediaGallery::default();
    for (i, item) in post.media.iter().enumerate() {
        let Some(data) = gallery.fetch(&item.url, TWITTER_DOWNLOAD_UA).await else {
            continue;
        };
        let (data, ext) = if item.is_gif {
            match shared::mp4_to_gif(data.clone()).await {
                Some(gif) if gif.len() <= gallery.remaining() => (gif, Some("gif")),
                _ => (data, Some("mp4")),
            }
        } else {
            (data, None)
        };
        gallery.attach(data, shared::media_filename("twitter", i, &item.url, ext));
    }

    shared::post_container(
        header,
        &render_text(&post.text, &post.mentions),
        &quote_block,
        gallery,
        user_id,
        TWITTER_ACCENT_COLOR,
    )
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

                match shared::send_reply(ctx, msg, "twitter", message).await {
                    None => any_sent = true,
                    Some(code) => failure = Some(code),
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
