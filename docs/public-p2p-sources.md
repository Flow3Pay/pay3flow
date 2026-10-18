# Public quote-source policy

Verified against the repository on 2026-10-18. SWAP discovery reads P2P ads,
direct-exchange quotes, exchanger listings and spot tickers. Discovery does
not reserve funds, create a platform order or request a wallet signature.
Provider availability in a catalog does not prove pair liquidity or execution
support.

Most adapters use anonymous public data. Authenticated quote integrations keep
credentials on the backend. The separate native wallet-execution endpoints,
provider-owned widgets and OTC desk have their own explicit actions; they are
not implicitly invoked by route search.

## Sources and access boundaries

| Source | Discovery data | Access and implementation boundary |
| --- | --- | --- |
| Binance, Bybit, OKX, Bitget, MEXC | P2P ads and spot tickers | Public read-only mappings; P2P ads are requested for the user's search |
| Rapira | P2P offers, currently USDT/RUB | Public read-only adapter; no order creation |
| Whitebird | Public exchanger calculator and supported spot data | Anonymous quotes; no order submission |
| Cifra Markets | Calculator and IMEX market data | Anonymous direct quotes and crypto paths |
| SkyLabs | Homepage rate data | Public direct AMD/USD quote discovery |
| bncex | Calculator quotes | Public direct fiat/crypto discovery; account transfers use the required destination parameter |
| Bitcoin Center | Direct rate API | AMD bank transfer ↔ USDT on Solana; returned output fee is included |
| Papa Change | Exchanger rate discovery | Read-only adapter; returned directions and limits govern eligibility |
| BestChange | Authenticated rates API and catalog | Requires server `BESTCHANGE_API_KEY`; a numeric referral ID is not this key |
| Dzengi and Gate | Spot market tickers | Anonymous crypto market data; not executable depth or withdrawal confirmation |
| ID Pay | Server-rendered AMD/RUB rate | Anonymous page read; indicative direct fiat route |
| CoW Swap | Token/bridge catalogs and amount-specific quotes | Public quote discovery; live token addresses and decimals |
| NEAR Intents | 1Click token catalog and dry quotes | Optional server `NEAR_INTENTS_JWT`; dry quotes still require valid chain-specific preview addresses |
| Symbiosis | Token catalog and quote API | Optional partner credential; preview address compatibility is checked separately from native execution contracts |
| 0x | Swap API price discovery | Server `ZEROX_API_KEY`; installed TypeScript SDK is not a browser swap form |
| Barter | Ethereum route API | Server `BARTER_API_KEY`; indicative discovery and an external app link |
| Crypto Office | Available pairs and amount-specific rates | Server public/secret key pair produces fresh IP-bound authorization; optional legacy token fallback |

Exact coverage, network aliases and response mappings live in
[`backend/providers/`](../backend/providers/). Providerfiles compile into the
backend and generated catalog SQL; see [Providerfile](Providerfile.md).
Do not infer adapter behavior from a provider's market name alone. Search
failures, minimum amounts and missing liquidity must remain visible as bounded
coverage or diagnostics rather than fabricated exchange rates.

## Execution and wallet separation

A public quote or a clickable route is not settlement. Native Pay3Flow wallet
actions require a signed execution descriptor and explicit user confirmation.
CoW and Aurora widgets perform their own quote, wallet and tracking flow.
Symbiosis can use native execution or its embedded public app. External app
embedding may be restricted; a provider link remains available. See
[SWAP widgets](swap-widgets.md).

`PUBLIC_AURORA_INTENTS_API_KEY` is a separate browser integration key for the
Aurora widget. It does not replace backend `NEAR_INTENTS_JWT`, and server
credentials must never be placed in `PUBLIC_` variables. OTC wallet proofs,
RFQs, booking and transfer attempts belong to the separate
[OTC service](otc-service.md).

The search form accepts route/filter parameters. It does not request full card
numbers, CVV, bank passwords or one-time bank confirmation codes. Provider and
advertiser links should preserve attribution, selected assets and network
context while keeping signed execution and tracking tokens out of shared URLs.

## Research versus active integration

Historical source research is not proof of an enabled adapter or a current
commercial contract. The loaded provider catalog, available credentials and
runtime token/market data determine what a search can query. Recorded audit
samples describe the checked pairs and amounts at their recorded time;
[route algorithms](route-virtualization-pipeline.md) describe current search
limits and [configuration](configuration.md) defines credential placement.
