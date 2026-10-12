# Route Aggregation Research

Date: 2026-09-18.

Goal: evaluate route and liquidity aggregation patterns that Pay3Flow could
use without replacing `fmatch` or coupling the core backend to one external
provider.

Main conclusion: the MVP must not depend on 0x, 1inch, Velora, OKX, or another
aggregator as its core. In Pay3Flow, `fmatch` remains the matcher for Pay3Flow
solver candidates, the backend remains the orderbook/auction/winner-selection
center, and external aggregators may only provide quotes or liquidity through
an adapter.

## Sources

- Matcha: https://matcha.xyz/
- 0x Docs: https://docs.0x.org/docs/introduction/welcome
- 1inch Business/API: https://business.1inch.com/
- Barter: https://barterswap.xyz/
- Bebop Docs: https://docs.bebop.xyz/
- Bitget: https://www.bitget.com/ and https://web3.bitget.com/
- Enso Docs: https://docs.enso.build/home
- KyberSwap Aggregator: https://docs.kyberswap.com/kyberswap-solutions/kyberswap-aggregator
- Lightning Labs builder docs: https://docs.lightning.engineering/
- Nordstern.Finance/Shardeum docs: https://docs.shardeum.org/docs/ecosystem/icecreamswap
- OKX Wallet / Onchain OS: https://web3.okx.com/
- Velora Docs: https://www.velora.xyz/docs/
- CoW Protocol Docs: https://docs.cow.fi/ and https://cowswap.mintlify.app/

## Comparison Table

