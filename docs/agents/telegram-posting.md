# Telegram channel posts

## Publishing command

Use the `TgPost` Cargo binary to publish a text post to the configured Telegram
channel:

```bash
cargo run --manifest-path backend/Cargo.toml --bin TgPost -- "Post text"
```

The command reads `TG_BOT_TOKEN` and `TG_CHANNEL_ID` from the process
environment. `TG_CHANNEL_ID` accepts the public channel username (normally
`@pay3flow`) or a numeric channel ID. The bot must be a channel administrator
with permission to publish.

The command sends immediately; it has no preview, confirmation prompt,
scheduling, or media upload. It prints the Telegram message ID on success and
returns an error if sending fails. See [`../project-overview.md`](../project-overview.md)
for environment setup.

## When asked to create a post

- Draft concise, natural copy in the language requested by the user; use
  Russian if no language is specified and the request is in Russian.
- Use only facts, links, claims, and calls to action supported by the user's
  brief or repository context. Do not invent announcements, dates, metrics, or
  product capabilities.
- If the user asks only to create or draft a post, return the text for review;
  do not publish it.
- Run `TgPost` only when the user explicitly asks to publish or send the post to
  the channel. Publishing is an external side effect and happens immediately.
- Before sending, verify the final text and channel target. Use the existing
  `TG_CHANNEL_ID` environment setting; do not silently override it with another
  channel.
- Never ask the user to paste a bot token into chat, print it, include it in
  source or documentation, or place it directly in a shell command. If the
  required environment variables are unavailable, provide the draft and say
  that publishing is blocked until deployment secrets are configured.
- After successful publication, report the target channel and Telegram message
  ID. If the command fails, report the error without claiming that the post was
  published.
