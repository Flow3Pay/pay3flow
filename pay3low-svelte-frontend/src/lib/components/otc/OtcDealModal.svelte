<script lang="ts">
  import { onMount, tick } from "svelte";
  import { locale } from "$lib/i18n";
  import { request, sendAttempt, type Attempt, type Config, type Session } from "$lib/otc/service";
  import { identiconDataUri } from "$lib/otc/identicon";
  import {
    p2pAction, p2pActive, p2pBothConfirmed, p2pKind, p2pLegActive, p2pLegDone,
    p2pLegFailed, p2pOpenAttempt, p2pShortAddress,
    type P2pDeal, type P2pLeg,
  } from "$lib/otc/p2p";

  export let config: Config | null = null;
  export let session: Session | null = null;
  export let locked = false;
  export let onError: (message: string) => void = () => {};
  export let onBusyChange: (value: boolean) => void = () => {};

  let dialog: HTMLDialogElement | undefined;
  let deals: P2pDeal[] = [];
  let selectedId = "";
  let busy = false;
  let error = "";
  let loadedActor = "";
  let configReady = false;
  let revision = 0;
  let autoOpen = true;
  let dismissed = new Set<string>();
  let autoTried = new Set<string>();

  const autoMarker = "pay3flow.otc.p2p.auto.";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  function markerKey(current: P2pDeal, kind: P2pLeg) {
    return `${autoMarker}${current.id}:${kind}`;
  }
  function autoBlocked(current: P2pDeal, kind: P2pLeg) {
    try { return sessionStorage.getItem(markerKey(current, kind)) === "1"; } catch { return false; }
  }
  function markAuto(current: P2pDeal, kind: P2pLeg, key: string) {
    autoTried.add(key);
    try { sessionStorage.setItem(markerKey(current, kind), "1"); } catch { /* private mode keeps the in-memory guard */ }
  }
  $: deal = deals.find(item => item.id === selectedId) || null;
  $: action = deal ? p2pAction(deal) : null;
  $: actionLabel = action === "retry" ? say("Retry", "Повторить") : say("Confirm", "Подтвердить");
  $: selfWallet = session?.wallets.find(wallet => wallet.chain === "everscale") || null;
  $: selfEver = deal?.my_ever || selfWallet?.address || "";
  $: counterpartyEver = deal?.counterparty_ever || "";
  $: selfAvatar = deal?.my_avatar || selfWallet?.avatar || (selfEver ? identiconDataUri(selfEver) : "");
  $: counterpartyAvatar = deal?.counterparty_avatar || (counterpartyEver ? identiconDataUri(counterpartyEver) : "");
  $: paymentDone = deal ? p2pLegDone(deal, "payment") : false;
  $: payoutDone = deal ? p2pLegDone(deal, "payout") : false;
  $: flowActive = deal ? p2pLegActive(deal, "payment") || p2pLegActive(deal, "payout") : false;

  $: {
    const actor = session?.actor || "";
    if (actor !== loadedActor) {
      loadedActor = actor;
      deals = [];
      selectedId = "";
      autoOpen = true;
      dismissed.clear();
      autoTried.clear();
      configReady = false;
      error = "";
      if (dialog?.open) dialog.close();
    }
    if (config?.p2p_available && session) {
      if (!configReady) { configReady = true; void refresh(); }
    } else {
      configReady = false;
    }
  }

  onMount(() => {
    const timer = setInterval(() => { if (!busy && !locked) void refresh(); }, 4500);
    return () => clearInterval(timer);
  });

  async function present() {
    await tick();
    if (dialog && !dialog.open) dialog.showModal();
  }

  function close() {
    if (selectedId) dismissed.add(selectedId);
    autoOpen = false;
    if (dialog?.open) dialog.close();
    selectedId = "";
    error = "";
  }

  function explain(reason: unknown) {
    const message = reason instanceof Error ? reason.message : say("Request failed", "Запрос не выполнен");
    if (/user rejected|user denied|rejected by user|denied by user|request rejected|user canceled|user cancelled|action_rejected|отклон/i.test(message)) {
      return say("You declined the transfer in your wallet.", "Вы отклонили перевод в кошельке.");
    }
    return message;
  }

  function applyDeal(updated: P2pDeal, previous?: P2pDeal | null) {
    const attempts = updated.attempts ?? previous?.attempts ?? [];
    const next = { ...updated, attempts };
    deals = deals.some(item => item.id === updated.id)
      ? deals.map(item => item.id === updated.id ? next : item)
      : [...deals, next];
  }

  function sortNewest(items: P2pDeal[]) {
    return [...items].sort((a, b) => Date.parse(b.created_at || "") - Date.parse(a.created_at || ""));
  }

  export async function refresh() {
    if (!config?.p2p_available || !session || busy) return;
    const owner = session;
    const current = ++revision;
    try {
      const page = await request<{ deals: P2pDeal[] }>("/p2p/trades", owner.token);
      if (revision !== current || session?.token !== owner.token) return;
      deals = page.deals || [];
      const active = deals.filter(p2pActive);
      if (selectedId && !active.some(item => item.id === selectedId)) {
        if (dialog?.open) dialog.close();
        selectedId = "";
        autoOpen = false;
      }
      if (!selectedId && autoOpen) {
        const candidate = sortNewest(active.filter(item => !dismissed.has(item.id)))[0];
        if (candidate) {
          selectedId = candidate.id;
          await present();
        }
      }
      const live = deals.find(item => item.id === selectedId);
      if (live) await maybeSettle(live);
    } catch (reason) {
      if (revision !== current) return;
      if (reason instanceof Error && !/fetch|network|failed to fetch/i.test(reason.message)) {
        error = explain(reason);
        onError(error);
      }
    }
  }

  export async function openDeal(id: string) {
    dismissed.delete(id);
    autoOpen = true;
    if (!deals.some(item => item.id === id)) await refresh();
    if (!deals.some(item => item.id === id)) return;
    selectedId = id;
    error = "";
    await present();
  }

  export async function openActive() {
    if (!session || !config?.p2p_available) return;
    autoOpen = true;
    await refresh();
    const next = sortNewest(deals.filter(p2pActive))[0];
    if (!next) return;
    dismissed.delete(next.id);
    selectedId = next.id;
    error = "";
    await present();
  }

  async function run(work: () => Promise<void>) {
    if (busy || locked || !session) return;
    busy = true;
    onBusyChange(true);
    error = "";
    try {
      await work();
    } catch (reason) {
      error = explain(reason);
      onError(error);
    } finally {
      busy = false;
      onBusyChange(false);
      await refresh();
    }
  }

  function settleKey(current: P2pDeal, kind: P2pLeg) {
    return `${current.id}:${kind}:${current.state}:${p2pOpenAttempt(current, kind)?.id || "new"}`;
  }

  async function sendLeg(current: P2pDeal, kind: P2pLeg) {
    if (!session) return;
    let attempt = p2pOpenAttempt(current, kind);
    if (!attempt) {
      // The prepare endpoint returns the deal with a single `attempt` field.
      const prepared = await request<P2pDeal & { attempt: Attempt }>(
        `/p2p/trades/${current.id}/prepare`,
        session.token,
        { kind },
        crypto.randomUUID(),
      );
      const withAttempt = {
        ...current,
        attempts: [...(current.attempts || []), prepared.attempt],
      };
      applyDeal(prepared, withAttempt);
      attempt = prepared.attempt;
    }
    if (attempt.state !== "prepared") return;
    // A previous wallet signature may have produced a reference whose POST was
    // lost. Reuse it instead of asking the wallet to sign a second transfer.
    const original = sessionStorage.getItem(`pay3flow.otc.reference.${attempt.id}`);
    if (original) {
      await request(attempt.instructions.submit_path || `/attempts/${attempt.id}/reference`, session.token, { reference: original });
      return;
    }
    await sendAttempt(session.token, attempt);
  }

  async function confirmDeal() {
    const current = deal;
    if (!current) return;
    await run(async () => {
      const confirmed = await request<P2pDeal>(`/p2p/trades/${current.id}/confirm`, session!.token, {});
      const merged = {
        ...confirmed,
        attempts: confirmed.attempts ?? current.attempts ?? [],
      };
      applyDeal(merged, current);
      if (!p2pBothConfirmed(merged)) return;
      const kind = p2pKind(merged);
      const next = p2pAction(merged);
      if (kind && (next === "send" || next === "retry")) {
        markAuto(merged, kind, settleKey(merged, kind));
        await sendLeg(merged, kind);
      }
    });
  }

  function settleDeal() {
    const current = deal;
    if (!current) return;
    const kind = p2pKind(current);
    if (!kind) return;
    markAuto(current, kind, settleKey(current, kind));
    return run(async () => { await sendLeg(current, kind); });
  }

  async function maybeSettle(current: P2pDeal) {
    if (selectedId !== current.id || !dialog?.open || busy || locked) return;
    if (!p2pBothConfirmed(current)) return;
    const kind = p2pKind(current);
    const next = p2pAction(current);
    if (!kind || next !== "send") return;
    const open = p2pOpenAttempt(current, kind);
    if (open && (open.state === "submitted" || open.state === "confirming")) return;
    const key = settleKey(current, kind);
    if (autoTried.has(key) || autoBlocked(current, kind)) return;
    markAuto(current, kind, key);
    await run(async () => { await sendLeg(current, kind); });
  }

  function act() {
    if (action === "confirm") void confirmDeal();
    else if (action) void settleDeal();
  }
