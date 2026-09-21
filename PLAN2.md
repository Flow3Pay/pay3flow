# PLAN2: Pay3Flow exchange architecture

This is the working plan for the Pay3Flow MVP. It describes the product
direction, service responsibilities, execution rules, and implementation
phases so a contributor can continue work without re-discovering the design.

## 1. Product intent

Pay3Flow is a solver-based cross-border exchange coordinator. A user states the
source country/currency/method, target country/currency/method, amount, and
constraints. The backend turns that intent into an order, discovers eligible
solvers through fmatch, collects quotes, selects a route, requests explicit
funding consent, and records settlement proofs.

Example:

```text
Armenia -> Pay3Flow order -> TOKEN settlement -> local money delivery -> Russia
```

The user does not need to choose an intermediary cryptocurrency or provider
manually. The UI must show the resulting route, rate, fees, ETA, funding
instruction, and any TOKEN/crypto settlement asset before confirmation.

The MVP uses fake solvers and a mock TOKEN ledger. It must not be represented
as authorization to process real funds.

## 2. Target flow

```text
1. User submits an exchange intent.
2. Backend validates corridor, limits, and idempotency.
3. Backend asks fmatch for solver candidates.
4. Backend collects normalized quotes within an auction window.
5. Backend scores and locks the best route.
6. Backend presents a funding instruction and terms.
7. User explicitly confirms the funding action.
8. Solver runs the TOKEN-leg and money-leg.
9. Solver submits proof.
10. Backend verifies proof, writes audit events, and publishes the final status.
```

Discovery and quote search should be streaming/fan-out where useful: an input
leg can trigger an output-leg search as soon as a viable intermediary asset is
found. The frontend may receive progressively better routes, but the route
becomes immutable after `locked`.

## 3. Service responsibilities

```text
backend/orderbook
  -> stores exchange orders and durable state
  -> validates requests and idempotency
  -> collects quotes and chooses the winner
  -> owns settlement, proof, disputes, and audit history

fmatch
  -> advertises and discovers solver capabilities through ActivityPub
  -> returns candidates
  -> does not choose the Pay3Flow winner or execute money

solver
  -> provides a quote
  -> acknowledges readiness
  -> executes the TOKEN-leg and money-leg
  -> submits proof

cowprotocol-services/
  -> local reference for orderbook and solver-auction design
  -> not a runtime dependency or replacement for fmatch
```

Legacy `transactions`, `routes`, `acquirers`, and acquiring provider adapters
remain fallback/compatibility code. New exchange work belongs under the
`exchange_*` domain and `/api/exchange/orders`.

## 4. Data and lifecycle rules

The first corridor is seed data: `AM/AMD -> RU/RUB`. Never hard-code that pair
in core logic. Countries and currencies remain independent fields. Amounts are
integer minor units. Order creation is idempotent by user and key.

Order lifecycle:

```text
created -> discovering -> quoting -> quoted -> locked -> token_settling
-> money_settling -> proof_pending -> done
```

Failure branches are `failed`, `expired`, `cancelled`, and `disputed`.
Disputes are resolved explicitly to `done` or `failed`; terminal statuses do
not change in the normal flow. Every status transition is a guarded database
update and produces an audit event where appropriate.

Funding lifecycle:

```text
not_started -> created -> shown_to_user -> user_confirmed
-> solver_acknowledged -> received_by_solver
```

Settlement must not begin before the user confirms the terms and funding
instruction. The consent record contains order ID, user ID, terms version,
disclosure flag, and timestamp; never store PAN, CVC, or payment credentials in
the consent or audit payload.

## 5. fmatch contract

The backend sends FEP-0837 JSON-LD proposals through
`ActivityPubService::submit_request("candidates", content)`. The legacy payment
and exchange discovery paths share the same HTTP-signature and retry behavior.
The exchange content includes order ID, source/target country and currency,
methods, and source amount in minor units.

Discovery fallback order:

1. Live fmatch request.
2. Redis candidate cache, with a short TTL.
3. Local `exchange_solvers` registry filtered by corridor, rails, amount, and
   active/discovered status.

