# B2B merchant portal and registration

Status: plan · 2026-11-11 · extends [`b2b-payment-api.md`](b2b-payment-api.md) ·
UI designs: pending (Figma from the product owner)

Parent plan: [`b2b-payment-api.md`](b2b-payment-api.md) — this document inherits
its model, money rules, delivery rules and security constraints. Where the two
documents disagree, the parent plan wins for routing/settlement and this
document wins for accounts, registration and portal pages.

## 1. Why this phase exists

Phase 1 deliberately reused the existing user Bearer token and shipped without
merchant accounts or a portal. Anonymous mode is enough for a one-off quote, a
transfer or a hosted checkout, and it stays available. It is not enough to run a
business product:

- there is no persistent business identity, team or role model;
- there is no place to monitor balances, fees, volume and settlement status;
- there is no webhook/settings UI, so every change needs support;
- there is no KYB, sanctions or audit trail for live volume;
- API keys were deferred without asking the owner. They belong to the account
  lifecycle, not to a separate undocumented step.

Phase 2 adds registration and a merchant portal. It does **not** remove the
anonymous swap/checkout path and does **not** introduce custody. Prefunded
balances and payouts remain Phase 3.

## 2. Scope

### 2.1 In scope

- registration and login for business users (email one-time code, reusing the
  existing `users` identity);
- business workspace creation, membership and roles;
- onboarding checklist for a first quote/checkout;
- overview, orders, order detail, balances, fees, webhooks, analytics, settings,
  team and API-key pages;
- read-only balance monitoring: watch-only on-chain addresses, fee ledger,
  volume and settlement status;
- test/live environments per merchant;
- audit log for account and settings mutations;
- API-key lifecycle in the portal (backend keys may land earlier as Phase 1.1);
- manual KYB/sanctions gate before live volume.

### 2.2 Out of scope

- custody, prefunded balance, internal ledger transfers and withdrawals —
  Phase 3;
- fiat on/off-ramp and bank payouts;
- automated KYC/KYB and sanctions screening providers;
- subscriptions and usage billing;
- team SSO/SAML and hardware keys;
- pixel-perfect implementation before Figma designs arrive.

### 2.3 Product rule

Everything that exists today must still work without registration:

- anonymous swap/route search;
- the B2B quote/transfer API with a user Bearer token;
- hosted checkout pages for payers.

Registration is required for the **business workspace**: saved settings,
balances, team, webhooks, analytics, API keys and live limits.

## 3. Identity model

| Concept | Table | Meaning |
| --- | --- | --- |
| Login identity | `users` | Email owner of a session. One person may later belong to several businesses. |
| Business workspace | `b2b_merchants` | The billing/legal unit that owns orders, webhooks and addresses. |
| Membership | `b2b_members` | `user_id` ↔ `merchant_id` with a role. |
| Environment | `b2b_orders.env` | `test` or `live`, already present in Phase 1. |

Roles for v2:

- **owner** — created the workspace; billing, live enablement, members, API keys;
- **admin** — orders, webhooks, addresses, API keys (no ownership transfer);
- **member** — read orders and balances, create test orders;
- **viewer** — read-only.

Phase 2 creates one workspace per registering user and supports invites with an
email one-time code. Multi-workspace switching is a follow-up; the data model
must allow it from the start.

## 4. Registration and onboarding

### 4.1 Flow

1. `GET /business/register` — value proposition, "0% visible fee, margin in the
   rate" disclosure, link to the API docs.
2. Email one-time code using the existing auth flow. No password.
3. Create the business profile: legal name, display name, country, website,
   support email and contact person.
4. Create the workspace and make the user its `owner`.
5. Choose the default environment (`test` first).
6. Optional onboarding steps: add a payout/refund address, create a webhook,
   create the first test quote.
7. Live environment stays disabled until manual KYB/sanctions approval; test
   quotes and checkout pages work immediately.

### 4.2 Onboarding checklist

The overview shows a checklist until the first live transfer:

- business profile completed;
- payout/refund address saved;
- webhook endpoint verified (a signed `ping` delivery succeeds);
- first test quote created;
- first test checkout paid;
- KYB submitted.

### 4.3 Account states

```text
draft ─► active ─► suspended
             │
             └─► live_enabled (only after manual KYB)
```

Phase 1 `b2b_merchants.status` already has `draft|active|suspended`; Phase 2 adds
`live_enabled_at` and the profile/KYB fields rather than a second status column.

## 5. Portal pages

The Figma layouts are pending. Routes and data requirements below are the
contract the designs should cover.

