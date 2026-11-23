# OTC temporary wallet incident runbook

Operational recovery for platform-owned temporary wallets. The behavior contract
is [`OTC_TEMP_MULTICHAIN_WALLETS.md`](OTC_TEMP_MULTICHAIN_WALLETS.md); this file
covers what an operator does when a settlement stops moving.

Scope: `OTC_TEMP_WALLETS=1` and an EVM stablecoin source
(Ethereum/Polygon/Avalanche C). Accept reserves two or three addresses. The
USD deposit and payout are custodial; for a cross-network deal, Intents pays
the `usd_out` temp, which then pays the recipient. Same-chain payouts come
from the per-chain treasury while the full deposit remains on the USD temp.
The EVER payer deposits on `temp_ever`; the worker forwards the booked amount
to `ever_payee` after USD settlement. Native gas auto-top-up is not
implemented, and non-EVM stablecoin custody is not available.

## 1. Enablement prerequisites

Do not enable the flag until every item holds on the target deployment:

- Two independent HTTPS RPC endpoints per EVM network used for settlement
  (`p2p_ethereum_rpc` for Ethereum, `p2p_evm_rpcs.<network>` for the rest).
  One endpoint is rejected at startup.
- The worker creates one permanent `treasury` row per configured supported EVM
  network. Operators must fund each treasury with stablecoins for same-chain
  payouts and native gas for its own transactions. The row is never deleted.
- `near_intents` is configured for every cross-network deal; without it Accept
  does not create a temporary wallet for that route.
- `temp_gas_reserve` (default `0.0005`) is below the native gas kept on each
  temporary address and on each treasury. The worker waits instead of
  signing when gas is short.
- Each treasury holds enough stablecoin to front at least one same-network net
  payout plus a native gas buffer.
- `SECRETS_KEY` is stable. Rotating it breaks decryption of every active row
  (see §6).
- Demo and live databases are separate; never point a demo worker at live rows.

## 2. Where the state lives

| Source of truth | Contents |
| --- | --- |
| `otc_p2p_trades.body.temp_wallet` | Funded USD input address, chain, token and asset id shown to both parties |
| `otc_p2p_trades.body.temp_wallets` | Full planned set (`ever`, `usd`, or `usd_in` + `usd_out`) with status |
| `otc_temp_wallets` | Role, chain, address, derivation index, salt, ciphertext, status |
| `otc_temp_settlements` | USD payout state machine, tx hashes, Intents quote id and deposit details, attempt count, `reserve_units` when the net came from the platform reserve, and `ever_state`/`ever_reference`/`ever_error`/`ever_attempts` for the EVER forward |
| `otc_temp_wallets` (`role = 'treasury'`) | Permanent per-chain treasury address, signing key ciphertext, stablecoin and native gas reserve |
| `otc_p2p_attempts.temp_wallet_id` | Links a payment attempt to the custody row that received the deposit |

Inspect one deal:

```sql
SELECT s.trade_id, s.state, s.attempts, s.last_error, s.reference,
       s.fee_reference, s.quote_id, s.deposit_address, s.deposit_memo,
       w.id AS wallet_id, w.chain, w.address, w.status, w.derivation_index
FROM otc_temp_settlements s
JOIN otc_temp_wallets w ON w.id = s.wallet_id
WHERE s.trade_id = '<trade-uuid>';
```

Inspect the payment attempt and its custody row:

```sql
SELECT a.id, a.state, a.reference, a.temp_wallet_id, w.address, w.chain
FROM otc_p2p_attempts a
LEFT JOIN otc_temp_wallets w ON w.id = a.temp_wallet_id
WHERE a.trade_id = '<trade-uuid>' AND a.kind = 'payment';
```

Never log, copy or export decrypted seeds. Sponsor and sweep operations use the
worker, not an operator-held key.

## 3. State machine

The settlement worker (`otc::temp_settle`) only picks rows whose trade is
`delivery_pending` or `completed`, whose state is one of `awaiting_payment`,
`payout_submitted`, `intents_submitted`, `fee_submitted`, and whose
`attempts < 10`. A stale `payout_claim` older than two minutes is reclaimed.
`stalled` is terminal until an operator hands the row back (§4.7).

