# CoW Protocol Services analysis

Date: 2026-09-18.

This document records which ideas from the local `cowprotocol-services/`
checkout are useful for Pay3Flow and which remain reference-only. CoW does not
replace `fmatch`: fmatch discovers solver candidates, while the Pay3Flow
backend stores orders, collects quotes, selects a winner, and owns the
settlement/proof lifecycle.

## Reviewed material

- `cowprotocol-services/README.md` and `docs/ONBOARDING.md`;
- orderbook API, order and quote endpoints, and OpenAPI definitions;
- autopilot run loop and solvable-order filtering;
- driver quote domain, run loop, and OpenAPI definitions;
- Matcha UI and 0x documentation as external UX/API references.

## Local build status

The source-level check is:

```bash
cd cowprotocol-services
cargo check -p orderbook -p autopilot -p driver --all-targets
```

The check was not run in the original environment because `cargo` was not
installed and the current user could not access the Docker socket. The upstream
playground also requires a configured `ETH_RPC_URL`:

```bash
cd cowprotocol-services
docker compose -f playground/docker-compose.fork.yml up --build
```

Repeat these checks when the Rust toolchain and Docker permissions are
available. This limitation does not affect the architectural conclusions below.

## Orderbook lessons

The CoW orderbook accepts orders, validates constraints, persists lifecycle
data, exposes status/history, and supplies eligible orders to solver tooling.
Useful patterns for Pay3Flow are:

- a dedicated exchange-order API instead of extending the old payment endpoint;
- typed request/response DTOs and consistent validation errors;
- separate order, status, quote, and history endpoints;
- idempotent creation and observable HTTP/status transitions.

Do not copy Ethereum signatures, ERC-20 allowance checks, on-chain token
validation, or the CoW order UID as Pay3Flow's domain model. Pay3Flow orders
store country, currency, method, amount, deadline, and corridor data. Amounts
are integer minor units and creation is idempotent by user and key.

## Autopilot and auction lessons

Autopilot builds auctions from eligible orders, filters non-executable work,
collects solutions, ranks them, and prevents in-flight orders from re-entering
competition. Pay3Flow should adopt:

- a short quote window, currently planned as three seconds;
- explicit rejection reasons for candidates and quotes;
- deterministic scoring and stable tie-break rules;
- an immutable winner after `locked`;
- audit events for selected winners and rejected quotes.

Pay3Flow does not need block-based scheduling, EVM deadlines, on-chain event
indexing, or calldata simulation. The backend receives fmatch candidates,
collects quotes, and selects the winner. The fallback order is Redis cache,
local solver registry, then failure.

## Driver and quote-source lessons

The CoW driver separates quote production from orderbook storage and converts a
solver solution into a quote. Pay3Flow should keep the same boundary:

```text
solver discovery -> quote collection -> winner selection -> settlement
```

Use a `RouteQuoteSource` abstraction with `name`, `health`, and `quote` methods.
The MVP implementation is `MockRouteQuoteSource`; later providers may become
adapters for TOKEN-leg liquidity. An adapter timeout or rejection must not fail
the complete auction, and raw provider data must remain separate from the
normalized public quote.

Do not copy EVM transaction simulation, calldata encoding, settlement contracts,
or chain-specific exclusivity into the fiat exchange core.

## Lifecycle mapping

The Pay3Flow lifecycle is exchange-specific:

```text
created -> discovering -> quoting -> quoted -> locked -> token_settling
-> money_settling -> proof_pending -> done
```

Failures and disputes branch to `failed`, `expired`, `cancelled`, or `disputed`
as described in [`exchange-domain.md`](exchange-domain.md). Every transition
must be a guarded database update using the current status. Terminal statuses
must not change without an explicit manual-resolution path.

## UX and API references

Matcha is useful as a product reference: start with a simple sell/receive
intent, expose price, fees, timing, route details, and warnings, and keep
provider selection behind route information. 0x is useful as a reference for
optional trading API adapters.

Pay3Flow should:

- keep the first screen focused on the exchange intent;
- display amount, rate, fee, ETA, selected route, and funding instruction;
- disclose TOKEN/crypto settlement in terms, consent, and details;
- load currencies and corridors from the backend rather than hard-coding them.

## Decisions

1. Keep CoW as a local reference, not a runtime dependency.
2. Keep fmatch as the solver-candidate matcher.
3. Keep winner selection and audit history in the Pay3Flow backend.
4. Start new work with typed domain models, migrations, and transition tests.
5. Keep real money and token integrations behind mock/stub implementations until
   legal, compliance, custody, and proof decisions are complete.
6. Keep `AM/AMD -> RU/RUB` in seed data rather than Rust conditionals.

Files worth revisiting for future work are the upstream orderbook API/database,
autopilot run loop and solvable-order modules, driver quote domain, and solver
competition persistence.
