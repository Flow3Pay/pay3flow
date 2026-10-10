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
GET /ws/otc
GET /ws/rates
GET /ws/payments
GET /api/exchange/orders/:id/live
```

The exchange live endpoint emits route and status updates while an order is
discovering or being quoted. Clients must tolerate reconnects and duplicate
events; the durable order endpoint remains authoritative.

The OTC test workspace uses `/ws/otc` for data and order commands. See [OTC WebSocket protocol](otc-websocket.md).

## Provider profiles

The frontend directory at `/providers` links to universal profiles at
`/providers/{slug}`. Profiles use the provider catalog's `market_types`
(`p2p`, `spot`, `exchanger`), local venue avatars, and the existing provider
review API. `market_types` describes integrations available in Pay3Flow.

`GET /api/providers/{slug}/statistics?period=30d` accepts `7d`, `30d` (default),
or `90d`. It returns `searches`, `top10`, `top1`, `average_rank`,
`response_samples`, `successful_responses`, `average_response_ms`, `site_opens`,
`first_seen`, `updated_at`, daily `days` and up to eight `directions`. Unknown
providers return 404; unsupported periods return 400. Periods use UTC calendar
days, including today. Every day is returned, including zero-search days.

Each explicitly counted, completed user search stores one final observation per
participating provider in `provider_search_observations`. Both REST and WebSocket
searches, including cached final results, record the final displayed ranking.
Progressive snapshots, auto-refreshes, failed and cancelled searches are excluded.
Multiple routes from a provider count as one appearance at its best displayed
rank. A provider without a displayed route remains in the denominator when its
source status was observed. Top-10 share is `top10 / searches`; it is unknown
when there are no observations. Average rank includes displayed routes only.
Cached source statuses do not contribute latency samples. Source response success
is calculated per observed search; it is not API uptime or completed-trade success.

`site_opens` counts service opens from route links in the selected period,
after spam rows are removed by the existing reputation rollback; it does not
measure completed trades. Historical ranking data starts when the feature is
deployed: older searches cannot be reconstructed from aggregate counters.
The profile shows explicit empty/error states instead of illustrative statistics.

Profile review ratings and filters use only the collected external-review sample.
Sharing copies the profile URL and period.