| name | category | what_it_does | how_it_routes | how_it_executes | api_or_sdk | useful_for_pay3flow | risks | decision |
|---|---|---|---|---|---|---|---|---|
| Meta Matcha | Meta-aggregator UX / DEX aggregator frontend | User-facing swap and cross-chain trading UI; useful pattern for sell/buy intent, route details, simple execution copy. | Uses aggregated liquidity/routing behind a simple token search and swap interface. Matcha is strongly tied to 0x infrastructure. | User connects wallet and signs/broadcasts on-chain actions. | Public user product; developer path points to 0x APIs. | UX reference for Pay3Flow exchange form, route details, slippage/fees/ETA disclosure. | Not a Pay3Flow solver matcher; not fiat; on-chain wallet UX only. | Use as UX/product reference only. Do not integrate directly. |
| 0x | DEX aggregator, RFQ/liquidity API, cross-chain API | Trading API across EVM, Solana, Tron and HyperCore; Swap, Cross-Chain and Gasless APIs. | Aggregates liquidity from many sources and can use RFQ/private market maker liquidity. | Returns quote/transaction data for wallet/app execution; gasless and cross-chain products exist. | REST APIs, docs, examples, API keys/dashboard. | Strong future `RouteQuoteSource` candidate for TOKEN-leg quotes. Useful for stablecoin/token swaps and cross-chain quotes. | API key, rate limits, sanctions/geo, token/chain support, on-chain execution and approvals, no fiat leg. | Later adapter, env-gated. Not MVP core. |
| 1inch | DEX aggregator/API suite | Swap API, Orderbook API, data APIs, SDKs; deep liquidity and route optimization. | Pathfinder-style liquidity aggregation across supported chains. | Returns route/transaction data for wallet execution. | API key or OAuth; SDKs and API docs. | Good future TOKEN-leg quote source; useful benchmark for route scoring/slippage. | Paid tiers/rate limits, geo/sanctions limitations, chain/token constraints, approval risks. | Later adapter, env-gated. Use mock in MVP. |
| Barter | DeFi router / liquidity ecosystem / solver-adjacent infrastructure | Ethereum-focused routing/liquidity network; site describes wallet-based quoting, router fills and fee sharing; presents Router-as-a-Service. | Aggregates AMMs, RFQ desks and off-chain liquidity; also associated with solver activity in CoW ecosystem. | On-chain router fills swaps against liquidity sources / wallet liquidity. | Public site links docs; integration appears partner-oriented. | Interesting research target for solver/liquidity design and non-custodial liquidity contribution. | Public API maturity unclear; Ethereum focus; partner/onboarding risk; not fiat; compliance unknown. | Watchlist/research later. No MVP dependency. |
| Bebop | RFQ API + Aggregation API / solver competition | Institutional DeFi liquidity infrastructure: PMM RFQ, Aggregation API, price stream, trade history. | RFQ quotes from market makers; Aggregation API routes through competing solvers. | Returns firm quote and ready-to-broadcast settlement transaction; supports on-chain settlement contracts. | REST APIs, OpenAPI specs, API key, docs. | Very relevant future TOKEN-leg adapter because it exposes RFQ and solver competition results. | API keys, supported chain/token limits, approvals/contracts, no fiat leg, potential compliance review. | High-priority later adapter; MVP remains `MockRouteQuoteSource`. |
| Bitget | CEX/UEX plus wallet/DEX aggregator | Centralized exchange products plus Bitget Wallet self-custody multi-chain trading and DEX aggregation; Bitget Onchain lets exchange-account users access on-chain assets. | CEX order books for exchange products; wallet DEX aggregation for on-chain swaps. | CEX account execution or wallet/on-chain execution depending product. | CEX APIs and wallet/developer surfaces; official docs are fragmented. | Could be a venue/liquidity source for a regulated solver, not Pay3Flow core. | Custody/KYC/geo/sanctions, account risk, exchange dependency, regulatory burden, API permissions. | Do not integrate in MVP. Maybe solver-owned venue later. |
| Enso | DeFi route/bundle API | Turns DeFi product intent into signer-ready transaction data; Route API finds path, Bundle API composes ordered actions. | Automated pathfinding across DeFi positions and token routes; cross-chain support exists in docs. | Returns executable transaction data; app/user keeps custody and policy control. | REST API, API key, docs. | Useful future adapter for complex TOKEN-leg routing beyond simple swaps. | API key, chain/protocol risk, approvals, quote/execution complexity, no fiat leg. | Later adapter for advanced DeFi flows, not MVP. |
| KyberSwap | DEX aggregator API | Aggregator APIs discover best DEX routes via single API call; integrates limit orders as liquidity source. | Smart order routing across supported DEXs/networks; considers liquidity sources for best route. | API returns encoded swap data for router contract execution. | Aggregator REST API docs. | Reasonable future TOKEN-leg quote source and benchmarking source. | Chain/token constraints, approvals/router contracts, API limits, no fiat leg. | Later adapter, lower priority than 0x/Bebop/Velora. |
| Lightning | Bitcoin payment network / rail | Off-chain Bitcoin payment channels; LND/Loop/Pool/Taproot Assets tooling. | Payment sender finds a route through Lightning nodes/channels to recipient. | Payments settle over HTLC/channel network; Loop bridges on/off-chain liquidity; Taproot Assets can move issued assets. | LND APIs, Lightning Labs tools/docs. | Not a DEX aggregator. Useful as possible money rail or BTC/Taproot asset settlement rail later. | Liquidity/channel management, invoice/payment failure, volatility, custody if using hosted node/provider, compliance. | Treat as rail research, not `RouteQuoteSource` for MVP TOKEN swap. |
| Nordstern | Chain-specific DEX aggregator API | Nordstern.Finance Aggregator API for Shardeum token swaps. | Finds efficient swap routes across Shardeum DEX liquidity considering pool liquidity, price impact and gas costs. | JSON API returns route; execution is on Shardeum/EVM flow. | Endpoint documented for Shardeum: `https://api.nordstern.finance/aggregator/8118`. | Useful only if Pay3Flow later uses Shardeum liquidity. | Single-chain/ecosystem concentration, maturity, API reliability, limited relevance to AM/RU corridor. | Not MVP. Watchlist for Shardeum-specific adapter only. |
| OKX | Wallet DEX aggregator + CEX/venue API | OKX Wallet DEX router finds/executes best prices across many liquidity pools/DEXs; OKX exchange offers CEX liquidity and API products. | DEX router aggregates on-chain liquidity; CEX uses order books/venue execution. | Wallet route execution is self-custody; CEX execution requires account/custody. | Onchain OS / DEX API docs, CEX REST/WebSocket APIs. | Potential later quote source or solver-owned venue. | CEX custody/KYC/geo restrictions, sanctions, API auth, regulatory burden, DEX contract approvals. | Do not use in MVP core. Consider only as env-gated adapter or solver venue. |
| Velora | Intent protocol + DEX aggregator | Former ParaSwap; unifies intent-based Delta execution and Market DEX aggregation; supports gasless swaps, crosschain, SDK/API/widget. | Delta uses Portikus solver network and sealed-bid auctions; Market routes across DEX/AMM liquidity. | Delta: user signs intent, relayer/solver network executes; Market: returns route/calldata for atomic on-chain swap. | REST API, TypeScript SDK, widget, API reference; no key required to start per docs index. | Excellent design reference for dual-mode `DELTA` vs `MARKET`; future adapter candidate. | On-chain only, supported chain/token limits, approvals/relayer trust model, not fiat. | High-priority later adapter. No MVP dependency. |
| CoW Protocol | Intent protocol / solver auction / batch auction | Users sign trade intents; solvers compete in batch auctions; protocol uses CoWs and on-chain liquidity. | Fair combinatorial batch auctions; solvers find settlement paths. | Solver/driver executes settlement on-chain via CoW contracts. | Orderbook/Solver/Driver APIs, SDKs; local `cowprotocol-services/` reference. | Architecture reference for orderbook, auction window, solver competition, winner selection, settlement lifecycle. | Ethereum-centric assumptions, ERC20 approvals, smart contracts, not fiat/cross-border by itself. | Reference only. Do not replace `fmatch`. |

