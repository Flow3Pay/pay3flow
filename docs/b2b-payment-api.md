# B2B crypto routing API

Status: phase 1 implemented · 2026-11-10 · scope: v1

## 1. Model

A business asks Pay3Flow to move value from A to B: "send X, the recipient gets
Y at address Z". The router searches **executable** sources only, the platform
executes, and the business makes no manual swap.

- Visible commission for the business: **0%**.
- Pay3Flow margin: embedded in the rate and collected by the platform.
- V1 is described in OpenAPI first. Authentication reuses the existing user
  Bearer token; merchant API keys are deferred until after the implementation
  works (see §20–21).
- Sources: NEAR Intents now; Pay3Flow OTC when it becomes instant and covers the
  assets B2B needs.
- Excluded: spot, P2P ads and ordinary exchangers (indicative only), and
  wallet-signed swaps (CoW, Symbiosis) unless the business delegates a wallet.

This is a different product from a generic checkout: it is a routing and
settlement API where the platform's revenue is the difference between the best
executable route and the rate quoted to the business.

## 2. Execution fork

"Zero manual operations", "live re-routing" and "unlimited size" cannot all be
true at once without control of the funds at execution time. Pick one model:

| Model | Business action | Who holds funds | Live re-routing | Size | Cost |
| --- | --- | --- | --- | --- | --- |
| Per-order deposit | one transfer to a deposit address | nobody (solver transiently) | before deposit only | solver/OTC liquidity | low |
| Prefunded balance | top up once, then API | platform holds business float | yes | platform balance sheet | custody + KYB |
| Delegated wallet | connect wallet/session once | business | yes | wallet balance | integration |

Recommended v1: **per-order deposit with NEAR Intents**. It is the only model
that is fully automatable, non-custodial and buildable now. Prefunded balance is
the next step if customers demand straight-through processing and live
re-routing; that is a custody and compliance decision, not an API feature.

## 3. What is executable today

| Source | Quote in router | Automated execution | B2B fit |
| --- | --- | --- | --- |
| NEAR Intents | yes | yes, one deposit, solver delivers | **v1** |
| Pay3Flow OTC | yes (`otc/route_provider.rs`) | no: manual accept + two signatures | after instant OTC |
| CoW Swap | yes | wallet signature required | no |
| Symbiosis | yes | wallet signature required | no |
| Spot / P2P / exchangers | yes, indicative | manual at the venue | no |

Important details:

- `Trustfile` already gates executable providers: `near-intents`, `cow-swap`,
  `symbiosis`. Use the same gate for B2B.
- OTC already appears in route search, but only for **EVER ↔ USDT**, and
  `route_executions` does not support `pay3flow-otc`. So OTC is a quote source,
  not an executable B2B rail.
- The OTC public book is `EVER/USDT` offers. It adds nothing to USDT/USDC,
  USDT/BTC or USDT/ETH transfers. Including OTC in B2B requires both instant
  execution and the pairs businesses use.

## 4. Validated Intents mechanics (2026-11-10)

`POST https://1click.chaindefuser.com/v0/quote` returns:

- `EXACT_INPUT` and `EXACT_OUTPUT`;
- a unique `depositAddress` per quote;
- `refundTo` for unfilled deposits;
- `appFees: [{ recipient, fee }]`, where `fee` is in bps.

Measured same-chain USDT Ethereum → USDT Ethereum:

| Mode | Input | Output | Platform fee |
| --- | --- | --- | --- |
| exact-in, 0 bps | 100.00 | 99.70 | 0 |
| exact-in, 50 bps | 100.00 | 99.20 | 0.50 |
| exact-out, 0 bps, invoice 49.00 | 49.798 | 49.00 | 0 |
| exact-out, 50 bps, invoice 49.00 | 50.048 | 49.00 | ~0.25 |

A dry exact-in quote for 1,000,000 USDT also returned successfully. That is an
API capability, not a liquidity guarantee.

Open question: the provider returned a 3-day `deadline` for a request that asked
for 2 hours. Verify `deadline` and `timeWhenInactive` before relying on them;
enforce our own TTL.

## 5. Edge capture

For every order:

1. Find the best executable output `best_out` (or the required input
   `best_in`).
2. Choose the platform margin `m` (bps), capped.
3. Quote the business `business_out = best_out - m` (exact-in) or require
   `business_in = best_in + m` (exact-out).
