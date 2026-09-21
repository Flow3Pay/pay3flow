# Roadmap: CoW-style exchange architecture

Pay3Flow is moving from an acquiring-first model to solver-based cross-border
exchange. Acquirers remain fallback rails and sources for individual payment
methods; the target product is intent/orderbook plus solver competition and
local money legs.

## Target model

```text
user intent -> Pay3Flow order -> solver candidates -> quotes
             -> selected route -> TOKEN-leg -> money-leg -> proof
```

The backend accepts the intent, validates corridor and limits, discovers
solvers, compares price/risk/timing, locks the route, and tracks settlement to
its final status. The user sees and accepts the route and any settlement-asset
disclosure before funding.

## Role of CoW Protocol Services

The local `cowprotocol-services/` checkout is a reference for orderbook,
auction windows, solver competition, winner selection, settlement status, and
observability. It does not replace fmatch. fmatch remains the ActivityPub
matcher for Pay3Flow solver candidates.

Do not copy Ethereum-only settlement, ERC-20 approvals, contract assumptions,
or chain-specific funding checks into the fiat exchange core without a new
design decision.

## Legacy scope

The old `transactions`, `routes`, `acquirers`, `/api/payments`, and provider
adapter paths remain for compatibility, fallback, and smoke tests. New product
work should use:

```text
exchange_order -> fmatch candidates -> backend auction -> selected quote
-> funding instruction -> mock TOKEN-leg -> money-leg -> proof
```

## Completed architecture work

- Defined solver-based cross-border exchange as the primary direction.
- Kept fmatch as candidate discovery rather than winner selection.
- Added the exchange domain document and typed status model.
- Added a local CoW reference checkout and documented its boundaries.
- Added risk/compliance gates, audit requirements, and kill-switch requirements.
- Added read-only P2P route search and explicit public-source policy.

## Next implementation phases

### EX-2 — Complete the domain model

- Add and maintain `exchange_orders`, `exchange_solvers`, `exchange_quotes`,
  `exchange_settlements`, `exchange_proofs`, `funding_instructions`, and
  `audit_events`.
- Keep the corridor in data: `AM/AMD -> RU/RUB` is seed data only.
- Cover all status transitions with guarded-update and transition tests.

### EX-3 — Orderbook, discovery, and auction

- Stabilize create/read order endpoints and idempotency behavior.
- Map fmatch candidates to active local solvers.
- Collect quotes within a bounded auction window.
- Score quotes by target amount, fees, ETA, limits, and risk.
- Cache short-lived candidate/quote data without replacing durable state.
- Add a smoke test with two fake solvers and a deterministic winner.

### EX-4 — TOKEN-leg and money-leg

- Keep the TOKEN implementation as a mock ledger until its legal and technical
  meaning is approved.
- Define reserve, lock, release, rollback, proof, and partial-failure behavior.
- Add dispute handling that freezes settlement and requires an explicit outcome.
- Prove the happy path and invalid-proof path in smoke tests.

### EX-5 — Simplify the reference and legacy paths

- Keep chain-specific CoW components isolated from the fiat domain.
- Keep acquiring adapters as fallback rails.
- Make the frontend display quotes, solver route, fees, timing, and final amount.
- Complete the registration -> order -> quote -> settlement -> history flow.

### EX-6 — Risk, compliance, and security

- Define KYC/AML and sanctions requirements for users and solvers.
- Enforce per-user, per-solver, per-corridor, and daily limits.
- Add velocity, repeated-identity, and manual-review controls.
- Preserve a complete audit trail and operator kill switches.
- Finish legal review before enabling real-money settlement.

### EX-7 — Production readiness

- Add authenticated solver APIs and signed callbacks.
- Establish custody, key management, reconciliation, independent proofs, and
  incident response.
- Add backups, restore drills, alerting, access reviews, and release runbooks.
- Gate every external provider behind configuration, rate limits, and health
  monitoring.
