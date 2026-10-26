<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale } from "$lib/i18n";
  import { decimal, otcP2pNetwork, request, sendAttempt, type Attempt, type Config, type Session } from "$lib/otc/service";
  import OtcTransferReview from "./OtcTransferReview.svelte";

  interface DealTerms {
    direction: "buy" | "sell"; ever_amount: string; usdt_amount: string;
    ever_units: string; usdt_units: string;
    usdt_payer: string; usdt_payee: string; ever_payer: string; ever_payee: string;
  }
  interface Deal {
    id: string; offer_id: string; state: string; role: "owner" | "taker"; counterparty: string;
    terms: DealTerms; pay_by: string; payout_by: string | null; created_at: string; attempts: Attempt[];
  }

  export let config: Config | null = null;
  export let session: Session | null = null;
  export let locked = false;
  export let now = Date.now();
  export let onNotice: (message: string) => void = () => {};
  export let onError: (message: string) => void = () => {};
  export let onBusyChange: (busy: boolean) => void = () => {};

  let deals: Deal[] = [], review: { deal: Deal; attempt: Attempt; kind: string } | null = null;
  let busy = false, loadedActor = "", revision = 0;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: networkLabel = otcP2pNetwork().label;
  $: if (session && (session.actor || "") !== loadedActor) {
    loadedActor = session.actor || ""; deals = []; review = null;
    if (loadedActor) void refresh();
  }

  const walletOf = (chain: "ethereum" | "everscale") => session?.wallets.filter(item => item.chain === chain).pop()?.address?.toLowerCase() ?? "";
  const legOf = (kind: string) => (kind === "payment" ? "ethereum" : "everscale") as "ethereum" | "everscale";
  const payerOf = (deal: Deal, kind: string) => (kind === "payment" ? deal.terms.usdt_payer : deal.terms.ever_payer).toLowerCase();
  const payeeOf = (deal: Deal, kind: string) => (kind === "payment" ? deal.terms.usdt_payee : deal.terms.ever_payee);
  const assetOf = (kind: string) => (kind === "payment" ? "USDT" : "EVER");
  const stepOf = (deal: Deal) => deal.state === "payment_pending" ? "payment" : deal.state === "delivery_pending" ? "payout" : null;
  const openOf = (deal: Deal, kind: string) => deal.attempts.filter(attempt => attempt.kind === kind && !["verified", "failed"].includes(attempt.state)).pop() || null;
  const short = (value: string) => (value.length > 18 ? `${value.slice(0, 8)}…${value.slice(-6)}` : value);

  function stateLabel(deal: Deal): string {
    if (deal.state === "completed") return say("Completed", "Завершена");
    if (deal.state === "expired") return say("Expired", "Истекла");
    if (deal.state === "cancelled") return say("Cancelled", "Отменена");
    const kind = stepOf(deal) || "payment";
    return payerOf(deal, kind) === walletOf(legOf(kind))
      ? say(`You send ${assetOf(kind)}`, `Вам отправить ${assetOf(kind)}`)
      : say(`Waiting for ${assetOf(kind)}`, `Ожидается ${assetOf(kind)}`);
  }
  function deadline(deal: Deal): string {
    const until = deal.state === "payment_pending" ? deal.pay_by : deal.state === "delivery_pending" ? deal.payout_by : null;
    if (!until) return "";
    const stamp = Date.parse(until);
    const left = Math.round((stamp - now) / 60000);
    const time = new Date(stamp).toLocaleTimeString($locale, { hour: "2-digit", minute: "2-digit" });
    return left > 0 ? say(`until ${time} · ${left} min left`, `до ${time} · осталось ${left} мин`) : say(`until ${time}`, `до ${time}`);
  }
  function amount(deal: Deal, kind: string): string {
    return kind === "payment" ? decimal(deal.terms.usdt_units, 6) : decimal(deal.terms.ever_units, 9);
  }

  export async function refresh() {
    if (!config?.p2p_available || !session) return;
    const owner = session, current = ++revision;
    try {
      const page = await request<{ deals: Deal[] }>("/p2p/trades", owner.token);
      if (revision !== current || session?.token !== owner.token) return;
      deals = page.deals;
    } catch { /* A transient failure keeps the last known deals on screen. */ }
  }
  async function run(action: () => Promise<void>) {
    if (!session || busy || locked) return;
    busy = true; onBusyChange(true); onError("");
    try { await action(); }
    catch (e) { onError(e instanceof Error ? e.message : "Request failed"); }
    finally { busy = false; onBusyChange(false); await refresh(); }
  }
  function prepare(deal: Deal, kind: string) {
    return run(async () => {
      const result = await request<{ deal: Deal; attempt: Attempt }>(`/p2p/trades/${deal.id}/prepare`, session!.token, { kind }, crypto.randomUUID());
      deals = deals.map(item => item.id === result.deal.id ? { ...result.deal, attempts: [result.attempt, ...(item.attempts || []).filter(attempt => attempt.id !== result.attempt.id)] } : item);
      review = { deal: result.deal, attempt: result.attempt, kind };
    });
  }
  function confirmTransfer() {
    const current = review;
    review = null;
    return run(async () => {
      if (!current) return;
      await sendAttempt(session!.token, current.attempt);
      onNotice(say("Transfer submitted. Waiting for the network to confirm it.", "Перевод отправлен. Ожидаем подтверждение сети."));
    });
  }
  function cancel(deal: Deal) {
    return run(async () => {
      await request(`/p2p/trades/${deal.id}/cancel`, session!.token, {});
      onNotice(say("Deal cancelled and the offer is public again.", "Сделка отменена, оффер снова опубликован."));
    });
  }
  const timer = setInterval(() => { if (!busy && !locked && !review) void refresh(); }, 5000);
  onDestroy(() => { revision++; clearInterval(timer); });
