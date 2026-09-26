# Configuration

Pay3Flow reads non-secret runtime settings from the repository-root
[`config.toml`](../config.toml). The backend also searches the parent directory
when launched from `backend/`. Set `PAY3FLOW_CONFIG_FILE` only when a deployment
needs to mount the TOML at another path.

For example:

```toml
http_addr = "0.0.0.0:8080"
ap_origin = "https://pay3flow.example"
fmatch_inbox = "https://lefine.pro/inbox/actra"
fmatch_actor_id = "https://lefine.pro/actors/actra"

fx_source = "http"
fx_url = "https://api.frankfurter.app/latest"
p2p_search_enabled = true
route_source_fiats = ["RUB", "AMD"]
route_intent_assets = ["USDT@tron", "USDT@avalanche-c", "USDT@solana", "USDC@solana"]

near_intents_quote_recipient = "<valid destination-chain address>"
near_intents_quote_refund_to = "<valid origin-chain address>"
near_intents_quote_recipients = [
  "solana=<valid Solana address>",
  "bitcoin=<valid Bitcoin address>",
  "near=<valid NEAR account>",
  "avalanche-c=<valid EVM address>",
]
near_intents_quote_refunds = [
  "solana=<valid Solana address>",
  "bitcoin=<valid Bitcoin address>",
  "near=<valid NEAR account>",
  "ethereum=<valid EVM address>",
]

# Optional same-chain CoW Protocol quotes.
cow_api_urls = ["ethereum=https://api.cow.fi/mainnet"]
cow_tokens = [
  "ethereum:USDT=0xdAC17F958D2ee523a2206206994597C13D831ec7",
]
cow_quote_address = "0x0000000000000000000000000000000000000001"
```

The TOML schema rejects unknown keys. This intentionally prevents credentials
from being placed in the file by mistake.

## Environment-only secrets

These values are never read from `config.toml`:

| Variable | Purpose |
| --- | --- |
| `DATABASE_URL` | PostgreSQL connection, possibly containing a password |
| `REDIS_URL` | Redis connection, possibly containing a password |
| `JWT_SECRET` | Signs user tokens |
| `SECRETS_KEY` | Encrypts stored sensitive values |
| `ADMIN_TOKEN` | Protects admin exchange controls |
| `NEAR_INTENTS_JWT` | Optional authenticated 1-Click API credential |

Development fallbacks exist for local startup; replace them in every
deployment that handles real users or funds.

## TOML sections

| Keys | Purpose |
| --- | --- |
| `http_addr`, `log_filter` | Backend listener and structured log filter |
| `ap_*`, `fmatch_*` | ActivityPub identity and fmatch endpoints |
| `service_fee_percent`, `fx_*`, `pairs_cache_ttl_secs` | Fees, FX source, and pair cache |
| `p2p_search_*`, `playwright_chromium_executable`, `p2p_workflow_debug_screenshot` | Read-only public P2P search and browser workflow behavior |
| `near_intents_*` | 1-Click API endpoint, public quote addresses, and refresh interval |
| `route_*` | Fiat, asset, withdrawal, and route-depth capabilities |
| `cow_*` | Optional CoW same-chain quote endpoints and token metadata |

Use TOML arrays for lists; the old comma-separated environment variables are no
longer configuration inputs.

## Production rules

- Keep `config.toml` free of credentials and private keys.
- Replace every development secret before production.
- Use TLS for browser, API, ActivityPub, database, and cache connections where
  the network is not fully trusted.
- Disable mock FX, mock settlement, and unreviewed external adapters before
  enabling real-money behavior.
