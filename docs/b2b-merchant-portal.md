# B2B merchant portal and registration

The business API and workspace are not enabled by the Georgian-support release.
The API/worker launch gate defaults off; [B2B launch blockers](b2b-launch-blockers.md)
remain part of its separate acceptance work.

Status: plan · 2026-11-11 · extends [`b2b-payment-api.md`](b2b-payment-api.md) ·
UI designs: registration page received (Figma); “Для бизнеса” and other portal
pages pending

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
- a private `/business` area: no page under it opens without sign-in, and a
  signed-out visitor is sent through `/register?next=…`;
- onboarding checklist for a first quote/checkout;
- “Для бизнеса” workspace home with all-time cashbox, metrics and graphs;
- projects — the folder that owns API keys — with per-project traffic graphs
  (Activity) and a period cashbox;
- orders, order detail, balances, fees, webhooks, analytics, settings and team;
- read-only balance monitoring: watch-only on-chain addresses, fee ledger,
  volume and settlement status;
- test/live environments per merchant;
- audit log for account and settings mutations;
- API-key lifecycle inside a project: name, environment, daily request limit,
  product scopes, rotation and revocation (backend keys may land earlier as
  Phase 1.1);
- API keys on every business API route — exchange, quotes, transfers, payments,
  OTC, P2P, solver — with a short explicit allowlist for infrastructure and
  payer-facing endpoints (§8.1);
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
| Project | `b2b_projects` | Folder that groups API keys; the reporting unit for traffic and revenue. |
| API key | `b2b_api_keys` | Belongs to exactly one project; carries environment, scopes and the daily request limit. |
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
- no conventional centred, fixed-width container: the page stretches to the
  full viewport width while keeping the horizontal padding. The logo row,
  headline, route cards and auth card all use the available width;
- desktop layout stays two-column — headline/routes on the left, auth card on
  the right — but both columns grow horizontally instead of being capped near
  1280 px. The auth card keeps a usable minimum width, not a fixed maximum;
- only long text blocks may keep a readable measure; the layout shell, cards and
  tables do not;
- at ≤1100 px the columns stack and the card fills the content width minus
  padding instead of being capped at 560 px;
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
create the first test project with a test API key, create the first test quote.
Live stays disabled until manual KYB/sanctions approval; test quotes and
checkout pages work immediately.

The overview shows a checklist until the first live transfer:

- business profile completed;
- payout/refund address saved;
- webhook endpoint verified (a signed `ping` delivery succeeds);
- first test project and API key created;
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

All Phase 2 pages use the full-width shell: the content spans the viewport with
the shared horizontal padding, not a centred fixed-width grid. Cards, tables and
charts expand horizontally; only long prose keeps a readable measure. This
applies to the registration page and to the portal pages below.

| Route | Purpose | Key data / actions |
| --- | --- | --- |
| `/register` | Public registration and sign-in | email code, GitHub/Telegram OAuth2, one-page register-or-login, no separate login prompt |
| `/business` | “Для бизнеса” workspace home; sign-in required | all-time cashbox, metrics and graphs, project switcher, recent orders, environment switch |
| `/business/onboarding` | Guided setup | profile, address, webhook, first project and key, first quote, KYB |
| `/business/activity` | Activity | requests, errors, latency, volume, fees and limit usage per project and period |
| `/business/projects` | Projects | list and create projects; a project is the folder for API keys |
| `/business/projects/:id` | Project | tabs: overview, keys, Activity, cashbox for a period |
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
| `/business/settings/security` | Security | sessions, audit log, live enablement state |

Every page must define loading, empty, error and permission-denied states. The
test environment gets a persistent banner; live gets a red/orange marker.

Keys have no page of their own: they are created and managed inside a project.
`/business/settings/api-keys` redirects to `/business/projects` so old links
keep working.

Every route in this table except `/register` is private: an unauthenticated
request redirects to `/register?next=<path>` and returns there after sign-in,
while the matching `/api/b2b/*` request answers `401` (§5.1).

### 5.1 “Для бизнеса” workspace home

Route: `/business`; navigation label: “Для бизнеса”. If a shorter public alias
is useful, `/for-business` redirects to `/business`. The route is **private**:
there is no guest version of the workspace.

**Access guard.** An unauthenticated visit, or a request with an expired or
invalid session, to `/business` and every page below it redirects to
`/register?next=<same-origin path>`; after a successful code or OAuth sign-in
the merchant lands on the page that was asked for, not on a generic home. The
guard runs server-side for the HTML response and again on every data request,
so hiding the UI is not the protection: portal APIs answer `401` without a
session (§7).

- the guard covers `/business/*` only; `/register`, the public site and hosted
  checkout pages stay anonymous;
- “Для бизнеса” stays in the header navigation for everyone; clicking it while
  signed out starts the sign-in flow instead of opening a guest landing page;
- marketing copy about the product lives on the public site, not behind the
  guard; `/business` is a working area, not a landing page;
- a signed-in user who opens `/register` without `?next=` is sent to
  `/business` (§4.3).

