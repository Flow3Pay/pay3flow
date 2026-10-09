# Pay3Flow

Pay3Flow is an experimental live route aggregator for exchanging fiat and
crypto across public P2P markets, direct exchangers, and spot markets. The web
application searches the selected venues in parallel, streams results as each
venue responds, and keeps the best route at the top of the ranking.

Pay3Flow currently supports seven route shapes:

```text
fiat   -> crypto -> fiat     AMD -> USDT -> RUB
cash   -> crypto -> fiat     USD cash -> USDT -> AMD
bank   -> crypto -> bank     USD account <-> USDT <-> Armenian USD account
fiat   -> crypto             RUB -> USDC
crypto -> fiat               USDC (ERC-20) -> RUB
crypto -> crypto             USDC (ERC-20) -> ETH
crypto -> crypto -> crypto   USDT (BEP-20) -> USDC -> USDT (BEP-20)
```

Search results are public market estimates. Eligible crypto swaps can be
executed in the app with Ethereum, NEAR, or TRON wallets. Wallets sign approvals,
deposits, and swap transactions; Pay3Flow tracks their completion without
holding funds. Other offers open at their venue and may change before execution.

## Current search sources

| Source | Integration | Coverage used by the router |
| --- | --- | --- |
| Binance | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| Bybit | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| OKX | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| Bitget | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| MEXC | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| Rapira | Public P2P API | `USDT/RUB` P2P ads |
| Whitebird | Anonymous quote API used by the public exchanger | Direct fiat/crypto quotes, including `USDC/RUB` |
| Cifra Markets | Public calculator and IMEX market API | Direct RUB/BYN/USD crypto quotes and crypto market paths |
| SkyLabs | Public homepage rate API | Direct AMD/USD fiat-to-crypto and crypto-to-fiat quotes |
| bncex | Public calculator quote API | Direct AMD/USDT and AMD/USDC quotes; available as the AMD exit for RUB→AMD routes |
| Bitcoin Center | Public route API | AMD bank transfer ↔ USDT on Solana, including the reported output fee |
| BestChange | Official authenticated rates API | Aggregated exchanger offers |
| Dzengi | Public market-data API | Crypto spot-market paths |
| CoW Swap | CoW Protocol quote API | Selectable swaps and bridge quotes discovered from live token catalogs |
| NEAR Intents | 1Click quote API | Selectable cross-chain and same-chain crypto swap quotes from the live token catalog |
| Symbiosis | Official cross-chain swap API | Selectable cross-chain crypto quotes from the live Symbiosis token catalog |
| ID Pay | Public server-rendered calculator rate | Selectable direct AMD/RUB and RUB/AMD transfer estimates |

CoW Swap loads token addresses and per-network precision from its live token
list and the bridge catalog. The protocol network registry supplies public API
endpoints; `cow_api_urls` can override them. The shared `cow_tokens` settings
remain an offline fallback for legacy adapters and custom deployments.
Bridge destinations and intermediate tokens are discovered at runtime. Quotes
combine CoW execution on the origin network with a dry NEAR Intents bridge
quote; the selected pair opens in CoW Swap for a fresh quote and execution.
Embedded CoW wallet execution supports same-chain ERC20 orders on EVM networks.
The header and swap instructions share wallet connections. Connecting in the
header does not initiate a swap. Starting a swap connects its source wallet,
validates the recipient, obtains a fresh quote, and opens the wallet's signing
prompt when funds are available. Approvals are confirmed before refreshing the
swap quote. Submitted transactions survive reloads and notification retries.

