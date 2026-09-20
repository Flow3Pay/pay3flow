# Live P2P Search

Pay3Flow has a read-only P2P search layer. It reads public advertisements and
does not contact advertisers, create platform orders, reserve crypto, or move
money.

## Sources

- Binance public C2C agent advertisement list.
- Bybit public web P2P advertisement list.
- OKX public web P2P advertisement list.
- Bitget public web P2P advertisement list.

These are the four sources currently configured. The included Docker Compose
deployment enables them by default; other deployments can enable them through
the provider-specific environment variables below. Other
venues from the research list are intentionally not presented as live sources:
MEXC's official P2P Open API requires merchant/API access, OKX's merchant API
has separate eligibility requirements, and no stable public advertisement API
has been approved for BingX or the remaining venues. Every new venue still
needs an explicit decision about its official API access and response mapping.

Provider connections are declared in `backend/providers/reg.json`. Each entry
points to a JSON request/response mapping under `backend/providers/<name>/`.
These files are validated and embedded into the backend during the Cargo build;
the running container does not need to mount the provider directory. A new
provider that exposes a compatible JSON API can therefore be added by creating
its mapping and adding one line to `reg.json`, without a new Rust adapter.

### Provider registry

The registry is a JSON object whose keys are source slugs and whose values are
paths relative to `backend/providers/`:

```json
{
  "binance": "./binance/binance.json",
  "new_exchange": "./new_exchange/new_exchange.json"
}
```

Each provider file contains these main sections:

```json
{
  "name": "new_exchange",
  "kind": "p2p",
  "enabled": false,
  "endpoint": "https://example.com/api/p2p",
  "request": {
    "method": "POST",
    "headers": {},
    "query": {},
    "body": {},
    "side": { "buy": "BUY", "sell": "SELL" }
  },
  "response": {
    "success": { "path": "/code", "equals": "00000" },
    "items_path": "/data/items",
    "fields": {},
    "advertiser": {},
    "merchant_rules": [],
    "urls": {}
  }
}
```

Request and URL strings support these templates:

- `{{fiat}}`, `{{asset}}` — normalized query codes;
- `{{side}}` — provider-specific side from `request.side`;
- `{{user_side}}` — Pay3Flow side, `buy` or `sell`;
- `{{fetch_limit}}`, `{{amount}}`, `{{payment_method}}` — query values.

Response field paths use JSON Pointer syntax. The `*` segment collects values
from an array, which is useful for nested payment-method objects. A field can
also specify `"transform": "ratio"` or `"transform": "percentage"` for
completion and positive-review rates.

Merchant rules support `present`, `nonempty`, `truthy`, `equals`, `equals_ci`
and `gt`. `verified_rules` is optional; when omitted, verified status follows
the merchant result. The normalized fields expected by the backend are:
`ad_id`, `fiat`, `asset`, `price`, `available_asset`, `min_fiat`,
`max_fiat`, `payment_methods`, `pay_time_limit_minutes`, plus advertiser
fields `id`, `nickname`, `user_type`, `completed_orders_30d`,
`completion_rate_30d` and `positive_rate`.

`enabled` is the default activation flag. It can be overridden at runtime with
`P2P_<SLUG>_ENABLED=true|false`; the endpoint can be overridden with
`P2P_<SLUG>_URL`. For example, `new_exchange` uses
`P2P_NEW_EXCHANGE_ENABLED` and `P2P_NEW_EXCHANGE_URL`.

During `cargo build`, `backend/build.rs` reads `reg.json`, checks every linked
file stays inside `backend/providers/`, and generates embedded `include_str!`
entries. Runtime configuration is deserialized from those embedded strings, so
changing a provider JSON requires rebuilding the backend image.

All sources are queried concurrently. A timeout or parsing failure from one
source is returned in `sources` without discarding successful results from the
other source. Successful leg searches are cached in memory for five seconds by
default; cached responses contain `cached: true`.

