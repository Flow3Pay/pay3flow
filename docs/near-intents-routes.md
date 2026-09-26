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
The two quote-address settings are required because 1Click validates address
syntax even for dry quotes. Credentials are read from `NEAR_INTENTS_JWT` and
are not included in route capabilities or logs.

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

`ROUTE_WITHDRAWALS` is deployment-owned capability data. Each entry has the
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
and returns a short-lived `RouteQuote`. No route graph or route-search result
is exposed through a public HTTP API.