CoW Swap fees are quote-dependent rather than a universal fixed percentage.
The live quote accounts for execution costs, while liquidity, gas, and optional
partner fees can affect the result. The provider directory exposes this fee
model and links to the [CoW Protocol documentation](https://docs.cow.fi/cow-protocol).

CoW Swap, NEAR Intents, and Symbiosis are separate selectable exchanges. When selected,
their quoted outputs are independent and can differ because each provider uses
its own liquidity, execution costs, and fee model. Symbiosis uses a read-only
preview address for route discovery; eligible EVM and TRON swaps are re-quoted
with the user's wallet addresses and signed in the app. NEAR Intents supports
deposits from Ethereum, NEAR, and TRON, including token transfers.
Routes without an execution descriptor retain provider instructions. BestChange requires
`BESTCHANGE_API_KEY` from the referral-program dashboard; the numeric referral
ID used in public links is not an API key. The other listed
public-data adapters make anonymous read-only requests.

Provider capabilities live in
[`backend/providers/*/Providerfile`](backend/providers/). They are compiled
into [`backend/migrations/providers.sql`](backend/migrations/providers.sql) and
loaded into PostgreSQL when the backend starts.
Providerfiles also define provider-specific guidance shown in route and order
instructions. P2P providers can declare `currency = ["all"]` and use
`currency_exceptions` for unsupported fiat codes. They can use
`max_results = "infinite"` with paged requests to search more ads. See the
[Providerfile reference](Providerfile.md) for the supported fields and limits.

Selecting the same crypto asset on the same network searches circular routes.
Public swap providers each search up to 48 intermediary asset/network candidates
from their own catalogs. The return quote consumes the actual output of the
first quote; different BestChange exchangers remain separate combinations.
Requests to each provider are serialized and paced; quota failures trigger a
short cooldown, and identical amount-specific jobs share one quote within the
search. BestChange pair rates are retained for five seconds, with limits and
reserves checked again for every exact input amount. Provider responses and
failures remain visible even when they produce no profitable route.

The selected spot venues also contribute two-trade cycles across venues and
three-trade cycles within one venue, searching up to 96 intermediary symbols.
These estimates use bid for sales, ask for purchases, and an estimated 0.1%
trading fee per trade. They do not verify order-book depth, account-specific
fees, network deposits/withdrawals, or transfer costs; instructions identify
the required deposits, transfers and withdrawal back to the original wallet.
Spot instructions link to the trading pairs, separately from P2P guidance.
Fiat balances are excluded as intermediate assets. Estimated gains above
`max_price_deviation_bps` (10% by default) are discarded as unverified outliers.

The router checks continuity, amounts, and quote expiry and returns only cycles
with a positive quoted or estimated gain at eight decimal places.
`allow_cross_venue` controls whether steps may use different venues.
Cached exchange coefficients are not used for wallet cycles. The bounded
provider search has a 60-second budget and reports `routes_exhaustive = false`.
Disconnecting cancels pending search work; auto-refresh waits for the current
search to finish.

Cycle results include ordered `cycle_legs` with providers and input/output amounts.
`profitability_decimals = 8` specifies the scale of their profitability integers;
existing fiat cycles retain the default scale of 2. Quoted swap outputs include
the costs reflected by each provider, but wallet gas, approvals and deposit costs
are not fully established. These results therefore show an estimated gain with
unconfirmed net profit. Quotes must be refreshed before executing each step;
the service does not guarantee a profit or automatically execute the cycle.

## How live routing works

```mermaid
flowchart LR
    browser[Browser :3000] -->|REST and WebSocket| api[Axum API :8080]
    api --> postgres[(PostgreSQL)]
    api --> redis[(Redis)]
    api --> p2p[P2P APIs]
    api --> spot[Spot tickers]
    api --> whitebird[Whitebird quote API]
    api --> cifra[Cifra Markets API]
    api --> skylabs[SkyLabs homepage API]
    api --> bncex[bncex calculator API]
    api --> bitcoincenter[Bitcoin Center route API]
    p2p --> ranker[Route builder and ranker]
    spot --> ranker
    whitebird --> ranker
    cifra --> ranker
    skylabs --> ranker
    bncex --> ranker
    bitcoincenter --> ranker
    ranker -->|progressive snapshots| browser
```

- Selected sources and intermediary assets are searched concurrently.
- P2P route legs read provider advertisements directly; Fmatch candidate
  pages can omit valid ads from a venue. Direct-exchange discovery may use
  Fmatch.
- Each user P2P search requests current advertisements from the selected
  venues. Direct exchanger quotes and completed exchanger-only routes may use
  cached responses. Failed venue responses never become cached answers.
- `/ws/p2p/routes` publishes a new ranked snapshot whenever another source
  finishes; the UI does not wait for every venue before showing results.
- A failed or slow source is reported in route status data without discarding
  results already returned by other sources.
- Routes are ranked by estimated target amount first, then verified
  payment-method matches and same-venue execution.
- Search and route limits reserve several candidates per P2P source while
  keeping the returned routes ranked by price, so a busy venue does not
  crowd out other markets.
- The best route remains selected while results arrive unless the user has
  explicitly selected another route.
- Venue icons summarize successful source statuses, not only cards that made
  the final ranking. An icon therefore remains visible when a venue returned
  matching offers but those offers did not produce a ranked route.
- Click a venue icon above the results to show its ranked routes; click it
  again to return to all venues. Shared `/swap/SOURCE/TARGET?amount=...` links
  restore the selected currencies in a new browser session.
- Route cards are rendered immediately for responses of up to 100 items. For
  larger snapshots, the frontend renders 100 cards per batch, waits for a
  browser paint and then 10 ms before the next batch. Progressive snapshots
  preserve the number of cards already revealed even when ranking changes, so
  neither the list nor its found-route counter collapses and grows again. A
  newer search cancels the remaining batches from the previous snapshot.
- Public P2P ads are protected by price-deviation filtering. Direct exchanger
  quotes such as Whitebird and SkyLabs are retained as independent quotes
  rather than compared as if they were P2P ads.

## Quick start with Docker

Requirements: Docker with Compose support.

Start the live-routing stack:

```bash
cp backend/.env.example backend/.env
# Add BESTCHANGE_API_KEY if you use the BestChange source.
docker compose up -d --build postgres redis backend pay3low-svelte-frontend
curl -fsS http://localhost:8080/health
```

Open <http://localhost:3000>. The API is available at
<http://localhost:8080>.

The backend image includes Chromium and the Playwright driver for providers
that use browser workflows; Whitebird itself uses its anonymous quote API. The
compose file also contains the older `fmatch` stack; that stack needs a
separate `fmatch/` checkout. Production direct-exchanger discovery can use
Lefine's Fmatch actor, while P2P advertisements are queried live from each
venue.

To stop the stack:

```bash
docker compose down
```

## Route search API

### Complete routes

`GET /api/p2p/routes` builds and ranks complete routes. Despite the legacy
field names `source_fiat` and `target_fiat`, either side may be a supported
crypto asset. Use `source_network` or `target_network` when a crypto asset is
selected.

For example, sell 100 USDC on Ethereum for RUB received through Sberbank and
search every live source:

```bash
curl -G 'http://localhost:8080/api/p2p/routes' \
  --data-urlencode 'source_fiat=USDC' \
  --data-urlencode 'source_network=ethereum' \
  --data-urlencode 'target_fiat=RUB' \
  --data-urlencode 'source_amount=100' \
  --data-urlencode 'target_payment_method=Sberbank' \
  --data-urlencode 'sources=binance,bybit,okx,bitget,rapira,whitebird,cifra-broker,skylabs' \
  --data-urlencode 'allow_cross_venue=true' \
  --data-urlencode 'limit=40'
```

Search a fiat-to-fiat route through one of several crypto assets:

```bash
curl -G 'http://localhost:8080/api/p2p/routes' \
  --data-urlencode 'source_fiat=AMD' \
  --data-urlencode 'target_fiat=RUB' \
  --data-urlencode 'source_amount=100000' \
  --data-urlencode 'intermediary_assets=USDT,USDC,BTC,ETH' \
  --data-urlencode 'allow_cross_venue=true'
```

Useful route parameters:

| Parameter | Meaning |
| --- | --- |
| `source_fiat`, `target_fiat` | Source and target currency or asset codes |
| `source_amount` | Positive amount in source-currency units |
| `source_network`, `target_network` | Canonical network IDs from `/api/networks` |
| `intermediary_assets` | Comma-separated crypto bridges for fiat routes or crypto cycles; omitted crypto-cycle intermediaries come from provider catalogs |
| `source_payment_method`, `target_payment_method` | Bank or payment-method filters |
| `sources` | Comma-separated source slugs; omit to search all configured adapters |
| `allow_cross_venue` | Allow routes whose legs execute on different venues |
| `merchant_only` | Keep merchant ads only |
| `min_orders` | Minimum completed orders reported by a venue |
| `min_completion_rate` | Completion-rate fraction from `0` to `1` |
| `max_price_deviation_bps` | P2P outlier threshold for multi-leg routes; default `1000` (10%). Direct fiat/crypto routes keep real ads and rank them by price. |
| `limit` | Returned route limit from `1` to `100`; default `20` |

`routes_found` counts all unique valid routes before `limit` is applied.

### Progressive WebSocket search

Connect to `ws://localhost:8080/ws/p2p/routes` and send exactly one JSON
request after the socket opens:

```json
{
  "anonymous_id": "2a97cff7-20ab-4550-a9bf-4ec5509510a0",
  "query": {
    "source_fiat": "USDC",
    "source_network": "ethereum",
    "target_fiat": "RUB",
    "source_amount": 100,
    "target_payment_method": "Sberbank",
    "sources": "binance,bybit,okx,bitget,rapira,whitebird,cifra-broker,skylabs",
    "allow_cross_venue": true,
    "min_orders": 20,
    "min_completion_rate": 0.9,
    "limit": 40
  }
}
```

The server responds with:

1. `search_started` — the search ID was allocated.
2. Zero or more `routes_updated` snapshots — sources are still completing.
3. `search_finished` with the final snapshot, or `search_failed`.

Every `routes_updated` and `search_finished` event has the same fields as the
REST response, plus `type`.

### Single P2P leg

`GET /api/p2p/search` returns raw public advertisements for one fiat/asset
side. It is useful for adapter diagnostics; the main UI uses complete routes.
`amount` is a fiat amount. For a crypto-to-fiat search, pass `asset_amount`
to retain ads whose available asset and fiat order limits cover that amount.

```bash
curl -G 'http://localhost:8080/api/p2p/search' \
  --data-urlencode 'fiat=RUB' \
  --data-urlencode 'asset=USDC' \
  --data-urlencode 'side=sell' \
  --data-urlencode 'amount=100' \
  --data-urlencode 'payment_method=Sberbank' \
  --data-urlencode 'sources=binance,bybit,okx,bitget,whitebird,cifra-broker,skylabs'
```

## Main API surface

| Method | Endpoint | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Backend liveness check |
| `GET` | `/api/providers` | Database-backed provider catalog |
| `GET` | `/api/banks` | Bank and payment-method catalog |
| `GET` | `/api/networks` | Crypto networks and compatible assets |
| `GET` | `/api/p2p/search` | Search one P2P/direct-exchange leg |
| `GET` | `/api/p2p/routes` | Build a final ranked route snapshot |
| `WS` | `/ws/p2p/routes` | Stream progressive ranked snapshots |
| `POST` | `/api/service-executions/open` | Record that a route service link was opened |
| `POST` | `/api/p2p/route-executions` | Prepare a wallet-bound quote from a signed route token |
| `GET` | `/api/p2p/route-executions/{id}` | Read execution status for its anonymous owner |
| `POST` | `/api/p2p/route-executions/{id}/submissions` | Record a broadcast transaction hash or CoW order UID |
| `PUT` | `/api/services/{id}/vote` | Add or change anonymous feedback |
| `PUT` | `/api/routes/{route_id}/vote` | Add or change anonymous feedback for one concrete route |

The live-routing UI keeps a random browser identifier in local storage; it is
not connected to a registered user. A browser can keep one like or dislike per
service, stored in `service_votes`. Like and dislike totals are revealed in the
UI only after that browser has voted for the service.

Route-card feedback is separate: it is stored in `route_votes` by the concrete
route fingerprint, so a vote on one composed route is not copied to every route
that happens to use the same exchange service.

The repository also retains authentication, payments, ActivityPub discovery,
and the earlier `/api/exchange/orders` workflow. Those APIs are secondary to
the current live-routing UI; see
[`docs/api-overview.md`](docs/api-overview.md) for their route families.

## Local development

### Backend

Requirements: Rust 1.97+, PostgreSQL 16, Redis 7, Chromium, and a Playwright
driver when workflow-based sources are enabled.

```bash
cargo test --manifest-path backend/Cargo.toml
set -a; source backend/.env; set +a
cargo run --manifest-path backend/Cargo.toml --bin pay3flow-backend
```

The default local database URL uses port `5432`. The Docker PostgreSQL service
is exposed on port `5435`, so set the URL when running the backend on the host:

```bash
DATABASE_URL=postgres://pay3flow:pay3flow@localhost:5435/pay3flow \
REDIS_URL=redis://localhost:6379 \
cargo run --manifest-path backend/Cargo.toml --bin pay3flow-backend
```

When a Providerfile changes, regenerate its embedded SQL and check the diff:

```bash
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- generate
cargo run --manifest-path backend/Cargo.toml --bin providerfile -- check
```

Optional `[test]` sections run before SQL generation and stop it on failure.
Use `cargo run --manifest-path backend/Cargo.toml --bin providerfile -- test`
to validate and run them without writing SQL;
HTTP assertions and standalone Rust examples are documented in [Providerfile.md](Providerfile.md#tests-before-sql-generation).

### Frontend

Requirements: Node.js 24 and npm.

```bash
cd pay3low-svelte-frontend
npm ci
PUBLIC_API_URL=http://localhost:8080 npm run dev
```

Verification commands:

```bash
npm run check
npm run build
npm run test:e2e
```

Production builds run with `npm start`, which serves the adapter-node output
through [`server.mjs`](pay3low-svelte-frontend/server.mjs).

## Configuration

Non-secret backend settings live in [`config.toml`](config.toml), including
listener, ActivityPub, FX, P2P, route, NEAR Intents, Symbiosis, and optional CoW settings.
Use TOML arrays for lists. Credentials and connection strings remain
environment-only: `DATABASE_URL`, `REDIS_URL`, `JWT_SECRET`, `SECRETS_KEY`,
`ADMIN_TOKEN`, `NEAR_INTENTS_JWT`, and the optional `SYMBIOSIS_PARTNER_ID`.

See [`docs/configuration.md`](docs/configuration.md) for the schema and the
secret boundary. Individual Providerfiles may override the default timeout.
Whitebird currently uses a 5-second HTTP timeout for its anonymous quote API.

## Repository layout

```text
pay3flow/
├── backend/
│   ├── providers/             # Providerfiles and adapter definitions
│   ├── migrations/            # Schema, catalogs and generated provider SQL
│   └── src/                   # Rust API and routing logic
├── pay3low-svelte-frontend/   # SvelteKit application
├── deploy/                    # Kubernetes templates and render scripts
├── docs/                      # Design, API and operations notes
├── scripts/                   # Health, smoke and secret-audit scripts
├── docker-compose.yml
└── flake.nix                  # Nix deployment helpers
```

## Development ports

| Service | Host port |
| --- | ---: |
| Frontend | `3000` |
| Backend REST/WebSocket API | `8080` |
| PostgreSQL | `5435` |
| Redis | `6379` |

The optional legacy fmatch services use `7277`, `5433`, and `8108`.

## Safety and limitations

- This is experimental software, not production-ready payment infrastructure.
- External sites can change or rate-limit undocumented public endpoints at any
  time.
- Payment-method names are not equally detailed across venues. The response
  exposes `payment_methods_verified` and route warnings when matching is
  uncertain.
- Cross-venue routes may require an external asset transfer. Check the route's
  `requires_asset_transfer`, network, fee, and warning fields before acting.
- Legal, compliance, sanctions, tax, custody, security, and operational review
  remain the responsibility of any real deployment.

## Further documentation

- [`docs/p2p-search.md`](docs/p2p-search.md) — route-search internals and source behavior.
- [`Providerfile.md`](Providerfile.md) — complete provider format and examples.
- [`docs/api-overview.md`](docs/api-overview.md) — broader API families.
- [`docs/development.md`](docs/development.md) — development and test conventions.
- [`docs/deployment.md`](docs/deployment.md) — deployment checklist and blockers.

## License

Pay3Flow is licensed under the GNU Affero General Public License, version 3 or
any later version (`AGPL-3.0-or-later`). See [`LICENSE`](LICENSE).