4. For Intents, set `appFees = m` with the platform fee recipient. The provider
   deducts the fee and pays the platform; the business sees the net rate.
5. Record `best_out`, `quoted_out`, `captured_bps`, the chosen provider and the
   fee transaction in `b2b_fee_ledger`.

This is the "0% commission, margin in the rate" model. Two conditions:

- **Disclose it.** Say "0% Pay3Flow fee; the rate includes liquidity margin".
  A hidden variable markup is discovered on the first audit and costs trust.
- **Cap it.** A fixed cap (for example 0.7%) keeps the quote competitive and
  prevents the router from selling a bad route at a large markup.

For OTC, the same model requires the platform to be the taker/counterparty: the
public offer price is the maker's price, and the platform cannot mark it up
unless it buys and resells. That is a desk/inventory decision, not a routing
feature. A separate fee transfer (the existing 3% P2P pattern) is the only
non-custodial option today, and it is visible.

## 6. Limits of "search OTC during the exchange"

- Once the payer sends funds to an Intents deposit address, the route is fixed.
  The platform cannot switch that deposit to an OTC order.
- Re-routing at fill time requires the platform to control the funds
  (prefunded balance or treasury) or a protocol that auctions after deposit.
  Intents auctions, but the quote is firm and the deposit address belongs to the
  chosen solver; there is no switch to OTC mid-flight.
- Therefore OTC can only compete **before** the deposit, at quote time. That is
  still useful: the router can compare Intents and OTC quotes and pick the
  better one. It just cannot change its mind after the customer pays.

## 7. Size

- The API can quote large amounts, but the actual fill is limited by solver and
  counterparty liquidity.
- Large orders should be split across multiple quotes or routed to the desk with
  confirmed liquidity.
- "Unlimited" is not a property of automation; it is a property of the balance
  sheet. Do not promise it in sales material.

## 8. Money flow (per-order deposit)

```text
Business backend
   │  POST /api/v1/quotes  (send 1000 USDT@ethereum -> USDT@tron)
   ▼
B2B API
   │  router: Intents quote, exact-in, appFees = margin
   ▼
depositAddress (per order)
   ▲
   │  business sends 1000 USDT
   │
   ├─ solver delivers net output to the recipient
   ├─ app fee goes to the Pay3Flow fee recipient
   └─ unfilled deposit is refunded to refundTo
```

The platform observes the deposit and delivery, records both, and emits
webhooks. It does not control the deposit address or the recipient.

## 9. Reuse map

| Need | Existing code | Reuse |
| --- | --- | --- |
| Quote search and route graph | `backend/src/route_engine` | yes |
| Executable gating | `backend/src/trustfile.rs` | yes |
| OTC as a quote source | `backend/src/otc/route_provider.rs` | yes, extend later |
| Intents provider | `NearIntentsProvider` | yes, add app fees and exact-in/out |
| Chain confirmation | `otc_observers` policy | extract shared helpers |
| Retries | `otc_outbox` | new webhook outbox |
| Auth | `users`, Bearer token | API-key middleware |
| Idempotency | existing `Idempotency-Key` | same convention |
| Money | `payments` uses `f64` | **no**, integer strings |

## 10. Data model

New migration `backend/migrations/b2b.sql`. Prefix `b2b_`.