## Slippage, Fees And Minimum Received

Common pattern across DEX aggregators:

- user provides sell token/amount and buy token or target amount;
- quote contains route, estimated buy amount, fees/gas and calldata or order to sign;
- slippage/minimum received is enforced by calldata, signed order constraints or
  solver guarantee;
- RFQ/intent systems often quote firm prices with expiry instead of purely
  indicative AMM estimates.

Pay3Flow adaptation:

- normalize all external quote sources into `RouteQuote` with
  `target_amount_minor`, `fee_minor`, `eta_minutes`, `expires_at`,
  `requires_user_funding`, `risk_score`, `settlement_plan`;
- for MVP, calculate quotes only from fake/mock sources;
- later, external adapters can provide TOKEN-leg estimates, but backend still
  chooses final route together with Pay3Flow solver quote/risk/limits.

## RouteQuoteSource Design

This is the planned abstraction. It is intentionally smaller than every external
provider API.

```rust
#[async_trait::async_trait]
pub trait RouteQuoteSource: Send + Sync {
    fn name(&self) -> &'static str;

    async fn health(&self) -> RouteQuoteSourceHealth;

    async fn quote(&self, request: RouteQuoteRequest) -> Result<Vec<RouteQuote>, RouteQuoteError>;
}

pub struct RouteQuoteRequest {
    pub order_id: uuid::Uuid,
    pub source_country: String,
    pub source_currency: String,
    pub source_amount_minor: i64,
    pub source_method_type: String,
    pub target_country: String,
    pub target_currency: String,
    pub target_amount_min_minor: Option<i64>,
    pub target_method_type: String,
    pub deadline_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct RouteQuote {
    pub source_name: String,
    pub solver_id: Option<uuid::Uuid>,
    pub source_amount_minor: i64,
    pub target_amount_minor: i64,
    pub source_currency: String,
    pub target_currency: String,
    pub funding_method_type: String,
    pub requires_user_funding: bool,
    pub rate: String,
    pub fee_minor: i64,
    pub eta_minutes: i32,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub settlement_plan: serde_json::Value,
    pub risk_score: i32,
    pub raw_response: serde_json::Value,
}
```

Rules:

- `RouteQuoteSource` never creates an exchange order.
- `RouteQuoteSource` never selects the global Pay3Flow winner.
- `RouteQuoteSource` never starts settlement.
- External adapters must be disabled by default and enabled by env/config.
- External raw responses are stored for audit/debug but never become the public
  contract.
- Timeout/reject from one source must not fail the whole auction.

MVP implementation:

```text
MockRouteQuoteSource
  -> deterministic fake quotes
  -> at least two fake solver profiles: fast-low-limit and slow-better-rate
  -> supports success, reject and timeout scenarios
```

Later adapter order:

1. `MockRouteQuoteSource` for MVP.
2. `VeloraRouteQuoteSource` or `BebopRouteQuoteSource` for intent/RFQ learning.
3. `ZeroXRouteQuoteSource` or `OneInchRouteQuoteSource` for broad DEX quotes.
4. `KyberSwapRouteQuoteSource`, `EnsoRouteQuoteSource`, `OkxDexRouteQuoteSource`
   as optional secondary adapters.
5. `LightningRailQuoteSource` only if Pay3Flow models Lightning as a rail, not
   as DEX/TOKEN swap routing.

## Decision For Pay3Flow MVP

- Keep `fmatch` as Pay3Flow solver matcher.
- Keep backend as orderbook, quote collector, scoring engine and settlement
  state machine.
- Do not make real external aggregator calls in MVP.
- Implement `RouteQuoteSource` abstraction later in EX-5.3a.
- Use only fake/mock quote sources until EX-12 smoke is complete.
- Real adapters require:
  API keys/secrets management, env gating, timeout budget, rate-limit handling,
  sanctions/geo review, custody/compliance decision and proof/audit mapping.

