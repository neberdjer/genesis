use crate::Data;
use crate::constants::{REPLY_WATCH_PRUNE_THRESHOLD, REPLY_WATCH_SECONDS};
use poise::serenity_prelude as serenity;
use serenity::model::guild::audit_log::{Action, MessageAction};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::warn;

struct Watch {
    reply_id: serenity::MessageId,
    channel_id: serenity::GenericChannelId,
    author_id: serenity::UserId,
    expires: Instant,
}

static WATCHES: OnceLock<Mutex<HashMap<serenity::MessageId, Watch>>> = OnceLock::new();

fn map() -> &'static Mutex<HashMap<serenity::MessageId, Watch>> {
    WATCHES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn watch(
    source_id: serenity::MessageId,
    reply_id: serenity::MessageId,
    channel_id: serenity::GenericChannelId,
    author_id: serenity::UserId,
) {
    if let Ok(mut m) = map().lock() {
        let now = Instant::now();
        if m.len() >= REPLY_WATCH_PRUNE_THRESHOLD {
            m.retain(|_, w| w.expires > now);
        }
        m.insert(
            source_id,
            Watch {
                reply_id,
                channel_id,
                author_id,
                expires: now + Duration::from_secs(REPLY_WATCH_SECONDS),
            },
        );
    }
}

pub async fn handle_delete(
    ctx: &serenity::Context,
    deleted_message_id: serenity::MessageId,
    guild_id: Option<serenity::GuildId>,
) {
    let Some(guild_id) = guild_id else {
        return;
    };

    let watch = {
        let Ok(mut m) = map().lock() else {
            return;
        };
        match m.remove(&deleted_message_id) {
            Some(w) => w,
            None => return,
        }
    };
    if watch.expires <= Instant::now() {
        return;
    }

    let data = ctx.data::<Data>();
    let enabled = crate::db::get_server_settings(&data.pool, &guild_id.to_string())
        .await
        .map(|s| s.reply_cleanup_enabled)
        .unwrap_or(false);
    if !enabled {
        return;
    }

    if !deleted_by_other(ctx, guild_id, watch.channel_id, watch.author_id).await {
        return;
    }

    if let Err(e) = watch
        .channel_id
        .delete_message(
            &ctx.http,
            watch.reply_id,
            Some("source message removed by a moderator"),
        )
        .await
    {
        warn!("Failed to delete reply after source removal: {}", e);
    }
}

async fn deleted_by_other(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    channel_id: serenity::GenericChannelId,
    author_id: serenity::UserId,
) -> bool {
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let logs = match guild_id
        .audit_logs(
            &ctx.http,
            Some(Action::Message(MessageAction::Delete)),
            None,
            None,
            None,
            None,
        )
        .await
    {
        Ok(logs) => logs,
        Err(e) => {
            warn!("Failed to read audit log for reply cleanup: {}", e);
            return false;
        }
    };

    let now = serenity::Timestamp::now().timestamp();
    logs.entries.iter().any(|entry| {
        let target_matches = entry.target_id.map(|t| t.get()) == Some(author_id.get());
        let channel_matches = entry.options.as_ref().and_then(|o| o.channel_id) == Some(channel_id);
        let by_other = entry.user_id != Some(author_id);
        let age = now - entry.id.created_at().timestamp();
        target_matches && channel_matches && by_other && (0..120).contains(&age)
    })
}
