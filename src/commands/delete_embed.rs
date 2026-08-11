use super::deny;
use crate::{Context, Error};
use poise::serenity_prelude as serenity;

/// Delete an embed that genesis posted
#[poise::command(
    context_menu_command = "Delete embed",
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn delete_embed(
    ctx: Context<'_>,
    #[description = "The message to delete"] message: serenity::Message,
) -> Result<(), Error> {
    authorize_and_delete(ctx, &message).await
}

/// Delete a genesis embed (yours by default, or one you name by id or link)
#[poise::command(
    slash_command,
    rename = "delete",
    install_context = "Guild|User",
    interaction_context = "Guild|BotDm|PrivateChannel"
)]
pub async fn delete(
    ctx: Context<'_>,
    #[description = "Message id or link (defaults to your most recent embed here)"] message: Option<
        String,
    >,
) -> Result<(), Error> {
    let channel = ctx.channel_id();

    let target = match message {
        Some(input) => {
            let Some(id) = parse_message_id(&input) else {
                return deny(ctx, "That doesn't look like a message id or link.").await;
            };
            let Ok(msg) = channel.message(ctx.serenity_context(), id).await else {
                return deny(ctx, "I couldn't find that message in this channel.").await;
            };
            msg
        }
        None => {
            let Some(msg) = latest_own_embed(ctx, channel).await else {
                return deny(
                    ctx,
                    "I couldn't find a recent embed of yours to delete here.",
                )
                .await;
            };
            msg
        }
    };

    authorize_and_delete(ctx, &target).await
}

async fn authorize_and_delete(ctx: Context<'_>, message: &serenity::Message) -> Result<(), Error> {
    let bot_id = ctx.serenity_context().cache.current_user().id;
    if message.author.id != bot_id {
        return deny(ctx, "That isn't one of my messages.").await;
    }

    let authorized = invoker_can_manage(ctx).await
        || original_author(ctx, message).await == Some(ctx.author().id);
    if !authorized {
        return deny(
            ctx,
            "You can only delete embeds for links you posted, unless you can manage messages here.",
        )
        .await;
    }

    if let Err(e) = message
        .channel_id
        .delete_message(ctx.http(), message.id, Some("Delete embed command"))
        .await
    {
        tracing::warn!("Failed to delete embed via command: {}", e);
        return deny(ctx, "I couldn't delete that message.").await;
    }

    deny(ctx, "Embed deleted.").await
}

async fn latest_own_embed(
    ctx: Context<'_>,
    channel: serenity::GenericChannelId,
) -> Option<serenity::Message> {
    let bot_id = ctx.serenity_context().cache.current_user().id;
    let invoker = ctx.author().id;
    let recent = channel
        .messages(
            ctx.serenity_context(),
            serenity::GetMessages::new().limit(50),
        )
        .await
        .ok()?;
    recent.into_iter().find(|m| {
        m.author.id == bot_id && m.referenced_message.as_ref().map(|r| r.author.id) == Some(invoker)
    })
}

fn parse_message_id(input: &str) -> Option<serenity::MessageId> {
    let last = input.trim().rsplit('/').next()?.trim();
    last.parse::<u64>().ok().map(serenity::MessageId::new)
}

async fn original_author(
    ctx: Context<'_>,
    message: &serenity::Message,
) -> Option<serenity::UserId> {
    if let Some(referenced) = &message.referenced_message {
        return Some(referenced.author.id);
    }
    let source_id = message.message_reference.as_ref()?.message_id?;
    message
        .channel_id
        .message(ctx.serenity_context(), source_id)
        .await
        .ok()
        .map(|m| m.author.id)
}

async fn invoker_can_manage(ctx: Context<'_>) -> bool {
    ctx.author_member()
        .await
        .and_then(|member| member.permissions)
        .is_some_and(|perms| perms.manage_messages())
}