```sql
CREATE TABLE b2b_merchants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft','active','suspended')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- b2b_api_keys is intentionally deferred. V1 authenticates with the existing
-- user Bearer token; keys are added in phase 1.1 (see §20–21).

CREATE TABLE b2b_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    env TEXT NOT NULL CHECK (env IN ('test','live')),
    kind TEXT NOT NULL CHECK (kind IN ('transfer','checkout')),
    external_id TEXT,
    input_asset TEXT NOT NULL,
    input_network TEXT NOT NULL,
    output_asset TEXT NOT NULL,
    output_network TEXT NOT NULL,
    recipient TEXT NOT NULL,
    refund_address TEXT,
    status TEXT NOT NULL DEFAULT 'quoted',
    metadata JSONB NOT NULL DEFAULT '{}',
    idempotency_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, env, idempotency_key)
);

CREATE TABLE b2b_quotes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    provider TEXT NOT NULL,
    provider_quote_id TEXT NOT NULL,
    exact TEXT NOT NULL CHECK (exact IN ('input','output')),
    input_amount TEXT NOT NULL,
    output_amount TEXT NOT NULL,
    best_output TEXT NOT NULL,
    captured_bps INTEGER NOT NULL DEFAULT 0,
    app_fee_bps INTEGER NOT NULL DEFAULT 0,
    deposit_address TEXT NOT NULL,
    refund_address TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    raw JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (order_id, provider_quote_id)
);

CREATE TABLE b2b_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    quote_id UUID REFERENCES b2b_quotes(id),
    kind TEXT NOT NULL CHECK (kind IN ('deposit','delivery','refund')),
    chain TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    from_address TEXT,
    amount TEXT NOT NULL,
    confirmations INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (chain, tx_hash, kind)
);

CREATE TABLE b2b_events (
    id BIGSERIAL PRIMARY KEY,
    order_id UUID NOT NULL REFERENCES b2b_orders(id),
    kind TEXT NOT NULL,
    body JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE b2b_webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    url TEXT NOT NULL,
    secret_ciphertext BYTEA NOT NULL,
    events TEXT[] NOT NULL,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE b2b_webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    endpoint_id UUID NOT NULL REFERENCES b2b_webhook_endpoints(id),
    event_id BIGINT NOT NULL REFERENCES b2b_events(id),
    event_kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status TEXT NOT NULL DEFAULT 'pending',
    response_code INTEGER,
    response_body TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE b2b_fee_ledger (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    order_id UUID REFERENCES b2b_orders(id),
    asset TEXT NOT NULL,
    amount TEXT NOT NULL,
    captured_bps INTEGER NOT NULL,
    recipient TEXT NOT NULL,
    tx_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Rules: exact decimal strings or minor units, never `f64`; one order may have
several quotes (re-quote), each with its own deposit address; `b2b_events` is
both the audit log and the webhook source.

## 11. API

Merchant API (`Authorization: Bearer <user token>`; API keys deferred):

```text
POST /api/v1/quotes          get a quote (input or output amount)
POST /api/v1/transfers       accept a quote and create the order
GET  /api/v1/transfers/:id
GET  /api/v1/transfers
POST /api/v1/transfers/:id/cancel
```

Checkout wrapper for hosted payments:

```text
POST /api/v1/payments        quote + hosted page token
GET  /pay/:token             public page
GET  /pay/:token/status
POST /pay/:token/quote       re-quote
```

Dashboard:

```text
POST   /api/b2b/webhooks
GET    /api/b2b/orders
GET    /api/b2b/analytics
```

The API-key management endpoints (`POST /api/b2b/keys`) are deferred with the
keys themselves.

### OpenAPI

Add the B2B endpoints to `backend/src/server/openapi.rs` under a new `B2B`
tag. The v1 document contains:

- schemas `B2BQuoteRequest`, `B2BQuote`, `B2BTransfer`, `B2BTransferEvent`;
- the existing `bearerAuth` security scheme, no API-key scheme yet;
- request/response examples for quote, transfer and `order.paid`;
- `Idempotency-Key` documented on create operations.

A compact shape:

```yaml
openapi: 3.0.3
paths:
  /api/v1/quotes:
    post:
      tags: [B2B]
      summary: Quote a crypto transfer
      security: [{ bearerAuth: [] }]
      requestBody:
        required: true
        content:
          application/json:
            schema: { $ref: '#/components/schemas/B2BQuoteRequest' }
      responses:
        '200':
          description: Quote with deposit address and expiry
          content:
            application/json:
              schema: { $ref: '#/components/schemas/B2BQuote' }
  /api/v1/transfers:
    post:
      tags: [B2B]
      summary: Accept a quote and create a transfer
      security: [{ bearerAuth: [] }]
      parameters:
        - in: header
          name: Idempotency-Key
          required: true
          schema: { type: string }
      responses:
        '201':
          description: Transfer created
          content:
            application/json:
              schema: { $ref: '#/components/schemas/B2BTransfer' }
