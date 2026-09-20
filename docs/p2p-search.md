# Live P2P Search

Pay3Flow has a read-only P2P search layer. It reads public advertisements and
does not contact advertisers, create platform orders, reserve crypto, or move
money.

## Sources

- Binance public C2C agent advertisement list.
- Bybit public web P2P advertisement list.
- OKX public web P2P advertisement list.
- Bitget public web P2P advertisement list.

These are the four sources currently implemented and enabled by default. Other
venues from the research list are intentionally not presented as live sources:
MEXC's official P2P Open API requires merchant/API access, OKX's merchant API
has separate eligibility requirements, and no stable public advertisement API
has been approved for BingX or the remaining venues. Adding a venue requires a
dedicated adapter and an explicit decision about its official API access.

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
public advertiser profile URLs; Bybit exposes only a masked public identifier,
so it still uses the venue fallback and asks the user to verify the nickname.
All four venues can contribute useful market offers without being silently
discarded.

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
