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
