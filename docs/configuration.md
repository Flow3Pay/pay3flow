# Configuration

The Compose file supplies development defaults. Copy values into a deployment
secret store for any environment that handles real users or data.

## Core variables

| Variable | Purpose | Development default |
| --- | --- | --- |
| `HTTP_ADDR` | Backend listen address | `0.0.0.0:8080` in Compose |
| `DATABASE_URL` | Pay3Flow PostgreSQL connection | Compose `postgres://...@postgres:5432/pay3flow` |
| `REDIS_URL` | Redis connection | `redis://redis:6379` |
| `JWT_SECRET` | Signs user tokens | `dev-secret-change-me` |
| `SECRETS_KEY` | Encrypts stored sensitive values | `dev-secrets-key-change-me` |
| `ADMIN_TOKEN` | Protects admin exchange controls | `dev-admin-token-change-me` |
| `RUST_LOG` | Rust log filter | `info` |

## ActivityPub and fmatch

| Variable | Purpose |
| --- | --- |
| `AP_ORIGIN` | Backend ActivityPub base URL |
| `AP_HANDLE` | Backend actor handle |
| `AP_KEY_PATH` | RSA private key used for HTTP signatures |
| `AP_REQUIRE_SIGNATURES` | Whether inbound signatures are required |
| `FMATCH_INBOX` | fmatch inbox URL |
| `FMATCH_ACTOR_ID` | fmatch actor IRI |
| `FMATCH_CANDIDATE_RESULT_INBOX_URL` | Callback inbox for fmatch |

Use a persistent, permission-restricted key path in deployments. Do not commit
private keys or copy development keys into a production image.

## Rates, pairs, and P2P search

| Variable | Purpose |
| --- | --- |
| `FX_SOURCE` | FX source; Compose uses `mock` |
| `FX_URL` | External FX endpoint when enabled |
| `FX_MARGIN_PERCENT` | FX margin applied by the backend |
| `SERVICE_FEE_PERCENT` | Service fee used by legacy payment paths |
| `PAIRS_CACHE_TTL_SECS` | Exchange-pair catalog cache lifetime |
| `P2P_SEARCH_ENABLED` | Enable read-only P2P search |
| `P2P_SEARCH_TIMEOUT_MS` | Default per-source timeout (4 seconds by default); a Providerfile may override it |
| `P2P_SEARCH_CACHE_TTL_MS` | P2P search cache lifetime |
| `P2P_SEARCH_ASSETS` | Optional comma-separated intermediary assets; defaults to the migrated network catalog |
| `PLAYWRIGHT_CHROMIUM_EXECUTABLE` | Chromium binary used by Providerfile browser workflows |
| `P2P_WORKFLOW_DEBUG_SCREENSHOT` | Optional failure-screenshot path for workflow diagnostics |

## Internal route capabilities

| Variable | Purpose | Development default |
| --- | --- | --- |
| `NEAR_INTENTS_URL` | 1Click API base URL; the adapter uses its `/v0` API | `https://1click.chaindefuser.com` |
| `NEAR_INTENTS_JWT` | Optional JWT for authenticated NEAR Intents quote/status calls | unset |
| `NEAR_INTENTS_QUOTE_RECIPIENT` | Valid destination-chain address used for internal dry quotes | unset |
| `NEAR_INTENTS_QUOTE_REFUND_TO` | Valid origin-chain refund address used for internal dry quotes | unset |
| `NEAR_INTENTS_REFRESH_SECS` | Supported-token and route-capability refresh interval | `300` |
| `ROUTE_MAX_DEPTH` | Maximum internal route-edge depth, from 1 to 8 | `4` |
| `ROUTE_SOURCE_FIATS` | Comma-separated fiat graph entry assets | `AMD` |
| `ROUTE_P2P_ASSETS` | Comma-separated P2P assets used to create entry edges | `USDT,USDC,XRP` |
| `ROUTE_INTENT_ASSETS` | Qualified assets eligible for NEAR Intents edges | `USDT@tron,USDC@solana,XRP@xrpl` |
| `ROUTE_WITHDRAWALS` | Deployment-owned edges in `FROM>TO\|provider[\|fee]` form; fee is in the origin asset | unset |

Route capabilities are published to fmatch as stable topology. Live prices and
quote expiry stay inside Pay3Flow and are returned only for a route id already
matched by fmatch; there is no new public route-search endpoint.

Individual sources, endpoints, mappings, and timeouts are defined in
Providerfiles and stored in the database migration, not environment flags.

## Production rules

- Replace every `dev-*` value.
- Keep secrets outside source control and outside public logs.
- Use TLS for browser, API, ActivityPub, database, and cache connections where
  the network is not fully trusted.
- Restrict admin credentials and rotate them through an auditable process.
- Disable mock FX, mock settlement, and unreviewed external adapters before
  enabling real-money behavior.
