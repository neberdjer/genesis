use crate::constants::{DISCORD_MESSAGE_LIMIT, TRUNCATED_MESSAGE_LIMIT};
use crate::handlers::git_diff_handler::CommitDiff;
use crate::handlers::git_diffs;
use crate::handlers::git_handler::GitFileLink;
use crate::handlers::shared;
use crate::{Context, Error};
use poise::CreateReply;
use poise::serenity_prelude as serenity;
use tracing::debug;

async fn deny(ctx: Context<'_>, message: &str) -> Result<(), Error> {
    ctx.send(CreateReply::default().content(message).ephemeral(true))
        .await?;
    Ok(())
}

/// Fetch and post a git file snippet or commit diff (GitHub, GitLab, Gitea, rustdoc)
#[poise::command(
    slash_command,
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn git(
    ctx: Context<'_>,
    #[description = "File URL, commit URL, or compare URL"] url: String,
    #[description = "Restrict pagination buttons to only you (commit diffs only)"] only_me: Option<
        bool,
    >,
) -> Result<(), Error> {
    let data = ctx.data();
    let pool = Some(data.pool.as_ref());
    if !shared::pre_check_user(ctx.author().id, pool).await {
        return deny(ctx, "You are blacklisted from using this bot.").await;
    }

    let trimmed = url.trim();

    if let Some(commit) = CommitDiff::parse(trimmed) {
        return send_diff(ctx, commit, only_me.unwrap_or(false)).await;
    }

    if let Some(link) = GitFileLink::parse(trimmed) {
        return send_file(ctx, link).await;
    }

    deny(
        ctx,
        "That URL does not look like a recognizable git file, commit, or compare URL.",
    )
    .await
}

async fn send_file(ctx: Context<'_>, link: GitFileLink) -> Result<(), Error> {
    if !shared::check_rate_limit(ctx.author().id, "git_links") {
        return deny(
            ctx,
            "You're being rate limited. Try again in a few seconds.",
        )
        .await;
    }

    ctx.defer().await?;
    debug!("Fetching git file via slash command");

    let response = match shared::spawn_blocking_fetch(move || link.format_response()).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to fetch git file via slash command: {}", e);
            return deny(ctx, "Failed to fetch that file.").await;
        }
    };

    let footer = format!("\n-# Sent by <@{}>", ctx.author().id);
    let max_body = DISCORD_MESSAGE_LIMIT - footer.len();
    let body = if response.len() <= max_body {
        response
    } else {
        let truncate_at = TRUNCATED_MESSAGE_LIMIT.min(max_body.saturating_sub(30));
        format!("{}...\n```\n(Content too long)", &response[..truncate_at])
    };
    let content = format!("{}{}", body, footer);

    ctx.send(
        CreateReply::default()
            .content(content)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
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
        Ok(_) => return deny(ctx, "No diff content was returned.").await,
        Err(e) => {
            tracing::warn!("Failed to fetch commit diff via slash command: {}", e);
            return deny(ctx, "Failed to fetch that commit diff.").await;
        }
    };

    let total_pages = chunked.len();
    let footer = format!("\n-# Sent by <@{}>", ctx.author().id);
    let first_page = &chunked[0];

    if first_page.len() + footer.len() > DISCORD_MESSAGE_LIMIT {
        return deny(ctx, "Diff response too long to display.").await;
    }

    let content = format!("{}{}", first_page, footer);
    let mut reply = CreateReply::default()
        .content(content)
        .allowed_mentions(serenity::CreateAllowedMentions::new());

    if total_pages > 1 {
        let lock = only_me.then(|| ctx.author().id.get());
        let buttons = git_diffs::create_pagination_buttons(0, total_pages, &commit, lock);
        reply = reply.components(vec![serenity::CreateComponent::ActionRow(buttons)]);
    }

    ctx.send(reply).await?;

    Ok(())
}