Design reference, received 2026-11-11:
[approximate dashboard layout](https://jmgbb.com/i/IaHeZ4f4Vv). It is a dark,
OpenRouter-like account dashboard: fixed left navigation, top bar with timezone,
filter and period, tabs, a row of KPI cards, two summary cards and a wide chart.
Use it for structure only; do not copy OpenRouter branding, copy or token/model
metrics.

The reference is read as a **portal with a sidebar on the left** and the working
area on the right. That left-column pattern already exists on the site: the
route instructions use a 246 px column with a right border, items connected by
a thin vertical line, the active item on the accent tint, and the content
column beside it (`src/lib/components/RouteInstructions.svelte`); below 760 px
the items turn into a horizontal chip row. The workspace mirrors that pattern
with section labels instead of step numbers and the same accent treatment, so
the portal reads as part of Pay3Flow rather than a separate product.

#### 5.1.1 Layout and navigation

- fixed left sidebar, roughly 240–260 px, full viewport height, using the same
  rail pattern as the route instructions (246 px there);
- workspace name at the top of the sidebar, then the navigation;
- main area takes the remaining width with the shared page padding; no centred
  fixed-width container;
- top bar: page title on the left; environment badge (`test`/`live`), timezone,
  period selector and “Создать проект” on the right (“Создать ключ” lives inside
  a project);
- tabs under the title: “Обзор”, “Тренды”, “Заказы”, “Лимиты” (limits may start
  as a disabled/`soon` tab);
- KPI row of five compact cards;
- two summary cards below the KPIs, then a wide chart and a recent-orders table;
- at ≤900 px the sidebar becomes a drawer and the KPI cards wrap.

Sidebar mapping:

| Reference item | Pay3Flow page |
| --- | --- |
| Activity | `/business/activity` — traffic, volume, fees and limit graphs per project |
| Logs | `/business/orders` — orders and events |
| Projects | `/business/projects` — projects, their keys and cashbox |
| Keys | `/business/projects/:id` — keys are created and managed inside a project |
| Files | `/business/checkout-links` |
| Guardrails | `/business/limits` (later, read-only limits first) |
| Routing | `/business/analytics` — corridors and asset mix |
| Observability | `/business/webhooks` and analytics |
| Settings | `/business/settings/profile`, team, addresses, security |

The sidebar also links to API docs and support at the bottom. “Files”, “BYOK”,
“Presets”, “Tools” and “Classifiers” from the reference are not copied unless a
Pay3Flow feature actually needs them.

Pay3Flow sidebar order: Обзор, Activity, Проекты, Заказы, Балансы, Аналитика,
Вебхуки, Настройки. Keys have no sidebar item of their own.

KPI cards:

1. **Касса** — received all time (primary number, per asset selector);
2. **Оборот** — cumulative paid volume with USD estimate;
3. **Заказы** — paid/delivered count, failed separately;
4. **Комиссия Pay3Flow** — cumulative fee ledger;
5. **Конверсия / средний чек** — choose one, or make the fifth card a period
   metric with a sparkline.

Each card shows the number, the comparison period (“к прошлому периоду”) and a
small sparkline or “—”, exactly like the reference. For “all time” there is no
prior period, so show “—”, not a fake percentage.

#### 5.1.2 “Касса” — all-time totals

The owner wants a single place that shows what has accumulated over the whole
life of the merchant account. Phase 2 defines it as a **read-only cumulative
report**, not a spendable wallet:

| Metric | Definition |
| --- | --- |
| Получено за всё время | cumulative target amount delivered to the merchant's payout addresses, per asset |
| Оборот | cumulative paid order value, per source asset, plus a clearly labelled USD estimate |
| Комиссия Pay3Flow | cumulative platform margin/fees recorded in `b2b_fee_ledger` |
| Заказы | paid and delivered order counts; failed and cancelled excluded and shown separately |
| Последний расчёт | timestamp of the latest settlement, plus a freshness/stale marker |

Rules:

- “за всё время” means from the merchant's first paid order, never a rolling
  window;
- show each asset separately; do not add BTC, USDT and RUB into one number
  without an explicit valuation method;
- state the timezone and the data timestamp in the UI;
- if a spendable balance or withdrawal is required, that is Phase 3 custody;
  the Phase 2 page must not imply it;
- empty state is zero values with “Пока нет оплаченных заказов”, not fake
  placeholders.

#### 5.1.3 Metrics and graphs

- volume, fees, order count, conversion and settlement duration over time;
- failure reasons and pending/failed order counts;
- ranges: 24 hours, 7 days, 30 days, 90 days, all time;
- filters: environment, asset, corridor, order kind;
- one main chart (volume) and compact secondary charts; no decorative charts
  without data;
- pre-aggregate per day/asset in a materialized view or summary table so “all
  time” and 90-day views do not scan the full order table on every request;
- CSV export for the current filter;
- respect permissions: members and viewers see metrics but not API secrets.

#### 5.1.4 API keys live in projects

A compact block on the workspace home, and the full management surface on the
project page (§5.3). A key cannot exist without a project:

- a project is the folder for keys, and creating a key starts by choosing or
  creating its project;
- the key form sets name, environment (`test`/`live`), daily request limit and
  the product scopes (курсы, свап, OTC, платёжная система);
- show `sk_` once at creation or rotation, with copy and “I saved it”
  confirmation;
- the list shows prefix, environment, scopes, daily limit, usage today, last
  used and status;
- rotate and revoke; a revoked key stops working immediately;
- owners and admins only; members/viewers see metadata without secrets;
- keys are never rendered in SSR, written to logs or shipped to the browser;
  the website's own key stays in the SvelteKit server environment and appears
  in Activity as the dedicated “Pay3Flow website” project;
- backend contract is Phase 1.1 (`b2b_api_keys`), now carrying `project_id`,
  the daily limit and usage counters.

The home block shows the two or three most recent keys with their project and a
“Создать проект” button; the key form itself stays inside a project.

#### 5.1.5 Recent orders and quick actions

- the latest 10 orders with status, amount, asset and created time;
- quick links to orders, balances, checkout links and webhooks;
- onboarding checklist until the first live transfer (profile, payout/refund
  address, verified webhook, first test quote, first test checkout, KYB);
- nothing renders before the access guard resolves: show a skeleton, never a
  flash of merchant widgets or “0” totals.

#### 5.1.6 Page rules

- full-width shell with side padding; no centred fixed-width grid;
- same visual language as `/register` unless a separate design says otherwise:
  dark surface, lime accent, **no glows, no hover effects, no decorative
  animation**;
- no fake metrics, fake logos or invented partner names;
- one `<h1>`, semantic sections, keyboard navigation, reduced-motion support;
- add “Для бизнеса” to the header navigation (mobile menu included) only when
  the page is ready; do not ship a dead link;
- the page is private: `noindex, nofollow`, no Open Graph card with merchant
  data, and responses are never cached publicly.

The layout and final copy need a Figma frame. Until it arrives, this section is
the content and data contract.

### 5.2 “Activity” — traffic and revenue graphs

Route: `/business/activity`; sidebar label “Activity”. One page for the whole
workspace with a project selector: “Все проекты” or a single project. The same
charts repeat inside the project page as its “Activity” tab (§5.3).

Filters: project, environment (`test`/`live`), asset, period (24 hours, 7 days,
30 days, 90 days, all time, custom range). The timezone is the one shown in the
top bar and is stated next to the charts.

Charts:

1. **Запросы к API** — requests per hour or day, stacked by status class
   (2xx/4xx/5xx);
2. **Ошибки** — the failure share, plus a table of the most frequent 4xx/5xx
   codes;
3. **Латентность** — p50 and p95 of API responses in the same buckets;
4. **Оборот** — quoted and paid volume per asset, with a clearly labelled USD
   estimate;
5. **Касса проекта** — fees earned per day and asset, so the selected period
   shows what the project brings in;
6. **Заказы** — created, paid and failed counts;
7. **Лимиты** — today's request count against the daily limit for every active
   key (meters, not a chart).

Rules:

- a request is an authenticated call made with a key of a project; anonymous
  public traffic is not attributable and is not shown;
- the graphs read `b2b_api_key_usage` (hourly buckets per key and status
  class), not raw request logs, so 90-day and all-time ranges stay cheap; the
  page prints the aggregation timestamp and marks today as partial;
- the rollup runs hourly; if it has not run, show the freshness/stale marker
  instead of silently old numbers;
- endpoint labels are normalised (`/api/b2b/quote`, not the raw path with ids);
  request bodies, headers and IP addresses are never shown;
- revoked keys keep their history and stop contributing new points;
- CSV export for the current filters;
- empty state: “Пока нет запросов” with a link to create a key;
- members and viewers see the graphs; key secrets and key mutations stay on the
  project page and are owner/admin only.

### 5.3 “Projects” — the folder for API keys

A **project** is the merchant's folder for keys: one product or token, its
keys, its traffic and its revenue. Every API key belongs to exactly one project,
so creating a key always starts with choosing or creating a project. Projects
scope the graphs in §5.2 and the cashbox below.

Route `/business/projects` lists the projects:

- columns: name, description, keys (active/total), requests today across its
  keys, period cashbox (fees and volume, 30 days by default), created date;
- “Создать проект” asks for a name (2–64 characters, unique inside the
  workspace) and an optional description; nothing else is created at this step;
- empty state explains the order project → key → integration, with “Создать
  проект” as the only button;
- search and pagination once a workspace has more than 20 projects.

Route `/business/projects/:id` has four tabs:

- **Обзор** — active keys, requests today against their limits, current-period
  cashbox, latest orders and a link to the full Activity page;
- **Ключи** — the key list and the create form below;
- **Activity** — the charts from §5.2 scoped to this project;
- **Касса** — revenue for a chosen period, described below.

#### 5.3.1 Key fields

Creating or editing a key sets:

| Field | Rule |
| --- | --- |
| Название ключа | required, unique inside the project, 2–64 characters, shown in Activity and the audit log |
| Environment | `test` or `live`; `live` requires live enablement/KYB, otherwise the option is disabled with a reason |
| Лимит запросов в день | required whole number; `0` means “без лимита” and must be picked explicitly, not left blank |
| Функции ключа | at least one scope from the table below |

Scopes map to the products a merchant uses:

| UI label | Scope | Allows |
| --- | --- | --- |
| Курсы и котировки | `rates:read` | corridor and asset prices, fee schedule |
| Свап | `quotes:write`, `orders:write` | swap quotes and transfer orders |
| OTC | `otc:write` | OTC quote requests and OTC orders |
| Платёжная система | `orders:read` | orders, statuses, balances, webhook delivery log |
| Платёжная система | `refunds:write` | refunds and checkout-link management |

The catalogue extends the Phase 1.1 scope list in
[`b2b-payment-api.md`](b2b-payment-api.md) with `rates:read` and `otc:write`.
A scope is offered only when the matching product has a merchant API; the UI
must not promise a function the backend refuses.

A key is shown as `sk_…` exactly once, at creation or rotation, with a copy
button and an “I saved it” confirmation. The list shows only the prefix, name,
environment, scopes, daily limit, usage today, last used and status. `pk_` is
not secret. Secrets never reach SSR output, logs, URLs or analytics.

Actions: edit name, limit and scopes; rotate (new secret shown once, the old
one stops working immediately); revoke (immediate, irreversible). Each action
writes a `b2b_audit_events` row. Owners and admins only; members and viewers
see metadata without secrets.

#### 5.3.2 Daily request limit

- the limit is per key, not per project: two keys of the same project each get
  their own counter;
- a request counts when it is authenticated with the key, whatever the response
  status; `OPTIONS` preflights and unauthenticated calls do not count;
- the day is a UTC calendar day in Phase 2 (the reset timezone is an open
  question, §12);
- the counter is an atomic upsert into `b2b_api_key_usage`, so it survives
  restarts and works across several backend instances;
- over the limit the API answers `429` with `Retry-After`, `X-RateLimit-Limit`,
  `X-RateLimit-Remaining` and `X-RateLimit-Reset`; the UI shows the same numbers
  in the limit meters;
- lowering the limit below today's usage blocks further calls until the next
  reset; raising it applies immediately.

#### 5.3.3 “Касса” inside a project

The project cashbox answers “сколько принёс этот проект за период”:

- period selector: today, 7 days, 30 days (default), 90 days, all time, custom
  range; the same asset and environment filters as Activity;
- headline numbers: **комиссия Pay3Flow** earned from the project, **оборот**
  (paid volume), **заказы** (paid/failed), **средний чек** and the
  quote-to-paid conversion;
- a daily series of fees and volume, plus a breakdown by asset and by key, so a
  merchant sees which key brings the revenue;
- CSV export for the current filters;
- comparison with the previous period of equal length; for “all time” show “—”;
- test and live are never mixed: the test environment is labelled and excluded
  from live totals.

Attribution rule: revenue belongs to the project whose key created the order;
a hosted checkout session inherits the project of the call that created it. An
order created in the portal itself, with no key involved, has no project and is
shown as “Без проекта” in the workspace totals but never inside a project
cashbox. The workspace “Касса” in §5.1.2 stays all-time and covers every order.

### 5.4 "Balances" semantics (important)

This page is separate from the “Касса” block on `/business`:

- “Касса” is a cumulative all-time report (received, volume, fees, orders);
- `/business/balances` is **watch-only monitoring** of saved on-chain addresses.

Neither is a Pay3Flow wallet in Phase 2:

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

Phase 1.1 (projects, keys, usage, order/fee attribution) ships in
`backend/migrations/b2b_api_keys.sql`, applied after `b2b.sql`. Portal-only
tables (profiles, members, watch addresses, audit) ship later in
`backend/migrations/b2b_portal.sql`. Money stays as exact decimal strings; no
`f64`.

```sql
-- b2b_api_keys.sql (Phase 1.1 / §10 step 1)
CREATE TABLE b2b_projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES b2b_merchants(id),
    name TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','archived')),
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX b2b_projects_name_idx
    ON b2b_projects (merchant_id, lower(name));

CREATE TABLE b2b_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES b2b_projects(id),
    name TEXT NOT NULL,
    env TEXT NOT NULL CHECK (env IN ('test','live')),
    prefix TEXT NOT NULL UNIQUE,
    secret_hash TEXT NOT NULL,
    scopes TEXT[] NOT NULL CHECK (cardinality(scopes) > 0),
    daily_request_limit INT NOT NULL DEFAULT 0
        CHECK (daily_request_limit >= 0),
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active','revoked')),
    created_by UUID REFERENCES users(id),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX b2b_api_keys_name_idx
    ON b2b_api_keys (project_id, lower(name));

CREATE TABLE b2b_api_key_usage (
    key_id UUID NOT NULL REFERENCES b2b_api_keys(id) ON DELETE CASCADE,
    bucket_start TIMESTAMPTZ NOT NULL,
    requests BIGINT NOT NULL DEFAULT 0,
    errors_4xx BIGINT NOT NULL DEFAULT 0,
    errors_5xx BIGINT NOT NULL DEFAULT 0,
    latency_sum_ms BIGINT NOT NULL DEFAULT 0,
    latency_p50_ms INT,
    latency_p95_ms INT,
    PRIMARY KEY (key_id, bucket_start)
);

ALTER TABLE b2b_orders
    ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES b2b_projects(id);
ALTER TABLE b2b_orders
    ADD COLUMN IF NOT EXISTS api_key_id UUID REFERENCES b2b_api_keys(id);
ALTER TABLE b2b_fee_ledger
    ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES b2b_projects(id);

-- b2b_portal.sql (portal / §10 step 7)
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

The Phase 1.1 tables in the first half of the block are what
`b2b_api_keys.sql` applies today. Profiles, members, watch addresses and audit
stay as the target shape for step 7 and are not applied until `b2b_portal.sql`
lands.

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

- `b2b_api_keys` belongs to Phase 1.1 and ships in `b2b_api_keys.sql` with the
  portal columns (`project_id`, `name`, `daily_request_limit`, scopes) so the
  shape is fixed in one place (§5.3); keys are always grouped under
  `b2b_projects`.
- A key belongs to exactly one project; a project is archived, not deleted,
  once it has keys or orders.
- `b2b_orders.project_id` and `b2b_fee_ledger.project_id` are the attribution
  source for Activity and the per-project cashbox; rows created before the
  migration keep `NULL` and appear as “Без проекта”.
- `b2b_api_key_usage` is written by the API middleware: one row per key and
  hour, upserted atomically, with 4xx/5xx counts and latency percentiles for
  that hour.
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
Business API routes (`/api/v1/*`, `/api/exchange/*`, `/api/p2p/*`, OTC and the
rest of §8.1) authenticate with a project API key, not with the portal session.

```text
GET    /api/b2b/profile
PUT    /api/b2b/profile
GET    /api/b2b/projects
POST   /api/b2b/projects
GET    /api/b2b/projects/:id
PATCH  /api/b2b/projects/:id
GET    /api/b2b/projects/:id/keys        metadata only, never secrets
POST   /api/b2b/projects/:id/keys
PATCH  /api/b2b/keys/:id                 name, daily limit, scopes
POST   /api/b2b/keys/:id/rotate
POST   /api/b2b/keys/:id/revoke
GET    /api/b2b/projects/:id/activity    usage series and limit meters
GET    /api/b2b/projects/:id/summary     period cashbox and per-key revenue
GET    /api/b2b/activity                 the same series across projects
GET    /api/b2b/summary        all-time cashbox totals and freshness
GET    /api/b2b/metrics        time series for volume, fees, orders, conversion
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

A merchant-wide `/api/b2b/keys` path is not part of the contract even if Phase
1.1 ships first: a key is always created under `/api/b2b/projects/:id/keys`.

Rules:

- list endpoints are paginated and filterable by `env`, `status`, asset and date;
- mutations require owner/admin as documented per route;
- every mutation writes a `b2b_audit_events` row in the same transaction;
- reads never expose webhook secrets or `sk_` values after creation;
- `GET /api/b2b/balances` reports the timestamp and `stale` flag it used;
- key authentication checks `status`, `env` and the required scope before the
  handler runs, then counts the request against `daily_request_limit` and
  answers `429` with the `X-RateLimit-*` headers when it is exhausted;
- the activity and summary routes resolve `project_id` before aggregation, so a
  viewer without access to a project sees nothing for it.

## 8. Authentication and security

- Email one-time codes are the primary passwordless method; GitHub and
  Telegram use OAuth2/OIDC on the backend (§4.5). No passwords.
- The portal should use `HttpOnly`, `Secure`, `SameSite=Lax` session cookies
  (`__Host-pay3flow_session`) with a random 256-bit token stored as a hash.
  The business API switches from the user Bearer token to project keys (§8.1);
  that token survives only for the `/api/b2b/*` and `/api/v1/*` deprecation
  window. The portal's own session transport is still an open decision because
  the current frontend stores a Bearer token.
- CSRF protection is mandatory for cookie-authenticated mutations.
- Owner/admin checks happen in the service layer, not only in the UI.
- OAuth2 state and PKCE values are single-use, cookie-bound and expire after
  10 minutes; validate the Telegram ID token through JWKS, issuer, audience,
  nonce and expiry.
- Never auto-link an OAuth identity by an unverified email. Linking requires a
  verified provider email and, for existing accounts, an already verified
  Pay3Flow email.
- API keys: `sk_` secret shown once, stored as an Argon2id hash plus prefix;
  `pk_` identifier is not secret; scopes `rates:read`, `quotes:write`,
  `orders:read`, `orders:write`, `refunds:write`, `otc:write`; separate
  test/live. A key always belongs to a project and carries a daily request
  limit; the counter lives in `b2b_api_key_usage` and enforcement returns `429`
  with `Retry-After` and `X-RateLimit-*`.
- Scope checks are server-side: a valid key without the scope gets `403` with a
  machine-readable `insufficient_scope` code, and the portal must not imply a
  function the scope does not grant.
- Live enablement requires manual KYB/sanctions approval and a signed agreement.
- Rate-limit code sends, code verification, OAuth starts/callbacks and invites.
- Audit all login, OAuth link, member, key, address and webhook changes.
- No private keys or seed phrases on the server.
- Production startup must fail if the demo auth code or a development OAuth
  shortcut is enabled.

### 8.1 Key coverage — every business route

A merchant integrates through project API keys. Every business route requires a
key; only the allowlist below may stay keyless. Enforcement is a router-wide
layer around the business groups, so a route added later is protected by
default and an exception has to be a deliberate entry.

Protected groups:

| Scope | Routes |
| --- | --- |
| `rates:read` | `/api/market-prices`, `/api/market-values`, `/api/reference-rates`, `/api/exchange-pairs` |
| `quotes:write` | `/api/v1/quotes`, `/api/exchange/*`, `/api/p2p/*`, `/api/solver/*`, `/api/routing/fallback`, `/api/route-instructions/open` |
| `orders:read` | reads under `/api/v1/transfers`, `/api/b2b/orders`, `/api/b2b/analytics`, `/api/payments`, `/api/share-links/:id` |
| `orders:write` | `/api/v1/transfers` and cancel, `/api/v1/payments`, `POST /api/payments`, `/api/share-links`, checkout-link management |
| `refunds:write` | refund endpoints |
| `otc:write` | OTC HTTP routes and `/ws/otc` |

The table is the product grouping, not a per-handler list: the exact scope of
every route is declared in OpenAPI, and an action that writes orders needs the
write scope even when it lives under `/api/exchange/*`.

`/ws`, `/ws/rates`, `/ws/payments` and `/ws/p2p/*` authenticate with the same key
as their HTTP counterparts; the key travels in the first message, not in the
URL.

Keyless allowlist:

| Route | Why it stays keyless |
| --- | --- |
| `GET`/`POST /health`, `/health/ready` | infrastructure probes |
| `GET /metrics` | Prometheus scrape; protect it with the ingress/network policy, not a merchant key |
| `/openapi.json`, `/scalar` | API reference |
| `/api/auth/*`, `/api/auth/oauth/*` | sign-in and OAuth bootstrap; no key can exist yet |
| `/pay/:token`, `/pay/:token/status`, `/pay/:token/quote` | payer-facing checkout; the unguessable token is the capability |
| provider webhooks, `/inbox*`, `/.well-known/webfinger`, `/actor*`, `/marketplace/*` | authenticated by provider signatures or public federation formats |
| `/api/networks`, `/api/banks`, `/api/providers*`, `/api/reviews/*` | public catalogue reads used by the site and public pages |
| site pages and static assets | not API |

Rules:

- a request is authenticated with `Authorization: Bearer sk_…`; `pk_` alone is
  not enough. Unknown or revoked key → `401`; key from the other environment →
  `403`; missing scope → `403 insufficient_scope`; over the daily limit → `429`
  with the `X-RateLimit-*` headers (§5.3.2);
- each route declares its scope in OpenAPI, and the middleware reads that
  declaration instead of duplicating checks inside handlers;
- `/api/admin/*` and staff tooling never accept merchant keys; they keep their
  own authentication;
- a route-table test walks the router and fails if a route is neither in a
  protected group nor on the allowlist, so a new business endpoint cannot ship
  unprotected;
- the legacy user Bearer token is accepted only on `/api/b2b/*` and `/api/v1/*`
  during the deprecation window (§7), then removed;
- keyless calls are counted separately; Activity (§5.2) shows only keyed
  traffic, so a merchant sees their own usage, not the site's.

First-party traffic. The site is a SvelteKit `adapter-node` app, and today it
calls exchange and P2P endpoints from the browser (`src/lib/exchange.ts`). A
`sk_` must never reach the browser, so those calls move behind the SvelteKit
server — a `+server.ts` proxy or server `load` — with a first-party key held in
the server environment. That key lives in a dedicated “Pay3Flow website”
project, scoped to exactly what the site uses, so first-party traffic is
visible in Activity and separable from merchant traffic. A route that cannot
move server-side yet gets a dated temporary exception in the allowlist, not an
accidental keyless hole.

## 9. Anonymous users and migration

- Anonymous users, swaps and checkouts continue to work unchanged; after
  §8.1 the site's own swap calls go through the SvelteKit server with the
  first-party key, so the visitor still needs no account and no key.
- Registration never silently takes ownership of anonymous orders. A later,
  explicit "claim by order id + checkout token/email" flow can be added; it is
  not part of Phase 2.
- A registered merchant's hosted checkout is still paid anonymously by the
  payer; the merchant identity stays private unless the merchant chooses to
  show it.
- Existing Phase 1 orders are backfilled to the creator's workspace; no orders
  are deleted or rewritten.

## 10. Delivery order

1. Phase 1.1 API keys, project-scoped from the start: `b2b_projects`,
   `b2b_api_keys.project_id`, name, `daily_request_limit`, scopes,
   `b2b_api_key_usage`, and `project_id` on orders and the fee ledger.
2. Business API routes accept only project keys: router-wide middleware, the
   keyless allowlist, the SvelteKit server proxy with a first-party key and
   per-route security in OpenAPI (§8.1).
3. `auth_oauth.sql`: email codes, OAuth identities, sessions, email-verified
   column; replace the `1234` stub and add a real mail adapter.
4. `POST /api/auth/code` and `POST /api/auth/authenticate`; keep the old
   register/login aliases for one release.
5. GitHub OAuth2 and Telegram OIDC adapters with state/PKCE and account linking.
6. Registration page `/register` from the Figma design, including the
   no-glow/no-hover/one-page rules.
7. `b2b_portal.sql`, membership/profile backfill, authorization middleware.
8. Business onboarding and workspace creation, including the first test project
   and test key.
9. `/business/projects`: projects list, project tabs, key creation with name,
   environment, daily limit and scopes, rotation and revocation (§5.3).
10. `/business/activity`: usage rollup job, per-project charts and limit meters
    (§5.2).
11. “Для бизнеса” home: cashbox, metrics and graphs, project switcher, recent
    orders; then orders list, order detail and fees.
12. Balance monitoring (watch-only) and analytics.
13. Webhooks, addresses, team, settings, audit.
14. Per-project cashbox report with attribution and CSV, live-enablement/KYB
    gate, remaining Figma pages and responsive polish.

Each step updates this document and the OpenAPI document. The state of every
step as of 2026-11-11 is recorded in §14.

## 11. Acceptance criteria

- a new business can register, create a workspace and quote in test without
  support;
- a signed-out visitor to `/business/projects` is redirected to
  `/register?next=%2Fbusiness%2Fprojects`, returns there after sign-in, and the
  same page's API calls answer `401` without a session;
- the `/register` page has no glow, no hover effect and no separate login
  prompt; an existing email signs in and a new email registers through the same
  form;
- the “Для бизнеса” home shows the all-time cashbox, dated graphs and a project
  switcher; totals match the summary and fee-ledger APIs and mark stale data;
- GitHub OAuth2 and Telegram OIDC complete a full round trip, reject bad state
  and link to an existing account only through a verified email;
- production auth rejects the demo code; a real code is emailed, expires,
  is single-use and is rate-limited;
- the owner can invite a member and the role is enforced server-side;
- the overview and balances pages show the same numbers as the API and mark
  stale data;
- fees shown in the portal match `b2b_fee_ledger`;
- a webhook secret is shown once and a signed delivery succeeds;
- a key cannot be created without a project; the project page is the only
  place keys are created, rotated and revoked, and an old key stops working at
  once;
- a key with only `rates:read` gets `403 insufficient_scope` on swap, OTC and
  payments endpoints, and a key over its daily limit gets `429` with
  `X-RateLimit-*` until the next reset;
- Activity and the project cashbox match `b2b_api_key_usage`, `b2b_orders` and
  `b2b_fee_ledger` for the same filters, mark an order without a project as
  “Без проекта”, and never mix test with live revenue;
- every business route rejects a keyless call with `401`, an unknown or revoked
  key with `401`, a wrong-environment key with `403` and a missing scope with
  `403 insufficient_scope`, while health, metrics, docs, auth and the payer
  checkout still answer without a key;
- a new business route cannot ship unprotected: the route-table test fails
  unless the route is in a protected group or on the allowlist;
- the website's first-party key is used only server-side and never appears in a
  browser bundle or a response body;
- every settings/member/key change appears in the audit log;
- live stays disabled until KYB approval;
- anonymous swap/checkout still works for users who never register;
- backend checks are green: `cargo fmt --check`, `cargo check`,
  `cargo test --all-features`; frontend checks when pages are touched:
  `pnpm run check`, `pnpm run test:unit`, `pnpm run build`.

## 12. Open questions

1. **Balance and cashbox definition.** Phase 2 treats “Касса” as a cumulative
   read-only report: received to payout addresses, gross volume, fees and order
   counts. If it must instead be a spendable prefunded balance, that pulls
   Phase 3 (custody, safeguarding, reconciliation) forward and needs a
   compliance decision.
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
10. **Project vs token.** Is one project exactly one token, or can a project
    group several tokens and corridors under one key? This plan treats a project
    as a product folder, not a token registry.
11. **Attribution without a key.** Should an order created in the portal or
    through a hosted checkout with no key belong to a chosen project, to a
    default project, or stay “Без проекта”?
12. **Daily limit reset.** UTC midnight, workspace timezone or a rolling 24-hour
    window? And is the limit per key (as planned) or shared by every key of a
    project?
13. **Scope granularity.** Confirm `rates:read`, `quotes:write`, `orders:read`,
    `orders:write`, `refunds:write`, `otc:write`, and whether swap and OTC need
    their own scopes once their merchant APIs exist.
14. **Project lifecycle.** Archive or delete a project that has keys or orders,
    and does the workspace need a default project for portal-created orders?
15. **First-party traffic.** Move the site's exchange and P2P calls behind the
    SvelteKit server with a first-party key before enforcement, or keep a dated
    anonymous exception? Who owns rotation of that key and what exactly is it
    scoped to?
16. **Public catalogue reads.** Do `/api/market-prices`, `/api/networks` and
    providers stay anonymous for the site and public pages, or move behind the
    first-party key with the rest?

## 13. Figma designs

| Figma frame | Route | Status |
| --- | --- | --- |
| [Registration](https://www.figma.com/make/MccHliZg3K4s9m37RHa69V/Collaborative-Drawing-Workspace?t=6OjvSj4O4ZxU7aXI-1) | `/register` | design received 2026-11-11; uncommitted draft in the working tree, not wired to the new auth flow (§14 step 6) |
| _pending_ | `/business` | waiting for design |
| _pending_ | `/business/orders` | waiting for design |
| _pending_ | `/business/balances` | waiting for design |
| _pending_ | `/business/activity` | waiting for design |
| _pending_ | `/business/projects` | waiting for design |
| _pending_ | `/business/projects/:id` | waiting for design: overview, keys, Activity, cashbox tabs |

The registration design is dark and includes the route cards and the social
buttons. It must be implemented with the modifications in §4.1: no glow, no
hover effects and no separate login prompt, plus the full-width shell with side
padding described in §4.2 and §5.

The remaining designs should cover: desktop and mobile, empty/loading/error
states, test/live banners, permission-denied states and the onboarding
checklist.

## 14. Implementation status

Updated 2026-11-11. This section separates the plan from what the repository
already contains; recording a step here is not a substitute for the code.

| §10 step | State | Evidence |
| --- | --- | --- |
| 1. Phase 1.1 API keys, project-scoped | Schema done | `migrations/b2b_api_keys.sql` creates `b2b_projects`, `b2b_api_keys`, `b2b_api_key_usage` and adds `project_id` / `api_key_id` on orders and the fee ledger; `db::apply_schema` runs it after `b2b.sql`. Issue/rotate/revoke APIs and key middleware are still later steps. |
| 2. Business API key middleware | In progress | `b2b::keys` mints/looks up keys; `b2b::key_coverage` is a hand-maintained inventory of known routes, without router-completeness enforcement (allowlist, legacy `/api/v1`+`/api/b2b`, staff, dated TemporarySite exceptions for browser-called exchange/P2P until the first-party proxy). Router-wide enforcement, OpenAPI security and the SvelteKit proxy are still open. |
| 3. `auth_oauth.sql`, real codes and mail adapter | Not started | no `auth_oauth.sql`; `users` has no `email_verified_at`; `service/user/mail.rs` still returns `code == "1234"`; `register::send_auth_code` only logs. |
| 4. `/api/auth/code` and `/api/auth/authenticate` | Not started | `server/routing/api.rs` registers only `/api/auth/register`, `/api/auth/login` and `/api/auth/me`. |
| 5. GitHub OAuth2 and Telegram OIDC | Not started | `POST /api/auth/oauth/{provider}` returns `501`; `service/user/oauth.rs` always errors. |
| 6. `/register` page | Draft, uncommitted | `src/routes/register/+page.svelte` and `src/lib/auth-session.ts` are untracked; no route links to `/register` yet. |
| 7–14. Portal, projects/keys UI, Activity, orders, settings | Not started | there is no `src/routes/business/` directory in the frontend, and no route outside the allowlist checks a key. |

The projects, Activity and API-key-coverage steps were added to §10 on
2026-11-11; the numbers in this table follow the current §10.

### 14.1 What the `/register` draft already does

- follows §4.2 visually: dark palette, lime underline, two route cards, no
  glow, no `:hover` rules, `:focus-visible` only;
- walks `email + consent → code → success` in one card and has no separate
  login prompt; `role="alert"` errors, `autocomplete="email"` and
  `autocomplete="one-time-code"` are present.

### 14.2 Where the draft differs from this plan

The draft predates the auth contract and must be reconciled before step 6 is
considered done:

- the code field accepts **four** digits (`maxlength="4"`, `/^\d{4}$/`);
  §4.4 specifies six digits;
- it calls `authenticate()` from `$lib/exchange`, which tries
  `/api/auth/login` and falls back to `/api/auth/register` on `404`; §4.3
  replaces that helper with `/api/auth/authenticate` and adds
  `/api/auth/code` before the code step;
- there is no `?next=` or `?ref=` handling, no redirect for an already
  authenticated visitor, and no `?oauth=ok` branch;
- the GitHub and Telegram buttons only show “скоро появится” instead of
  starting the backend OAuth2 flow;
- the page is hardcoded `lang="ru"` with Russian-only copy while the rest of
  the site switches en/ru/hy; the plan does not yet say whether `/register`
  follows the shared locale or stays Russian-only;
- the dev hint advertises the fixed code `1234`; §4.4 allows a fixed code only
  behind `AUTH_DEV_MODE`, and production must refuse to start with it.

### 14.3 Existing auth behaviour the plan changes

- a token is stored in `localStorage` under `pay3flow.auth.token`
  (`src/lib/auth-session.ts`); §12 question 4 still asks whether the portal
  should use cookie sessions instead;
- `/api/auth/login` answers `404` for an unknown email, which is how the
  current helper decides to register. §4.3 forbids revealing whether an email
  exists, so that signal must disappear with `/api/auth/authenticate`;
- `POST /api/auth/register` and `POST /api/auth/login` stay as deprecated
  aliases for one release (§7), so both keep working until the new flow ships.
