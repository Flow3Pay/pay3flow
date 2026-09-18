# Exchange Domain

Pay3Flow exchange flow хранит пользовательский intent как `exchange_order`.
Эквайеры остаются legacy/fallback rail, а основной MVP идет через solver
candidates, quotes, selected route, funding instruction, mock TOKEN settlement,
money leg и proof.

## Core Invariants

- Country и currency всегда отдельные поля:
  `source_country`, `source_currency`, `target_country`, `target_currency`.
- Все суммы хранятся в minor units (`BIGINT` / `Minor`).
- Первый MVP corridor живет в данных:
  `AM/AMD -> RU/RUB` в `exchange_corridors`.
- Core logic не должен содержать hardcoded `AMD`/`RUB` checks.
- Создание order идемпотентно по `(user_id, idempotency_key)`.
- Все status changes должны идти guarded update через текущий status.
- Terminal order statuses (`done`, `failed`, `expired`, `cancelled`) не меняются
  обычным flow.
- `disputed` не terminal: спор закрывается вручную в `done` или `failed`.
- Funding instruction описывает действие пользователя и solver/rail. Pay3Flow
  не списывает фиат автоматически.
- TOKEN/crypto settlement asset раскрывается в terms/consent/details, даже если
  основной UI показывает простой перевод.

## Tables

`exchange_corridors`

Enabled corridors and limits. Adding a new corridor should require data/admin
change, not Rust business-logic change.

`exchange_orders`

Stored user intent. Starts as `created`, then moves through discovery, quote,
lock, settlement and final statuses.

`exchange_solvers`

Local solver registry. `fmatch` candidates must be mapped to active local
solvers before quotes are requested.

`exchange_quotes`

Normalized solver offers. Raw solver payload stays in `raw_response`; scoring
and selected route use normalized fields.

`funding_instructions`

User-facing funding/payment instruction created after selected quote. Settlement
must not start before user confirmation.

`exchange_settlements`

Execution record for TOKEN-leg, money-leg and proof lifecycle.

`exchange_proofs`

Receipts or machine-readable proof payloads submitted by solver/manual flow.

`audit_events`

Append-only audit trail for important create/update/status/proof/funding events.

## Order Status Flow

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

## Funding Status Flow

Order and settlement use `not_started` until an instruction exists.

```text
not_started -> created -> shown_to_user -> user_confirmed
-> solver_acknowledged -> received_by_solver
```

Failure/stop branches:

```text
created -> expired | cancelled
shown_to_user -> expired | cancelled
user_confirmed -> failed
solver_acknowledged -> failed
```

## Current Rust Surface

- `backend/src/exchange/status.rs`
  contains typed status enums and transition tables.
- `backend/src/exchange/model.rs`
  contains DB-shaped domain structs.
- `backend/src/exchange/repo.rs`
  currently covers:
  corridor lookup, idempotent order creation, order lookup, guarded order
  status transition and audit insert.

The remaining repo work is CRUD/insert helpers for solvers, quotes,
funding instructions, settlements and proofs.
