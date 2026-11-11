# B2B merchant portal and registration

Status: plan · 2026-11-11 · extends [`b2b-payment-api.md`](b2b-payment-api.md) ·
UI designs: registration page received (Figma); other portal pages pending

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

- registration and sign-in for business users: one `/register` page,
  passwordless email code, GitHub OAuth2 and Telegram OIDC, reusing the existing
  `users` identity;
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
- portal UI beyond the registration page until the remaining Figma layouts
  arrive.

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
| OAuth identity | `auth_oauth_accounts` | GitHub or Telegram subject linked to one `users` row. |
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

## 4. Registration and sign-in

### 4.1 Design source

Figma, received 2026-11-11:
[Collaborative Drawing Workspace — registration](https://www.figma.com/make/MccHliZg3K4s9m37RHa69V/Collaborative-Drawing-Workspace?t=6OjvSj4O4ZxU7aXI-1).

Only the registration page has a design so far. The portal pages in §5 still
need frames.

The page is dark: Pay3Flow logo top-left, the headline “Один аккаунт. Все
маршруты.” with a lime underline, two overlapping route cards (RUB→USDT and
USDT→AMD), and a right-hand auth card. The card contains the eyebrow
“БЕСПЛАТНАЯ РЕГИСТРАЦИЯ”, heading “Создайте аккаунт”, the subtitle “Начните
искать лучшие маршруты уже сегодня”, an email field, an agreement checkbox, a
lime “Создать аккаунт →” button, the divider “или продолжить через”, and GitHub
and Telegram buttons.

The owner requested three changes before implementation:

- remove every glow — flat background, no green halos, no ambient shadows;
- remove hover effects — no hover styles, transitions or animations;
- remove the bottom “Уже есть аккаунт? Войти” prompt and login button. One page
  registers new emails and signs in existing ones.

### 4.2 Page and visual contract

Route: `/register`. It is the only public auth page; there is no separate
`/login` page. Legacy `/login` links, if any, redirect to `/register`.

Copy and controls:

| Area | Content |
| --- | --- |
| Header | Pay3Flow mark and wordmark, links to `/` |
| Headline | “Один аккаунт.” / “Все маршруты.” with a lime underline on the second line |
| Route cards | “БЫСТРЫЙ МАРШРУТ”, “100 000 ₽”, RUB → USDT; “1148.20 USDT”, USDT → AMD |
| Card eyebrow | “БЕСПЛАТНАЯ РЕГИСТРАЦИЯ” |
| Card title | “Создайте аккаунт” |
| Card subtitle | “Начните искать лучшие маршруты уже сегодня” |
| Email | label “Email”, placeholder `name@example.com`, envelope icon |
| Consent | “Я принимаю Условия использования и Политику конфиденциальности” with links |
| Primary action | “Создать аккаунт →” |
| Divider | “или продолжить через” |
| Social actions | GitHub and Telegram |
| Code step | “Введите код”, “Продолжить →”, “Изменить email” |
| Success state | “Вы в аккаунте”, “Перейти к обмену →” |

The bottom login prompt and the separate login button are explicitly **not**
part of the page.

Visual rules:

- background `#090a09`, card `#111211`, border `#252725`, lime `#b5f500`,
  text `#f6f6f2`, muted text `#8f918b`;
- Manrope with tight tracking for the headline; DM Mono only for the code
  field;
- no `box-shadow`, `text-shadow`, `filter: drop-shadow`, radial gradient or
  any other glow;
- no `:hover` selector, no `transition`, no animation on this page. Only
  `:focus-visible` may draw a visible outline;
- desktop grid: headline/routes on the left, auth card on the right; the card
  is about 500 px wide and the content container is capped near 1280 px;
- at ≤1100 px the columns stack, the card is centred and capped at 560 px;
- at ≤680 px the route cards stack, the second card is inset instead of
  overlapping, and the card padding shrinks;
- the page is usable with keyboard only, has visible labels, `role="alert"`
  errors, `autocomplete="email"` and `autocomplete="one-time-code"`, and
  respects reduced motion.

### 4.3 One-page register-or-login flow

```text
email + consent
      │
      ▼
POST /api/auth/code          generic 202, no account-existence leak
      │
      ▼
code
      │
      ▼
POST /api/auth/authenticate  login if the user exists, register if not
      │
      ├─ existing user ─► session, continue to `next` or `/business`
      └─ new user ──────► create user, session, business onboarding
```

Rules:

- the UI never asks “register or sign in?” and never reveals whether an email
  is already registered;
- `POST /api/auth/code` always returns the same status and body whether the
  email exists or not, subject to rate limiting;
- `POST /api/auth/authenticate` verifies the code, resolves the user, and
  returns the same session shape for both paths;
- a valid `?next=` path is preserved through the code step; it must be a
  same-origin path, never an absolute external URL;
- a referral code from `?ref=` is validated and applied only when a new user is
  created. It is never applied to an existing account;
- an already-authenticated visitor on `/register` is redirected to `/business`;
- the current frontend helper that tries `/api/auth/login` and then
  `/api/auth/register` is replaced by `/api/auth/authenticate`. The old
  endpoints remain as deprecated aliases for one release, then are removed.

### 4.4 Email code delivery

The current backend accepts only the hardcoded code `1234` and the mail sender
is a log stub. Phase 2 replaces both.

- new `auth_codes` table or Redis key: email, code hash, expiry, attempts,
  sent time;
- six-digit numeric code, 10-minute lifetime, single use;
- store an Argon2id hash, never the plaintext code;
- maximum five verification attempts, then invalidate the code;
- rate limits: one send per email per minute, five per hour per email, plus an
  IP-based limit;
- resend is allowed after the cooldown and invalidates the previous code;
- email provider adapter (Resend, Postmark, SES or SMTP) configured by env;
- development may use a fixed code only when an explicit
  `AUTH_DEV_CODE`/`AUTH_DEV_MODE` flag is set; production startup fails if the
  flag is enabled;
- successful verification sets `users.email_verified_at`;
- `mail.rs` no longer contains `code == "1234"`;
- OpenAPI and `docs/api-overview.md` document the new flow and remove the
  demo-code wording.

### 4.5 OAuth2 social sign-in

GitHub and Telegram must complete on the backend; the buttons must not call a
provider from the browser or use the current stub.

Providers:

- **GitHub** — OAuth 2.0 authorization code flow, scopes `read:user` and
  `user:email`, PKCE;
- **Telegram** — OpenID Connect authorization code flow with PKCE, discovery
  document `https://oauth.telegram.org/.well-known/openid-configuration`;
  validate the ID token signature via JWKS, plus issuer, audience, nonce and
  expiry. The legacy Login Widget is not used for new integrations.

Endpoints:

```text
GET  /api/auth/oauth/{provider}/start     302 to the provider, sets state cookie
GET  /api/auth/oauth/{provider}/callback  validates state, exchanges the code
POST /api/auth/oauth/{provider}/link      links a provider to the current user
```

Backend rules:

- add the `oauth2` crate and a provider adapter per provider; do not hand-roll
  token exchange;
- `state` and the PKCE verifier live in a short-lived, `HttpOnly`, `Secure`,
  `SameSite=Lax` cookie, are single-use and expire after 10 minutes;
- reject missing, expired or mismatched state; never accept a callback without
  the cookie;
- GitHub email comes from `/user/emails`, choosing the primary verified
  address. If no verified email is available, ask the user to add and verify
  one with the email-code flow;
- match an existing account by verified email only. If the existing user has
  no `email_verified_at`, do not auto-link: require an email-code sign-in first
  and then link from settings;
- otherwise create a new `users` row with `email_verified_at` set from the
  provider claim, then send the user to business onboarding;
- one provider identity per `(provider, provider_user_id)`, and at most one
  identity per `(user_id, provider)`; a user may link both GitHub and Telegram;
- do not store provider access or refresh tokens unless a feature needs them.
  If that changes, encrypt them with an application key from the environment;
- after success redirect to `/register?oauth=ok&next=…` or `/business`; never
  redirect to an unvalidated external URL;
- return a generic error to the page for state, provider, email and linking
  failures; never expose provider error bodies;
- the current `POST /api/auth/oauth/{provider}` stub that returns
  `501 Not Implemented` is replaced by the routes above.

Provider configuration:

```text
GITHUB_CLIENT_ID / GITHUB_CLIENT_SECRET / GITHUB_REDIRECT_URI
TELEGRAM_CLIENT_ID / TELEGRAM_CLIENT_SECRET / TELEGRAM_REDIRECT_URI
OAUTH_STATE_SECRET
```

The GitHub and Telegram buttons stay in the design. They can be hidden behind
a config flag only if a provider is not configured; shipping a button that
pretends to work is not acceptable.

### 4.6 Business profile onboarding

After the first successful sign-in, the user creates the business profile:
legal name, display name, country, website, support email and contact person.
The workspace is created and the user becomes its `owner`. The default
environment is `test`.

Optional onboarding steps: add a payout/refund address, create a webhook,
create the first test quote. Live stays disabled until manual KYB/sanctions
approval; test quotes and checkout pages work immediately.

The overview shows a checklist until the first live transfer:

- business profile completed;
- payout/refund address saved;
- webhook endpoint verified (a signed `ping` delivery succeeds);
- first test quote created;
- first test checkout paid;
- KYB submitted.

### 4.7 Account states

```text
draft ─► active ─► suspended
             │
             └─► live_enabled (only after manual KYB)
```

Phase 1 `b2b_merchants.status` already has `draft|active|suspended`; Phase 2 adds
`live_enabled_at` and the profile/KYB fields rather than a second status column.

## 5. Portal pages

The registration frame is available; the remaining portal layouts are pending.
Routes and data requirements below are the contract the designs should cover.

| Route | Purpose | Key data / actions |
| --- | --- | --- |
| `/register` | Public registration and sign-in | email code, GitHub/Telegram OAuth2, one-page register-or-login, no separate login prompt |
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

### 6.1 Auth and OAuth2 tables

A separate migration `backend/migrations/auth_oauth.sql` runs before the portal
migration. It replaces the demo code with real one-time codes, stores provider
identities and optional server-side sessions.

```sql
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ;

CREATE TABLE auth_codes (
    email TEXT PRIMARY KEY,
    code_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts INT NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    sent_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE auth_oauth_accounts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('github','telegram')),
    provider_user_id TEXT NOT NULL,
    email TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT false,
    display_name TEXT,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider, provider_user_id),
    UNIQUE (user_id, provider)
);

CREATE TABLE auth_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    csrf_hash TEXT NOT NULL,
    user_agent TEXT,
    ip_hash TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX auth_sessions_user_idx ON auth_sessions (user_id);
CREATE INDEX auth_sessions_expiry_idx ON auth_sessions (expires_at);
```

Notes:

- `auth_codes` may instead live in Redis with a TTL; the SQL shape is the
  durable fallback and the source of truth for tests.
- provider access and refresh tokens are intentionally not stored.
- `auth_sessions` is used only if the portal moves to cookie sessions; the
  JWT/Bearer path can stay during the transition.
- existing users get `email_verified_at` on their next successful email-code
  sign-in, not by a blanket backfill.

Portal notes:

- `b2b_api_keys` is defined by Phase 1.1 and surfaced by the portal.
- `b2b_members` is the authorization source; `b2b_merchants.user_id` remains the
  creator and is not used for permission checks after the migration backfill.
- Migration backfills one `owner` membership per existing merchant and one
  profile row using the merchant name.
- Delivered/payout addresses are validated per network before saving.
- Audit events are append-only.

## 7. API surface

Public auth routes:

```text
POST /api/auth/code                       send a one-time email code
POST /api/auth/authenticate               verify code, login or register
GET  /api/auth/oauth/{provider}/start     begin GitHub/Telegram OAuth2
GET  /api/auth/oauth/{provider}/callback  finish OAuth2 and issue a session
POST /api/auth/oauth/{provider}/link      link a provider to the current user
POST /api/auth/logout                     revoke the current session
```

`POST /api/auth/register` and `POST /api/auth/login` stay as deprecated aliases
for one release, then are removed. The old `POST /api/auth/oauth/{provider}`
stub is removed.

Portal routes require an authenticated session or Bearer token and are scoped
to a merchant membership. OpenAPI gets a `B2B Portal` tag in the same pass.

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

- Email one-time codes are the primary passwordless method; GitHub and
  Telegram use OAuth2/OIDC on the backend (§4.5). No passwords.
- The portal should use `HttpOnly`, `Secure`, `SameSite=Lax` session cookies
  (`__Host-pay3flow_session`) with a random 256-bit token stored as a hash.
  The public API keeps `Authorization: Bearer` until merchant API keys land.
  This is still an open decision because the current frontend stores a Bearer
  token.
- CSRF protection is mandatory for cookie-authenticated mutations.
- Owner/admin checks happen in the service layer, not only in the UI.
- OAuth2 state and PKCE values are single-use, cookie-bound and expire after
  10 minutes; validate the Telegram ID token through JWKS, issuer, audience,
  nonce and expiry.
- Never auto-link an OAuth identity by an unverified email. Linking requires a
  verified provider email and, for existing accounts, an already verified
  Pay3Flow email.
- API keys: `sk_` secret shown once, stored as an Argon2id hash plus prefix;
  `pk_` identifier is not secret; scopes `quotes:write`, `orders:read`,
  `orders:write`, `refunds:write`; separate test/live.
- Live enablement requires manual KYB/sanctions approval and a signed agreement.
- Rate-limit code sends, code verification, OAuth starts/callbacks and invites.
- Audit all login, OAuth link, member, key, address and webhook changes.
- No private keys or seed phrases on the server.
- Production startup must fail if the demo auth code or a development OAuth
  shortcut is enabled.

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
2. `auth_oauth.sql`: email codes, OAuth identities, sessions, email-verified
   column; replace the `1234` stub and add a real mail adapter.
3. `POST /api/auth/code` and `POST /api/auth/authenticate`; keep the old
   register/login aliases for one release.
4. GitHub OAuth2 and Telegram OIDC adapters with state/PKCE and account linking.
5. Registration page `/register` from the Figma design, including the
   no-glow/no-hover/one-page rules.
6. `b2b_portal.sql`, membership/profile backfill, authorization middleware.
7. Business onboarding and workspace creation.
8. Overview, orders, order detail, fees.
9. Balance monitoring (watch-only) and analytics.
10. Webhooks, addresses, team, settings, audit.
11. API-key UI and live-enablement/KYB gate.
12. Remaining Figma pages and responsive polish.

Each step updates this document and the OpenAPI document.

## 11. Acceptance criteria

- a new business can register, create a workspace and quote in test without
  support;
- the `/register` page has no glow, no hover effect and no separate login
  prompt; an existing email signs in and a new email registers through the same
  form;
- GitHub OAuth2 and Telegram OIDC complete a full round trip, reject bad state
  and link to an existing account only through a verified email;
- production auth rejects the demo code; a real code is emailed, expires,
  is single-use and is rate-limited;
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
7. **OAuth2 credentials.** Who owns the GitHub OAuth app and the Telegram bot /
   Web Login client, and which redirect URIs are allowed in production?
8. **Account linking.** Confirm that an OAuth identity may link to an existing
   account only when both emails are verified.
9. **Remaining designs.** The registration frame is received; the rest of the
   portal page inventory still needs Figma frames.

## 13. Figma designs

| Figma frame | Route | Status |
| --- | --- | --- |
| [Registration](https://www.figma.com/make/MccHliZg3K4s9m37RHa69V/Collaborative-Drawing-Workspace?t=6OjvSj4O4ZxU7aXI-1) | `/register` | design received 2026-11-11; implementation deferred until Phase 2 is approved |
| _pending_ | `/business` | waiting for design |
| _pending_ | `/business/orders` | waiting for design |
| _pending_ | `/business/balances` | waiting for design |
| _pending_ | `/business/settings/api-keys` | waiting for design |

The registration design is dark and includes the route cards and the social
buttons. It must be implemented with the modifications in §4.1: no glow, no
hover effects and no separate login prompt.

The remaining designs should cover: desktop and mobile, empty/loading/error
states, test/live banners, permission-denied states and the onboarding
checklist.