| Route | Purpose | Key data / actions |
| --- | --- | --- |
| `/business` | Overview | volume, paid orders, conversion, captured fees, recent orders, onboarding checklist, environment switch |
| `/business/onboarding` | Guided setup | profile, address, webhook, first quote, KYB |
| `/business/orders` | All orders | filters (env, kind, status, asset, date), pagination, export |
| `/business/orders/:id` | Order detail | quote, deposit address, deposit/delivery tx, timeline, captured fee, re-quote link, webhook delivery log |
| `/business/balances` | Balance monitoring | watch-only on-chain balances, fees earned, volume, settlement status, freshness/stale marker |
| `/business/fees` | Fee ledger | captured bps, asset, amount, order, recipient, tx hash, CSV export |
| `/business/checkout-links` | Hosted pages | create/copy/disable checkout links, success URL, status |
| `/business/webhooks` | Endpoints | create endpoint, events, secret reveal-once, delivery log, replay, disable |
| `/business/analytics` | Reports | volume/ticket by asset, conversion, failure reasons, margin, date range |
| `/business/settings/profile` | Business profile | legal/display name, country, website, support email |
| `/business/settings/addresses` | Saved addresses | payout, refund and watch-only addresses per network |
| `/business/settings/team` | Members | invite, role change, remove; owner-only actions |
| `/business/settings/api-keys` | API keys | create/rotate/revoke, `sk_` reveal once, `pk_` prefix, scopes, last used |
| `/business/settings/security` | Security | sessions, audit log, live enablement state |

Every page must define loading, empty, error and permission-denied states. The
test environment gets a persistent banner; live gets a red/orange marker.

### 5.1 "Balances" semantics (important)

Phase 2 is **watch-only monitoring**, not a Pay3Flow wallet:

- on-chain balances of addresses the merchant saved, grouped by network/asset;
- fees earned from `b2b_fee_ledger`;
- volume and paid orders;
- provider settlement status for open orders;
- data timestamp and a clear `stale` marker when a refresh fails.

The page must not say "your Pay3Flow balance" or offer withdrawals. If the
business needs a spendable balance, that is Phase 3 (custody, safeguarding,
reconciliation and KYB). The exact balance definition is an open decision
(§12).

## 6. Data model additions

New migration `backend/migrations/b2b_portal.sql`, applied after `b2b.sql`.
Money stays as exact decimal strings; no `f64`.

