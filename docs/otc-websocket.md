# OTC test WebSocket

## Share settings and messenger previews

The browser keeps market, side, order type, decimal amount/price and settlement
networks in `/#/otc?...`. Opening this URL restores the form without creating an
order. Share OTC stores the settings and the current public chart/book snapshot
in PostgreSQL and copies a short `/otc/{id}` URL. Its server-rendered metadata
points to `/otc/{id}/preview.png`, a 1200×630 Bridge, Chart and Orderbook image.
Browsers expand the short link into the form URL; crawlers receive metadata
without JavaScript. The preview records prices at sharing time.

SWAP and instruction shares use `/s/{id}` with the same persistent storage.
`POST /api/share-links` accepts `{target, preview?}`; `GET /api/share-links/{id}`
reads it. Targets are restricted to internal exchange/settings paths; snapshots
contain public prices only. The additive `share_links` table is applied at
backend startup. Frontend server requests use `SHARE_API_URL` (Docker Compose:
`http://backend:8080`, k3s default: `http://pay3flow-backend:8080`).

## Market transport

The OTC workspace uses `GET /ws/otc` for its book, trades, chart ranges, network
catalog, orders and mutations. It resolves the backend through `PUBLIC_API_URL`,
like the existing frontend WS clients. HTTPS deployments use WSS. Existing
Caddy and Kubernetes `/ws` rules cover this endpoint.

This is a **test market** with generated sample liquidity and simulated market
orders. It does not submit wallet transactions, perform settlement, or use the
fiat exchange-order API. The server validates amounts, precision, networks and
market-order liquidity; market execution prices are calculated by the server.

## Subscribe and recover

```json
{"type":"subscribe","sessionId":"398d2c4e-d58f-4baa-9fb5-71c2cb8a1987","marketId":"EVER-USDT"}
```

The frontend generates an opaque UUID session credential and keeps it in
`sessionStorage`. Treat it as a bearer credential for **test orders only**.
A connection can change its market by subscribing again, but cannot change
its session. Markets are EVER-USDT, BTC-USDT, ETH-USDT and SOL-USDT.

The server immediately sends `{"type":"snapshot", ...}` and pushes another
snapshot every five seconds. Fields are:

- `mode`: `test`.
- `market`: price, display metadata and sample 24h statistics.
- `snapshot`: `marketId`, `mode: "test"`, `bids`, `asks`, `trades`.
- `candles`: arrays keyed by `1D`, `7D`, `1M`, `1Y`.
- `networks`: the test asset/network catalog.
- `orders`: all orders belonging to the subscribed session, across markets.

Times are Unix milliseconds. Prices and amounts are decimal currency units.
The same UUID recovers orders after WS reconnects and page reloads. Orders are
held **in memory per backend process**, for up to 24 hours without activity;
a server restart clears them. There is a cap of 1,000 sessions and 100 orders
per session. For multiple replicas use a sticky session. Durable storage and
an authenticated production trading engine are separate work.

## Create and cancel

```json
{"type":"create","id":"6f8acba4-cc31-43ef-b041-1d2a12848198","order":{"marketId":"EVER-USDT","side":"buy","type":"limit","price":0.01,"amount":100,"sendNetwork":"Ethereum (ERC-20)","receiveNetwork":"Everscale"}}
```

`id` is both the correlation UUID and the new order UUID. Repeating the same
create command returns the same order. Reusing the UUID for a different order
returns `id_conflict`. A limit order has status `open`; a market order has
status `simulated` and uses the server's opposite book. This test contour does
not match resting limit orders or consume shared liquidity.

```json
{"type":"cancel","id":"47523f91-345e-4c36-89d3-17a6a145d204","orderId":"6f8acba4-cc31-43ef-b041-1d2a12848198"}
```

A successful mutation returns `{"type":"order","id":"<correlation UUID>",
"order":{...}}`. Order fields are `id`, `marketId`, `side`, `type`, `price`,
`amount`, `sendNetwork`, `receiveNetwork`, `createdAt`, `status`. Cancellation
is idempotent and scoped to the subscribed session; simulated orders cannot
be cancelled.

Failures return `{"type":"error","id":"<UUID or null>","code":"..."}`.
Codes include `invalid_message`, `unknown_market`, `subscribe_first`,
`session_conflict`, `invalid_amount_or_price`, `precision`, `invalid_network`,
`liquidity`, `order_not_found`, `order_not_open`, `id_conflict`,
`session_capacity` and `order_capacity`. `{"type":"ping"}` returns
`{"type":"pong"}`; WebSocket ping frames are also supported.

The frontend waits for a valid snapshot before enabling operations and for a
server acknowledgment before updating an order. It reconnects with bounded
backoff, resubscribes to the current market and closes silent connections.
Pending commands fail on disconnect or timeout, and are never automatically
replayed. After an uncertain result, a snapshot reconciles server state;
retrying the same review draft reuses the creation UUID.
