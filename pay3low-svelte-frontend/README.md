# Pay3Flow web frontend

The frontend is a SvelteKit application for creating exchange orders, viewing
quotes, confirming funding instructions, and following settlement status.
The default development port is `3000`.

A fresh SWAP session starts with USD → AMD using Ameriabank's USD and AMD
accounts. Saved payment methods and shared swap links override this default.

## Local development

```bash
npm install
npm run dev -- --host 127.0.0.1 --port 3000
```

The frontend expects the backend at `http://localhost:8080` during local
development. Set `PUBLIC_API_URL` when the backend is hosted elsewhere.

## Live result rendering

WebSocket route snapshots can arrive while other providers are still being
searched. The result header and the ranked cards deliberately represent two
different facts:

- a venue icon means that the source status reported one or more matching
  offers, even when none of those offers produced a card in the final ranking;
- a route card means that the backend assembled and ranked a complete route.

The frontend keeps previously reported venue icons for the duration of the
current search so a provider does not appear to disappear when a later ranked
snapshot replaces its cards.

Snapshots containing up to 100 routes are rendered in one update. Larger
snapshots are rendered in batches of 100 routes. After each batch, the
frontend yields for a browser animation frame and waits 10 ms before rendering
the next batch. A reordered progressive snapshot keeps the number of cards
already revealed instead of restarting at 100, and the found-route counter is
monotonic for the duration of a search. Starting a newer search cancels any
pending frame or timer from the older snapshot. This keeps large DOM updates
responsive without delaying normal search results.

## Animated route instructions

The route guide opens below the existing site header at `#/guide/SOURCE/TARGET` as a walkthrough: overview, one chapter per
operation, and a final checklist. Scene playback never advances a chapter;
users explicitly confirm each completed operation. Progress is kept for the
selected route in session storage and restored when reopening the guide.

`src/lib/route-tutorial.ts` builds chapters from route data, ordered cycle legs,
provider guidance, and service links. `InstructionScene.svelte` renders shared
buy, sell, swap, and transfer demonstrations; new currency corridors do not
need separate pages. Demonstrations are illustrative, while platform links,
advertiser details, reviews, and wallet execution belong to the active chapter.
The guide supports keyboard navigation, reduced motion, and mobile layouts.

The wallet menu connects Ethereum/EVM, NEAR, TRON, and Everscale accounts.
Everscale uses the injected EVER Wallet provider on Everscale mainnet, including
the wallet's mobile browser. Authorized accounts are restored without prompts;
account changes, network changes, logout, and permission revocation update the
session. Disconnecting Everscale revokes the site's wallet permissions. Everscale
connection support does not enable swaps: the current execution providers expose
only EVM, NEAR, and TRON actions.

Native EVER is listed under the Everscale blockchain in both asset selectors.
Its backend catalog identity is `EVER@everscale`; TON remains a separate network.
Searches use the selected blockchain and return only routes actually supported
by the selected providers. Catalog presence does not imply provider liquidity.

## Verification and production build

```bash
npm run check
npm run build
PORT=3000 PUBLIC_API_URL=http://localhost:8080 node build
```

Run the complete stack from the repository root:

```bash
docker compose up --build backend pay3low-svelte-frontend
```

The frontend is not a custody or payment-processing component. It must show
the selected route, fees, timing, consent terms, and any TOKEN or crypto
settlement asset clearly before the user confirms funding.

Guide links carry the amount, payment method IDs, networks, search settings, and
an ordered operation signature. A new browser fetches fresh routes and matches
that signature; it shows an unavailable state if the route is gone. Execution
and tracking tokens are never included. Browser Back/Forward returns between
routes and the guide. The action footer stays fixed to the viewport.

The sidebar has a separate customer reviews view. Its summaries describe only
collected reviews; Binance feedback remains a positive/negative percentage.
Review sources, rating filters, expandable text, and links to originals preserve
provider and advertiser attribution.

The Share guide button copies `/share/guide/SOURCE/TARGET?...`. This server-rendered
page exposes Open Graph/Twitter metadata and a dynamic 1200×630 PNG inspired by
the guide overview, then opens the corresponding hash guide. The image endpoint
uses only bundled asset/venue icons, bounded parameters and a bounded cache;
Sharp renders it on Node. The runtime image includes DejaVu fonts.

## Spot exchange guides

Crypto-to-crypto exchange guides select Spot chapters from `market_path` and
`cycle_legs.market_pair`. Binance, Bybit, MEXC, Bitget and Whitebird have dedicated Spot
illustrations and five scenes: funding, pair/direction, order type/amount,
review, and actual fills. The market symbol determines Buy/Sell and the
spending currency for every pair, including non-stablecoin pairs. These steps
use canonical Spot market links and official Spot help, rather than the
provider catalog's P2P profile instructions. P2P offer chapters continue to
use the advertiser walkthroughs.

## OTC frontend preview

Open `/#/otc` for the default EVER/USDT market, or share `/#/otc?market=EVER-USDT`.
The header switches between SWAP and OTC, keeps the current swap corridor when navigating to OTC, and moves
API Docs, Telegram and GitHub into a left drawer on all screen sizes. Exchange
sharing sits after the theme button.

The OTC workspace uses the existing theme variables, fonts, asset icons and
bridge visual language. Bridge, chart, orderbook and market activity each have
a collapse arrow. The preview includes EVER/USDT, BTC/USDT, ETH/USDT and SOL/USDT
markets, buy/sell and limit/market modes, line/candle charts, range and zoom controls,
price grouping, clickable book levels, review, cancellation and order history.
Market orders simulate consuming available opposite offers and reject amounts
that exceed the demo liquidity.

EVER uses five decimal places for quote prices and six for order amounts; its
0.01 USDT starting price is a synthetic fixture, not a live exchange quote.

All OTC prices, book levels, candles and trades are **demo data**, explicitly
marked in the interface. Orders only live in tab-scoped `sessionStorage`, capped
at 100, and never call an order API, sign a wallet request or move funds. A
simulated market order is labelled `simulated`, not settled or filled.

Integration boundaries for the backend implementation:

- `src/lib/otc/model.ts` defines `OtcMarket`, `OtcSnapshot`, `BookLevel`,
  `Candle`, `OtcTrade`, `OrderDraft` and `DemoOrder`, and holds the demo fixtures.
- `OtcWorkspace.svelte` owns the selected market and demo order lifecycle;
  replace `demoSnapshot`, storage, `confirmOrder` and `cancelOrder` with the
  agreed API adapter. Request/response paths are intentionally not assumed.
- `OtcChart.svelte` currently gets candles from `demoCandles`; provide server
  candles per selected market and range. The buy/sell lines are illustrative
  offsets around candle closes, not historical bid/ask measurements.
- `OtcOrderbook.svelte` receives the snapshot and emits a side and selected
  price; selecting a level fills a limit order and does not execute it.
- `OtcBridge.svelte` produces a draft; `OtcOrderReview.svelte` reviews it.
  Before live execution, obtain a server-validated quote and show actual fees,
  settlement network, expiry, limits and funding requirements. Use server
  order IDs, idempotency, authoritative statuses and authenticated ownership.

The preview uses JavaScript numbers for illustrative calculations. The live
money API should serialize exact prices and amounts as decimal strings or
atomic units and perform precision, tick-size and liquidity validation on the
server. Do not use `prepareDemoOrder` as authoritative matching logic.

Run `npm run test:unit` for quote/amount validation and `npm run test:e2e --
tests/otc.spec.ts` for desktop/mobile navigation, panels, chart/book controls,
review, cancellation and history. No OTC backend is required for these tests.
