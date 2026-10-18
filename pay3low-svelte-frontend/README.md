# Pay3Flow web frontend

The frontend is a SvelteKit application for searching exchange routes, opening
provider swaps and guides, and using the separate backend-connected OTC desk.
The default development port is `3000`.

A fresh SWAP session starts with USD → AMD using Ameriabank's USD and AMD
accounts. Saved payment methods and shared swap links override this default.

## Local development

```bash
npm ci
npm run dev -- --host 127.0.0.1 --port 3000
```

The frontend expects the backend at `http://localhost:8080` during local
development. Set `PUBLIC_API_URL` when the backend is hosted elsewhere.

Set `PUBLIC_AURORA_INTENTS_API_KEY` in `.env.local` for local development, or in
the frontend server environment in production. Obtain the widget integration
key from [Aurora Widget Studio](https://studio.aurora.dev/). The official NEAR
Intents standalone widget handles its own EVM, Solana, NEAR and Stellar wallet
connections and receives the route's assets, networks and amount.

Swap steps provide `SWAP` and `SWAP guide` modes. Switching modes keeps the live
widget mounted so its wallet connection and entered addresses are preserved.
The mode also persists in shared links through `view=swap|guide`. Per-leg assets,
networks and exact amounts come from the current route chapter. See the
[widget integration reference](../docs/swap-widgets.md) for provider behavior,
wallet boundaries, NEAR catalog aliases and iframe limitations.
CoW uses its official widget; Symbiosis uses its embedded app for routes without
native wallet execution. Platforms that cannot be embedded retain their quote
and a link to the provider in `SWAP` mode.

`@0x/swap-ts-sdk` is installed for server-side integrations. Its API key belongs
in a server environment variable, rather than a `PUBLIC_` variable; the SDK does
not support browser use.

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
npm run test:unit
NODE_OPTIONS=--max-old-space-size=6144 npm run build
PORT=3000 PUBLIC_API_URL=http://localhost:8080 npm start
```

The standalone wallet SDK needs a larger Node heap during builds and cold dev
dependency optimization. If Node reaches its heap limit, use
`NODE_OPTIONS=--max-old-space-size=6144 npm run build` (or `npm run dev`).

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
routes and the guide. Chapter navigation appears after the active chapter;
the overview starts through its own action button. Legacy TON guide paths are
canonicalized to GRAM without renaming providers, advertisers or networks.

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

## Backend-connected OTC

`/#/otc` loads `OtcWorkspace.svelte`, which calls `/api/otc` for availability,
listings, wallet sessions, RFQs, trades, attempts and desk decisions. The current
workspace handles native EVER on Everscale and issuer USDT on Ethereum. It does
not implement an authoritative client-side orderbook or reuse SWAP route quotes.

`OTC_CONFIG_FILE` controls backend availability. Without configuration the page
shows the unavailable/paused state; it does not silently populate a demo market.
Configured demo mode remains separate from live mode and does not permit live
wallet transfers. The backend binds those environments to their original data.

Wallet sign-in proves account ownership. An RFQ requires the linked wallet
families and an available desk listing; booking and outgoing transfer preparation
are explicit actions. Transfer review, attempt IDs and transaction references
support recovery after reload and block a second send while an attempt remains
unresolved. The backend owns trade state, amount validation and reconciliation.

Implementation entry points:

- `src/lib/components/otc/OtcWorkspace.svelte`: availability, session, RFQ and trade UI.
- `src/lib/otc/service.ts`: HTTP adapter and wallet authentication.
- `src/lib/otc/terminal.ts`: exact-input and trade presentation helpers.
- `src/lib/components/otc/OtcTransferReview.svelte`: explicit transfer review.
- `src/lib/otc/escrow.ts`: the optional Ethereum escrow wallet adapter.

Older synthetic orderbook/chart helpers and their unit fixtures remain in the
repository. Their prices and JavaScript-number calculations do not define live
trade amounts, liquidity or settlement. Use [OTC setup and operations](../docs/otc-service.md),
[verification evidence](../docs/otc-evidence.md), and the browser `/otc-runbook`
for the implemented service and its separate live gate.
