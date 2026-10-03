# API overview

The backend listens on `http://localhost:8080` in the development Compose
stack. Authenticated routes use `Authorization: Bearer <token>`.

## Health and authentication

```text
GET  /health
POST /api/auth/register
POST /api/auth/login
GET  /api/auth/me
GET  /api/referrals/me
```

The development auth flow uses an email and demo code. It is not a production
identity or account-verification system.

Registration accepts an optional `referral_code`. Attribution is permanent and
must be set when the referred account is created. `GET /api/referrals/me`
returns the caller's share code, direct and total network counts, balances by
currency, and recent commissions. A direct referrer earns 10% of Pay3Flow's
service fee after a referred exchange reaches `done`; retries cannot create a
duplicate commission for the same order.

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
POST /api/routes/instruction-open
POST /api/service-executions/open
```

The payment and acquiring routes are retained for compatibility. New exchange
clients should use the exchange order API.

`/api/routes/instruction-open` accepts `{ "anonymous_id": "<browser UUID>",
"instruction_token": "<signed token from instruction_tokens[route_id]>" }`.
The click is queued for anonymous Redis deduplication and aggregate persistence;
the response is `{ "accepted": true }`. `/api/service-executions/open` accepts
the existing `anonymous_id` and `tracking_token` body and returns the redirect
URL. New link opens are aggregated without writing browser IDs to the
`service_executions` table.
Service statistics also include `reputation_score` (0–100); the aggregate
route reputation includes `score_average`. These scores prioritize background
provider polling. Close-priced route ordering uses only likes and dislikes.
`PUT /api/routes/{route_id}/vote` accepts the optional signed
`instruction_token` returned for that route. The browser sends it so a route
vote also updates its providers' service vote totals; older clients can still
vote without that attribution.

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
