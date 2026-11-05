use super::shared::{self, MediaGallery, SettingCheck};
use super::twitter_handler::{self, TwitterEntities, TwitterError, TwitterPoll, TwitterPost};
use crate::constants::{
    FAILURE_FETCH, TWITTER_ACCENT_COLOR, TWITTER_DOWNLOAD_UA, TWITTER_MAX_QUOTE_CHARS,
    TWITTER_POLL_BAR_WIDTH,
};
use poise::serenity_prelude as serenity;
use regex::Regex;
use sqlx::PgPool;
use std::fmt::Write as _;
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

fn x_link(sigil: &str, name: &str, path: &str) -> String {
    format!("[{sigil}{name}](<https://x.com/{path}>)")
}

fn mention_link(username: &str) -> String {
    x_link("@", username, username)
}

fn author_line(author: &str, username: &str) -> String {
    format!(
        "**{}** ({})",
        shared::escape_markdown(author),
        mention_link(username)
    )
}

fn contains_tag(list: &[String], name: &str) -> bool {
    let name = name.to_lowercase();
    list.iter().any(|t| t.to_lowercase() == name)
}

fn entity_link(sigil: &str, name: &str, entities: &TwitterEntities) -> Option<String> {
    let (sigil, list, path) = match sigil {
        "@" if name.len() <= 15 && name.is_ascii() => ("@", &entities.mentions, name.to_string()),
        "#" | "\u{ff03}" => (
            "#",
            &entities.hashtags,
            format!("hashtag/{}", urlencoding::encode(name)),
        ),
        "$" => (
            "$",
            &entities.cashtags,
            format!("search?q=%24{}", urlencoding::encode(name)),
        ),
        _ => return None,
    };
    contains_tag(list, name).then(|| x_link(sigil, name, &path))
}

fn render_text(text: &str, entities: &TwitterEntities) -> String {
    static ENTITY_PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = ENTITY_PATTERN
        .get_or_init(|| Regex::new(r"\B([@#\x{ff03}$])([\p{L}\p{M}\p{N}_]+)").unwrap());

    shared::map_outside_urls(text, |segment| {
        let mut out = String::with_capacity(segment.len());
        let mut last = 0;
        for caps in pattern.captures_iter(segment) {
            let Some(link) = entity_link(&caps[1], &caps[2], entities) else {
                continue;
            };
            let whole = caps.get(0).unwrap();
            out.push_str(&shared::escape_markdown(&segment[last..whole.start()]));
            out.push_str(&link);
            last = whole.end();
        }
        out.push_str(&shared::escape_markdown(&segment[last..]));
        out
    })
}

fn format_count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn poll_bar(fraction: f64) -> String {
    let halves = (fraction * (TWITTER_POLL_BAR_WIDTH * 2) as f64).round() as usize;
    let full = halves / 2;
    let half = halves % 2;
    let mut bar = "\u{2588}".repeat(full);
    if half == 1 {
        bar.push('\u{258c}');
    }
    bar.push_str(&"\u{b7}".repeat(TWITTER_POLL_BAR_WIDTH - full - half));
    bar
}

fn poll_block(poll: &TwitterPoll) -> String {
    let total: u64 = poll.choices.iter().map(|(_, count)| count).sum();
    let width = poll
        .choices
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    let mut block = String::from("\n```");
    for (label, count) in &poll.choices {
        let fraction = if total == 0 {
            0.0
        } else {
            *count as f64 / total as f64
        };
        let _ = write!(
            block,
            "\n{:<width$}  {}  {:>5.1}%",
            shared::escape_code_block(label),
            poll_bar(fraction),
            fraction * 100.0
        );
    }
    let votes = if total == 1 { "vote" } else { "votes" };
    let status = match (poll.is_final, poll.ends_at) {
        (true, _) => "Final results".to_string(),
        (false, Some(ends)) => format!("Ends <t:{}:R>", ends),
        (false, None) => "Open".to_string(),
    };
    let _ = write!(
        block,
        "\n```\n-# {} {} · {}",
        format_count(total),
        votes,
        status
    );
    block
}

pub async fn build_container(
    post: &TwitterPost,
    user_id: serenity::UserId,
) -> (
    Vec<serenity::CreateAttachment<'static>>,
    serenity::CreateContainer<'static>,
) {
    let mut header = author_line(&post.author, &post.username);
    if let Some(created_at) = post.created_at {
        let _ = write!(header, " · <t:{}:R>", created_at);
    }
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
            let quoted = render_text(quote_text, &post.quote_entities).replace('\n', "\n> ");
            format!(
                "\n\n> {}\n> {}",
                author_line(quote_author, quote_username),
                shared::truncate_text(&quoted, TWITTER_MAX_QUOTE_CHARS)
            )
        }
        _ => String::new(),
    };
    let trailer = post.poll.as_ref().map(poll_block).unwrap_or_default() + &quote_block;

    let mut gallery = MediaGallery::default().spoiler(post.sensitive);
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
        &render_text(&post.text, &post.entities),
        &trailer,
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
