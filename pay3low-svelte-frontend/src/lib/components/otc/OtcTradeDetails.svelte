<script lang="ts">
  import { decimal, rate, type Trade, type Config, type Session, type Attempt } from "$lib/otc/service";
  export let selected: Trade;
  export let config: Config | null;
  export let session: Session | null;
  export let attempts: Attempt[] = [];
  export let receipts: { chain: string; leg: { amount: string }; included_at: string }[] = [];
  export let updates: { message: string; at: string }[] = [];
  export let busy = false, unresolvedBooking = false, handoffUnknown = false, now = Date.now();
  export let onApply: () => void;
  export let onDecide: (accept: boolean) => void;
  export let onTransfer: (kind: string) => void;
  export let onTrack: (attempt: Attempt, reference: string) => void;
  export let onRetry: (attempt: Attempt, evidence: string) => void;
  let reference = "", incidentEvidence = "";
</script>
<div class="serviceControls">
        <article class="detail"><h3>{selected.terms.direction === "buy" ? "Buy" : "Sell"} EVER · {selected.state.replaceAll("_", " ")}</h3><p class="address">{selected.id}</p>
          {#each updates as update}<p class="message">{update.at} · {update.message}</p>{/each}
          <dl><dt>Customer input</dt><dd>{selected.terms.input} {selected.terms.direction === "buy" ? "USDT · Ethereum" : "EVER · Everscale"}</dd><dt>Fixed net output</dt><dd>{selected.terms.output} {selected.terms.direction === "buy" ? "EVER · Everscale" : "USDT · Ethereum"}</dd><dt>Rate</dt><dd>{rate(selected.terms)} USDT per EVER</dd><dt>Desk completion fee</dt><dd>{decimal(selected.terms.fee_policy.fee_units, 6)} USDT · paid by desk</dd><dt>Network costs</dt><dd>{selected.terms.network_costs} Receiving allowance: {decimal(selected.terms.ever_receiving_cost_units, 9)} EVER.</dd><dt>Quote expires</dt><dd>{selected.terms.quote_by}</dd><dt>Payment inclusion deadline</dt><dd>{selected.terms.pay_by}</dd><dt>Payout deadline</dt><dd>{selected.terms.payout_by}</dd><dt>Refund policy</dt><dd>{selected.terms.refund_policy}</dd></dl>
          {#if !session?.desk && selected.state === "quoted"}<button class="primary" disabled={busy || unresolvedBooking || now > Date.parse(selected.terms.quote_by)} on:click={onApply}>Accept fixed quote</button>{/if}
          {#if session?.desk && selected.state === "booking_pending"}<button disabled={busy} on:click={() => onDecide(true)}>Reserve and accept</button><button disabled={busy} on:click={() => onDecide(false)}>Reject offer</button>{/if}
          {#if !session?.desk && selected.state === "accepted"}<button class="primary" disabled={busy || handoffUnknown || attempts.some(a => a.kind === "payment" && a.state !== "failed") || config?.demo || selected.terms.demo || now > Date.parse(selected.terms.pay_by)} on:click={() => onTransfer("payment")}>Review and send customer payment</button>{/if}
          {#if session?.desk && selected.state === "funded"}<button class="primary" disabled={busy || handoffUnknown || attempts.some(a => ["payout", "refund"].includes(a.kind) && a.state !== "failed") || config?.demo || selected.terms.demo} on:click={() => onTransfer("payout")}>Review and send desk payout</button>{/if}
          {#if session?.desk && ["review", "funded"].includes(selected.state) && !attempts.some(a => ["payout", "refund"].includes(a.kind) && a.state !== "failed")}<button disabled={busy || handoffUnknown || attempts.some(a => ["payout", "refund"].includes(a.kind) && a.state !== "failed") || config?.demo || selected.terms.demo} on:click={() => onTransfer("refund")}>Review validated refund</button>{/if}
          {#each receipts as receipt}<p class="muted">Verified chain receipt: {decimal(receipt.leg.amount, receipt.chain === "ethereum" ? 6 : 9)} {receipt.chain === "ethereum" ? "USDT" : "EVER"} · {receipt.included_at}</p>{/each}
          {#each attempts as attempt}<div class="attempt"><strong>{attempt.kind} · {attempt.state}</strong><p class="address">{attempt.instructions.leg.sender} → {attempt.instructions.leg.recipient}</p><p>{attempt.instructions.leg.amount} base units net · {attempt.instructions.leg.chain}</p><p class="address">{attempt.reference || "Original result unknown. Another send is blocked."}</p>{#if attempt.state === "failed"}<label for={`retry-${attempt.id}`}>Failure review evidence</label><input id={`retry-${attempt.id}`} bind:value={incidentEvidence} /><button disabled={busy || !incidentEvidence.trim()} on:click={() => onRetry(attempt, incidentEvidence)}>Authorize retry after verified failure</button>{/if}{#if !attempt.reference && (attempt.kind === "payment" ? !session?.desk : session?.desk)}<label for={`reference-${attempt.id}`}>Original transaction hash</label><input id={`reference-${attempt.id}`} bind:value={reference} /><button disabled={busy} on:click={() => onTrack(attempt, reference)}>Track original transfer</button>{/if}</div>{/each}
        </article>

</div>
