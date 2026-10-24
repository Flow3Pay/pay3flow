# Development guide

## Prerequisites

- Docker Engine and Docker Compose
- Ruby 3.1+ for the exchange smoke test
- Node.js 24 and pnpm (Corepack) for frontend-only work
- Rust 1.97+ for backend-only work, if you are not using Docker

## Start the stack

```bash
docker compose up -d --build
docker compose ps
```

Check the main services directly:

```bash
curl -fsS http://localhost:8080/health
curl -fsS http://localhost:3000/
```

Check optional fmatch/Typesense health only when those services are running;
they are not part of the dedicated production Pay3Flow deployment.

Stop the stack with `docker compose down`. Do not add `-v` unless you intend
to remove the local database and cache volumes.

## Backend workflow

The backend lives in `backend/` and applies the SQL schema during startup.
Providerfiles are converted into a committed SQL migration before the Rust
build and are never read by the running service; see
[`Providerfile.md`](Providerfile.md).
Useful commands when Rust is installed:

```bash
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo check --manifest-path backend/Cargo.toml
cargo test --all-features --manifest-path backend/Cargo.toml
cargo clippy --all-targets --all-features --manifest-path backend/Cargo.toml
```

Use `backend/.env.example` as a reference. Development defaults are not safe
for production.

## Frontend workflow

```bash
cd pay3low-svelte-frontend
corepack enable
pnpm install --frozen-lockfile
pnpm run check
pnpm run test:unit
NODE_OPTIONS=--max-old-space-size=6144 pnpm run build
pnpm run test:e2e -- tests/exchange-flow.spec.ts tests/wallet-execution.spec.ts
```

The configured Playwright suite starts its own Vite server and share-backend
fixture. Browser tests mock the relevant APIs; the default suite does not need
a live production backend. Install Chromium with `pnpm exec playwright install chromium`
or set `PLAYWRIGHT_CHROMIUM_EXECUTABLE` to an installed compatible browser.
Use `--workers=1` for cold standalone NEAR widget checks on constrained hosts.
Explicit live-audit scripts are separate and issue read-only requests.

Set `PUBLIC_AURORA_INTENTS_API_KEY` in ignored `.env.local` for a real local
Aurora widget. Production supplies it on the frontend process at runtime.
The SDK needs the browser polyfills and optimization rules in `vite.config.ts`;
see [SWAP widgets](swap-widgets.md) before changing them.

Use `pnpm start` to run the built production server. It wraps adapter-node with
compression, frontend metrics and static-cache headers. For an end-to-end
production deploy, run `nix run .#deploy` from a clean committed worktree;
[deployment details](../deploy/README.md) include runtime secrets and verification.

## Verification checklist

Before opening a change:

1. Run the relevant Rust or frontend checks.
2. Run `ruby ./scripts/exchange_flow.rb` against a fresh development stack.
3. Run `ruby ./scripts/secret_audit.rb`.
4. Confirm new endpoints, states, and configuration are documented.
5. Confirm user consent and settlement disclosures still appear in the UI.

The exchange smoke test intentionally exercises both a successful proof and a
bad proof that enters a dispute. It also tests the global and corridor kill
switches. This makes it a useful regression test for state-machine changes.

## Working with fmatch

The backend sends signed ActivityPub proposals to the `fmatch_inbox` configured
in `config.toml`; general application settings are TOML, credentials are environment-only. For the
request format and retry/idempotency behavior, read
[`backend-to-fmatch.org`](backend-to-fmatch.org). For fmatch's public surfaces
and accepted activities, read [`fmatch-api.md`](fmatch-api.md).

## Data reset

For disposable local data only:

```bash
docker compose down -v
docker compose up -d --build
```

This removes Compose-managed PostgreSQL, Redis, and Typesense volumes. Never
run it against a shared or production project.
