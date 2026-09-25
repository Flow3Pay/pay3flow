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

The background capability refresh reads:

```text
NEAR_INTENTS_URL=https://1click.chaindefuser.com
NEAR_INTENTS_QUOTE_RECIPIENT=<valid destination-chain address>
NEAR_INTENTS_QUOTE_REFUND_TO=<valid origin-chain address>
NEAR_INTENTS_REFRESH_SECS=300
ROUTE_MAX_DEPTH=4
ROUTE_SOURCE_FIATS=AMD
ROUTE_P2P_ASSETS=USDT,USDC,XRP
ROUTE_INTENT_ASSETS=USDT@tron,USDC@solana,XRP@xrpl
ROUTE_WITHDRAWALS=USDT@binance>USDT@tron|binance|1.5
```

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