Final choice: for MVP, use local fake solvers plus `MockRouteQuoteSource`.
External aggregators are research-backed future adapters; they do not replace
`fmatch`, and they do not own Pay3Flow order lifecycle.

## Provider coverage audit — 2026-10-12

This audit covers every active exchange Providerfile and the optional templates.
Coverage means either an advertised capability or a successfully observed quote;
it does not mean every pair, amount, bank, or account is executable. No orders
were created and no funds were moved.

### Findings and changes

- Fiat workflow jobs were cut off by the **display** limit, with one common
  queue for all swap providers. Each provider now has an independent bounded
  quote budget; completed quotes stream immediately. A one-slot result limit
  no longer prevents discovery from the other providers.
- Fiat entry/exit discovery repeated identical requests for every token pair.
  Each symbol and side is now discovered once and searches run concurrently.
- Workflow joins did not match the networks of fiat offers to quote endpoints.
  Both sides now match before a quote job is created. This enables valid bncex
  and Bitcoin Center combinations without mixing incompatible networks.
- Offer selection reserves one offer per venue before filling remaining slots.
  Network filtering happens before truncation. Intermediate limits count
  **symbols**, retaining all catalog networks for the selected symbols.
- The Fmatch-backed fiat workflow used a fabricated 1:1 token conversion when
  no price was available. Such output is removed. A route needs an exact cached
  or live quote. Amount-specific quotes are cached for their existing short TTL.
- Cold capability snapshots no longer suppress every provider in the first
  search. Provider quote failures and search-budget exhaustion are reported;
  progressive snapshots retain `routes_exhaustive=false` when bounded.
- bncex's current non-cash API requires `fiatDestination=ACCOUNT`. Missing it
  caused HTTP 400 for all nine network variants. Live requests with this field
  returned quotes. Its request amount is numeric after template rendering.
  Exact crypto inputs now take precedence over the generic asset probe amount.
- bncex only advertises USDT/USDC; the current Bitcoin Center adapter only
  advertises its verified USDT/Solana corridor. Unsupported tokens no longer
  trigger requests for these fixed calculators.
- Symbiosis's `/v2/tokens` returned **7469 entries across 96 chain IDs**. The
  adapter's previous network mapping covered only 15 IDs. The audit adds 38
  identified EVM networks for read-only discovery, using EVM preview addresses.
  Unknown chain IDs remain excluded. Wallet execution retains its separate
  configured-contract registry.
- Abstract, Berachain, Robinhood, Bitcoin Cash, and Zcash aliases now agree
  across catalogs, allowing supported entries to join by canonical network.
- Gate spot tickers and authenticated 0x/Barter quotes are registered. 0x and
  Barter load token precision from the shared public token list at startup,
  with configured metadata as fallback. Native ETH and WETH retain distinct
  identities. Empty API keys leave those two providers as catalog entries.

### Active providers