```sql
CREATE TABLE b2b_business_profiles (
    merchant_id UUID PRIMARY KEY REFERENCES b2b_merchants(id),
    legal_name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    country TEXT NOT NULL,
    website TEXT,
    support_email TEXT,
    contact_name TEXT,
    live_enabled_at TIMESTAMPTZ,
    kyb_status TEXT NOT NULL DEFAULT 'not_started'
        CHECK (kyb_status IN ('not_started','pending','approved','rejected')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE b2b_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    user_id UUID NOT NULL REFERENCES users(id),
    role TEXT NOT NULL CHECK (role IN ('owner','admin','member','viewer')),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('invited','active','disabled')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, user_id)
);

CREATE TABLE b2b_watch_addresses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    network TEXT NOT NULL,
    address TEXT NOT NULL,
    label TEXT,
    kind TEXT NOT NULL DEFAULT 'watch'
        CHECK (kind IN ('payout','refund','watch')),
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (merchant_id, network, address)
);

CREATE TABLE b2b_audit_events (
    id BIGSERIAL PRIMARY KEY,
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    actor_user_id UUID REFERENCES users(id),
    kind TEXT NOT NULL,
    body JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Notes:

- `b2b_api_keys` is defined by Phase 1.1 and surfaced by the portal.
- `b2b_members` is the authorization source; `b2b_merchants.user_id` remains the
  creator and is not used for permission checks after the migration backfill.
- Migration backfills one `owner` membership per existing merchant and one
  profile row using the merchant name.
- Delivered/payout addresses are validated per network before saving.
- Audit events are append-only.

## 7. API surface

All routes require an authenticated session or Bearer token and are scoped to a
merchant membership. OpenAPI gets a `B2B Portal` tag in the same pass.

```text
GET    /api/b2b/profile
PUT    /api/b2b/profile
GET    /api/b2b/members
POST   /api/b2b/members/invites
PATCH  /api/b2b/members/:id
DELETE /api/b2b/members/:id
GET    /api/b2b/watch-addresses
POST   /api/b2b/watch-addresses
DELETE /api/b2b/watch-addresses/:id
GET    /api/b2b/balances        watch-only balances + fees + volume + freshness
GET    /api/b2b/fees            fee ledger with filters and CSV
GET    /api/b2b/audit           audit log
GET    /api/b2b/settings        env, limits, live enablement
PUT    /api/b2b/settings
```

Phase 1.1 endpoints (`/api/b2b/keys`) are linked from the settings page.

Rules:

- list endpoints are paginated and filterable by `env`, `status`, asset and date;
- mutations require owner/admin as documented per route;
- every mutation writes a `b2b_audit_events` row in the same transaction;
- reads never expose webhook secrets or `sk_` values after creation;
- `GET /api/b2b/balances` reports the timestamp and `stale` flag it used.

## 8. Authentication and security

- Reuse the existing email one-time-code flow and `users` table. No passwords.
- Portal sessions should use `HttpOnly`, `Secure`, `SameSite=Lax` cookies to
  reduce XSS token theft; the public API keeps `Authorization: Bearer`.
  This is an open decision because the frontend currently stores a Bearer token.
- CSRF protection is mandatory if cookies are used.
- Owner/admin checks happen in the service layer, not only in the UI.
- API keys: `sk_` secret shown once, stored as an Argon2id hash plus prefix;
  `pk_` identifier is not secret; scopes `quotes:write`, `orders:read`,
  `orders:write`, `refunds:write`; separate test/live.
- Live enablement requires manual KYB/sanctions approval and a signed agreement.
- Rate-limit registration, code verification and invites.
- Audit all member, key, address and webhook changes.
- No private keys or seed phrases on the server.

## 9. Anonymous users and migration

- Anonymous users, swaps and checkouts continue to work unchanged.
- Registration never silently takes ownership of anonymous orders. A later,
  explicit "claim by order id + checkout token/email" flow can be added; it is
  not part of Phase 2.
- A registered merchant's hosted checkout is still paid anonymously by the
  payer; the merchant identity stays private unless the merchant chooses to
  show it.
- Existing Phase 1 orders are backfilled to the creator's workspace; no orders
  are deleted or rewritten.

## 10. Delivery order

1. Phase 1.1 API keys (backend) if not already shipped.
2. `b2b_portal.sql`, membership/profile backfill, authorization middleware.
3. Registration, workspace creation, onboarding.
4. Overview, orders, order detail, fees.
5. Balance monitoring (watch-only) and analytics.
6. Webhooks, addresses, team, settings, audit.
7. API-key UI and live-enablement/KYB gate.
8. Figma implementation and responsive polish.

Each step updates this document and the OpenAPI document.

## 11. Acceptance criteria

- a new business can register, create a workspace and quote in test without
  support;
- the owner can invite a member and the role is enforced server-side;
- the overview and balances pages show the same numbers as the API and mark
  stale data;
- fees shown in the portal match `b2b_fee_ledger`;
- a webhook secret is shown once and a signed delivery succeeds;
- API keys can be created, rotated and revoked, and an old key stops working;
- every settings/member/key change appears in the audit log;
- live stays disabled until KYB approval;
- anonymous swap/checkout still works for users who never register;
- backend checks are green: `cargo fmt --check`, `cargo check`,
  `cargo test --all-features`; frontend checks when pages are touched:
  `pnpm run check`, `pnpm run test:unit`, `pnpm run build`.

## 12. Open questions

1. **Balance definition.** Watch-only on-chain addresses, fees earned, or a
   spendable prefunded balance? A spendable balance pulls Phase 3 (custody,
   safeguarding, reconciliation) forward and needs a compliance decision.
2. **Legal entity and KYB.** Who approves, what documents, which jurisdictions.
3. **Team model.** Are invites needed at launch, or owner-only first?
4. **Session transport.** Cookie sessions for the portal vs the current Bearer
   token in browser storage.
5. **Email delivery.** Which provider sends login codes and invites.
6. **Environments.** Should test and live share a workspace, or be separate
   workspaces with separate keys and webhooks?
7. **Designs.** Page inventory above must be mapped to Figma frames before UI
   implementation.

## 13. Figma designs

Pending. When the designs arrive, add links and a frame-to-route map here:

| Figma frame | Route | Status |
| --- | --- | --- |
| _pending_ | `/business` | not started |
| _pending_ | `/business/orders` | not started |
| _pending_ | `/business/balances` | not started |
| _pending_ | `/business/settings/api-keys` | not started |

The designs should cover: desktop and mobile, empty/loading/error states,
test/live banners, permission-denied states and the onboarding checklist.
