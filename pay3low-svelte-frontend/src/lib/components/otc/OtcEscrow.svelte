<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { request, decimal, type Trade, type Session, type Config } from "$lib/otc/service";
  import { validateEscrow, everSigningAvailable, signEscrow, sendEscrowCall, type Escrow, type EscrowCall, type EscrowProposal } from "$lib/otc/escrow";
  export let trade: Trade;
  export let session: Session;
  export let config: Config | null;
  export let now = Date.now();
  export let busy = false;
  export let onTransfer: (kind: string) => void;
  export let onBusyChange: (value: boolean) => void = () => {};
  export let onRefresh: () => Promise<void>;
  let escrow: Escrow | null = null, error = "", notice = "", working = false, pending: EscrowCall | null = null;
  let reason = "", disputeEvidence = "", reference = "", reviewing: EscrowProposal | null = null;
  let mounted = false, loading = false, revision = 0;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: owner = session.wallets.find(w => w.chain === "ethereum")?.address.toLowerCase() || "";
  $: trader = session.actor === trade.terms.customer_actor || session.actor === trade.terms.desk_actor;
  $: expires = escrow?.expires_at ? Date.parse(escrow.expires_at) : Infinity;
  $: expired = now >= expires;
  $: canSign = !busy && !working && !config?.demo && !trade.terms.demo;
  $: everSender = trade.terms.direction === "buy" ? session.desk : session.actor === trade.terms.customer_actor;
  $: everKind = trade.terms.direction === "buy" ? "payout" : "payment";
  $: walletPending = !!escrow?.calls.some(c => ["unknown", "submitted"].includes(c.state) && c.instructions.from.toLowerCase() === owner);
  $: canSendCall = canSign && !walletPending;
  $: pendingAvailable = !!pending && (pending.action === "deploy" ? now <= (escrow?.deployment.valid_until || 0) * 1000 : pending.action === "refund_expired" ? expired : !expired);
  $: consented = !!escrow?.consents.some(c => c.owner === owner);
  $: canSendEver = !!escrow && everSigningAvailable(escrow, now) && everSender
    && !["payout_pending", "payment_confirming", "review", "completed", "refunded"].includes(trade.state);
  async function load() {
    if (loading || !mounted) return;
    loading = true; const version = revision;
    try {
      const data = await request<Escrow>(`/trades/${trade.id}/escrow`, session.token);
      if (!mounted || version !== revision) return;
      validateEscrow(trade, data); escrow = data;
    } catch (e) { if (mounted && version === revision) error = e instanceof Error ? e.message : "Escrow unavailable"; }
    finally { loading = false; }
  }
  async function run(action: () => Promise<void>) {
    if (working || busy) return; working = true; onBusyChange(true); error = ""; notice = "";
    try { await action(); } catch (e) { error = e instanceof Error ? e.message : "Escrow action failed"; }
    finally { working = false; onBusyChange(false); await load(); try { await onRefresh(); } catch (e) { if (mounted) error = e instanceof Error ? e.message : "Status unavailable"; } }
  }
  async function sign(proposal?: EscrowProposal) {
    if (!escrow) return;
    await signEscrow(session, trade, escrow, owner, proposal); reviewing = null;
    notice = say("Approval saved. Funds move only after execution.", "Подпись сохранена. Средства переводятся только после выполнения.");
  }
  async function prepare(action: string, proposal_id?: string, retry = false) {
    if (walletPending) throw new Error("Track the original handoff. Do not send it again.");
    const key = `pay3flow.otc.escrow.key.${trade.id}.${session.actor}.${action}.${proposal_id || ""}`;
    const idempotency = !retry && sessionStorage.getItem(key) || crypto.randomUUID(); sessionStorage.setItem(key, idempotency);
    const call = await request<EscrowCall>(`/trades/${trade.id}/escrow/prepare`, session.token, { action, proposal_id: proposal_id || null }, idempotency);
    // A persisted reference/previous signing marker is recovery data, never permission to resend.
    if (call.state !== "unknown" || sessionStorage.getItem(`pay3flow.otc.escrow.sent.${call.id}`)) throw new Error("Track the original handoff. Do not send it again.");
    pending = call;
  }
  async function confirm() {
    const call = pending; pending = null; if (!call) return;
    sessionStorage.setItem(`pay3flow.otc.escrow.sent.${call.id}`, "unknown");
    await sendEscrowCall(session, trade.id, call);
    notice = say("Submitted. Waiting for finalized chain evidence.", "Отправлено. Ожидается подтверждённое событие блокчейна.");
  }
  async function propose(action: "release" | "refund_early", dispute = false) {
    let evidence: Record<string, unknown> = {};
    if (dispute) { evidence = JSON.parse(disputeEvidence); if (!reason.trim() || !evidence || Array.isArray(evidence) || typeof evidence !== "object" || !Object.keys(evidence).length) throw new Error("Enter a dispute reason and JSON evidence object."); }
    await request(`/trades/${trade.id}/escrow/proposals`, session.token, { action, dispute, reason: dispute ? reason : "", evidence }, crypto.randomUUID());
  }
  async function track(call: EscrowCall) {
    const original = reference.trim() || sessionStorage.getItem(`pay3flow.otc.escrow.reference.${call.id}`);
    if (!original) throw new Error("Enter the original transaction hash.");
    await request(`/trades/${trade.id}/escrow/reference`, session.token, { call_id: call.id, reference: original }, crypto.randomUUID()); reference = "";
  }
  function exportRecovery() {
    if (!escrow?.vault) return;
    const blob = new Blob([JSON.stringify({ chainId: 1, vault: escrow.vault, expiresAt: escrow.expires_at, refundRecipient: escrow.deployment.buyer, abi: [{ type: "function", name: "refundExpired", inputs: [], outputs: [], stateMutability: "nonpayable" }], command: `cast send ${escrow.vault} 'refundExpired()' --rpc-url "$ETHEREUM_RPC_URL" --account YOUR_LOCAL_KEYSTORE` }, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob), link = document.createElement("a"); link.href = url; link.download = "otc-escrow-recovery.json"; link.click(); URL.revokeObjectURL(url);
  }
  onMount(() => { mounted = true; void load(); const timer = setInterval(() => { if (!working) void load(); }, 5000); return () => { mounted = false; revision++; clearInterval(timer); }; });
</script>

<article class="escrow" data-testid="otc-escrow">
  <h3>{say("Four-hour multisig escrow", "Мультиподпись: эскроу на 4 часа")}</h3>
  <p>{say("Two of three owners approve settlement. USDT is held on Ethereum; EVER is transferred separately on Everscale. A USDT refund cannot reverse EVER delivery.", "Расчёт подтверждают двое из трёх владельцев. USDT хранится в Ethereum, EVER переводится отдельно в Everscale. Возврат USDT не отменяет перевод EVER.")}</p>
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if escrow}
    <dl>
      <dt>{say("Status", "Статус")}</dt><dd>{escrow.state}</dd>
      <dt>{say("EVER trade size", "Объём EVER")}</dt><dd>{decimal(escrow.deployment.ever_amount, 9)} / 100,000 EVER</dd>
      <dt>{say("USDT deposit", "Депозит USDT")}</dt><dd>{decimal(escrow.deployment.usdt_amount, 6)} USDT</dd>
      <dt>{say("Buyer / depositor", "Покупатель / отправитель")}</dt><dd>{escrow.deployment.buyer}</dd>
      <dt>{say("Seller / release recipient", "Продавец / получатель")}</dt><dd>{escrow.deployment.seller}</dd>
      <dt>{say("Arbitrator", "Арбитр")}</dt><dd>{escrow.policy.arbitrator}</dd>
      <dt>Safe</dt><dd>{escrow.safe || say("Awaiting deployment", "Ожидается создание")}</dd>
      <dt>{say("USDT vault", "Хранилище USDT")}</dt><dd>{escrow.vault || "—"}</dd>
      <dt>{say("Created on chain", "Создано в блокчейне")}</dt><dd>{escrow.created_at || "—"}</dd>
      <dt>{say("Refund available at", "Возврат доступен с")}</dt><dd>{escrow.expires_at || "—"}</dd>
    </dl>
    {#if !escrow.safe && escrow.state !== "expired"}
      <p>{say("Trader consent", "Согласие сторон")}: {escrow.consents.length}/2</p>
      <button disabled={!canSign || !trader || consented || now > escrow.deployment.valid_until * 1000} on:click={() => run(() => sign())}>{say("Sign deployment consent", "Подписать создание эскроу")}</button>
      <button disabled={!canSendCall || !trader || escrow.consents.length < 2 || now > escrow.deployment.valid_until * 1000} on:click={() => run(() => prepare("deploy"))}>{say("Review deployment", "Проверить создание")}</button>
    {/if}
    {#if escrow.state === "created" && !expired && owner === escrow.deployment.buyer}
      <p>{say("Approve only the exact deposit. If an existing USDT allowance is nonzero, reset it first.", "Разрешите только точную сумму. Если разрешение USDT уже существует, сначала обнулите его.")}</p>
      <button disabled={!canSendCall} on:click={() => run(() => prepare("approve_reset"))}>{say("Review allowance reset", "Проверить обнуление")}</button>
      <button disabled={!canSendCall} on:click={() => run(() => prepare("approve"))}>{say("Review exact allowance", "Проверить разрешение")}</button>
      <button disabled={!canSendCall} on:click={() => run(() => prepare("fund"))}>{say("Review USDT deposit", "Проверить депозит USDT")}</button>
    {/if}
    {#if escrow.state === "funded" && !expired}
      {#if everSender}<button disabled={!canSign || !canSendEver} on:click={() => onTransfer(everKind)}>{say("Review and send EVER", "Проверить и отправить EVER")}</button>{/if}
      {#if expires - now <= 1800000}<p>{say("The final 30 minutes are reserved for settlement. New EVER signing is closed.", "Последние 30 минут отведены на расчёт. Новые переводы EVER закрыты.")}</p>{/if}
      <button disabled={!canSign} on:click={() => run(() => propose("release"))}>{say("Propose release after EVER delivery", "Предложить выплату после EVER")}</button>
      <button disabled={!canSign} on:click={() => run(() => propose("refund_early"))}>{say("Propose early refund", "Предложить досрочный возврат")}</button>
      <details><summary>{say("Dispute settlement", "Разрешение спора")}</summary>
        <label>{say("Reason", "Причина")}<input bind:value={reason} /></label>
        <label>{say("Evidence (JSON)", "Доказательства (JSON)")}<textarea bind:value={disputeEvidence} placeholder={'{"reference":"..."}'}></textarea></label>
        <button disabled={!canSign || !reason.trim() || !disputeEvidence.trim()} on:click={() => run(() => propose("release", true))}>{say("Propose disputed release", "Предложить выплату по спору")}</button>
        <button disabled={!canSign || !reason.trim() || !disputeEvidence.trim()} on:click={() => run(() => propose("refund_early", true))}>{say("Propose disputed refund", "Предложить возврат по спору")}</button>
      </details>
      {#each escrow.proposals as proposal}<div class="proposal">
        <strong>{proposal.action === "release" ? say("Release to seller", "Выплата продавцу") : say("Refund to depositor", "Возврат отправителю")} · {proposal.approvals.length}/2</strong>
        {#if proposal.dispute}<p>{say("Dispute", "Спор")}: {proposal.reason}</p><pre>{JSON.stringify(proposal.evidence, null, 2)}</pre>{/if}
        <p>{proposal.hash}</p>
        <button disabled={!canSign || proposal.approvals.some(a => a.owner === owner)} on:click={() => reviewing = proposal}>{say("Review approval", "Проверить подпись")}</button>
        <button disabled={!canSendCall || proposal.approvals.length < 2} on:click={() => run(() => prepare("execute", proposal.id))}>{say("Review execution", "Проверить выполнение")}</button>
      </div>{/each}
    {/if}
    {#if expired && ["created", "funded", "review"].includes(escrow.state)}
      <p>{say("Refund is claimable without owner approvals. Someone must submit a transaction and pay Ethereum gas.", "Возврат доступен без подписей владельцев. Нужно отправить транзакцию и оплатить газ Ethereum.")}</p>
      <button disabled={!canSendCall} on:click={() => run(() => prepare("refund_expired"))}>{say("Review timeout refund", "Проверить возврат по сроку")}</button>
    {/if}
    {#if escrow.vault}<button on:click={exportRecovery}>{say("Download direct refund instructions", "Скачать инструкции возврата")}</button>{/if}
    {#if escrow.evidence.length}<details>
      <summary>{say("Chain receipts", "Квитанции блокчейна")}</summary>
      {#each escrow.evidence as proof}<div class="call">
        <strong>{proof.kind} · {proof.valid ? say("Finalized", "Подтверждено") : say("Invalidated — review required", "Недействительно — нужна проверка")}</strong>
        {#if proof.evidence}<pre>{JSON.stringify(proof.evidence, null, 2)}</pre>{/if}
      </div>{/each}
    </details>{/if}
    {#if reviewing}<div class="review" role="region" aria-label="Review multisig approval">
      <h4>{say("Review multisig approval", "Проверить мультиподпись")}</h4>
      <p>{reviewing.action === "release" ? escrow.deployment.seller : escrow.deployment.buyer} · {decimal(escrow.deployment.usdt_amount, 6)} USDT</p>
      <p>{reviewing.dispute ? reviewing.reason : say("Agreed settlement", "Согласованный расчёт")}</p>
      <button disabled={!canSign || expired} on:click={() => run(() => sign(reviewing!))}>{say("Sign this approval", "Подписать")}</button>
      <button disabled={working} on:click={() => reviewing = null}>{say("Close", "Закрыть")}</button>
    </div>{/if}
    {#if pending}<div class="review" role="region" aria-label="Review escrow transaction">
      <h4>{say("Review escrow transaction", "Проверить транзакцию эскроу")}</h4><p>{pending.action}</p>
      <p>{pending.instructions.from} → {pending.instructions.to}</p><pre>{pending.instructions.data}</pre>
      <p>{say("Ethereum gas is paid by the executing wallet.", "Газ Ethereum оплачивает выполняющий кошелёк.")}</p>
      <button disabled={!canSign || !pendingAvailable} on:click={() => run(confirm)}>{say("Confirm in wallet", "Подтвердить в кошельке")}</button>
      <button disabled={working} on:click={() => pending = null}>{say("Close review", "Закрыть проверку")}</button>
    </div>{/if}
    {#each escrow.calls as call}<div class="call"><strong>{call.action} · {call.state}</strong><p>{call.reference || say("Original result unknown. Another send from this wallet is blocked.", "Результат неизвестен. Повторная отправка из кошелька заблокирована.")}</p>
      {#if !call.reference && call.actor === session.actor}<label>{say("Original transaction hash", "Хеш исходной транзакции")}<input bind:value={reference} /></label><button disabled={working || busy} on:click={() => run(() => track(call))}>{say("Track original transaction", "Отследить транзакцию")}</button>{/if}
      {#if call.state === "failed" && call.actor === session.actor}<button disabled={!canSendCall} on:click={() => run(() => prepare(call.action, call.proposal_id || undefined, true))}>{say("Review retry after confirmed failure", "Проверить повтор после подтверждённой ошибки")}</button>{/if}
    </div>{/each}
  {:else}<p>{say("Loading escrow…", "Загрузка эскроу…")}</p>{/if}
</article>

<style>
  .escrow { padding: 18px 0; display: grid; gap: 12px; min-width: 0; }
  h3,h4,p { margin: 0; } p { line-height: 1.6; overflow-wrap: anywhere; }
  dl { display: grid; grid-template-columns: minmax(110px, .6fr) minmax(0, 1fr); gap: 8px 16px; font-size: 12px; }
  dt { color: var(--color-text-soft); } dd { margin: 0; overflow-wrap: anywhere; font-family: var(--font-mono); }
  button { min-height: 44px; padding: 10px 14px; border: 1px solid var(--color-border); border-radius: 7px; text-align: left; }
  button:disabled { opacity: .5; cursor: not-allowed; } .error { color: var(--color-danger); }
  .proposal,.review,.call { border: 1px solid var(--color-border); border-radius: 8px; padding: 14px; display: grid; gap: 10px; min-width: 0; }
  .review { background: var(--color-panel-soft); } label { display: grid; gap: 6px; margin: 8px 0; }
  input,textarea { min-width: 0; width: 100%; padding: 10px; border: 1px solid var(--color-border); border-radius: 5px; background: var(--color-paper); }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 11px; max-height: 180px; overflow: auto; }
  @media (max-width: 480px) { dl { grid-template-columns: 1fr; gap: 5px; } dd { margin-bottom: 8px; } }
</style>
