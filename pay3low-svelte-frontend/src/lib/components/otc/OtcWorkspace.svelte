<script lang="ts">
  import { onMount } from "svelte";
  import OtcPanel from "./OtcPanel.svelte";
  import { request, bindWallet, sendAttempt, decimal, rate, type Config, type Session, type Proposal, type Trade, type Rfq, type Attempt, type Direction } from "$lib/otc/service";
  let config: Config | null = null, session: Session | null = null;
  let actor = "", credential = "", input = "100", direction: Direction = "buy";
  let listings: Proposal[] = [], trades: Trade[] = [], rfqs: Rfq[] = [];
  let receipts: { chain: string; leg: { amount: string }; included_at: string }[] = [];
  let selected: Trade | null = null, attempts: Attempt[] = [], updates: { message: string; at: string }[] = [];
  let error = "", notice = "", busy = false, bound: string[] = [];
  let rfqKey = "", quoteOutput = "", networkCosts = "Sender pays sending gas; receiving costs disclosed separately.", everCost = "0", quoteRfq = "";
  let dashboard: Record<string, any> | null = null, reference = "";
  let incidentAction = "acknowledge", incidentEvidence = "", customerUpdate = "";
  let entryKind = "fee_collected", entryAmount = "", entryEvidence = "";
  $: listing = listings.find(p => p.publishes.resourceConformsTo === (direction === "buy" ? config?.ever_resource : config?.usdt_resource));
  $: unresolvedBooking = trades.some(t => t.state === "booking_pending");
  $: selectedQuoted = selected?.state === "quoted";
  const sessionKey = "pay3flow.otc.session.v1";
  async function run(action: () => Promise<void>) {
    if (busy) return;
    busy = true; error = ""; notice = "";
    try { await action(); } catch (e) { error = e instanceof Error ? e.message : "Request failed"; } finally { busy = false; }
  }
  async function refresh() {
    config = await request<Config>("/config");
    listings = await request<Proposal[]>("/listings");
    if (!session) return;
    const page = await request<{ trades: Trade[]; rfqs: Rfq[] }>("/trades", session.token);
    trades = page.trades; rfqs = page.rfqs;
    if (selected) await selectTrade(selected.id);
    if (session.desk) dashboard = await request("/desk/dashboard", session.token);
  }
  async function selectTrade(id: string) {
    if (!session) return;
    const details = await request<{ trade: Trade; attempts: Attempt[]; customer_updates?: { message: string; at: string }[][]; evidence?: typeof receipts }>(`/trades/${id}`, session.token);
    selected = details.trade; attempts = details.attempts; updates = details.customer_updates?.flat() || []; receipts = details.evidence || [];
    sessionStorage.setItem("pay3flow.otc.selected", id);
  }
  function edited() { rfqKey = ""; if (selectedQuoted) { selected = null; attempts = []; sessionStorage.removeItem("pay3flow.otc.selected"); } }
  async function link() {
    session = await request<Session>("/session", "", { actor, credential }); credential = "";
    sessionStorage.setItem(sessionKey, JSON.stringify(session));
    await refresh();
  }
  async function verify(chain: "ethereum" | "everscale") {
    if (!session) return;
    const proof = await bindWallet(session.token, chain);
    bound = [...bound.filter(b => !b.startsWith(`${chain}:`)), `${chain}: ${proof.address}`];
    notice = `${chain === "ethereum" ? "Ethereum" : "Everscale"} wallet ownership verified.`;
  }
  async function rfq() {
    if (!session || !listing) return;
    rfqKey ||= crypto.randomUUID();
    const result = await request<{ id: string }>("/rfqs", session.token, { direction, input, listing_id: listing.id }, rfqKey);
    notice = `Quote requested. Reference: ${result.id}`; await refresh();
  }
  async function apply() { if (!session || !selected) return; await request(`/trades/${selected.id}/apply`, session.token, {}); await refresh(); }
  async function decide(accept: boolean) { if (!session || !selected) return; await request(`/desk/trades/${selected.id}/decision`, session.token, { accept }); await refresh(); }
  async function transfer(kind: string) {
    if (!session || !selected) return;
    const attempt = await request<Attempt>(`/trades/${selected.id}/prepare`, session.token, { kind }, crypto.randomUUID());
    // Update the UI before opening the signer; refresh never offers another send.
    await selectTrade(selected.id);
    try { await sendAttempt(session.token, attempt); notice = "Transfer submitted. Waiting for independently verified receipt."; }
    catch (e) { notice = "The saved attempt remains unresolved. Track its original reference; do not send again."; throw e; }
    finally { await refresh(); }
  }
  async function track(attempt: Attempt) {
    if (!session) return;
    const original = reference || sessionStorage.getItem(`pay3flow.otc.reference.${attempt.id}`);
    if (!original) throw new Error("Enter the original transaction reference.");
    await request(`/attempts/${attempt.id}/reference`, session.token, { reference: original }); reference = ""; await refresh();
  }
  async function makeQuote() {
    if (!session) return;
    await request("/desk/quotes", session.token, { rfq_id: quoteRfq, output: quoteOutput, network_costs: networkCosts, ever_receiving_cost: everCost }); await refresh();
  }
  async function changePause(paused: boolean) { if (!session) return; await request("/desk/control", session.token, { paused }); await refresh(); }
  function parseEvidence(value: string) { try { return JSON.parse(value); } catch { return { note: value }; } }
  async function updateIncident(key: string) {
    if (!session) return;
    await request(`/desk/incidents/${encodeURIComponent(key)}`, session.token, { action: incidentAction, evidence: parseEvidence(incidentEvidence), customer_update: customerUpdate || null }); await refresh();
  }
  async function recordEntry() {
    if (!session || !selected) return;
    await request("/desk/accounting", session.token, { trade_id: selected.id, kind: entryKind, amount: entryAmount, reference: entryEvidence, idempotency_key: crypto.randomUUID() }); await refresh();
  }
  async function disconnect() { if (session) await request("/session/revoke", session.token, {}); session = null; selected = null; attempts = []; bound = []; sessionStorage.removeItem(sessionKey); }
  onMount(() => {
    let stopped = false, polling = false;
    void run(async () => {
      try { session = JSON.parse(sessionStorage.getItem(sessionKey) || "null"); } catch { session = null; }
      await refresh(); const id = sessionStorage.getItem("pay3flow.otc.selected"); if (id && session) await selectTrade(id);
    });
    const timer = setInterval(async () => {
      if (stopped || busy || polling) return; polling = true;
      try { await refresh(); } catch (e) { error = e instanceof Error ? e.message : "Status unavailable"; } finally { polling = false; }
    }, 5000);
    return () => { stopped = true; clearInterval(timer); };
  });
