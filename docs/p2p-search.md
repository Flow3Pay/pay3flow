# Live P2P Search

Pay3Flow has a read-only P2P search layer. It reads public advertisements and
does not contact advertisers, create platform orders, reserve crypto, or move
money.

## Sources

- Binance public C2C agent advertisement list.
- Bybit public web P2P advertisement list.

Both sources are queried concurrently. A timeout or parsing failure from one
source is returned in `sources` without discarding successful results from the
other source. Successful leg searches are cached in memory for five seconds by
default; cached responses contain `cached: true`.

The public website endpoints can change without notice. Keep both adapters
enabled, monitor `sources[].ok`, and do not treat a search result as a firm
quote until the platform confirms it.

## Find one P2P leg

```text
GET /api/p2p/search?fiat=AMD&asset=USDT&side=buy&amount=100000&min_orders=20&min_completion_rate=0.9&limit=20
```

`side` is always from the Pay3Flow user's perspective:

- `buy`: pay fiat and receive the crypto asset;
- `sell`: give the crypto asset and receive fiat.

The response contains normalized prices, fiat limits, available asset amount,
payment methods, public advertiser reputation, source status, and a link back
to the venue.

## Build the AMD -> asset -> RUB puzzle

```text
GET /api/p2p/routes?source_fiat=AMD&target_fiat=RUB&source_amount=100000&assets=USDT,USDC,BTC,ETH&min_orders=20&min_completion_rate=0.9&limit=20
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
P2P_SEARCH_ASSETS=USDT,USDC,BTC,ETH
```

Endpoint URLs can be overridden with `P2P_BINANCE_URL` and `P2P_BYBIT_URL` for
tests or when a venue changes its public endpoint.

Run the opt-in live smoke test:

```bash
cd backend
cargo test live_amd_to_rub_route_search -- --ignored --nocapture
```
