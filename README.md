# Genesis

Discord bot with GitHub/GitLab integration, Twitter/TikTok/Instagram link handling, and moderation features.

## Docker Registry

```
registry.heliopolis.live/atums/genesis
```

## Environment Variables

Required:
- `DISCORD_TOKEN` - Discord bot token
- `DATABASE_URL` - PostgreSQL database connection string

Optional:
- `ENVIRONMENT` - Environment name (default: "development")
- `PREFIX` - Command prefix (default: "!")
- `OWNER_ID` - Discord user ID for owner-only commands

## Commands

### General
- `/ping` - Check bot latency

### Utility
- `/checkperms` - Check bot permissions
- `/time` - Get current time in a timezone

### Moderation
- `/ban` - Ban a user from the server
- `/kick` - Kick a user from the server
- `/blacklist` - Manage user/server blacklists (owner only)
  - `/blacklist add` - Add user to blacklist
  - `/blacklist remove` - Remove user from blacklist
  - `/blacklist add_server` - Add server to blacklist
  - `/blacklist remove_server` - Remove server from blacklist
- `/leave_server` - Make the bot leave a server (owner only)

### Settings
- `/settings toggle` - Toggle features per server (requires Administrator)
  - `git_diffs` - GitHub/GitLab commit diffs
  - `git_compares` - GitHub/GitLab compare views
  - `git_links` - GitHub/GitLab file links
  - `twitter` - Twitter link embeds
  - `tiktok` - TikTok link embeds
  - `instagram` - Instagram post embeds

## Features

### Automatic Link Handling
- GitHub/GitLab commit diffs with pagination
- GitHub/GitLab file links with code snippets
- Twitter post embeds with media
- TikTok video embeds
- Instagram post embeds with media

All automatic features can be toggled per server using the `/settings toggle` command.
