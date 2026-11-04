use super::deny;
use crate::constants::{FAILURE_FETCH, FAILURE_TOO_LONG};
use crate::handlers::file_pages;
use crate::handlers::git_diff_handler::CommitDiff;
use crate::handlers::git_diffs;
use crate::handlers::git_handler::{FileResponse, GitError, GitFileLink};
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
        Ok(Some(FileResponse::Single(r))) => r,
        Ok(Some(FileResponse::Paged(pages))) => {
            return send_paged_file(ctx, &link, pages, only_me).await;
        }
        Ok(None) => {
            return deny(ctx, "Could not find a file at that URL.").await;
        }
        Err(e) => {
            tracing::warn!("Failed to fetch git file via slash command: {}", e);
            let code = e
                .downcast_ref::<GitError>()
                .map_or(FAILURE_FETCH, GitError::code);
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "git_links",
                code,
                Some(&link.original_url),
                &e.to_string(),
            );
            return deny(ctx, &shared::failure_reason("git_links", code)).await;
        }
    };

    let content = shared::with_sent_by(response, ctx.author().id);

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
    let Some(content) = shared::try_with_sent_by(first_page, ctx.author().id) else {
        shared::report_failure(
            ctx.serenity_context(),
            ctx.guild_id(),
            service,
            FAILURE_TOO_LONG,
            None,
            "first page exceeds the message limit",
        );
        return deny(ctx, "Response too long to display.").await;
    };

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
        shared::report_failure(
            ctx.serenity_context(),
            ctx.guild_id(),
            "git_links",
            FAILURE_FETCH,
            Some(&link.original_url),
            "no file content was returned",
        );
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
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "git_diffs",
                FAILURE_FETCH,
                None,
                &format!(
                    "{}/{}@{}: no diff content was returned",
                    commit.owner, commit.repo, commit.commit
                ),
            );
            return deny(ctx, "No diff content was returned.").await;
        }
        Err(e) => {
            tracing::warn!("Failed to fetch commit diff via slash command: {}", e);
            shared::report_failure(
                ctx.serenity_context(),
                ctx.guild_id(),
                "git_diffs",
                FAILURE_FETCH,
                None,
                &format!("{}/{}@{}: {}", commit.owner, commit.repo, commit.commit, e),
            );
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