```

Quote request:

```json
{
  "input": { "asset": "USDT", "network": "ethereum", "amount": "1000.00" },
  "output": { "asset": "USDT", "network": "tron" },
  "recipient": "T...",
  "refund_address": "0x..."
}
```

Quote response:

```json
{
  "quote_id": "q_...",
  "provider": "near-intents",
  "input_amount": "1000.00",
  "output_amount": "994.50",
  "best_output": "995.00",
  "captured_bps": 50,
  "deposit_address": "0x...",
  "expires_at": "2026-11-10T18:20:00Z"
}
```

The business accepts the quote within its TTL, sends the input, and receives
`output_amount` at the recipient. The hosted checkout uses the same quote
engine and adds a payment page, QR and countdown.

### Payment page

One screen, no wallet connection and no signature:

- amount and asset/network chosen by the payer;
- the rate and the exact `input_amount` (all fees included);
- deposit address, QR code, copy button and a visible network warning;
- countdown and status: `awaiting_deposit` → `processing` → `paid`;
- an "I have paid" button that only refreshes the status, never marks paid;
- a link to the transaction and the merchant's `success_url`.

Rules:

- the page never shows `paid` from the browser; it polls the backend and the
  backend waits for delivery, not for the deposit hash;
- the quote is locked for the whole payment window. Default 15 minutes, hard
  maximum 60 minutes. Stablecoin pairs may use the full hour; volatile assets
  stay at 10–15 minutes;
- after expiry the page shows `expired`, offers a re-quote with a new deposit
  address, and any late deposit is refunded to `refundTo` or handled manually;
- the page does not ask for an email, account or wallet;
- mobile-first: QR and address are the primary actions, the network is always
  visible.

## 12. State machine

```text
quoted ─► awaiting_deposit ─► processing ─► paid
   │            │                ├─► underpaid
   │            │                ├─► overpaid
   │            ├─► expired      └─► failed
   └─► cancelled
