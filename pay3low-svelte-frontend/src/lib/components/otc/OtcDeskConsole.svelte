<script lang="ts">
  import OtcPanel from "./OtcPanel.svelte";
  import { request, decimal, type Config, type Session, type Trade, type Rfq } from "$lib/otc/service";
  export let session: Session;
  export let config: Config | null;
  export let selected: Trade | null;
  export let rfqs: Rfq[] = [];
  export let dashboard: Record<string, any> | null = null;
  export let busy = false;
  export let run: (action: () => Promise<void>) => Promise<void>;
  export let onRefresh: () => Promise<void>;
  export let onSelect: (id: string) => Promise<void>;
  let quoteRfq = "", quoteOutput = "", everCost = "0", networkCosts = "";
  let entryKind = "fee_collected", entryAmount = "", entryEvidence = "";
  let incidentAction = "acknowledge", incidentEvidence = "", customerUpdate = "";
  async function makeQuote() {
    await request("/desk/quotes", session.token, { rfq_id: quoteRfq, output: quoteOutput, network_costs: networkCosts, ever_receiving_cost: everCost }); await onRefresh();
  }
  async function changePause(paused: boolean) { await request("/desk/control", session.token, { paused }); await onRefresh(); }
  async function recordEntry() {
    if (!selected) return;
    await request("/desk/accounting", session.token, { trade_id: selected.id, kind: entryKind, amount: entryAmount, reference: entryEvidence, idempotency_key: crypto.randomUUID() }); await onRefresh();
  }
  async function updateIncident(key: string) {
    let evidence: unknown;
    try { evidence = JSON.parse(incidentEvidence); } catch { evidence = { note: incidentEvidence }; }
    await request(`/desk/incidents/${encodeURIComponent(key)}`, session.token, { action: incidentAction, evidence, customer_update: customerUpdate || null }); await onRefresh();
  }
</script>
<div class="serviceControls">
  {#if session?.desk}
    <OtcPanel id="otc-desk" title="Desk console" collapsible={false}>
      <div class="card"><div class="actions"><button disabled={busy} on:click={() => run(() => changePause(true))}>Pause bookings</button><button disabled={busy} on:click={() => run(() => changePause(false))}>Resume healthy route</button><button disabled={busy} on:click={() => run(async () => { await request("/desk/listings", session.token, {}); await onRefresh(); })}>Publish desk Proposals</button></div>
        <form on:submit|preventDefault={() => run(makeQuote)}><h3>Private fixed quote</h3><label for="rfq">RFQ</label><select id="rfq" required bind:value={quoteRfq}><option value="">Select customer request</option>{#each rfqs.filter(r => r.state === "waiting") as r}<option value={r.id}>{r.direction} · {r.input} · {r.actor}</option>{/each}</select><label for="quote-output">Promised net output</label><input id="quote-output" inputmode="decimal" required bind:value={quoteOutput} /><label for="ever-cost">Receiving cost allowance in EVER (additional to net leg)</label><input id="ever-cost" inputmode="decimal" required bind:value={everCost} /><label for="network-costs">Separate network costs disclosure</label><textarea id="network-costs" required bind:value={networkCosts}></textarea><button class="primary" disabled={busy || !config?.available}>Issue immutable quote</button></form>
        {#if selected}<form on:submit|preventDefault={() => run(recordEntry)}><h3>Accounting for selected trade</h3><label for="entry-kind">Entry</label><select id="entry-kind" bind:value={entryKind}>{#each ["fee_collected", "fee_compensation", "fee_refunded", "support_cost", "monitoring_cost", "exception_cost", "operator_minutes"] as kind}<option value={kind}>{kind.replaceAll("_", " ")}</option>{/each}</select><label for="entry-amount">Amount ({entryKind === "operator_minutes" ? "whole minutes" : "USDT"})</label><input id="entry-amount" required inputmode="decimal" bind:value={entryAmount} /><label for="entry-evidence">Evidence / collection reference</label><input id="entry-evidence" required bind:value={entryEvidence} /><button disabled={busy}>Record once</button></form>{/if}
      </div>
    </OtcPanel>
    {#if dashboard}
      <OtcPanel id="otc-monitoring" title="Internal monitoring" collapsible={false}>
        <div class="card"><div class="metrics"><div><span>Quote coverage</span><strong>{dashboard.quote_coverage.numerator}/{dashboard.quote_coverage.denominator} · {dashboard.quote_coverage.rate === null ? "N/A" : `${Math.round(dashboard.quote_coverage.rate * 100)}%`}</strong></div><div><span>On-time payout</span><strong>{dashboard.on_time_payout.numerator}/{dashboard.on_time_payout.denominator} · {dashboard.on_time_payout.rate === null ? "N/A" : `${Math.round(dashboard.on_time_payout.rate * 100)}%`}</strong></div><div><span>Open funded cases</span><strong>{dashboard.open_funded_cases.length}</strong></div><div><span>Collected / unpaid fees</span><strong>{decimal(dashboard.economics.collected_usdt_units, 6)} / {decimal(dashboard.economics.unpaid_usdt_units, 6)} USDT</strong></div></div>
          {#each dashboard.observers as observer}<p>{observer.chain}: {observer.paused || !observer.last_scan || Date.now() - Date.parse(observer.last_scan) > 60000 ? "UNHEALTHY" : "observing"} · last complete scan {observer.last_scan || "missing"}</p>{/each}
          <details><summary>Funnel, inventory, obligations and accounting evidence</summary><pre>{JSON.stringify({ funnel: dashboard.funnel, inventory: dashboard.inventory, obligations: dashboard.open_funded_cases, economics: dashboard.economics, fees: dashboard.fees, conversion: dashboard.conversion_7d, reporting: dashboard.reporting }, null, 2)}</pre></details>
          <h3>Incidents</h3><label for="incident-action">Action</label><select id="incident-action" bind:value={incidentAction}>{#each ["acknowledge", "update", "service_restored", "financially_resolved", "close"] as action}<option value={action}>{action.replaceAll("_", " ")}</option>{/each}</select><label for="incident-evidence">Checks, actions and evidence (JSON for P0/P1 closure)</label><textarea id="incident-evidence" bind:value={incidentEvidence}></textarea><label for="customer-update">Participant update</label><textarea id="customer-update" bind:value={customerUpdate}></textarea>
          {#each dashboard.incidents.filter((i: any) => !i.closed_at) as incident}<div class="attempt"><strong>{incident.severity} · {incident.owner}</strong><p class="address">{incident.key}</p><p><a href={incident.runbook}>Runbook</a> · Next update {incident.next_update_at}</p>{#if incident.trade_id}<button on:click={() => run(() => onSelect(incident.trade_id))}>Open affected trade</button>{/if}<button disabled={busy || !incidentEvidence.trim()} on:click={() => run(() => updateIncident(incident.key))}>Record {incidentAction.replaceAll("_", " ")}</button></div>{:else}<p class="muted">No open incidents. Missing observer telemetry is unhealthy.</p>{/each}
        </div>
      </OtcPanel>
    {/if}
  {/if}

</div>
