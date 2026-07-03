use super::deny;
use crate::constants::{DISCORD_MESSAGE_LIMIT, TRUNCATED_MESSAGE_LIMIT};
use crate::handlers::file_pages;
use crate::handlers::git_diff_handler::CommitDiff;
use crate::handlers::git_diffs;
use crate::handlers::git_handler::{FileResponse, GitFileLink};
use crate::handlers::shared;
use crate::{Context, Error, db};
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use tracing::debug;

/// Fetch and post a git file snippet or commit diff (GitHub, GitLab, Gitea, rustdoc)
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn git(
    ctx: Context<'_>,
    #[description = "File URL, commit URL, or compare URL"] url: String,
    #[description = "Restrict pagination buttons to only you"] only_me: Option<bool>,
) -> Result<(), Error> {
    let data = ctx.data();
    let pool = Some(data.pool.as_ref());
    if !shared::pre_check_user(ctx.author().id, pool).await {
        return deny(ctx, "You are blacklisted from using this bot.").await;
    }

    let trimmed = url.trim();

    let gitea_hosts = db::list_git_hosts(&data.pool).await.unwrap_or_default();
    if let Some(commit) = CommitDiff::parse(trimmed, &gitea_hosts) {
        return send_diff(ctx, commit, only_me.unwrap_or(false)).await;
    }

    if let Some(link) = GitFileLink::parse(trimmed) {
        return send_file(ctx, link, only_me.unwrap_or(false)).await;
    }

    deny(
        ctx,
        "That URL does not look like a recognizable git file, commit, or compare URL.",
    )
    .await
}

async fn send_file(ctx: Context<'_>, link: GitFileLink, only_me: bool) -> Result<(), Error> {
    if link.is_plain_markdown() {
        return deny(
            ctx,
            "That file can't be embedded. Link specific lines instead (e.g. `#L10-L20`).",
        )
        .await;
    }

    if !shared::check_rate_limit(ctx.author().id, "git_links") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching git file via slash command");

    let fetch_link = link.clone();
    let response = match shared::spawn_blocking_fetch(move || fetch_link.format_response()).await {
        Ok(FileResponse::Single(r)) => r,
        Ok(FileResponse::Paged(pages)) => {
            return send_paged_file(ctx, &link, pages, only_me).await;
        }
        Err(e) => {
            tracing::warn!("Failed to fetch git file via slash command: {}", e);
            db::record_embed(&ctx.data().pool, "git_links", false).await;
            return deny(ctx, "Failed to fetch that file.").await;
        }
    };

    let footer = format!("\n-# Sent by <@{}>", ctx.author().id);
    let max_body = DISCORD_MESSAGE_LIMIT - footer.len();
    let body = if response.len() <= max_body {
        response
    } else {
        let truncate_at = TRUNCATED_MESSAGE_LIMIT.min(max_body.saturating_sub(30));
        let truncate_at = shared::floor_char_boundary(&response, truncate_at);
        format!("{}...\n```\n(Content too long)", &response[..truncate_at])
    };
    let content = format!("{}{}", body, footer);

    ctx.send(
        CreateReply::default()
            .content(content)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;
    db::record_embed(&ctx.data().pool, "git_links", true).await;

    Ok(())
}

async fn send_pages(
    ctx: Context<'_>,
    service: &str,
    first_page: &str,
    buttons: Option<serenity::CreateActionRow<'_>>,
) -> Result<(), Error> {
    let footer = format!("\n-# Sent by <@{}>", ctx.author().id);
    if first_page.len() + footer.len() > DISCORD_MESSAGE_LIMIT {
        db::record_embed(&ctx.data().pool, service, false).await;
        return deny(ctx, "Response too long to display.").await;
    }

    let content = format!("{}{}", first_page, footer);
    let mut reply = CreateReply::default()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new());

    if let Some(buttons) = buttons {
        reply = reply.components(vec![serenity::CreateComponent::ActionRow(buttons)]);
    }

    ctx.send(reply).await?;
    db::record_embed(&ctx.data().pool, service, true).await;

    Ok(())
}

async fn send_paged_file(
    ctx: Context<'_>,
    link: &GitFileLink,
    pages: Vec<String>,
    only_me: bool,
) -> Result<(), Error> {
    if pages.is_empty() {
        db::record_embed(&ctx.data().pool, "git_links", false).await;
        return deny(ctx, "No file content was returned.").await;
    }

    let total_pages = pages.len();
    let first_page = pages[0].clone();
    let key = file_pages::cache_pages(link, pages);
    let buttons = (total_pages > 1).then(|| {
        let lock = only_me.then(|| ctx.author().id.get());
        file_pages::create_file_pagination_buttons(&key, 0, total_pages, lock)
    });

    send_pages(ctx, "git_links", &first_page, buttons).await
}

async fn send_diff(ctx: Context<'_>, commit: CommitDiff, only_me: bool) -> Result<(), Error> {
    if !shared::check_rate_limit(ctx.author().id, "git_diffs") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching commit diff via slash command");

    let chunked = match git_diffs::fetch_or_cached(&commit).await {
        Ok(c) if !c.is_empty() => c,
        Ok(_) => {
            db::record_embed(&ctx.data().pool, "git_diffs", false).await;
            return deny(ctx, "No diff content was returned.").await;
        }
        Err(e) => {
            tracing::warn!("Failed to fetch commit diff via slash command: {}", e);
            db::record_embed(&ctx.data().pool, "git_diffs", false).await;
            return deny(ctx, "Failed to fetch that commit diff.").await;
        }
    };

    let total_pages = chunked.len();
    let buttons = (total_pages > 1).then(|| {
        let lock = only_me.then(|| ctx.author().id.get());
        git_diffs::create_pagination_buttons(0, total_pages, &commit, lock)
    });

    send_pages(ctx, "git_diffs", &chunked[0], buttons).await
}
