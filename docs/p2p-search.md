# Live P2P Search

Pay3Flow has a read-only P2P search layer. It reads public advertisements and
does not contact advertisers, create platform orders, or reserve crypto. An
optional wallet-execution layer can prepare provider actions, but funds move
only after the user confirms the transaction in their own wallet.

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
published as FEP-0837 offers to the configured Fmatch actor. The route endpoint
queries Fmatch for matching offers and uses the existing local composer to build
complete routes. Other venues from the research list remain outside the live
path until a legitimate read-only interface and adapter review exist.

At startup, a background poller begins walking the configured fiat currencies
and supported assets immediately. It starts at most 25 background pipelines
per minute and keeps no more than five active across catalog polls and quote
refreshes. When all five slots are occupied, later catalog entries wait at the
cursor instead of building an in-memory task queue. Fiat currencies declared
by provider adapters are included alongside `route_source_fiats`; network
assets are included alongside `p2p_search_assets`. The poller publishes offers
as bounded ActivityPub `OrderedCollection` batches of up to 64, allowing Fmatch
to refresh its read snapshot once per batch instead of once per advertisement.

Provider-only searches keep generic five-minute snapshots in process memory.
Interactive searches apply amount, payment-method, merchant, order-count, and
completion-rate filters to those offers. More specific searches that need a
larger provider page than the snapshot contains continue to query the providers.
Public route legs are resolved through Fmatch when it is available; local
provider results remain the fallback.

Completed route responses with at least one route are also cached in Redis for
15 seconds. The key uses a normalized search query and excludes the anonymous
viewer ID, so identical searches share the result. The cached route graph is
enriched with the current viewer's votes and fresh service links after the
cache read. Redis reads have a short timeout and cache writes run in the
background.
Successful Fmatch answers are stored in PostgreSQL. If Fmatch is unavailable,
the newest answer within `p2p_fmatch_stale_secs` is used and the response has
`source: "database_cache"` and `stale: true`. A live Fmatch answer has
`source: "fmatch"`. Empty Fmatch replies and empty cached answers are not
considered usable. If neither Fmatch nor the bounded-stale database cache can
provide offers, Pay3Flow queries its live providers and returns
`source: "provider_fallback"` with the Fmatch rejection recorded in
`sources`; streaming searches forward each provider result as soon as it
arrives instead of waiting for the complete fallback fan-out. Provider-only
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
5. returns complete routes ranked by maximum estimated RUB output, then
   verified payment methods and same-venue execution. Circular searches use
   the profitability ordering described below instead.

## Search AMD cycles

An `AMD -> AMD` request automatically searches both configured cycle families:

```text
AMD -> crypto asset -> AMD
AMD -> entry crypto -> NEAR Intents / CoW Swap / Symbiosis -> exit crypto -> AMD
AMD -> configured fiat intermediary -> AMD
```

Crypto intermediaries come from `intermediary_assets` (or the configured P2P
asset catalog). Route-provider cycles may use two different assets: this enables
same-chain swaps such as `USDT -> USDC` through CoW Swap as well as cross-chain
paths through NEAR Intents or Symbiosis. Fiat intermediaries come from
`route_source_fiats`, and each
leg must be supported by a loaded fiat route provider; the route composer does
not contain a hardcoded currency list.

Circular routes have `route_kind` equal to `crypto_cycle` or `fiat_cycle` and
include a tagged `profitability` object. `status=confirmed` is returned only
when the payment-method fees and every route/provider or transfer fee needed by
the route are known. It contains `net_profit_minor` and `profit_bps`.
Otherwise `status=unconfirmed` contains the gross values and `missing_costs`,
so a route is not presented as profitable before its costs are known. The
optional `source_payment_fee_percent` and `target_payment_fee_percent` query
parameters supply the backend-catalog payment fees; omitting either one keeps
the result unconfirmed.

Confirmed profitable cycles rank first, followed by unconfirmed candidates and
then confirmed break-even or loss-making cycles. The best available cycles are
still returned when none has confirmed positive profit.

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

## Execute provider routes with a wallet

When `wallet_execution_enabled = true`, eligible NEAR Intents, CoW Swap, and
Symbiosis routes include a short-lived signed `execution` descriptor. The route
instructions then show an embedded **Execute with wallet** panel in the exact
provider step.

The browser connects an EVM wallet through Reown AppKit or a NEAR wallet through
NEAR Wallet Selector. The user chooses the recipient on every execution, either
from a connected destination wallet or by entering an address. Pay3Flow requests
a fresh quote for those source and recipient addresses, checks the source
balance automatically, and asks the wallet to confirm every approval and
transfer. ERC-20 approvals use the exact input amount rather than an unlimited
allowance.

Execution state is stored in `route_executions`; only public addresses, quote
data, transaction hashes, and provider status are persisted. Private keys and
wallet signatures never reach the backend. Pending operations are restored in
the browser and their NEAR Intents deposit, CoW order, or Symbiosis transaction
status is polled until a terminal state.

The frontend needs a WalletConnect Cloud project id at runtime:

```text
PUBLIC_REOWN_PROJECT_ID=your-project-id
```

Keep `wallet_execution_enabled = false` until the configured mainnet token
addresses, provider credentials, and end-to-end wallet flows have been smoke
tested. Manual provider links remain available while the flag is disabled.
Symbiosis execution additionally requires audited
`symbiosis_execution_contracts` entries in
`chain_id=MetaRouter,MetaRouterGateway` form; returned calldata and approval
spenders are rejected when they do not match.

## Configuration

The backend search settings are TOML keys in [`config.toml`](../config.toml):

```toml
p2p_search_enabled = true
p2p_search_timeout_ms = 4000
p2p_search_cache_ttl_ms = 5000
p2p_fmatch_stale_secs = 900
p2p_search_assets = ["USDT", "USDC", "BTC", "ETH", "BNB", "SOL", "TRX"]
wallet_execution_enabled = false
playwright_chromium_executable = "/usr/bin/chromium"
```

`p2p_workflow_debug_screenshot` can also be set in TOML for failure diagnostics.

Sources, endpoint URLs, response mappings, and browser workflows are declared
in `backend/providers/*/Providerfile`. Regenerate the provider migration and
rebuild after changing one; see [`../Providerfile.md`](../Providerfile.md).

Fmatch offer publication is best-effort. A provider refresh is retained locally
when Lefine is unavailable. Public route discovery prefers a live Fmatch answer,
then a bounded-stale PostgreSQL answer, and finally fans out to the configured
live providers so an empty Fmatch catalog does not make route search unavailable.

Run the opt-in live smoke test:

```bash
cd backend
cargo test live_amd_to_rub_route_search -- --ignored --nocapture
```