paid ─► refunded
```

`paid` means delivery to the recipient, not the payer's transaction. Never infer
success from the deposit hash alone.

`awaiting_deposit` never lasts longer than 60 minutes. After that the order
expires or is re-quoted; a late deposit is refunded to `refundTo` or handled
manually.

## 13. Webhooks

```text
order.quoted
order.awaiting_deposit
order.processing
order.paid
order.underpaid
order.overpaid
order.expired
order.failed
order.refunded
order.cancelled
```

Headers: `Pay3Flow-Event-Id`, `Pay3Flow-Delivery-Id`,
`Pay3Flow-Signature: t=...,v1=<HMAC-SHA256("t.body")>`. At-least-once delivery,
exponential backoff, dead-letter, replay from the dashboard. Reuse the
`otc_outbox` lease/attempt pattern.

## 14. Security

- V1 uses the existing user Bearer token. Merchant API keys are deferred to
  phase 1.1; until then the API serves the internal/first merchant account.
  Phase 2 adds one-page business registration (email code + GitHub OAuth2 /
  Telegram OIDC), membership and the portal; see
  [`b2b-merchant-portal.md`](b2b-merchant-portal.md).
- When keys land: `sk_` secrets shown once, stored as an Argon2id hash + prefix;
  `pk_` identifiers are not secrets; separate test/live environments; scopes
  `rates:read`, `quotes:write`, `orders:read`, `orders:write`, `refunds:write`,
  `otc:write`; every key belongs to a project and has a daily request limit.
- Per-account rate limits and per-merchant volume limits.
- Idempotency on quote/transfer creation; same key + same body returns the
  original, a different body conflicts.
- No private keys or seed phrases on the server.
- Manual KYB, sanctions screening and a signed agreement before live use.

## 15. Dashboard

- orders: quote, deposit tx, delivery tx, captured margin, timeline;
- webhooks with delivery log and replay;
- analytics: volume, conversion, average ticket, margin bps, failure reasons;
- settings: default recipients and refund addresses;
- test mode with simulated chain events;
- API-key management is added with the keys in phase 1.1.

Phase 2 replaces this read-only dashboard with a registered merchant portal
(account, team, watch-only balances, fees, settings, API-key UI). The detailed
page, data-model and registration plan is
[`b2b-merchant-portal.md`](b2b-merchant-portal.md).

## 16. Phases

### Phase 1 — Intents-only transfer API (weeks 1–3)

- `b2b.sql`, quote/transfer/create/get/cancel;
- Intents exact-in/out with `appFees`, per-order deposit, delivery polling;
- webhook outbox with signature;
- edge capture and fee ledger;
- OpenAPI entries under the `B2B` tag;
- first real transfer: Pay3Flow sells whitelabel/API access and moves the
  proceeds.

Acceptance: a real payer sends $25–100, the recipient receives the quoted
amount, the platform margin lands at the fee recipient, `order.paid` is
delivered and signed, re-quote and expiry behave, an underpaid case does not
mark paid.

### Phase 1.1 — merchant API keys (after v1 works)

- `b2b_api_keys` under `b2b_projects`, issue/rotate/revoke, test/live, scopes,
  daily request limit and usage counters;
- switch authentication from the user Bearer token to `sk_`/`pk_`;
- document the security scheme in OpenAPI;
- remind the user about this step when v1 is running.

### Phase 2 — merchant accounts, registration and portal (weeks 4–8)

Full plan: [`b2b-merchant-portal.md`](b2b-merchant-portal.md).

Phase 1 works without registration, and that stays true for anonymous swaps,
quotes and hosted checkout. A business product cannot run on anonymous mode
alone: balances, team access, webhooks, analytics, API keys and live limits all
need a persistent workspace.

- business registration and sign-in on `/register`: one page, passwordless
  email code, and existing email signs in while a new email registers;
- GitHub OAuth2 and Telegram OIDC handled on the backend, with verified-email
  account linking;
- “Для бизнеса” workspace home: all-time cashbox, metrics and graphs, API-key
  creation, recent orders and environment switch; cashbox is a read-only
  cumulative report in Phase 2, not a spendable wallet;
- workspace, membership and roles (owner/admin/member/viewer);
- onboarding and test/live environments;
- portal pages: overview, orders, order detail, balances, fees, checkout links,
  webhooks, analytics, settings, team and API keys;
- **read-only balance monitoring** — watch-only on-chain addresses, fee ledger,
  volume and settlement status; no custody and no withdrawals in Phase 2;
- audit log for account/settings mutations;
- API-key lifecycle surfaced in settings (backend keys land in Phase 1.1);
- manual KYB/sanctions gate before live volume;
- the registration Figma is received: implement it without the green glow,
  without hover effects, and without the separate login prompt/button; other
  portal Figma pages are still pending, so their routes and data contracts are
  fixed in the portal plan until designs arrive;
- Phase 2 pages use a full-width shell with side padding instead of the usual
  centred fixed-width grid; cards, tables and columns expand horizontally to
  use the available space.

Acceptance: a new business can register, finish onboarding and quote in test
without support; an existing email signs in through the same page; GitHub and
Telegram OAuth2 round trips work; the “Для бизнеса” home shows cashbox, graphs
and API-key creation; roles and audit are enforced; balances and fees match the
ledger; live stays disabled until approval; anonymous checkout still works.

### Phase 2.1 — instant OTC (weeks 4–8, only with a counterparty)

- firm OTC quote API with a TTL and reserved size;
- automatic accept and settlement agent, no two-wallet dance;
- dynamic fee/spread per quote;
- add OTC to the router only for pairs B2B actually uses.

### Phase 3 — prefunded balance and live re-routing

- business balance, straight-through processing, split routes;
- live comparison of Intents and OTC before each execution;
- custody/safeguarding, KYB and reconciliation;
- subscriptions/credits for API/AI customers.

## 17. Non-goals for v1

- custody of business or customer funds;
- fiat on/off-ramp and bank payouts;
- automated KYC/KYB;
- subscriptions and usage billing;
- n8n-style workflow builder;
- all networks and tokens at once;
- promising unlimited size;
- merchant API keys (deferred to phase 1.1).

## 18. Open decisions

1. Margin cap and the exact disclosure wording ("0% fee, rate includes margin").
2. OTC timing: Phase 2.1 unless a counterparty with instant fills is ready.
3. KYB/sanctions owner and launch gate.

## 19. Risks

- **Intents dependency.** Availability, quote quality, refund behavior and
  deadline semantics become the SLA.
- **Small-ticket economics.** Observed all-in cost is 4.6% at $10 and 32% at $1
  on Ethereum. Enforce minimums.
- **OTC mismatch.** EVER/USDT ads are not a B2B liquidity source; making OTC
  programmable is a separate product.
- **Margin trust.** A variable hidden spread can look like a bad rate. Cap and
  disclose.
- **Liquidity, not automation, limits size.** Do not sell "unlimited".
- **Regulatory.** Non-custodial lowers, but does not remove, KYB, sanctions and
  legal review before public launch.

## 20. Delivery rules

- Commit in small logical steps and push `master` at the end of the work.
- All checks must be green before the work is called done:
  - backend: `cargo fmt --check`, `cargo check`, `cargo test --all-features`;
  - frontend when touched: `pnpm run check`, `pnpm run test:unit`,
    `pnpm run build`;
  - e2e when relevant: `pnpm run test:e2e`.
  If the host cannot run a suite, report the exact blocker and do not call the
  work green.
- Update this plan and `backend/src/server/openapi.rs` with every API change.

## 21. Reminders

- [ ] **After B2B v1 is implemented and running:** add merchant API keys
  (`b2b_api_keys`, issue/rotate/revoke, test/live, scopes) and switch the API
  from the user Bearer token to `sk_`/`pk_`. Remind the user about this at that
  point.
- [ ] **Phase 2:** registration, merchant workspace, team roles and the portal
  documented in [`b2b-merchant-portal.md`](b2b-merchant-portal.md). Use backend
  OAuth2 for GitHub and Telegram; do not ship the current OAuth stub. Do not
  remove anonymous swap/checkout.
- [ ] **Figma:** the registration page design is received and must be built
  without glow, without hover effects and without a separate login prompt. The
  owner will send the remaining portal designs later; map them to the page
  inventory before implementing those pages.

## 22. Phase 1 implementation notes (2026-11-11)

What is in the tree:

- `backend/migrations/b2b.sql` and its registration in `db::apply_schema`.
  Besides the tables above it adds `b2b_orders.accepted_quote_id` (the quote the
  merchant actually funded after a re-quote), `b2b_orders.transfer_key`
  (transfer-creation idempotency, separate from quote idempotency) and
  `b2b_checkout_sessions` (hosted page token, `success_url`, expiry).
- `backend/src/b2b/`: quote/transfer/checkout service, fee ledger, analytics,
  webhook outbox with HMAC signatures, background settlement worker and the
  dependency-free QR renderer for the hosted page.
- Each quote is requested twice when a margin is configured: a fee-free
  reference quote records `best_output` in the ledger, and the executable quote
  carries `appFees = margin`. Without `B2B_FEE_RECIPIENT` the margin is zero and
  no fee is charged.
- `backend/src/server/routing/b2b.rs`: `/api/v1/quotes`, `/api/v1/transfers`,
  `/api/v1/payments`, `/pay/{token}` (page), `/pay/{token}/status`,
  `/pay/{token}/quote`, `POST /api/b2b/webhooks`, `GET /api/b2b/orders` and
  `GET /api/b2b/analytics`.
- OpenAPI operations under the `B2B` tag with the schemas and examples listed
  above, including `Idempotency-Key` on create operations.
- `NearIntentsProvider` now accepts `app_fees` in `NearQuoteRequest` and sends
  them as `appFees`; every other caller passes an empty list.

Configuration:

- `config.toml`: `b2b_margin_bps` (default 50), `b2b_margin_cap_bps`
  (default 70, hard maximum 70) and `b2b_quote_ttl_secs` (default 900, clamped
  to 60–3600).
- Environment: `B2B_FEE_RECIPIENT` — the NEAR Intents account that receives
  `appFees`. Keep it out of the TOML file.
- The hosted page URL uses `ap_origin`, so set it to the public origin.

Behaviour and deviations to keep in mind:

- `order.quoted` and `order.awaiting_deposit` are emitted when a transfer is
  created (or a checkout is created/re-quoted), not when the provisional quote
  is first requested. A cancelled transfer emits `order.cancelled`.
- `paid` follows the provider's delivered status, never the deposit hash.
  `underpaid` is terminal until the provider refunds or the order is handled
  manually.
- The worker polls accepted orders every 15 seconds, expires stale quotes and
  delivers webhooks with a 5-minute lease, exponential backoff and a dead
  letter after 8 attempts.
- Re-quoting is exposed on the hosted page only
  (`POST /pay/{token}/quote`). The merchant transfer API creates a new quote
  instead of mutating an accepted one.
- `Unavailable` B2B provider failures map to HTTP 503.

Process note (2026-11-11): deferring merchant API keys and business
registration without asking the owner was a mistake. API keys return in
Phase 1.1; registration, the merchant workspace and the portal are Phase 2,
documented in [`b2b-merchant-portal.md`](b2b-merchant-portal.md). Anonymous
swaps and checkout remain supported.


