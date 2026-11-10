# Agent instructions

Task-specific guidance lives in [`docs/agents/`](docs/agents/README.md). Read
only the guide relevant to the task at hand. For Telegram channel posts, follow
[`docs/agents/telegram-posting.md`](docs/agents/telegram-posting.md).

## Working rules

- Commit in small, logical steps while working. Do not leave a finished change
  uncommitted.
- Push to `master` at the end of the work. Do not create a side branch unless
  the user asks for one.
- Before finishing, run the project checks and make sure they are green:
  - backend: `cargo fmt --check`, `cargo check`, `cargo test --all-features`;
  - frontend (when touched): `pnpm run check`, `pnpm run test:unit`,
    `pnpm run build`;
  - e2e (when relevant): `pnpm run test:e2e`.
  If the host cannot run a suite (for example, missing Chromium libraries),
  report the exact blocker and do not call the work green.
- Update `docs/` and `backend/src/server/openapi.rs` when behavior or the API
  surface changes.