| Provider | Actual integrated surface | Coverage and audit decision |
| --- | --- | --- |
| Binance | Public P2P + spot | Fiat/crypto ads, crypto spot paths and cycles. RUB remains explicitly excluded from its P2P adapter. Public spot probe returned 3727 ticker entries. |
| Bybit | Public P2P + spot | Runtime fiat/asset ads; crypto spot paths. Public spot response succeeded. Opaque bank IDs remain unverified. |
| OKX | Public P2P + spot | Runtime fiat/asset ads; crypto spot paths. Spot probe returned 1160 entries. |
| Bitget | Public P2P + spot | Runtime fiat/asset ads; crypto spot paths. Spot probe returned 3389 entries. |
| MEXC | Public P2P + spot | Runtime fiat/asset ads; crypto spot paths. Spot probe returned 1882 entries. |
| Rapira | Public P2P | RUB legs only, with runtime asset availability and paged ads. No other fiat corridor is claimed without evidence. |
| Whitebird | Anonymous exchanger quote + spot order books | BYN/USD/EUR/RUB calculator coverage and configured live spot instruments. Crypto network aliases are retained; exact sell amount now reaches its calculator. |
| Cifra Markets | Public calculator + IMEX markets | Live calculator confirms BYN/RUR/USD and crypto arrays. RUB maps to RUR. Crypto spot routing uses IMEX. |
| SkyLabs | Rate API + method commissions + withdrawal fees | Existing AMD/USD routes retain method and network fee adjustments. Its general rate matrix also contains RUB/EUR/GEL/UAH/CZK/KES, but a reference rate does not establish settlement availability; these fiats are not enabled solely from that matrix. |
| bncex | Public amount-specific quote calculator | AMD ↔ USDT/USDC on nine declared networks. ACCOUNT added to all non-cash requests. Independently available as entry or exit around a swap provider. |
| Bitcoin Center | Public fixed route calculator | AMD bank transfer ↔ USDT/Solana, including output fee. Existing adapter does not discover the provider's other route IDs, so no unsupported token is advertised. |
| BestChange | Authenticated official rates/catalog | Runtime exchanger and payment/network directions, direct fiat legs and crypto quotes. Requires BESTCHANGE_API_KEY; reserves and limits still apply. |
| Dzengi | Public market data | Probe returned 518 ticker entries. Integrated coverage is crypto spot; catalog fiat names do not create bank payout quotes. |
| CoW Swap | Quote APIs + live token catalog + NEAR bridge | Token-list probe returned 278 entries. Same-chain and composed bridge capabilities remain quote-dependent. Fiat participation uses independent entry/exit venues. |
| NEAR Intents | 1Click token and dry quote APIs | Token probe returned 204 entries. Same-chain/cross-chain crypto and fiat compositions, subject to valid preview addresses and amount minimums. No direct bank fiat settlement is inferred. |
| Symbiosis | Cross-chain token and swap quote APIs | v1 list had 220 entries; the adapter actually uses v2 (7469 entries). Added identified EVM networks; unsupported non-EVM preview formats remain excluded. Fiat participation uses separate entry/exit providers. |
| ID Pay | Server-rendered calculator | Direct AMD/RUB and RUB/AMD transfer estimates and fiat cycles. No crypto swap capability is inferred. |
| Papa Change | Public directions + exchange rates | Dynamic fiat/crypto directions, payment methods, network aliases, commissions and amount tiers; route feasibility comes from its current catalog and reserves. |
| Gate.io | Public API v4 spot tickers | Live probe returned 2204 entries. Added crypto spot paths/cycles and market links. No undocumented Gate P2P or bank settlement adapter is advertised. |
| 0x | Swap API v2 `/swap/allowance-holder/price` | Same-chain EVM quotes, native ETH handling, liquidity rejection and exact atomic amounts. Requires ZEROX_API_KEY. Public token metadata broadens discovery; a supported token still requires a successful quote. Cross-chain, Solana and Gasless products are not integrated by this adapter. |
| Barter | Official Ethereum `/route` API | Source/target addresses and exact sellAmount; bearer credential BARTER_API_KEY; only Normal routes accepted. Ethereum/WETH coverage. Other Barter deployments are not inferred from its schema's chain enum. |

### Optional templates

1inch, Bebop, Enso, LI.FI, KyberSwap, Nordstern and Velora remain optional
`Providerfile.example` integrations. They are not compiled or advertised as live
sources. The shared DEX helper is not registration by itself. Activating them
requires a reviewed Providerfile, its current API/auth contract, and startup
registration; no provider is added to results merely because it has an example.

### Evidence and remaining limits

All public probe counts above were observed from this workspace on the audit
date; catalogs and quotes change. Individual pairs are validated at quote time.
The 0x and Barter request/response contracts were checked against official
documentation. After configuring the local 0x credential, authenticated read-only
price requests for 1 WETH → USDC on Ethereum and Base both returned HTTP 200,
liquidityAvailable=true, and positive buyAmount. Barter authenticated live quotes
remain untested without its credential. Local configuration is not a deployment.
The complete provider audit does not mean an exhaustive Cartesian search of all
token pairs.

The fiat workflow uses at most 32 amount-specific jobs per provider and a
45-second provider search budget; the overall workflow quote stage is capped at
60 seconds and cancels pending work while retaining streamed routes. Provider limits and incomplete searches remain
visible. Same-chain token swaps and cross-chain swaps can sit between fiat
providers, but arbitrary chains of several independent swap providers are not
implemented. Fiat quote prices remain estimates: bank availability, withdrawal
fees, order-book depth, account fees and final re-quoting must still be checked.

Primary references:

- [Gate API v4](https://www.gate.com/docs/developers/apiv4/en/)
- [0x Swap API quickstart](https://docs.0x.org/docs/introduction/quickstart/swap-tokens-with-0x-swap-api)
- [Barter integration documentation](https://barterdefi-1.gitbook.io/barter/barter-router-system/router-integrations.md)
- [Barter OpenAPI](https://api2.eth.barterswap.xyz/openapi)
- [Symbiosis chain catalog](https://api.symbiosis.finance/crosschain/v1/chains)
- [Symbiosis v2 tokens](https://api.symbiosis.finance/crosschain/v2/tokens)
- [NEAR Intents tokens](https://1click.chaindefuser.com/v0/tokens)