</script>

<div class="workspace">
  <header class="heading"><div><p class="eyebrow">PAY3FLOW OTC</p><h1>EVER / USDT</h1><p>Native EVER · Everscale ↔ USDT · Ethereum</p></div><span class="badge">{config?.demo ? "Simulation" : config?.available ? "Desk available" : "Route paused"}</span></header>
  <p class="risk">{config?.counterparty_risk ?? "Two separate transfers. The desk must deliver or refund; there is no atomic swap or automatic rollback."}</p>
  {#if error}<p class="message error" role="alert">{error}</p>{/if}
  {#if notice}<p class="message" role="status">{notice}</p>{/if}
  <div class="columns">
    <OtcPanel id="otc-exchange" title="Exchange" collapsible={false}>
      <div class="card">
        <div class="sides" role="group" aria-label="Trade direction"><button class:active={direction === "buy"} aria-pressed={direction === "buy"} on:click={() => { direction = "buy"; edited(); }}>Buy EVER</button><button class:active={direction === "sell"} aria-pressed={direction === "sell"} on:click={() => { direction = "sell"; edited(); }}>Sell EVER</button></div>
        <label for="otc-input">You send · {direction === "buy" ? "USDT on Ethereum" : "EVER on Everscale"}</label>
        <input id="otc-input" inputmode="decimal" bind:value={input} on:input={edited} autocomplete="off" />
        <p class="outputLabel">You receive · {direction === "buy" ? "EVER on Everscale" : "USDT on Ethereum"}</p>
        <div class="output">{selectedQuoted && selected?.terms.direction === direction && selected?.terms.input === input ? selected.terms.output : "Awaiting fixed quote"}</div>
        <p class="muted">{config?.desk_name || "One onboarded desk"} · Desk pays max(0.25%, 5 USDT) after completion.</p>
        <button class="primary" disabled={busy || !session || !listing || !config?.available || bound.length !== 2 || unresolvedBooking} on:click={() => run(rfq)}>Request exact quote</button>
        {#if unresolvedBooking}<p class="muted">The original booking is awaiting its decision. Another selection is blocked.</p>{/if}
        <p class="muted">Existing accepted trades retain their booked terms. {config?.support_owner ? `Support: ${config.support_owner}` : "Live deposits are disabled until the release gate is configured."}</p>
      </div>
    </OtcPanel>
    <OtcPanel id="otc-identity" title="Actor and wallets" collapsible={false}>
      <div class="card">
        {#if session}
          <p class="address">{session.actor}</p><p>{session.desk ? "Desk operator" : "Customer"}</p>
          <button disabled={busy} on:click={() => run(() => verify("ethereum"))}>Verify Ethereum wallet</button><button disabled={busy} on:click={() => run(() => verify("everscale"))}>Verify EVER Wallet</button>
          {#each bound as wallet}<p class="address muted">{wallet}</p>{/each}
          <button disabled={busy} on:click={() => run(disconnect)}>Revoke session and relay access</button>
        {:else}
          <form on:submit|preventDefault={() => run(link)}><label for="otc-actor">Existing LF actor</label><input id="otc-actor" type="url" required bind:value={actor} placeholder="https://relay.example/actors/customer" /><label for="otc-credential">LF credential</label><input id="otc-credential" type="password" required bind:value={credential} autocomplete="off" /><button class="primary" disabled={busy || !config?.enabled}>Link actor</button></form>
        {/if}
      </div>
    </OtcPanel>
  </div>
  <OtcPanel id="otc-listings" title="OTC listings" subtitle="Original marketplace Proposals" collapsible={false}>
    <div class="card listings">{#each listings as p}<div><strong>{p.name || "Desk listing"}</strong><a class="address" href={p.id} target="_blank" rel="noreferrer">{p.id}</a><p class="muted">Whole trades · private exact quote · customer pays first</p></div>{:else}<p class="muted">No current public desk Proposals.</p>{/each}</div>
  </OtcPanel>
  <OtcPanel id="otc-trades" title="My trades" collapsible={false}>
    <div class="card"><div class="tradeList">{#each trades as trade}<button class:selected={selected?.id === trade.id} on:click={() => run(() => selectTrade(trade.id))}><strong>{trade.terms.direction === "buy" ? "Buy" : "Sell"} EVER</strong><span>{trade.terms.input} → {trade.terms.output}</span><span>{trade.state.replaceAll("_", " ")}</span></button>{:else}<p class="muted">Quotes and accepted trades appear here. Refresh restores the saved trade.</p>{/each}</div>
      {#if selected}
        <article class="detail"><h3>{selected.terms.direction === "buy" ? "Buy" : "Sell"} EVER · {selected.state.replaceAll("_", " ")}</h3><p class="address">{selected.id}</p>
          {#each updates as update}<p class="message">{update.at} · {update.message}</p>{/each}
          <dl><dt>Customer input</dt><dd>{selected.terms.input} {selected.terms.direction === "buy" ? "USDT · Ethereum" : "EVER · Everscale"}</dd><dt>Fixed net output</dt><dd>{selected.terms.output} {selected.terms.direction === "buy" ? "EVER · Everscale" : "USDT · Ethereum"}</dd><dt>Rate</dt><dd>{rate(selected.terms)} USDT per EVER</dd><dt>Desk completion fee</dt><dd>{decimal(selected.terms.fee_policy.fee_units, 6)} USDT · paid by desk</dd><dt>Network costs</dt><dd>{selected.terms.network_costs} Receiving allowance: {decimal(selected.terms.ever_receiving_cost_units, 9)} EVER.</dd><dt>Quote expires</dt><dd>{selected.terms.quote_by}</dd><dt>Payment inclusion deadline</dt><dd>{selected.terms.pay_by}</dd><dt>Payout deadline</dt><dd>{selected.terms.payout_by}</dd><dt>Refund policy</dt><dd>{selected.terms.refund_policy}</dd></dl>
          {#if !session?.desk && selected.state === "quoted"}<button class="primary" disabled={busy || unresolvedBooking || Date.now() > Date.parse(selected.terms.quote_by)} on:click={() => run(apply)}>Accept fixed quote</button>{/if}
          {#if session?.desk && selected.state === "booking_pending"}<button disabled={busy} on:click={() => run(() => decide(true))}>Reserve and accept</button><button disabled={busy} on:click={() => run(() => decide(false))}>Reject offer</button>{/if}
          {#if !session?.desk && selected.state === "accepted"}<button class="primary" disabled={busy || config?.demo || selected.terms.demo || Date.now() > Date.parse(selected.terms.pay_by)} on:click={() => run(() => transfer("payment"))}>Review and send customer payment</button>{/if}
          {#if session?.desk && selected.state === "funded"}<button class="primary" disabled={busy || config?.demo || selected.terms.demo} on:click={() => run(() => transfer("payout"))}>Review and send desk payout</button>{/if}
          {#if session?.desk && ["review", "funded"].includes(selected.state) && !attempts.some(a => ["payout", "refund"].includes(a.kind) && a.state !== "failed")}<button disabled={busy || config?.demo || selected.terms.demo} on:click={() => run(() => transfer("refund"))}>Review validated refund</button>{/if}
          {#each receipts as receipt}<p class="muted">Verified chain receipt: {decimal(receipt.leg.amount, receipt.chain === "ethereum" ? 6 : 9)} {receipt.chain === "ethereum" ? "USDT" : "EVER"} · {receipt.included_at}</p>{/each}
          {#each attempts as attempt}<div class="attempt"><strong>{attempt.kind} · {attempt.state}</strong><p class="address">{attempt.instructions.leg.sender} → {attempt.instructions.leg.recipient}</p><p>{attempt.instructions.leg.amount} base units net · {attempt.instructions.leg.chain}</p><p class="address">{attempt.reference || "Original result unknown. Another send is blocked."}</p>{#if attempt.state === "failed"}<label for={`retry-${attempt.id}`}>Failure review evidence</label><input id={`retry-${attempt.id}`} bind:value={incidentEvidence} /><button disabled={busy || !incidentEvidence.trim()} on:click={() => run(async () => { await request(`/attempts/${attempt.id}/retry`, session!.token, { evidence: { note: incidentEvidence } }); await refresh(); })}>Authorize retry after verified failure</button>{/if}{#if !attempt.reference && (attempt.kind === "payment" ? !session?.desk : session?.desk)}<label for={`reference-${attempt.id}`}>Original transaction hash</label><input id={`reference-${attempt.id}`} bind:value={reference} /><button disabled={busy} on:click={() => run(() => track(attempt))}>Track original transfer</button>{/if}</div>{/each}
        </article>
      {/if}
    </div>
  </OtcPanel>
  {#if session?.desk}
    <OtcPanel id="otc-desk" title="Desk console" collapsible={false}>
      <div class="card"><div class="actions"><button disabled={busy} on:click={() => run(() => changePause(true))}>Pause bookings</button><button disabled={busy} on:click={() => run(() => changePause(false))}>Resume healthy route</button><button disabled={busy} on:click={() => run(async () => { await request("/desk/listings", session!.token, {}); await refresh(); })}>Publish desk Proposals</button></div>
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
          {#each dashboard.incidents.filter((i: any) => !i.closed_at) as incident}<div class="attempt"><strong>{incident.severity} · {incident.owner}</strong><p class="address">{incident.key}</p><p><a href={incident.runbook}>Runbook</a> · Next update {incident.next_update_at}</p>{#if incident.trade_id}<button on:click={() => run(() => selectTrade(incident.trade_id))}>Open affected trade</button>{/if}<button disabled={busy || !incidentEvidence.trim()} on:click={() => run(() => updateIncident(incident.key))}>Record {incidentAction.replaceAll("_", " ")}</button></div>{:else}<p class="muted">No open incidents. Missing observer telemetry is unhealthy.</p>{/each}
        </div>
      </OtcPanel>
    {/if}
  {/if}
</div>

<style>
  .workspace { max-width: 1180px; margin: 0 auto; padding: 28px 20px 70px; display: grid; gap: 18px; color: var(--color-text); }
  .heading { display: flex; justify-content: space-between; align-items: center; gap: 15px; } h1 { font-size: 32px; margin: 4px 0; } h3 { margin: 12px 0; font-size: 17px; }
  .eyebrow { font-size: 11px; letter-spacing: .15em; } .badge { border: 1px solid var(--color-border-strong); padding: 8px 12px; border-radius: 20px; font-size: 12px; white-space: nowrap; }
  .risk, .muted { color: var(--color-text-soft); font-size: 13px; line-height: 1.6; } .risk { max-width: 800px; }
  .columns { display: grid; grid-template-columns: 1.2fr 1fr; gap: 18px; } .card { padding: 15px 19px 22px; min-width: 0; }
  form { display: grid; gap: 10px; margin-bottom: 20px; } label, .outputLabel { display: block; font-size: 13px; margin: 12px 0 6px; }
  input, select, textarea { width: 100%; min-width: 0; box-sizing: border-box; border: 1px solid var(--color-border-strong); background: var(--exchange-card-bg); color: inherit; border-radius: 8px; padding: 12px; font: inherit; } textarea { min-height: 80px; }
  #otc-input { font-size: 30px; } .output { font-size: 23px; padding: 18px 0; }
  button { border: 1px solid var(--color-border-strong); background: var(--color-panel); color: inherit; padding: 11px 15px; margin: 4px 4px 4px 0; border-radius: 8px; min-height: 44px; cursor: pointer; font-size: 13px; }
  button:disabled { opacity: .45; cursor: default; } button.primary { background: #b9ee50; color: #151b0b; font-weight: 650; } .primary { width: 100%; } .sides { display: flex; } .sides button { flex: 1; } .active { border-color: #8caf40; } button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible { outline: 2px solid #8caf40; outline-offset: 3px; }
  .address { overflow-wrap: anywhere; font-size: 12px; font-family: var(--font-mono); line-height: 1.6; } a { text-decoration: underline; }
  .listings { display: flex; flex-wrap: wrap; gap: 20px; } .listings > div { flex: 1; min-width: 200px; } .listings a { display: block; }
  .tradeList { display: grid; gap: 7px; } .tradeList button { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 8px; text-align: left; } .selected { border-color: #8caf40; }
  .detail { margin-top: 20px; border-top: 1px solid var(--color-border-strong); padding-top: 15px; } dl { display: grid; grid-template-columns: 160px 1fr; gap: 10px; margin: 15px 0; font-size: 13px; } dt { color: var(--color-text-soft); } dd { margin: 0; overflow-wrap: anywhere; }
  .attempt { border: 1px solid var(--color-border-strong); border-radius: 8px; padding: 14px; margin-top: 12px; } .metrics { display: grid; grid-template-columns: repeat(2, 1fr); gap: 18px; } .metrics span, .metrics strong { display: block; font-size: 13px; }
  .message { padding: 14px; background: var(--color-panel); border: 1px solid var(--color-border-strong); border-radius: 8px; overflow-wrap: anywhere; } .error { border-color: #b65344; } pre { overflow: auto; font-size: 12px; max-height: 350px; } summary { cursor: pointer; padding: 14px 0; font-size: 13px; }
  @media(max-width: 700px) { .columns { grid-template-columns: 1fr; } .workspace { padding: 20px 10px 50px; } .heading { flex-wrap: wrap; } dl { grid-template-columns: 1fr; gap: 5px; } dd { margin-bottom: 8px; } .metrics { grid-template-columns: 1fr; } .card { padding-inline: 14px; } }
</style>
