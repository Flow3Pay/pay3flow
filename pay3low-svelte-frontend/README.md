# Pay3Flow web frontend

The frontend is a SvelteKit application for creating exchange orders, viewing
quotes, confirming funding instructions, and following settlement status.
The default development port is `3000`.

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