</script>

<dialog
  bind:this={dialog}
  class="dealDialog"
  aria-label={say("P2P deal", "Сделка P2P")}
  data-testid="otc-p2p-modal"
  on:cancel|preventDefault={close}
  on:click={(event) => { if (event.target === dialog) close(); }}
>
  {#if deal}
    <div class="dealHead">
      <div class="dealAmount">
        <strong>{deal.terms.ever_amount}</strong><small>EVER</small>
        <span>·</span>
        <strong>{deal.terms.usdt_amount}</strong><small>USDT</small>
      </div>
      <button type="button" class="closeButton" aria-label={say("Close", "Закрыть")} on:click={close}>
        <svg width="18" height="18" viewBox="0 0 20 20" fill="none" aria-hidden="true"><path d="m5 5 10 10M15 5 5 15" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
      </button>
    </div>

    <div class="people">
      <div class="person self">
        <span class="avatarFrame">
          <img src={selfAvatar} alt="" data-testid="otc-p2p-avatar-self" />
          {#if deal.my_confirmed}
            <span class="confirmed" aria-hidden="true"><svg width="11" height="11" viewBox="0 0 20 20" fill="none"><path d="m5 10 3.2 3.2L15 6.5" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
          {/if}
        </span>
        <span class="personLabel">{say("You", "Вы")}</span>
      </div>
      <div class="person other" class:pending={!deal.counterparty_confirmed}>
        <span class="avatarFrame">
          <img src={counterpartyAvatar} alt="" data-testid="otc-p2p-avatar-counterparty" />
          {#if deal.counterparty_confirmed}
            <span class="confirmed" aria-hidden="true"><svg width="11" height="11" viewBox="0 0 20 20" fill="none"><path d="m5 10 3.2 3.2L15 6.5" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/></svg></span>
          {/if}
        </span>
        <span class="personLabel" title={counterpartyEver}>{p2pShortAddress(counterpartyEver)}</span>
      </div>
    </div>

    <div class="flow" class:active={flowActive} class:done={paymentDone && payoutDone}>
      <span class="token usdt" class:done={paymentDone} class:active={p2pLegActive(deal, "payment")} class:failed={p2pLegFailed(deal, "payment")}>
        <img src="/icons/assets/usdt.png" alt="" width="24" height="24" />
        <b>{deal.terms.usdt_amount}</b>
      </span>
      <span class="rail" aria-hidden="true"><span></span></span>
      <span class="token ever" class:done={payoutDone} class:active={p2pLegActive(deal, "payout")} class:failed={p2pLegFailed(deal, "payout")}>
        <img src="/icons/assets/ever.svg" alt="" width="24" height="24" />
        <b>{deal.terms.ever_amount}</b>
      </span>
    </div>

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <div class="action">
      {#if action}
        <button type="button" class="confirmButton" disabled={busy || locked} data-testid="otc-p2p-confirm" on:click={act}>
          {#if busy}<span class="spinner" aria-hidden="true"></span>{/if}
          <span>{actionLabel}</span>
        </button>
      {:else}
        <span class="buttonPlaceholder" aria-hidden="true"></span>
      {/if}
    </div>
  {/if}
</dialog>

<style>
  .dealDialog { position:fixed; top:0; right:0; bottom:0; left:0; margin:auto; width:min(430px,calc(100% - 32px)); height:fit-content; padding:0; overflow:hidden; border:1px solid var(--color-border-strong); border-radius:18px; background:var(--color-paper); color:var(--color-text); box-shadow:0 24px 80px #0005; }
  .dealDialog::backdrop { background:#10151bcc; backdrop-filter:blur(4px); }
  .dealHead { display:flex; align-items:center; justify-content:space-between; gap:12px; padding:18px 20px 0; }
  .dealAmount { display:flex; min-width:0; align-items:baseline; gap:5px; overflow:hidden; font-family:var(--font-mono); font-variant-numeric:tabular-nums; white-space:nowrap; }
  .dealAmount strong { overflow:hidden; text-overflow:ellipsis; font-size:15px; font-weight:650; }
  .dealAmount small { color:var(--color-text-faint); font-family:var(--font-sans,inherit); font-size:9px; font-weight:650; }
  .dealAmount span { color:var(--color-text-faint); font-size:11px; }
  .closeButton { display:grid; width:34px; height:34px; flex:0 0 auto; place-items:center; border:1px solid var(--color-border); border-radius:9px; color:var(--color-text-soft); }
  .people { display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); gap:16px; padding:26px 26px 0; }
  .person { display:grid; min-width:0; justify-items:start; gap:8px; }
  .person.other { justify-items:end; }
  .avatarFrame { position:relative; display:grid; width:64px; height:64px; place-items:center; border:1px solid var(--color-border); border-radius:18px; background:transparent; }
  .avatarFrame img { width:100%; height:100%; border-radius:inherit; object-fit:cover; }
  .person.other.pending .avatarFrame img { filter:grayscale(1); opacity:.28; }
  .confirmed { position:absolute; right:-5px; bottom:-5px; display:grid; width:19px; height:19px; place-items:center; border:2px solid var(--color-paper); border-radius:50%; background:var(--color-accent); color:#152016; }
  .personLabel { max-width:100%; overflow:hidden; color:var(--color-text-soft); font-family:var(--font-mono); font-size:10px; text-overflow:ellipsis; white-space:nowrap; }
  .person.self .personLabel { color:var(--color-text); font-family:var(--font-sans,inherit); font-weight:650; }
  .flow { display:grid; grid-template-columns:minmax(0,1fr) 46px minmax(0,1fr); align-items:center; padding:24px 26px 0; }
  .token { display:flex; min-width:0; min-height:42px; align-items:center; gap:7px; padding:7px 10px; border:1px solid var(--color-border); border-radius:12px; background:var(--color-panel); font-family:var(--font-mono); font-size:11px; }
  .token.ever { justify-content:flex-end; }
  .token img { width:24px; height:24px; flex:0 0 auto; }
  .token b { overflow:hidden; text-overflow:ellipsis; font-weight:650; white-space:nowrap; }
  .token.done { border-color:color-mix(in srgb, var(--color-good-ink) 38%, var(--color-border)); background:var(--otc-buy-soft); }
  .token.active { border-color:var(--color-accent-strong); box-shadow:inset 0 0 0 1px var(--color-accent-strong); }
  .token.failed { border-color:var(--color-danger); }
  .rail { position:relative; height:2px; background:var(--color-border); }
  .rail span { position:absolute; top:50%; left:50%; width:7px; height:7px; border-radius:50%; background:var(--color-text-faint); transform:translate(-50%,-50%); }
  .flow.active .rail span { background:var(--color-accent-strong); animation:travel 1.4s ease-in-out infinite; }
  .flow.done .rail span { background:var(--color-good-ink); }
  .error { margin:14px 26px 0; color:var(--color-danger); font-size:11px; line-height:1.45; text-align:center; }
  .action { padding:22px 26px 26px; }
  .confirmButton, .buttonPlaceholder { display:flex; width:100%; min-height:46px; align-items:center; justify-content:center; gap:8px; border-radius:11px; }
  .confirmButton { border:1px solid var(--color-accent-strong); background:var(--color-accent); color:#152016; font-size:13px; font-weight:750; }
  .confirmButton:disabled { opacity:.45; cursor:default; }
  .confirmButton:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  .buttonPlaceholder { display:block; }
  .spinner { width:13px; height:13px; border:2px solid #15201655; border-top-color:#152016; border-radius:50%; animation:spin .8s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }
  @keyframes travel { 0%,100% { transform:translate(-50%,-50%); } 50% { transform:translate(calc(-50% + 13px),-50%); } }
  @media (max-width:420px) { .people, .flow, .action { padding-right:18px; padding-left:18px; } .dealHead { padding-right:18px; padding-left:18px; } }
  @media (prefers-reduced-motion:reduce) { .flow.active .rail span, .spinner { animation:none; } }
</style>
