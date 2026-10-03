# Live P2P Search

Pay3Flow has a read-only P2P search layer. It reads public advertisements and
does not contact advertisers, create platform orders, reserve crypto, or move
money.

## Sources

- Binance, Bybit, OKX, Bitget, MEXC, and Rapira public P2P advertisement lists.
- Whitebird, Cifra Markets, SkyLabs, bncex, and Bitcoin Center public
  direct-quote APIs. Bitcoin Center is restricted to AMD bank transfers and
  USDT on Solana.
- BestChange public catalog and direction pages.
- Binance, Bybit, OKX, Bitget, MEXC, Cifra Markets, and Dzengi spot tickers for
  crypto-to-crypto paths.

Providerfile-backed sources are discovered from the generated provider catalog;
the list is not hardcoded in the route API. Their normalized advertisements are
published as FEP-0837 offers to the configured Fmatch actor. Background workers
query Fmatch for direct exchanger offers across every supported asset and fiat
direction. User searches compose routes from those snapshots. P2P searches run
on demand, while common P2P directions also have background snapshots. Other
venues from the research list remain outside the live
path until a legitimate read-only interface and adapter review exist.

Background refreshes query the public adapters concurrently so their latest
offers can be published. One provider response is published as bounded
ActivityPub `OrderedCollection` batches of up to 64 offers, allowing Fmatch to
refresh its read snapshot once per batch instead of once per advertisement.
Public route legs are then resolved through Fmatch.
Successful Fmatch answers are stored in PostgreSQL. If Fmatch is unavailable,
the newest answer within `p2p_fmatch_stale_secs` is used and the response has
`source: "database_cache"` and `stale: true`. A live Fmatch answer has
`source: "fmatch"`. Empty Fmatch replies and empty cached answers are not
considered usable. If neither Fmatch nor the bounded-stale database cache can
provide offers, the background worker queries its live providers and stores
`source: "provider_fallback"` with the Fmatch rejection recorded in
`sources`. Provider-only
local searches retain `source: "provider"`.
Route-search snapshots use the same source labels and mark `stale: true` only
when results actually came from the bounded-stale database cache.

Pay3Flow requests up to 64 candidates per Fmatch page. `exchange_mode=all`
queries `market=p2p` and `market=direct_exchange` concurrently, merges the two
partitions, and preserves source diversity. P2P volume therefore cannot evict a
direct source such as Whitebird before local route composition. See
[`route-virtualization-pipeline.md`](route-virtualization-pipeline.md) for the
complete runtime pipeline, lazy top-K algorithm, provider snapshots, caches,
and background quote refresh.

At startup the first background pass starts immediately. Direct exchanger
targets cover both sides of every fiat in the payment picker and every digital
asset in the enabled network catalog. Targets stay in a priority queue; two
workers poll them concurrently instead of creating one task per direction.
One worker covers the full catalog with a five-second pause between polls; the
second serves recently requested cold directions immediately. Each target
refreshes five minutes after its previous poll; a large catalog can take longer
than five minutes to complete a
whole pass. Redis holds the shared offer snapshots, while the local memory
cache keeps at most 128 snapshots. Common P2P directions refresh every 60
seconds when idle, 30 seconds when used, and 15 seconds at high demand. Other
P2P directions are searched on demand and enter background refresh after
repeated use. Spot tickers and public provider quotes each use at most two
concurrent background polls; fiat quotes use one. Public provider quotes cover
all supported ordered asset pairs. Requested pairs take priority, and quote
requests are throttled between batches. Offer and spot
snapshots are shared through Redis. A cold direct exchanger corridor
waits for its first background snapshot for up to 8 seconds. If no snapshot
arrives by then, the response reports `background_pending`.
Streaming searches publish cached market partitions as they become available.
Fiat workflow legs for different assets
also run concurrently, so a slow or unsupported asset does not hold back routes
from another asset. A provider scan that returns no raw offers is not repeated
with looser local filters.

Completed route searches are cached in Redis for up to 15 seconds using
normalized search parameters (less when a provider quote expires sooner). The
cache is shared across users; viewer votes, reputation and execution links are
added after cached routes are read.

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
the venue's fixed transfer network when known, payment methods, public
advertiser reputation, source status, the advertiser profile URL when the
venue exposes a stable public profile, and the source advertisement URL.
`source_url_is_exact` marks whether the latter opens the
exact advertisement returned by the source. The frontend instructions focus on
the advertiser profile and nickname; when no stable profile URL is available,
they open the venue's P2P market and tell the user to find the advertiser by
nickname and verify the ad ID. Routes keep valuable offers even when a venue
only provides a generic market URL. Binance, Bybit, OKX, Bitget, MEXC, and
Rapira expose public advertiser profile URLs. Rapira currently contributes only
to the USDT/RUB market.

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
5. returns complete routes ranked by estimated RUB output with a capped
   likes/dislikes vote adjustment (up to 2%), then
   verified payment methods and same-venue execution.

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
p2p_search_cache_ttl_ms = 15000
p2p_fmatch_stale_secs = 900
p2p_search_assets = ["USDT", "USDC", "BTC", "ETH", "BNB", "SOL", "TRX"]
playwright_chromium_executable = "/usr/bin/chromium"
```

`p2p_workflow_debug_screenshot` can also be set in TOML for failure diagnostics.

Sources, endpoint URLs, response mappings, and browser workflows are declared
in `backend/providers/*/Providerfile`. Regenerate the provider migration and
rebuild after changing one; see [`../Providerfile.md`](../Providerfile.md).

Fmatch offer publication is best-effort. A provider refresh is retained locally
when Lefine is unavailable. Background discovery prefers a live Fmatch answer,
then a bounded-stale PostgreSQL answer, and finally fans out to the configured
providers so an empty Fmatch catalog does not make route search unavailable.

## Anonymous engagement

Every route response contains `instruction_tokens[route_id]`. The frontend sends
the token and browser-generated `anonymous_id` to
`POST /api/routes/instruction-open` when instructions open. Provider links use
the signed token at `POST /api/service-executions/open`. The ID is used for
30-minute Redis deduplication and is not saved with new click aggregates in
PostgreSQL. Redis groups accepted opens by provider and minute. Repeated opens
of the same route action are ignored. If one browser ID produces more than 12
distinct actions of one kind in a session, Redis removes that session's actions
after its first one before any affected bucket reaches PostgreSQL. A background
worker flushes completed buckets after the session window with retry-safe batch
IDs. Provider reputation starts at 50, gains 10 per like and 5 per instruction
or link open, loses 15 per dislike and 15 per inactive day, and is bounded to
0–100. It is recalculated in the background, cached in Redis for all backend
instances, and used to prioritize background provider polling. Reputation does
not change route order. Only likes and dislikes affect close-priced routes;
when there are no votes, the previous price order applies. Vote ranking uses a
confidence-adjusted like ratio, so 100 likes with 4 dislikes outranks 16 clean
likes, while 100 likes with 68 dislikes does not.
The frontend includes the signed route token with a route vote, allowing one
vote to update the service totals of that route's actual providers. Repeated
votes from the same browser change the previous vote instead of adding a new
one.

Run the opt-in live smoke test:

```bash
cd backend
cargo test live_amd_to_rub_route_search -- --ignored --nocapture
```
