use crate::constants::{DISCORD_MESSAGE_LIMIT, EMBED_SUPPRESS_DELAY_MS, FILES_PER_PAGE};
use crate::git_diff_handler::GitHubCommitDiff;
use poise::serenity_prelude as serenity;
use std::time::Duration;
use tracing::{error, warn};

pub fn is_github_commit_url(word: &str) -> bool {
    word.contains("github.com") && word.contains("/commit/")
}

pub fn clean_url(url: &str) -> &str {
    url.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
}

async fn suppress_embeds(ctx: &serenity::Context, msg: &serenity::Message) {
    tokio::time::sleep(Duration::from_millis(EMBED_SUPPRESS_DELAY_MS)).await;

    if let Err(e) = msg
        .channel_id
        .edit_message(
            &ctx.http,
            msg.id,
            serenity::EditMessage::new().suppress_embeds(true),
        )
        .await
    {
        warn!(
            "Failed to suppress embed in channel {} (needs Manage Messages permission): {}",
            msg.channel_id, e
        );
    }
}

fn create_pagination_buttons(
    current_page: usize,
    total_pages: usize,
    owner: &str,
    repo: &str,
    commit_sha: &str,
) -> serenity::CreateActionRow {
    let prev_button = serenity::CreateButton::new(format!(
        "diff_prev_{}_{}_{}_{}",
        owner, repo, commit_sha, current_page
    ))
    .label("Previous")
    .style(serenity::ButtonStyle::Primary)
    .disabled(current_page == 0);

    let page_info = serenity::CreateButton::new("page_info")
        .label(format!("{}/{}", current_page + 1, total_pages))
        .style(serenity::ButtonStyle::Secondary)
        .disabled(true);

    let next_button = serenity::CreateButton::new(format!(
        "diff_next_{}_{}_{}_{}",
        owner, repo, commit_sha, current_page
    ))
    .label("Next")
    .style(serenity::ButtonStyle::Primary)
    .disabled(current_page >= total_pages - 1);

    serenity::CreateActionRow::Buttons(vec![prev_button, page_info, next_button])
}

async fn send_paginated_diff(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    commit: &GitHubCommitDiff,
    responses: Vec<String>,
) {
    if responses.is_empty() {
        return;
    }

    let total_pages = responses.len();
    let first_page = &responses[0];

    if first_page.len() > DISCORD_MESSAGE_LIMIT {
        warn!("Diff response too long, skipping");
        return;
    }

    let mut message_builder = serenity::CreateMessage::new()
        .content(first_page)
        .reference_message(msg);

    if total_pages > 1 {
        let buttons = create_pagination_buttons(0, total_pages, &commit.owner, &commit.repo, &commit.commit);
        message_builder = message_builder.components(vec![buttons]);
    }

    if let Err(e) = msg.channel_id.send_message(&ctx.http, message_builder).await {
        error!("Failed to send diff message: {}", e);
    }
}

pub async fn handle_commit_diffs(ctx: &serenity::Context, msg: &serenity::Message) {
    if msg.author.bot {
        return;
    }

    let found_commits: Vec<GitHubCommitDiff> = msg
        .content
        .split_whitespace()
        .filter(|word| is_github_commit_url(word))
        .filter_map(|word| GitHubCommitDiff::parse(clean_url(word)))
        .collect();

    if found_commits.is_empty() {
        return;
    }

    suppress_embeds(ctx, msg).await;

    for commit in found_commits {
        match commit.format_diff_response() {
            Ok(responses) => {
                let chunked: Vec<String> = responses
                    .chunks(FILES_PER_PAGE)
                    .map(|chunk| chunk.join("\n\n"))
                    .collect();

                send_paginated_diff(ctx, msg, &commit, chunked).await;
            }
            Err(e) => warn!("Failed to fetch commit diff: {}", e),
        }
    }
}

pub async fn handle_diff_pagination(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
) {
    let custom_id = &interaction.data.custom_id;

    if !custom_id.starts_with("diff_prev_") && !custom_id.starts_with("diff_next_") {
        return;
    }

    let parts: Vec<&str> = custom_id.split('_').collect();
    if parts.len() < 6 {
        return;
    }

    let direction = parts[1];
    let owner = parts[2];
    let repo = parts[3];
    let commit_sha = parts[4];
    let current_page: usize = parts[5].parse().unwrap_or(0);

    let new_page = match direction {
        "prev" => current_page.saturating_sub(1),
        "next" => current_page + 1,
        _ => return,
    };

    let commit = GitHubCommitDiff {
        owner: owner.to_string(),
        repo: repo.to_string(),
        commit: commit_sha.to_string(),
        diff_hash: None,
    };

    match commit.format_diff_response() {
        Ok(responses) => {
            let chunked: Vec<String> = responses
                .chunks(FILES_PER_PAGE)
                .map(|chunk| chunk.join("\n\n"))
                .collect();

            if new_page >= chunked.len() {
                return;
            }

            let page_content = &chunked[new_page];
            let total_pages = chunked.len();

            let buttons = create_pagination_buttons(new_page, total_pages, owner, repo, commit_sha);

            let response = serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::new()
                    .content(page_content)
                    .components(vec![buttons]),
            );

            if let Err(e) = interaction.create_response(&ctx.http, response).await {
                error!("Failed to update message: {}", e);
            }
        }
        Err(e) => {
            warn!("Failed to fetch commit diff for pagination: {}", e);
        }
    }
}
