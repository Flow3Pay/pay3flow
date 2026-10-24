# Gonka trading on HEX

Open `/#/otc?market=GNK-USDT`, or choose **GNK / USDT · HEX** in OTC.
The market supports immediate buy/sell orders, resting limit orders, owned
order cancellation, claiming order-book balances, and HOT bridge deposits and
withdrawals. Users select their Omni trading account explicitly. Every order,
cancellation and claim requires that account's wallet signature.

## Protocol and data

The supported mainnet pair is:

| Setting | Value |
| --- | --- |
| Contract | `orderbook.fi.tg` |
| GNK | `nep245:v2_1.omni.hot.tg:4444119_wyixUKCL` (9 decimals) |
| USDT | `nep141:usdt.tether-token.near` (6 decimals) |
| Contract price | Human USDT/GNK price × 10^-3, up to 24 decimals |
| Additional frontend referral fee | None (`refs: []`) |

The Rust backend only reads the chain and indexer. Its `/api/hex/config`,
`/snapshot`, `/account?account=…&skip=…&limit=…`, `/history?account=…&skip=…&limit=…`
and `POST /quote` routes do not accept signing keys or publish transactions.
Quote inputs are `{side, order_type, amount, price?}`; `amount` is an integer
input-token quantity and `price` is the exact contract price. Buy input is USDT;
sell input is GNK. Responses preserve large integer timestamps as strings.

Books, balances, protocol fees and simulations come from NEAR mainnet RPC.
History comes from `https://hex-api.app/orderbook/v1/orderbook.fi.tg/transactions`.
History pagination is bounded to 100 records per response. The chart shows
confirmed fills in the most recent history page, with its sample size; order
publications and cancellations do not become chart prices. Indexer decimal
numbers are display observations and are never used to construct signed amounts.

Asks hold GNK; bids hold USDT. Book display converts bid quantities into GNK
using exact arithmetic. Contract price strings remain exact when selected for
a new limit order. No synthetic GNK price, chart, or liquidity is generated.

## Wallets and recovery

HOT Kit 1.6.4 is loaded only for the GNK workspace. Its default connectors plus
the Gonka Cosmos connector expose supported wallet families. HOT's live registry
and bridge UI determine available funding networks, fees, and routes. Deposit
recipients default to the selected trading account. Funding and order placement
are separate actions: a completed deposit never executes an old quote.

`pnpm dev` and `pnpm build` first build a local browser SDK bundle with esbuild.
This keeps HOT's wallet dependencies out of the main application's Rollup graph.
The generated `static/vendor/hex-hot-kit.js` and its license notices are included
in the deployed frontend, loaded from the same origin, and excluded from Git.
The build cache is keyed by the lockfile, SDK entry and build script. No external
JavaScript CDN is used.

Market orders default to 0.5% maximum slippage. Review displays the exact signed
minimum output, protocol fee, immediate output, resting input and refunded input.
Quotes expire after 15 seconds; confirmation of an expired quote refreshes the
review and requires another click before signing.

The browser stores a signed commitment before publishing it and records the
returned intent/transaction references. After a timeout or reload, **Check
original submission** retries the same signed nonce or checks its existing
reference. It never requests a replacement signature. New orders remain locked
while the outcome is unknown. A replay rejection alone does not clear this lock.
Confirmed failures clear it; success requires NEAR transaction/receipt validation.
Submission and reconciliation share an account-specific browser Web Lock so a
second tab cannot overwrite a newer pending operation. Trading requires Web Locks
in a current HTTPS browser (localhost development is supported).
HOT's **Wallet & transfers** view handles bridge progress and pending withdrawal
recovery. No wallet private key is sent to Pay3Flow.

## Production configuration

Two settings control execution:

| Process | Setting | Purpose |
| --- | --- | --- |
| Backend | `HEX_TRADING_ENABLED=true` | Advertise enabled HEX execution |
| Frontend | `PUBLIC_HOT_KIT_API_KEY` | HOT's browser integration API key |

Both are required for trading and funding controls. The market remains readable
when execution is disabled. `PUBLIC_REOWN_PROJECT_ID` is optional for HOT's
WalletConnect connections; injected wallets remain available without it.

Run production inspection and deployment from the `lion` session that owns the
SSH identity. The documented target is `root@195.123.218.200`, namespace
`pay3flow`, URL `https://pay3flow.lefine.pro`.

Inspect secret **names**, without printing their values:

```sh
k3s kubectl -n pay3flow get secret pay3flow-secrets \
  -o go-template='{{range $key, $value := .data}}{{printf "%s\n" $key}}{{end}}'
```

Reuse an existing HOT credential only after confirming it is intended for browser
integration. The key is public in the browser by design. Do not substitute a
backend-only NEAR Intents JWT, signing key, or unrelated provider credential.
Reference the existing credential's Secret key in the frontend Deployment under
`PUBLIC_HOT_KIT_API_KEY`; the rendered manifest defaults to a same-named optional
Secret key. If no suitable credential exists, leave execution disabled and report
the missing configuration.

The image-patch workflow `nix run .#deploy` preserves existing environment
settings. Carry the HOT Secret reference into the live frontend Deployment and
enable `HEX_TRADING_ENABLED` on the backend only once the credential and read-only
market checks pass. The separate manifest renderer accepts
`PAY3FLOW_HEX_TRADING_ENABLED=true`; its default is false.

Before deployment record existing image IDs and these environment references.
Commit validated code, run the existing release workflow, verify both rollouts,
and smoke-check GNK book/history/quote simulation plus the existing EVER market.
Record revision, immutable image IDs and check results in `deploy/releases/`.
Roll back image tags and these environment references if verification fails.
Production smoke checks do not deposit, withdraw, publish orders, or trade funds.

## Validation

```sh
cd pay3low-svelte-frontend
pnpm install --frozen-lockfile
pnpm check
pnpm test:unit
pnpm build
PLAYWRIGHT_PORT=3237 PLAYWRIGHT_SHARE_PORT=8097 pnpm test:e2e tests/hex.spec.ts
```

On a shared host with limited free memory, the production build can be run with
`NODE_OPTIONS=--max-old-space-size=2560 pnpm build` to keep V8's heap bounded.

From the repository root, run `cargo fmt --manifest-path backend/Cargo.toml --
--check`, `cargo check --manifest-path backend/Cargo.toml` and
`cargo test --manifest-path backend/Cargo.toml --lib hex::tests`.

Protocol sources: [HEX SDK documentation](https://hot-labs.gitbook.io/hex/concepts-and-systems/python-sdk),
[HOT Kit](https://github.com/hot-dao/kit), and the live HEX frontend's contract
messages. The legacy OTC listing API and old Python cancellation examples differ
from the current deployed contract; the adapter uses the current v1 cancellation
message containing both the order hash and token pair.