| State | Meaning | Next automatic step |
| --- | --- | --- |
| `awaiting_payment` | Waiting for the gross deposit or for native gas | Balance check, then direct or Intents payout |
| `payout_claim` | A worker is signing; stale claims self-recover | Back to `awaiting_payment` after 2 minutes |
| `payout_submitted` | Introduced; receipt polled by hash | `after_paid` on status 1, `failed` on revert |
| `intents_submitted` | Intents deposit sent; status polled by deposit/memo | Sweep on `SUCCESS`, stop on `REFUNDED`/`FAILED` |
| `fee_submitted` | Fee sweep sent; receipt polled by hash | `settled` on status 1, `fee_retained` on revert |
| `fee_retained` | Net paid, fee left on the temporary address | Manual sweep or ops write-off |
| `intents_refunded` | Deposit returned to the temporary address | Manual retry or manual return |
| `stalled` | Ten failed attempts; the worker no longer selects the row | Operator recovery (§4.7) |
| `settled` / `failed` | Terminal for the worker; the deal's `ever` row is marked `deleted` | None |

## 4. Triage

### 4.1 `awaiting_payment` with `last_error` about native gas

Top up native gas on the reported address from the network reserve, then wait
one worker tick. Do not reset `attempts` unless the count is already at the cap.
Record the top-up transaction in the incident.

### 4.2 `awaiting_payment` with a funded balance but repeated errors

The worker signs only after both RPCs agree on chain id, nonce, gas estimate and
fees. Check that both endpoints answer and return the same chain id. Fix or
replace the failing endpoint; the next tick retries. If `attempts` reached 10,
reset it after the fix:

```sql
UPDATE otc_temp_settlements
SET attempts = 0, last_error = NULL, updated_at = now()
WHERE trade_id = '<trade-uuid>' AND state = 'awaiting_payment';
```

Never reset a row in `payout_submitted`, `intents_submitted` or `fee_submitted`:
the balance gate protects `awaiting_payment`, but forcing a submitted row back
can double-send.

### 4.3 `payout_submitted` does not advance

Look up `reference` on a block explorer.

- Confirmed success, row still waiting: check that both RPCs return the receipt.
  A quorum disagreement keeps the row in place; fix the endpoint.
- Reverted: the worker moves to `failed`; the deposit is still on the temporary
  address. Treat as §4.6.
- Dropped/unknown: do not re-send blindly. The nonce was consumed by signing;
  confirm the transaction is absent from the mempool and the sender nonce before
  allowing a retry.

### 4.4 `intents_submitted` does not advance

Poll the 1Click status with the stored `deposit_address` and `deposit_memo`.
The worker accepts `SUCCESS` only when the delivered amount is at least
`minimum_units` (the deal `net_units`).

- `SUCCESS` below the minimum: the row intentionally stays put and logs a
  shortfall. Open an incident; do not pay the EVER leg from the desk until the
  stablecoin net is delivered or refunded.
- `REFUNDED`: see §4.5.
- `FAILED`: the deposit may still sit on the temporary address or be in flight;
  verify the token balance before any manual action.
- `PENDING` beyond the quote window: confirm the deposit transaction is mined
  and the provider still knows the deposit; escalate with the quote id.

### 4.5 `intents_refunded`

The refund lands back on the temporary address we control. Only retry after
confirming the provider status is `REFUNDED` and the gross token balance is back
on the address:

```sql
UPDATE otc_temp_settlements
SET state = 'awaiting_payment', attempts = 0, last_error = NULL, updated_at = now()
WHERE trade_id = '<trade-uuid>' AND state = 'intents_refunded';
```

The worker re-quotes and signs a fresh deposit. If the retry also fails, stop
retrying and return the balance to the payer's verified main wallet; that manual
return is currently outside the worker and needs an engineering-signed transfer.

### 4.6 `failed` or `fee_retained`

Funds remain on the temporary address:

- `failed`: the payee was not paid (or the payout reverted). Verify the token
  balance, then either return it to the payer or retry by moving the row back to
  `awaiting_payment` **only** when no payout transaction exists. Check
  `reference` first.
- `fee_retained`: the net was delivered; sweep the remaining fee to the
  configured recipient for that chain. Prefer fixing the recipient/RPC and
  resetting the row to `awaiting_payment` is **not** safe here because that
  would pay the net again. Manual sweep or write-off only.

### 4.7 `stalled` (ten attempts)

The worker increments `attempts` on every failure and moves the row to `stalled`
when the count reaches ten, so it stops being selected instead of freezing in a
live state. `last_error` names the last failure. Diagnose and fix the blocker
first, then hand the row back to the worker:

