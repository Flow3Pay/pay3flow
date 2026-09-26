# Pay3Flow

Pay3Flow is an experimental live route aggregator for exchanging fiat and
crypto across public P2P markets, direct exchangers, and spot markets. The web
application searches the selected venues in parallel, streams results as each
venue responds, and keeps the best route at the top of the ranking.

Pay3Flow currently supports four route shapes:

```text
fiat   -> crypto -> fiat     AMD -> USDT -> RUB
fiat   -> crypto             RUB -> USDC
crypto -> fiat               USDC (ERC-20) -> RUB
crypto -> crypto             USDC (ERC-20) -> ETH
```

Search results are public market estimates. Pay3Flow does not place an order,
contact an advertiser, hold funds, or guarantee that a displayed offer will
still be available when the venue is opened.

## Current search sources

| Source | Integration | Coverage used by the router |
| --- | --- | --- |
| Binance | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| Bybit | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| OKX | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| Bitget | Public P2P API and spot ticker | P2P ads plus crypto market paths |
| Rapira | Public P2P API | `USDT/RUB` P2P ads |
| Whitebird | Anonymous quote API used by the public exchanger | Direct fiat/crypto quotes, including `USDC/RUB` |
| Cifra Markets | Public calculator and IMEX market API | Direct RUB/BYN/USD crypto quotes and crypto market paths |
| SkyLabs | Public homepage rate API | Direct AMD/USD fiat-to-crypto and crypto-to-fiat quotes |
| bncex | Public calculator quote API | Direct AMD/USDT and AMD/USDC quotes; available as the AMD exit for RUB→AMD routes |
| BestChange | Public catalog and direction pages | Aggregated exchanger offers |
| Dzengi | Public market-data API | Crypto spot-market paths |
| Exnode | HMAC-authenticated merchant quote API | Direct quotes when API credentials are configured |
| CoW Swap | CoW Protocol quote API | Selectable same-chain crypto swap quotes for configured chains and tokens |
| NEAR Intents | 1Click quote API | Selectable cross-chain and same-chain crypto swap quotes from the live token catalog |
| ID Pay | Public server-rendered calculator rate | Selectable direct AMD/RUB and RUB/AMD transfer estimates |

CoW Swap fees are quote-dependent rather than a universal fixed percentage.
The live quote accounts for execution costs, while liquidity, gas, and optional
partner fees can affect the result. The provider directory exposes this fee
model and links to the [CoW Protocol documentation](https://docs.cow.fi/cow-protocol).

CoW Swap and NEAR Intents are separate selectable exchanges. When selected,
their quoted outputs are independent and can differ because each provider uses
its own liquidity, execution costs, and fee model. Exnode is the only
Providerfile adapter in the table that requires provider credentials. Set
`EXNODE_API_PUBLIC` and `EXNODE_API_PRIVATE` to use it; the other listed
public-data adapters make anonymous read-only requests.

Provider capabilities live in
[`backend/providers/*/Providerfile`](backend/providers/). They are compiled
into [`backend/migrations/providers.sql`](backend/migrations/providers.sql) and
loaded into PostgreSQL when the backend starts.

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
    p2p --> ranker[Route builder and ranker]
    spot --> ranker
    whitebird --> ranker
    cifra --> ranker
    skylabs --> ranker
    bncex --> ranker
    ranker -->|progressive snapshots| browser
```

- Selected sources and intermediary assets are searched concurrently.
- `/ws/p2p/routes` publishes a new ranked snapshot whenever another source
  finishes; the UI does not wait for every venue before showing results.
- A failed or slow source is reported in route status data without discarding
  results already returned by other sources.
- Routes with verified payment-method matches rank first, followed by target
  amount and same-venue execution.
- The best route remains selected while results arrive unless the user has
  explicitly selected another route.
- Public P2P ads are protected by price-deviation filtering. Direct exchanger
  quotes such as Whitebird and SkyLabs are retained as independent quotes
  rather than compared as if they were P2P ads.

## Quick start with Docker

Requirements: Docker with Compose support.

Start the live-routing stack:

```bash
docker compose up -d --build postgres redis backend pay3low-svelte-frontend
curl -fsS http://localhost:8080/health
```

Open <http://localhost:3000>. The API is available at
<http://localhost:8080>.

The backend image includes Chromium and the Playwright driver for providers
that use browser workflows; Whitebird itself uses its anonymous quote API. The
compose file also contains the older `fmatch` stack;
that stack needs a separate `fmatch/` checkout and is not required for the
read-only live route search.

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
| `intermediary_assets` | Comma-separated crypto bridges for fiat-to-fiat routes |
| `source_payment_method`, `target_payment_method` | Bank or payment-method filters |
| `sources` | Comma-separated source slugs; omit to search all configured adapters |
| `allow_cross_venue` | Allow routes whose legs execute on different venues |
| `merchant_only` | Keep merchant ads only |
| `min_orders` | Minimum completed orders reported by a venue |
| `min_completion_rate` | Completion-rate fraction from `0` to `1` |
| `max_price_deviation_bps` | P2P outlier threshold; default `1000` (10%) |
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
listener, ActivityPub, FX, P2P, route, NEAR Intents, and optional CoW settings.
Use TOML arrays for lists. Credentials and connection strings remain
environment-only: `DATABASE_URL`, `REDIS_URL`, `JWT_SECRET`, `SECRETS_KEY`,
`ADMIN_TOKEN`, and `NEAR_INTENTS_JWT`.

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
