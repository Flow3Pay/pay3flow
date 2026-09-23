# API overview

The backend listens on `http://localhost:8080` in the development Compose
stack. Authenticated routes use `Authorization: Bearer <token>`.

## Health and authentication

```text
GET  /health
POST /api/auth/register
POST /api/auth/login
GET  /api/auth/me
```

The development auth flow uses an email and demo code. It is not a production
identity or account-verification system.

## Exchange order lifecycle

```text
GET  /api/exchange/corridors
POST /api/exchange/orders
GET  /api/exchange/orders
GET  /api/exchange/orders/:id
GET  /api/exchange/orders/:id/quotes
POST /api/exchange/orders/:id/discover
POST /api/exchange/orders/:id/auction
POST /api/exchange/orders/:id/confirm
POST /api/exchange/orders/:id/funding/confirm
GET  /api/exchange/orders/:id/settlement
POST /api/exchange/orders/:id/settlement
GET  /api/exchange/orders/:id/ledger
GET  /api/exchange/orders/:id/proofs
POST /api/exchange/orders/:id/proof
POST /api/exchange/orders/:id/cancel
```

Order creation requires an `Idempotency-Key` header. A minimal request is:

```bash
curl -fsS -X POST http://localhost:8080/api/exchange/orders \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Idempotency-Key: demo-order-1' \
  -H 'Content-Type: application/json' \
  -d '{
    "source_country":"AM",
    "source_currency":"AMD",
    "source_amount_minor":10000000,
    "source_method_type":"bank_card",
    "target_country":"RU",
    "target_currency":"RUB",
    "target_amount_min_minor":null,
    "target_method_type":"bank_card",
    "target_method_ref":"demo-recipient"
  }'
```

Amounts are integer minor units. The API validates the corridor, limits,
idempotency key, and runtime safety controls before creating an order.

## Solver API

```text
GET  /api/solver/orders/open
POST /api/solver/orders/:id/quotes
```

These routes are an MVP surface for solver integration. They require an
authentication and callback-signing design before production use.

## Legacy payments and directories

```text
POST /api/payments
GET  /api/payments
GET  /api/payments/:id
POST /api/providers/:provider/webhooks
GET  /api/banks
GET  /api/networks
GET  /api/providers
GET  /api/exchange-pairs
GET  /api/p2p/search
GET  /api/p2p/routes
```

The payment and acquiring routes are retained for compatibility. New exchange
clients should use the exchange order API.

## Admin and debugging routes

Admin routes use `Authorization: Bearer $ADMIN_TOKEN` and must be protected by
network policy in addition to the token:

```text
POST /api/admin/exchange/controls
POST /api/admin/exchange/corridors/:id
POST /api/admin/exchange/solvers/:id
POST /api/admin/exchange/orders/:id/manual-review
POST /api/admin/exchange/orders/:id/dispute
POST /api/admin/banks
POST /api/admin/banks/:name/status
POST /api/admin/exchange-pairs
POST /api/admin/exchange-pairs/:id
```

Debug and ActivityPub routes include `/api/debug/quote`, `/api/debug/task`,
`/api/debug/exchange/orders/:id/audit`, `/inbox`, `/actor`, and
`/.well-known/webfinger`. Treat them as operational interfaces, not a stable
public API.

## WebSockets

```text
GET /ws
GET /ws/rates
GET /ws/payments
GET /api/exchange/orders/:id/live
```

The exchange live endpoint emits route and status updates while an order is
discovering or being quoted. Clients must tolerate reconnects and duplicate
events; the durable order endpoint remains authoritative.
