# NEAR Intents route capabilities

Pay3Flow owns a private bounded route graph. It turns currently available P2P
offers, configured withdrawal/transfer edges, and the supported NEAR Intents
token catalog into stable route capabilities. Only those capabilities are
published to fmatch; prices and quote expiry remain inside Pay3Flow.

The graph uses qualified assets:

```text
USDT@binance -> USDT@tron -> XRP@xrpl
```

An unqualified asset such as `AMD` is allowed only for a fiat graph entry. A
route ID includes every qualified path node, so changing the network changes
the ID and cannot accidentally reuse a route for a different chain.

## NEAR Intents provider

The provider uses the documented 1-Click API surfaces:

- `GET /v0/tokens` refreshes the supported token catalog;
- `POST /v0/quote` requests a dry quote for internal pricing;
- `POST /v0/quote` with `dry=false` is available through the explicit executable
  provider method for the later execution phase;
- `GET /v0/status?depositAddress=...` tracks an executable swap.

The normal `QuoteProvider::quote` implementation always uses `dry=true`.
1Click validates addresses even for dry quotes. Network-specific preview
addresses take precedence; a legacy global address is used only on networks
with a compatible address format. NEAR, TON, TRON, Aptos, Sui, and Cardano have built-in
preview addresses used only for dry quotes. Paths without an address for both
chains are excluded before requesting prices. NEAR previews use `intents.near`;
`wrap.near` is the token contract and is rejected as a quote recipient by 1Click.
The canonical `nep141:wrap.near` entry is exposed as `NEAR@near`, and
the HOT bridge's native TON entry (`nep245:v2_1.omni.hot.tg:1117_`, named `GRAM`)
is exposed as `GRAM@ton` (legacy `TON` queries normalize to `GRAM`); quote requests retain their original API asset IDs and
token decimals. Catalog aliases and preview-address rules live in
`backend/providers/near-intents/Providerfile`. NEAR wallet funding wraps native
NEAR before transferring wNEAR.
Executable quotes always use
the wallet addresses supplied by the user. Credentials are read from
`NEAR_INTENTS_JWT` and are not included in route capabilities or logs.

During crypto cycle searches, a provider minimum-amount limit or Symbiosis's
explicit unavailable-swap response excludes that path without marking the
provider as failed. The reason remains in the provider status. Invalid
requests, authentication errors, rate limits, and server errors remain
provider failures. Cached minimum-amount rejections retain that distinction
and apply only to the exact origin, destination, and input amount; they do
not suppress other destinations or amounts.

Symbiosis previews likewise use chain-specific sender and recipient addresses
for Bitcoin, TRON, and Solana, and the configured EVM preview address for EVM
chains. Networks without a preview wallet address are excluded before quoting.

## Refresh configuration

The background capability refresh reads these keys from `config.toml`:

```toml
near_intents_url = "https://1click.chaindefuser.com"
near_intents_quote_recipient = "<valid destination-chain address>"
near_intents_quote_refund_to = "<valid origin-chain address>"
near_intents_quote_recipients = [
  "solana=<valid Solana address>",
  "bitcoin=<valid Bitcoin address>",
  "near=<valid NEAR account>",
  "avalanche-c=<valid EVM address>",
]
near_intents_quote_refunds = [
  "solana=<valid Solana address>",
  "bitcoin=<valid Bitcoin address>",
  "near=<valid NEAR account>",
  "ethereum=<valid EVM address>",
]
near_intents_refresh_secs = 300
route_max_depth = 4
route_source_fiats = ["AMD", "RUB"]
route_p2p_assets = ["USDT", "USDC", "XRP"]
route_intent_assets = ["USDT@tron", "USDT@solana", "USDC@solana", "XRP@xrpl"]
route_withdrawals = ["USDT@binance>USDT@tron|binance|1.5"]
```

The authenticated 1-Click credential remains environment-only as
`NEAR_INTENTS_JWT`.

The public route search also composes the same live intent quotes with fiat
entry and exit offers. For example, a RUB → AMD search can return a path such
as `RUB → USDT@tron → USDT@solana → AMD` when the configured P2P offers and
NEAR Intents token catalog both support those legs. The route is a dry quote
for display; Pay3Flow does not create an order or move funds.

`route_withdrawals` is deployment-owned TOML capability data. Each entry has the
form `FROM>TO|provider[|fee]`; an optional fee is denominated in the origin
asset. Route discovery and matching remain fmatch’s job.

When a route disappears, Pay3Flow republishes its stable offer with
`status=disabled`. When it returns, the same offer ID is upserted as enabled.

## Quote request boundary

fmatch may request a live quote through the signed ActivityPub inbox with a
`Proposal` whose `command` is `quote` and whose content contains:

```text
route_id=pay3flow:amd-usdt-binance-usdt-tron-xrp-xrpl;
amount=500000;
asset=AMD
```

The amount may also be sent as `amount=500000 AMD`; the route id is still
validated against Pay3Flow’s private registry before any edge is priced.

Pay3Flow validates the route ID against its private registry, prices each edge,
and returns a short-lived `RouteQuote`. The private registry graph is not exposed as a public graph endpoint. Public
`GET /api/p2p/routes` and `/ws/p2p/routes` do expose bounded, quoted route-search
results; these are separate from this ActivityPub quote-by-ID contract.

Fiat workflow discovery now reserves quote work for every selected provider,
regardless of the result display limit. Token limits count symbols and retain
all matching networks. Entry and exit offers must match the quote's networks.
A missing quote no longer creates a 1:1 token estimate; successful live quotes
stream immediately and exact cached quotes honor their expiry.

The 2026-10-12 provider audit also extends Symbiosis read-only discovery to 38
identified EVM networks. Execution still uses its separate configured-contract
registry. bncex non-cash requests include the required `fiatDestination=ACCOUNT`.

## Frontend widget and credentials

Public search uses the backend 1Click adapter described above. The SWAP chapter
uses the official Aurora standalone React widget as a Svelte client island.
The widget has its own wallet connections, quotes and status flow and can remain
available without a Pay3Flow execution descriptor or header Solana wallet.

`PUBLIC_AURORA_INTENTS_API_KEY` is a frontend runtime integration key from
[Aurora Widget Studio](https://studio.aurora.dev/). It is distinct from the
server-only `NEAR_INTENTS_JWT`. Configure it in ignored `.env.local` locally
and the frontend process environment in production. The SDK's raw token
allowlists include `wNEAR` and `USDT0` aliases; current-leg networks and exact
input amounts seed the widget. See [SWAP widgets](swap-widgets.md).

For ordinary crypto routes, discovery also searches bounded two-provider paths
when `allow_cross_venue=true`, joining the exact first output to the next input.
Catalog availability, native execution support and actual pair liquidity are
separate checks. See the [current algorithm](route-virtualization-pipeline.md).
