# Exchange domain

The exchange flow stores a user's intent as an `exchange_order`. Acquirers are
legacy or fallback rails; the primary MVP path is solver discovery, quote
collection, route selection, an explicit funding instruction, mock TOKEN
settlement, a money leg, and proof verification.

## Core invariants

- Country and currency are separate fields: `source_country`,
  `source_currency`, `target_country`, and `target_currency`.
- Amounts are stored in minor units (`BIGINT`/`Minor`), never floating-point
  database values.
- The first seeded corridor is `AM/AMD -> RU/RUB` in `exchange_corridors`.
- Core logic must not contain hard-coded `AMD` or `RUB` checks.
- Order creation is idempotent by `(user_id, idempotency_key)`.
- Status changes use guarded updates that include the current status.
- Terminal statuses (`done`, `failed`, `expired`, `cancelled`) do not change in
  the normal flow.
- `disputed` is not terminal: an operator resolves it to `done` or `failed`.
- A funding instruction describes a user action and a solver/rail. Pay3Flow
  does not automatically debit fiat.
- The terms, consent, and route details must disclose any TOKEN or crypto
  settlement asset.

## Tables

`exchange_corridors` stores enabled corridors and limits. Adding a corridor
should normally be a data or admin change, not a Rust business-logic change.

`exchange_orders` stores the user intent and its lifecycle.

`exchange_solvers` stores local solver profiles. fmatch candidates must be
mapped to an active or discovered local solver before quotes are requested.

`exchange_quotes` stores normalized offers. The raw solver response remains in
`raw_response`; scoring and the selected route use normalized fields.

`funding_instructions` stores the user-facing action created after a quote is
selected. Settlement must not start before user confirmation.

`exchange_settlements` stores TOKEN-leg, money-leg, and proof lifecycle data.

`exchange_proofs` stores receipts or machine-readable proof payloads.

`audit_events` is an append-only record of important order, quote, funding,
status, proof, dispute, and admin actions.

## Order status flow

```text
created -> discovering -> quoting -> quoted -> locked -> token_settling
-> money_settling -> proof_pending -> done
```

Allowed branches:

```text
discovering -> failed
quoting -> failed | expired
quoted -> expired | cancelled
token_settling -> disputed | failed
money_settling -> disputed | failed
proof_pending -> disputed | failed
disputed -> done | failed
```

## Funding status flow

An order and settlement remain `not_started` until an instruction exists.

```text
not_started -> created -> shown_to_user -> user_confirmed
-> solver_acknowledged -> received_by_solver
```

Failure and stop branches:

```text
created -> expired | cancelled
shown_to_user -> expired | cancelled
user_confirmed -> failed
solver_acknowledged -> failed
```

## Current Rust surface

- `backend/src/exchange/status.rs` contains typed status enums and transition tables.
- `backend/src/exchange/model.rs` contains database-shaped domain structs.
- `backend/src/exchange/repo.rs` covers corridor lookup, idempotent order
  creation, order lookup, guarded status transitions, and audit insertion.

Additional CRUD helpers for solvers, quotes, funding instructions, settlements,
and proofs should preserve the same invariants.
