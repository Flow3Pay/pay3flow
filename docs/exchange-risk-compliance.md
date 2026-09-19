# Exchange risk and compliance gates

## Scope

The current Pay3Flow exchange is a fake-money MVP. It demonstrates order intake,
solver discovery, route selection, explicit user funding consent, an internal
mock TOKEN ledger, proof verification, disputes and audit history. It is not an
authorization to process real funds.

The customer-facing terms must say that Pay3Flow selects an exchange and
settlement route, that the route may use a TOKEN or crypto settlement asset,
and that the user initiates the fiat funding action. The product must not claim
that the route is a simple direct fiat transfer when that is not true.

## Runtime controls

Safety state is stored in PostgreSQL so an operator can change it without a
deployment. Admin endpoints require `Authorization: Bearer $ADMIN_TOKEN`.

- Global kill switch: `GET|POST /api/admin/exchange/controls`.
- Corridor kill switch: `POST /api/admin/exchange/corridors/:id` with
  `{"enabled": false}`.
- Solver kill switch: `POST /api/admin/exchange/solvers/:id` with status
  `paused` or `blocked`.
- User, corridor and solver daily limits are checked in minor units. Global
  limits and the manual-review threshold live in `exchange_controls`; corridor
  limits live in `exchange_corridors`.
- Orders at or above `manual_review_threshold_minor` receive a pending review
  and cannot lock until an administrator calls
  `POST /api/admin/exchange/orders/:id/manual-review`.
- A disputed order can only be resolved to `done` or `failed` through
  `POST /api/admin/exchange/orders/:id/dispute`.

Every consent stores only the order ID, user ID, terms version, disclosure flag
and acceptance timestamp. Payment credentials, PAN and CVC are not part of the
consent record or audit payload.

## Production launch blockers

Before enabling real money, the owner must obtain corridor-specific legal
advice and define the regulated roles of Pay3Flow and each solver. KYC/AML,
sanctions screening, source-of-funds rules, data retention, travel-rule duties,
consumer disclosures, complaints, chargebacks and suspicious-activity handling
must be selected for every jurisdiction. Solver authentication and signed
callbacks are mandatory; the MVP solver endpoints are not production-safe.

Production also requires custody and key-management review, provider contracts,
real reconciliation, independent proof sources, incident response, alerting,
limit ownership, four-eyes admin approval and tested rollback/runbooks.

## Operational checks

Run `scripts/secret-audit.sh`, backend tests, frontend lint/build, the exchange
smoke script and Playwright before release. Rotate the development JWT, admin,
database and encryption values; no default from `.env.example` or compose is a
production secret. Logs must retain IDs, statuses and reason codes, never PAN,
CVC, private keys, bearer tokens or raw sensitive proof payloads.