An empty successful fmatch response is authoritative and must not be replaced
with local candidates merely because it is empty. A transport failure may use a
cache or local registry according to the endpoint contract.

## 6. Quote and winner selection

Use a small abstraction for quote sources:

```text
RouteQuoteSource
  name() -> stable source name
  health() -> source health
  quote(request) -> normalized RouteQuote list
```

The source never creates an order, selects the global winner, or starts
settlement. External raw responses are retained for audit/debug, but the
normalized `RouteQuote` is the public contract.

MVP quote sources:

- `MockRouteQuoteSource` with deterministic success, reject, and timeout cases;
- at least a fast/low-limit and slow/better-rate fake solver profile.

Possible later adapters include Velora, Bebop, 0x, 1inch, KyberSwap, Enso, or
OKX. They remain disabled by default and are only TOKEN-leg/liquidity sources;
they do not replace fmatch or own order lifecycle.

The MVP scoring model should be deterministic and include target amount, fees,
ETA, risk, limits, manual-review penalties, expiration, and stable tie-breaks.
The selected winner cannot change after `locked`.

## 7. TOKEN-leg and money-leg rules

Until the settlement asset is defined, TOKEN means only a mock ledger unit.
Implement reserve, lock, release, and rollback with explicit audit events.
Define what happens when one leg succeeds and the other fails. A proof may be
a machine receipt, provider reference, or manual artifact in the MVP; invalid
proof enters `disputed` rather than silently completing the order.

The product must describe the route honestly. It may simplify infrastructure
language in the primary UI, but terms, confirmation, and compliance material
must disclose that a route may use a TOKEN/crypto settlement asset and that the
user initiates the fiat funding action.

## 8. Safety and launch gates

Before real money:

- obtain corridor-specific legal advice and define regulated roles;
- select KYC/AML, sanctions, source-of-funds, travel-rule, retention,
  complaint, chargeback, and suspicious-activity requirements;
- authenticate solvers and sign callbacks;
- review custody, key management, reconciliation, and proof independence;
- implement limits, manual review, alerting, four-eyes admin actions, and kill
  switches;
- test backups, rollback, incident response, and operator runbooks.

Run `scripts/secret-audit.sh`, backend tests, frontend checks/build, the
exchange smoke test, and Playwright before release. Development values from
`.env.example` and `docker-compose.yml` are not production secrets.

## 9. Implementation phases

### EX-0 — Architecture

- [x] Adopt solver-based exchange as the primary MVP direction.
- [x] Keep fmatch as candidate matcher and acquiring as fallback.
- [x] Document the CoW reference boundary and exchange domain.

### EX-1 — Reference research

- [x] Add the local CoW reference checkout.
- [x] Inspect orderbook, autopilot, driver, and solver-competition patterns.
- [x] Record build limitations and reuse decisions.

### EX-2 — Domain model

- [ ] Complete CRUD for corridors, orders, solvers, quotes, funding
  instructions, settlements, proofs, and audit events.
- [ ] Cover every transition with unit and integration tests.
- [ ] Keep all amounts and limits in minor units.

### EX-3 — Discovery and auction

- [ ] Stabilize order create/read endpoints and idempotency behavior.
- [ ] Connect fmatch discovery with cache and local fallback.
- [ ] Implement bounded quote collection and deterministic winner selection.
- [ ] Add a two-fake-solver smoke test.

### EX-4 — Settlement

- [ ] Implement the mock ledger and explicit partial-failure behavior.
- [ ] Add funding consent gate, proof verification, and disputes.
- [ ] Prove happy and invalid-proof flows end to end.

### EX-5 — Frontend and legacy isolation

- [ ] Show quotes, route details, fees, timing, terms, and history.
- [ ] Keep acquiring paths behind fallback/compatibility boundaries.
- [ ] Complete registration -> order -> quote -> settlement -> history E2E.

### EX-6 — Production readiness

- [ ] Complete legal/compliance review and operational controls.
- [ ] Add signed solver callbacks, reconciliation, monitoring, backups, and
  incident runbooks.
- [ ] Enable real providers only through reviewed, env-gated adapters.
