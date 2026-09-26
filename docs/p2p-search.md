# Live P2P Search

Pay3Flow has a read-only P2P search layer. It reads public advertisements and
does not contact advertisers, create platform orders, reserve crypto, or move
money.

## Sources

- Binance, Bybit, OKX, Bitget, and Rapira public P2P advertisement lists.
- Whitebird, Cifra Markets, and SkyLabs public direct-quote APIs.
- BestChange public catalog and direction pages.
- Exnode authenticated quote API when its environment credentials are set.
- Binance, Bybit, OKX, Bitget, Cifra Markets, and Dzengi spot tickers for
  crypto-to-crypto paths.

Providerfile-backed sources are discovered from the generated provider catalog;
the list is not hardcoded in the route API. Other venues from the research list
remain outside the live path until a legitimate read-only interface and adapter
review exist.

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
only provides a generic market URL. Binance, Bybit, OKX, Bitget, and Rapira
expose public advertiser profile URLs. Rapira currently contributes only to
the USDT/RUB market.

`payment_method` accepts the bank selected in the frontend. Named payment
methods are matched after punctuation/case normalization. Some venues return
only opaque numeric payment-method IDs; those offers remain visible as
unverified estimates instead of being silently discarded.

## Build the AMD -> asset -> RUB puzzle

```text
GET /api/p2p/routes?source_fiat=AMD&target_fiat=RUB&source_amount=100000&intermediary_assets=USDT,USDC,BTC,ETH,BNB,SOL,TRX,TON,DOGE,LTC,DAI,FDUSD,XRP,ADA,DOT,LINK,AVAX,MATIC,BCH,NEAR,APT,ATOM,UNI,SUI&min_orders=20&min_completion_rate=0.9&sources=binance,okx&limit=20
```

`intermediary_assets` accepts one or more comma-separated crypto codes. If it is
omitted, the backend searches the configured `p2p_search_assets` catalog. The older
`assets` parameter remains supported as a compatibility alias.

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
loaded source slugs, for example `binance,okx,whitebird,skylabs`. If omitted,
all configured sources are queried. The selected source list is part of the
search cache key.

Routes whose two selected banks are explicitly named by both advertisements
are ranked ahead of routes with opaque payment IDs. The response exposes this
as `payment_methods_verified`; unverified routes also include a warning.

For crypto-to-crypto requests, the route builder uses spot-market tickers from
the selected enabled venues instead of P2P fiat advertisements. It searches a
direct pair or a crypto-only path such as `ETH -> USDT -> USDC`; `bridge_fiat`
is retained only for wire compatibility and is ignored by this route type. No
bank payment or AMD/RUB leg is introduced into a crypto-to-crypto route.

By default only same-venue routes are returned. They do not require moving the
asset from one exchange to another. Set `allow_cross_venue=true` to include
cross-venue candidates; they are marked `requires_asset_transfer=true`, and
their network compatibility and transfer fee are deliberately not claimed as
verified.

Important: `target_amount` is a search estimate. Platform fees, account/KYC
eligibility, ad availability at execution time, sanctions/geographic rules,
and payment confirmation are not verified by this read-only layer.

## Configuration

The backend search settings are TOML keys in [`config.toml`](../config.toml):

```toml
p2p_search_enabled = true
p2p_search_timeout_ms = 4000
p2p_search_cache_ttl_ms = 5000
p2p_search_assets = ["USDT", "USDC", "BTC", "ETH", "BNB", "SOL", "TRX"]
playwright_chromium_executable = "/usr/bin/chromium"
```

`p2p_workflow_debug_screenshot` can also be set in TOML for failure diagnostics.

Sources, endpoint URLs, response mappings, and browser workflows are declared
in `backend/providers/*/Providerfile`. Regenerate the provider migration and
rebuild after changing one; see [`../Providerfile.md`](../Providerfile.md).

Run the opt-in live smoke test:

```bash
cd backend
cargo test live_amd_to_rub_route_search -- --ignored --nocapture
```
