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
p2p_fmatch_stale_secs = 900
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

# CoW discovers tokens and bridge routes; token entries provide an offline fallback.
cow_api_urls = ["ethereum=https://api.cow.fi/mainnet"]
cow_token_list_url = "https://files.cow.fi/tokens/CowSwap.json"
cow_tokens = [
  "ethereum:USDT=0xdAC17F958D2ee523a2206206994597C13D831ec7",
]
cow_quote_address = "0x0000000000000000000000000000000000000001"

# Optional Symbiosis cross-chain quotes.
symbiosis_url = "https://api.symbiosis.finance/crosschain"
symbiosis_quote_address = "0x0000000000000000000000000000000000000001"
symbiosis_slippage_bps = 300

wallet_execution_enabled = true
# Only explicitly configured Symbiosis routers and approval gateways may execute.
symbiosis_execution_contracts = [
  "728126428=0x0863786bbf4561f4a2a8be5a9ddf152afd8ae25c,0x49e1816a2cf475515e7c80c9f0f0e16ae499198b",
]
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
| `SYMBIOSIS_PARTNER_ID` | Optional Symbiosis partner credential |

Development fallbacks exist for local startup; replace them in every
deployment that handles real users or funds.

## TOML sections

| Keys | Purpose |
| --- | --- |
| `http_addr`, `log_filter` | Backend listener and structured log filter |
| `ap_*`, `fmatch_*` | ActivityPub identity and fmatch endpoints |
| `service_fee_percent`, `fx_*`, `pairs_cache_ttl_secs` | Fees, FX source, and pair cache |
| `p2p_search_*`, `p2p_fmatch_stale_secs`, `playwright_chromium_executable`, `p2p_workflow_debug_screenshot` | Read-only public P2P search, Fmatch cache, and browser workflow behavior |
| `near_intents_*` | 1-Click API endpoint, public quote addresses, and refresh interval |
| `route_*` | Fiat, asset, withdrawal, and route-depth capabilities |
| `cow_*` | Optional CoW same-chain quote endpoints and token metadata |
| `symbiosis_*` | Symbiosis cross-chain quote endpoint, preview address, and slippage |
| `wallet_execution_enabled` | Attach executable descriptors to supported wallet routes; set false to disable new execution quotes |

Use TOML arrays for lists; the old comma-separated environment variables are no
longer configuration inputs.

## Wallet swaps

The frontend connects injected Ethereum wallets, NEAR Wallet Selector wallets,
and TronLink on mainnet. Connections are shared between the header and route
instructions. The optional frontend `PUBLIC_REOWN_PROJECT_ID` enables the Reown
wallet picker; injected Ethereum wallets work without it.

NEAR Intents execution supports deposits from the supported EVM networks, NEAR,
and TRON. CoW execution is limited to supported same-chain ERC20 pairs.
Symbiosis execution requires a source-chain entry in
`symbiosis_execution_contracts`, formatted as `chain-id=router,gateway`; NEAR
origins and destinations use NEAR Intents. The repository configuration includes
the TRON and supported EVM Symbiosis contracts. Other routes continue to use provider instructions.

Quotes use live token precision and expire. Source and recipient addresses are
validated for their respective networks. Wallet account or network changes
invalidate prepared signing actions. After broadcast, the browser retains the
reference so tracking can resume without signing again.

## Production rules

- Keep `config.toml` free of credentials and private keys.
- Replace every development secret before production.
- Use TLS for browser, API, ActivityPub, database, and cache connections where
  the network is not fully trusted.
- Disable mock FX, mock settlement, and unreviewed external adapters before
  enabling real-money behavior.