```sql
UPDATE otc_temp_settlements
SET state = 'awaiting_payment', attempts = 0, last_error = NULL, updated_at = now()
WHERE trade_id = '<trade-uuid>' AND state = 'stalled';
```

Only do this when no payout or fee transaction is in flight (`reference` and
`fee_reference` are null, or already confirmed absent on chain); otherwise treat
the row as §4.3 or §4.5.

## 5. Recycle pool and dust

When a settlement leaves less than $1 in the USD temp wallet, the worker
detaches that same row into the reusable pool. A later deal may reuse it, but
the accept transaction locks the row and attaches it to only one active deal.
At $1 or more, the worker sweeps the full remaining token balance, then calls
`create_based_on`: same seed, `derivation_index + 1`, fresh salt and
ciphertext, `parent_wallet_id` set, and the swept row marked `recycled`. The
child has `trade_id = NULL` and is available for the next deal. Do not
hard-delete rows; a recycled row may be needed for forensics.

## 6. `SECRETS_KEY` handling

Per-row salts make the key irreversible for active rows without the original
value. There is no re-encryption job yet, so:

- Keep the current key until every settlement row is `settled`,
  `fee_retained` or `failed` with no pending action.
- Rotate only into a maintenance window, then re-run the worker to confirm old
  rows are no longer needed.
- Losing the old key strands any funded temporary address. Escalate as a P0;
  the on-chain funds are recoverable only with the key that sealed the seed.

## 7. Treasury and gas

The worker provisions one permanent `treasury` wallet per supported EVM network
with configured temp-wallet RPCs. Same-network payouts are always sent from
that treasury rather than the funded temporary address. The treasury uses the
same per-row encryption and is never recycled or deleted:

```sql
SELECT chain, address, status FROM otc_temp_wallets
WHERE role = 'treasury' ORDER BY chain;
```

Fund each address with stablecoins for a same-chain payout plus a native-gas
buffer before enabling custody. Accept preflights the stablecoin and native gas
balances and returns a 409 naming the treasury when it cannot front the payout.
The payer's full deposit remains on the USD temp and its accumulated balance is
swept to treasury. The worker does not yet top up temp-wallet gas automatically;
if a temp address is short, the settlement records the reason and an operator
must fund it from the treasury before the worker can continue.

## 7.1 EVER temporary leg

When Accept reserved an `ever` row, the payout handoff sends the EVER payer to
`temp_ever` instead of `ever_payee`. After the USD settlement reaches `settled`
or `fee_retained` and the payer's deposit attempt is `verified`, the worker
forwards the booked `ever_units` to `ever_payee` and completes the deal. State
is on the settlement row:

```sql
SELECT trade_id, ever_state, ever_reference, ever_attempts, ever_error
FROM otc_temp_settlements WHERE ever_state IS NOT NULL ORDER BY updated_at DESC;
```

`ever_state = 'pending'` with "not funded enough" means the temp holds less than
`ever_units + p2p_ever_fee_reserve` (the buffer the payout handoff adds); top it
up and the next tick retries.
`submitted` means the signed external message was accepted; the worker polls
`transactions(in_msg: reference)` until it is included. `failed` means the
message aborted or 60 attempts elapsed; inspect the temp address and the
`ever_reference` message. The USD side is already final at this point, so treat
an EVER failure as a P1 and do not reopen the USD leg.

## 8. Evidence to collect for an incident

Trade id, wallet id, chain and address, settlement state and attempt count,
`last_error`, every tx hash (`reference`, `fee_reference`), Intents `quote_id`,
`deposit_address` and `deposit_memo`, explorer links, and the deploy/flag state.
Do not include seeds, salts or ciphertext.

## 9. Demo-to-live gate

Before flipping the flag on live:

1. Run a two-wallet deal (same network) end to end on demo: Accept → deposit to
   the temporary address → worker pays the net → fee sweep → recycle.
2. Run a three-wallet deal shape (cross-network) end to end: exact-output quote,
   signed deposit, `SUCCESS`, fee withheld and swept.
3. Replay Accept and the settlement tick to confirm no duplicate address and no
   double payout.
4. Verify `temp_wallets` and `temp_wallet_id` appear on deal details and attempts.
5. Confirm dual RPCs, fee recipients and gas balances for every enabled network.