The public website endpoints can change without notice. Keep the adapters
enabled, monitor `sources[].ok`, and do not treat a search result as a firm
quote until the platform confirms it.

## Find one P2P leg

```text
GET /api/p2p/search?fiat=AMD&asset=USDT&side=buy&amount=100000&min_orders=20&min_completion_rate=0.9&sources=binance,okx&limit=20
```

`side` is always from the Pay3Flow user's perspective:

- `buy`: pay fiat and receive the crypto asset;
- `sell`: give the crypto asset and receive fiat.

The response contains normalized prices, fiat limits, available asset amount,
payment methods, public advertiser reputation, source status, the advertiser
profile URL when the venue exposes a stable public profile, and the source
advertisement URL. `source_url_is_exact` marks whether the latter opens the
exact advertisement returned by the source. The frontend instructions focus on
the advertiser profile and nickname; when no stable profile URL is available,
they open the venue's P2P market and tell the user to find the advertiser by
nickname and verify the ad ID. Routes keep valuable offers even when a venue
only provides a generic market URL. Binance, OKX, and Bitget currently expose
public advertiser profile URLs, including Bybit's profile route built from its
masked public identifier. All four venues can contribute useful market offers
without being silently discarded.

`payment_method` accepts the bank selected in the frontend. Named payment
methods are matched after punctuation/case normalization. Some venues return
only opaque numeric payment-method IDs; those offers remain visible as
unverified estimates instead of being silently discarded.

## Build the AMD -> asset -> RUB puzzle

```text
GET /api/p2p/routes?source_fiat=AMD&target_fiat=RUB&source_amount=100000&assets=USDT,USDC,BTC,ETH&min_orders=20&min_completion_rate=0.9&sources=binance,okx&limit=20
```

For every asset the backend concurrently searches:

```text
AMD -> asset  (user buys crypto)
asset -> RUB  (user sells crypto)
```

It then:

1. rejects price outliers beyond `max_price_deviation_bps` (default 1000,
   or 10% from the median for the leg);
2. checks that the AMD amount fits the entry advertisement limits;
3. calculates the acquired asset amount and checks entry liquidity;
4. calculates the RUB output and checks the exit advertisement limits and
   liquidity;
5. returns complete routes ordered by maximum estimated RUB output.

The optional `sources` parameter limits both legs to a comma-separated list of
enabled venues: `binance`, `bybit`, `okx`, and `bitget`. If omitted, all enabled
venues are queried. The selected venue list is part of the search cache key.

Routes whose two selected banks are explicitly named by both advertisements
are ranked ahead of routes with opaque payment IDs. The response exposes this
as `payment_methods_verified`; unverified routes also include a warning.

By default only same-venue routes are returned. They do not require moving the
asset from one exchange to another. Set `allow_cross_venue=true` to include
cross-venue candidates; they are marked `requires_asset_transfer=true`, and
their network compatibility and transfer fee are deliberately not claimed as
verified.

Important: `target_amount` is a search estimate. Platform fees, account/KYC
eligibility, ad availability at execution time, sanctions/geographic rules,
and payment confirmation are not verified by this read-only layer.

## Configuration

```text
P2P_SEARCH_ENABLED=true
P2P_SEARCH_TIMEOUT_MS=4000
P2P_SEARCH_CACHE_TTL_MS=5000
P2P_BINANCE_ENABLED=true
P2P_BYBIT_ENABLED=true
P2P_OKX_ENABLED=true
P2P_BITGET_ENABLED=true
P2P_SEARCH_ASSETS=USDT,USDC,BTC,ETH
```

Endpoint URLs can be overridden with `P2P_BINANCE_URL`, `P2P_BYBIT_URL`,
`P2P_OKX_URL`, and `P2P_BITGET_URL` for tests or when a venue changes its
public endpoint.

Run the opt-in live smoke test:

```bash
cd backend
cargo test live_amd_to_rub_route_search -- --ignored --nocapture
```
