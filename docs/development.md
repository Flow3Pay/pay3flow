# Development guide

## Prerequisites

- Docker Engine and Docker Compose
- `curl` and `jq` for the shell smoke test
- Node.js and npm for frontend-only work
- Rust tooling for backend-only work, if you are not using Docker

## Start the stack

```bash
docker compose up -d --build
docker compose ps
./scripts/healthcheck.ps1 # Windows PowerShell, optional
```

On Unix-like systems, check the main services directly:

```bash
curl -fsS http://localhost:8080/health
curl -fsS http://localhost:7277/health
curl -fsS http://localhost:8108/health
```

Stop the stack with `docker compose down`. Do not add `-v` unless you intend
to remove the local database and cache volumes.

## Backend workflow

The backend lives in `backend/` and applies the SQL schema during startup.
Useful commands when Rust is installed:

```bash
cargo fmt --manifest-path backend/Cargo.toml -- --check
cargo check --manifest-path backend/Cargo.toml
cargo test --manifest-path backend/Cargo.toml
```

Use `backend/.env.example` as a reference. Development defaults are not safe
for production.

## Frontend workflow

```bash
cd pay3low-svelte-frontend
npm ci
npm run check
npm run build
npm test -- --run
```

The Playwright configuration may require the backend and frontend services to
be running. See `playwright.config.ts` and `tests/` for the current browser
test setup.

## Verification checklist

Before opening a change:

1. Run the relevant Rust or frontend checks.
2. Run `./scripts/exchange-flow.sh` against a fresh development stack.
3. Run `./scripts/secret-audit.sh`.
4. Confirm new endpoints, states, and configuration are documented.
5. Confirm user consent and settlement disclosures still appear in the UI.

The exchange smoke test intentionally exercises both a successful proof and a
bad proof that enters a dispute. It also tests the global and corridor kill
switches. This makes it a useful regression test for state-machine changes.

## Working with fmatch

The backend sends signed ActivityPub proposals to `FMATCH_INBOX`. For the
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
