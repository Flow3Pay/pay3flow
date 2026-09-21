# Pay3Flow glossary

Use these terms consistently in code, API documentation, and user-facing
copy. Add new domain terms here before introducing them in multiple places.

## Acquirer

A provider or intermediary that processes a payment between sender and
recipient. In the current architecture an acquirer is a legacy/fallback rail
or a way to execute one local leg; it is no longer the central product model.

## Exchange intent / order

The user's request to exchange and deliver money between a source and target
country/currency, for example sending `100,000 AMD` from Armenia to a Russian
recipient in RUB. An order records methods, amounts, deadline, user, status,
and idempotency key. It does not mean that funds have been delivered.

## Solver / liquidity provider

A network participant that offers to execute an exchange order. A solver may
accept one local money leg, perform a TOKEN-leg or internal accounting step,
and deliver the other local leg. Its profile includes rails, countries,
currencies, limits, fees, speed, risk score, status, and possibly an
ActivityPub actor.

## Candidate

A solver surfaced by fmatch or a local registry as potentially eligible for an
order. A candidate is not yet the selected route and has no authority to start
settlement.

## Quote

A solver's offer for a particular order. It includes rate, fees, expiration,
expected timing, limits, and a settlement plan. The backend compares multiple
quotes during an auction window.

## Auction

The bounded period in which Pay3Flow collects and scores quotes before locking
one route. Winner selection is deterministic, auditable, and separate from
solver discovery.

## Route

The selected execution plan: solver, rails, source and target methods, rate,
fees, ETA, and settlement steps. A route is only authoritative after the
backend locks it.

## Settlement

Execution of an exchange order. The MVP models at least two legs:

1. `TOKEN-leg`: reserve, transfer, or internal accounting of a settlement asset.
2. `money-leg`: local delivery to the recipient through the selected rail.

The MVP uses a mock ledger while the real settlement model is undecided.

## TOKEN

The settlement asset used by the domain model. It may eventually be a stablecoin,
internal ledger unit, voucher, or another instrument. Until the legal and
technical model is approved, TOKEN means only the mock-ledger concept and must
not be presented as real stored value.

## Funding instruction

A user-facing instruction created after quote selection. It describes what the
user must review and initiate. Settlement must not begin before explicit terms
acceptance and funding confirmation.

## Proof

A receipt, bank reference, provider webhook, or machine-readable artifact that
supports a solver's claim that it completed a settlement step. MVP proofs may
be reviewed manually; production proofs should be independently verifiable.

## Dispute

A state in which a leg is delayed, a proof is invalid, an amount differs, or
receipt is not confirmed. A dispute freezes normal progression until an
authorized manual or automated resolution sets the order to `done` or `failed`.

## Transaction

The legacy payment entity created by `POST /api/payments`. It follows:

```text
pending -> matched -> executing -> done | failed
```

New exchange functionality should use `exchange_orders` instead.

## Fee

The cost of processing a payment or exchange. It may include a solver/provider
fee, a Pay3Flow service fee, and an FX margin. All API and database amounts are
minor units; the user interface should show the currency and precision clearly.

## Idempotency

A guarantee that retrying a request with the same key does not create a second
order or charge. Exchange creation uses `Idempotency-Key` and stores the key
with the user. ActivityPub delivery also uses activity IDs and delivery records
to avoid duplicate sends.

## Legacy / fallback rail

An existing acquiring or payment path retained for compatibility, fallback,
or demos. It is not a reason to build new exchange flows around provider
selection.