</script>

{#if config?.p2p_available && session && deals.length}
  <div class="deals" data-testid="otc-p2p-deals">
    {#each deals as deal (deal.id)}
      {@const kind = stepOf(deal)}
      {@const open = kind ? openOf(deal, kind) : null}
      {@const mine = !!kind && payerOf(deal, kind) === walletOf(legOf(kind))}
      <article class="deal" class:settled={!kind} data-testid="otc-p2p-deal">
        <header>
          <strong>{deal.terms.ever_amount} EVER · {deal.terms.usdt_amount} USDT</strong>
          <span class="state" class:ok={deal.state === "completed"} class:bad={deal.state === "expired" || deal.state === "cancelled"}>{stateLabel(deal)}</span>
        </header>
        <p class="meta">{deal.role === "owner" ? say("Your offer", "Ваш оффер") : say("Accepted offer", "Принятый оффер")} · {say("Counterparty", "Контрагент")}: <code title={deal.counterparty}>{short(deal.counterparty)}</code>{#if deadline(deal)} · {deadline(deal)}{/if}</p>
        {#if kind && mine}
          <dl class="legs">
            <div><dt>{say("You send", "Вы отправляете")}</dt><dd>{amount(deal, kind)} {assetOf(kind)} · <code title={payerOf(deal, kind)}>{short(payerOf(deal, kind))}</code></dd></div>
            <div><dt>{say("Goes to", "Получатель")}</dt><dd><code title={payeeOf(deal, kind)}>{payeeOf(deal, kind)}</code></dd></div>
          </dl>
          {#if open && open.state !== "prepared"}
            <p class="pending" role="status">{say(`Saved ${assetOf(kind)} transfer ${short(open.reference || "")} is being confirmed on ${legOf(kind) === "ethereum" ? networkLabel : "Everscale"}.`, `Сохранённый перевод ${assetOf(kind)} ${short(open.reference || "")} подтверждается в сети ${legOf(kind) === "ethereum" ? networkLabel : "Everscale"}.`)} <button type="button" class="link" on:click={() => review = { deal, attempt: open, kind }}>{say("Open saved transfer", "Открыть сохранённый перевод")}</button></p>
          {:else}
            <div class="actions">
              <button type="button" class="primary" disabled={busy || locked} data-testid="otc-p2p-send" on:click={() => open ? review = { deal, attempt: open, kind } : prepare(deal, kind)}>{open ? say("Finish sending", "Завершить отправку") : say(`Send ${amount(deal, kind)} ${assetOf(kind)}`, `Отправить ${amount(deal, kind)} ${assetOf(kind)}`)}<span aria-hidden="true">↗</span></button>
              {#if deal.state === "payment_pending" && !open}<button type="button" class="ghost" disabled={busy || locked} on:click={() => cancel(deal)}>{say("Cancel deal", "Отменить сделку")}</button>{/if}
            </div>
          {/if}
        {:else if kind}
          <p class="pending" role="status">{say(`Waiting for the counterparty to send ${assetOf(kind)}.`, `Контрагент отправляет ${assetOf(kind)}.`)}</p>
        {/if}
      </article>
    {/each}
  </div>
{/if}

{#if review}<OtcTransferReview kind={review.kind} instructions={review.attempt.instructions} networkCosts={review.kind === "payment" ? say(`Token transfer on ${networkLabel}; gas is paid from your own wallet.`, `Перевод токена в сети ${networkLabel}; газ платится с вашего кошелька.`) : say("The EVER transfer includes the receiving transaction cost.", "Перевод EVER включает стоимость приёма средства.")} onClose={() => review = null} onConfirm={confirmTransfer} disabled={busy || locked} />{/if}

<style>
  .deals { display:grid; gap:12px; padding:var(--otc-panel-padding,20px); }
  .deal { display:grid; gap:10px; padding:14px 16px; border:1px solid var(--color-border); border-radius:10px; background:var(--color-paper); }
  .deal.settled { opacity:.72; }
  .deal header { display:flex; align-items:baseline; justify-content:space-between; gap:12px; }
  .deal header strong { font-family:var(--font-mono); font-size:13px; font-weight:600; }
  .state { flex:0 0 auto; padding:3px 8px; border:1px solid var(--color-border); border-radius:999px; color:var(--color-text-soft); font-size:11px; white-space:nowrap; }
  .state.ok { border-color:color-mix(in srgb, var(--color-good) 40%, transparent); background:var(--otc-buy-soft, rgba(36,115,56,.12)); color:var(--color-good); }
  .state.bad { color:var(--color-danger); }
  .meta { margin:0; color:var(--color-text-soft); font-size:11px; line-height:1.6; }
  .meta code { font-size:11px; }
  .legs { display:grid; gap:6px; margin:0; padding:10px 0 0; border-top:1px solid var(--color-border); }
  .legs > div { display:grid; grid-template-columns:minmax(0,120px) minmax(0,1fr); align-items:baseline; gap:10px; }
  .legs dt { color:var(--color-text-soft); font-size:11px; }
  .legs dd { margin:0; overflow-wrap:anywhere; font-family:var(--font-mono); font-size:11px; }
  .pending { margin:0; color:var(--color-text-soft); font-size:11px; line-height:1.6; }
  .actions { display:flex; flex-wrap:wrap; gap:8px; }
  .actions .primary { display:flex; align-items:center; justify-content:space-between; gap:10px; min-height:42px; padding:9px 14px; border:1px solid var(--color-accent-strong); border-radius:8px; background:var(--color-accent); color:#152016; font-size:12px; font-weight:700; }
  .actions .ghost { min-height:42px; padding:9px 12px; border:1px solid var(--color-border); border-radius:8px; color:var(--color-text-soft); font-size:12px; }
  .actions button:disabled { opacity:.45; cursor:default; }
  .link { padding:0; border:0; background:none; color:var(--color-accent-text); font-size:11px; text-decoration:underline; text-underline-offset:3px; }
  .deal :is(.primary,.ghost,.link):focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
</style>
