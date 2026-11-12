<script lang="ts">
  import { onMount, tick } from "svelte";
  import { locale } from "$lib/i18n";
  import { otcP2pNetwork, request, sendAttempt, walletNetwork, type Attempt, type Config, type Session } from "$lib/otc/service";
  import { connectWallet } from "$lib/wallet-session";
  import type { RouteExecution } from "$lib/exchange";
  import { identiconDataUri } from "$lib/otc/identicon";
  import {
    p2pAction, p2pActive, p2pBothConfirmed, p2pCompatibleSources, p2pDirectAssetMatch, p2pDirectDirectionAllowed, p2pKind, p2pLegActive, p2pLegAsset, p2pLegDone,
    p2pFeeStatus, p2pFormatUnits, p2pLegFailed, p2pOpenAttempt, p2pOverdue, p2pShortAddress, p2pViewerLegs,
    type P2pDeal, type P2pLeg,
  } from "$lib/otc/p2p";

  export let config: Config | null = null;
  export let session: Session | null = null;
  export let locked = false;
  export let onError: (message: string) => void = () => {};
  export let onBusyChange: (value: boolean) => void = () => {};
  export let onActiveCount: (count: number) => void = () => {};
  export let onActiveDeals: (deals: P2pDeal[]) => void = () => {};

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
  let reportedActive = -1;
  let reportedActiveIds = "";
  type RouteAsset = { asset: string; symbol: "USDT" | "USDC"; network: string; asset_id: string; decimals: number; contract_address: string | null; destination_asset_ids: string[] };
  type DestinationAsset = Omit<RouteAsset, "destination_asset_ids">;
  let sourceAssets: RouteAsset[] = [];
  let destinationAssets: DestinationAsset[] = [];
  let selectedSourceAsset = "";
  let routePickerOpen = false;
  let assetLoadError = "";

  const autoMarker = "pay3flow.otc.p2p.auto.";
  const dismissMarker = "pay3flow.otc.p2p.dismissed.v1.";
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
  function dismissKey(actor: string) {
    return `${dismissMarker}${encodeURIComponent(actor)}`;
  }
  function loadDismissed(actor: string): Set<string> {
    if (!actor || typeof localStorage === "undefined") return new Set<string>();
    try {
      const saved = JSON.parse(localStorage.getItem(dismissKey(actor)) || "[]");
      const ids = Array.isArray(saved) ? saved.filter((id: unknown) => typeof id === "string") : [];
      return new Set<string>(ids as string[]);
    } catch { return new Set<string>(); }
  }
  function saveDismissed(actor: string) {
    if (!actor || typeof localStorage === "undefined") return;
    try { localStorage.setItem(dismissKey(actor), JSON.stringify([...dismissed])); } catch { /* private mode keeps the in-memory dismissal */ }
  }
  $: deal = deals.find(item => item.id === selectedId) || null;
  $: action = deal ? p2pAction(deal) : null;
  $: actionKind = deal ? p2pKind(deal) : null;
  $: actionAsset = actionKind ? p2pLegAsset(actionKind, deal?.terms.stablecoin) : "";
  $: outputNetwork = deal?.terms.stablecoin_network || "ethereum";
  $: outputAsset = `${deal?.terms.stablecoin || "USDT"}@${outputNetwork}`;
  $: outputAssetId = deal?.terms.stablecoin_asset_id
    || destinationAssets.find(item => item.asset === outputAsset)?.asset_id
    || "";
  $: compatibleSourceAssets = p2pCompatibleSources(sourceAssets, outputAssetId);
  $: directPaymentAvailable = !!deal
    && p2pDirectDirectionAllowed(deal.terms.direction, outputNetwork)
    && p2pDirectAssetMatch(selectedSource, outputAsset, outputAssetId, outputNetwork);
  $: selectedSource = sourceAssets.find(item => item.asset === selectedSourceAsset) || null;
  $: intentQuote = deal?.attempts?.find(attempt => attempt.kind === "payment" && !!attempt.instructions.source_asset && attempt.state !== "failed") || null;
  $: directPaymentAttempt = deal?.attempts?.find(attempt => attempt.kind === "payment" && !attempt.instructions.source_asset && attempt.state !== "failed") || null;
  $: actionLabel = action === "retry"
    ? actionKind === "payment" && deal && !p2pOpenAttempt(deal, "payment") ? say("Choose payment network", "Выбрать сеть оплаты")
      : actionAsset ? say(`Retry ${actionAsset}`, `Повторить ${actionAsset}`) : say("Retry", "Повторить")
    : action === "send"
      ? actionKind === "payment" && deal && !p2pOpenAttempt(deal, "payment") ? say("Choose payment network", "Выбрать сеть оплаты")
        : directPaymentAttempt ? say("Review transfer and send", "Проверить перевод и отправить")
          : intentQuote ? say("Review quote and send", "Проверить котировку и отправить")
          : actionAsset ? say(`Send ${actionAsset}`, `Отправить ${actionAsset}`) : say("Send", "Отправить")
      : say("Confirm", "Подтвердить");
  $: legs = deal ? p2pViewerLegs(deal) : null;
  $: myLegDone = deal && deal.my_leg ? p2pLegDone(deal, deal.my_leg) : false;
  $: overdue = !!deal && deal.state === "delivery_pending" && p2pOverdue(deal);
  $: waitingForChain = !!(deal && actionKind && p2pOpenAttempt(deal, actionKind));
  $: waitingLabel = !deal ? ""
    : !deal.counterparty_confirmed
      ? say("Waiting for the counterparty to confirm.", "Ожидаем подтверждения контрагента.")
      : myLegDone
        ? say("Your leg is complete. Waiting for the counterparty.", "Ваша часть выполнена. Ожидаем контрагента.")
        : waitingForChain
          ? say("Waiting for the transfer to confirm.", "Ожидаем подтверждения перевода.")
          : deal.state === "delivery_pending" && feeStatus !== "collected"
            ? say("Waiting for the service fee to be confirmed.", "Ожидаем подтверждения комиссии сервиса.")
          : say("Waiting for the counterparty.", "Ожидаем контрагента.");
  $: {
    const active = deals.filter(p2pActive);
    if (active.length !== reportedActive) {
      reportedActive = active.length;
      onActiveCount(active.length);
    }
    const ids = active.map(item => item.id).join(",");
    if (ids !== reportedActiveIds) {
      reportedActiveIds = ids;
      onActiveDeals(active);
    }
  }
  $: selfWallet = session?.wallets.find(wallet => wallet.chain === "everscale") || null;
  $: selfEver = deal?.my_ever || selfWallet?.address || "";
  $: counterpartyEver = deal?.counterparty_ever || "";
  $: selfAvatar = deal?.my_avatar || selfWallet?.avatar || (selfEver ? identiconDataUri(selfEver) : "");
  $: counterpartyAvatar = deal?.counterparty_avatar || (counterpartyEver ? identiconDataUri(counterpartyEver) : "");
  $: paymentDone = deal ? p2pLegDone(deal, "payment") : false;
  $: payoutDone = deal ? p2pLegDone(deal, "payout") : false;
  $: feeStatus = deal ? p2pFeeStatus(deal) : "not_due";
  $: feeAttempts = deal?.attempts?.filter(attempt => attempt.kind === "fee") || [];
  $: feeAttempt = feeAttempts[feeAttempts.length - 1] || null;
  $: feeAmount = deal?.fee_policy ? p2pFormatUnits(deal.fee_policy.fee_units, deal.fee_policy.precision) : "";
  $: netAmount = deal?.fee_policy ? p2pFormatUnits(deal.fee_policy.net_units, deal.fee_policy.precision) : deal?.terms.usdt_amount || "";
  $: isFeePayer = !!deal && deal.terms.fee_payer_actor === session?.actor;
  $: canPrepareFee = isFeePayer && feeStatus === "accrued";
  $: flowActive = deal ? p2pLegActive(deal, "payment") || p2pLegActive(deal, "payout") : false;
  $: canCancel = !!deal && deal.state === "payment_pending";


  $: {
    const actor = session?.actor || "";
    if (actor !== loadedActor) {
      loadedActor = actor;
      deals = [];
      selectedId = "";
      autoOpen = true;
      dismissed = loadDismissed(actor);
      autoTried.clear();
      configReady = false;
      routePickerOpen = false;
      selectedSourceAsset = "";
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
    void request<{ assets: RouteAsset[]; destination_assets: DestinationAsset[] }>("/p2p/assets")
      .then(result => {
        sourceAssets = result.assets || [];
        destinationAssets = result.destination_assets || [];
        if (routePickerOpen && deal && !selectedSourceAsset) selectedSourceAsset = defaultSourceAsset(deal);
      })
      .catch(() => { assetLoadError = say("Payment networks are temporarily unavailable.", "Сети оплаты временно недоступны."); });
    const timer = setInterval(() => { if (!busy && !locked) void refresh(); }, 4500);
    return () => clearInterval(timer);
  });

  async function present() {
    await tick();
    if (dialog && !dialog.open) dialog.showModal();
  }

  function close() {
    if (selectedId) {
      dismissed.add(selectedId);
      saveDismissed(session?.actor || "");
    }
    autoOpen = false;
    if (dialog?.open) dialog.close();
    selectedId = "";
    error = "";
  }

  async function cancelDeal() {
    const current = deal;
    const token = session?.token;
    if (!current || !token) return;
    await run(async () => {
      applyDeal(await request<P2pDeal>(`/p2p/trades/${current.id}/cancel`, token, {}), current);
      close();
    });
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

  function defaultSourceAsset(current: P2pDeal) {
    const destination = `${current.terms.stablecoin || "USDT"}@${current.terms.stablecoin_network || "ethereum"}`;
    const destinationId = current.terms.stablecoin_asset_id
      || destinationAssets.find(item => item.asset === destination)?.asset_id
      || "";
    return sourceAssets.find(item => item.asset === destination && item.asset_id === destinationId)?.asset
      || sourceAssets.find(item => item.destination_asset_ids.includes(destinationId))?.asset
      || "";
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
    saveDismissed(session?.actor || "");
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
    const active = deals.filter(p2pActive);
    // Prefer a deal where this viewer still has to sign something.
    const next = sortNewest(active).find(item => p2pKind(item)) || sortNewest(active)[0];
    if (!next) return;
    dismissed.delete(next.id);
    saveDismissed(session?.actor || "");
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
      const preparedAttempt = { ...prepared.attempt, kind };
      const withAttempt = {
        ...current,
        attempts: [...(current.attempts || []), preparedAttempt],
      };
      applyDeal(prepared, withAttempt);
      attempt = preparedAttempt;
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

  async function prepareFee() {
    const current = deal;
    const owner = session;
    if (!current || !owner) return;
    await run(async () => {
      const paymentAttempt = current.attempts?.find(attempt => attempt.kind === "payment" && attempt.state === "verified");
      const paymentSource = paymentAttempt?.instructions.source_asset
        || paymentAttempt?.instructions.fee_source_asset
        || `${current.terms.stablecoin || "USDT"}@${outputNetwork}`;
      const paymentSourceId = paymentAttempt?.instructions.source_asset_id
        || paymentAttempt?.instructions.fee_source_asset_id;
      const directFee = paymentSource === `USDT@${outputNetwork}`
        && (!current.terms.fee_usdt_asset_id || paymentSourceId === current.terms.fee_usdt_asset_id);
      const needsRoute = !directFee;
      const storageKey = `pay3flow.otc.fee-key.${current.id}`;
      const key = sessionStorage.getItem(storageKey) || crypto.randomUUID();
      sessionStorage.setItem(storageKey, key);
      const prepared = await request<P2pDeal & { attempt: Attempt }>(
        `/p2p/trades/${current.id}/prepare`,
        owner.token,
        { kind: needsRoute ? "near-intents-fee" : "fee" },
        key,
      );
      sessionStorage.removeItem(storageKey);
      const feeTransfer = { ...prepared.attempt, kind: "fee" };
      applyDeal(prepared, {
        ...current,
        attempts: [
          ...(current.attempts || []).filter(attempt => attempt.kind !== "fee" || attempt.state === "failed"),
          feeTransfer,
        ],
      });
    });
  }

  async function sendFee() {
    const current = deal;
    const owner = session;
    const attempt = feeAttempt;
    if (!current || !owner || !attempt || attempt.state !== "prepared") return;
    await run(async () => {
      const original = sessionStorage.getItem(`pay3flow.otc.reference.${attempt.id}`);
      if (original) {
        await request(attempt.instructions.submit_path || `/attempts/${attempt.id}/reference`, owner.token, { reference: original });
      } else {
        await sendAttempt(owner.token, attempt);
      }
    });
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
        if (kind === "payment" && !p2pOpenAttempt(merged, kind)) {
          routePickerOpen = true;
          selectedSourceAsset = defaultSourceAsset(merged);
          return;
        }
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
    if (kind === "payment" && !p2pOpenAttempt(current, kind)) {
      routePickerOpen = true;
      selectedSourceAsset = defaultSourceAsset(current);
      return;
    }
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
    if (kind === "payment" && !open) return;
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

  async function preparePayment() {
    const current = deal;
    const owner = session;
    const source = selectedSource;
    if (!current || !owner || !source) return;
    await run(async () => {
      if (source.asset === outputAsset && directPaymentAvailable) {
        const storageKey = `pay3flow.otc.direct-key.${current.id}`;
        const key = sessionStorage.getItem(storageKey) || crypto.randomUUID();
        sessionStorage.setItem(storageKey, key);
        const prepared = await request<P2pDeal & { attempt: Attempt }>(
          `/p2p/trades/${current.id}/prepare`,
          owner.token,
          { kind: "payment" },
          key,
        );
        sessionStorage.removeItem(storageKey);
        const paymentAttempt = { ...prepared.attempt, kind: "payment" };
        applyDeal(prepared, {
          ...current,
          attempts: [...(current.attempts || []), paymentAttempt],
        });
        routePickerOpen = false;
        return;
      }
      const connected = await connectWallet(walletNetwork(source.network, otcP2pNetwork()));
      const storageKey = `pay3flow.otc.quote-key.${current.id}`;
      const quoteKey = sessionStorage.getItem(storageKey) || crypto.randomUUID();
      sessionStorage.setItem(storageKey, quoteKey);
      const prepared = await request<P2pDeal & { attempt: Attempt }>(
        `/p2p/trades/${current.id}/prepare`,
        owner.token,
        {
          kind: "near-intents",
          source_asset: source.asset,
          source_asset_id: source.asset_id,
          source_address: connected.address,
          slippage_bps: 100,
        },
        quoteKey,
      );
      sessionStorage.removeItem(storageKey);
      const paymentAttempt = { ...prepared.attempt, kind: "payment" };
      applyDeal(prepared, {
        ...current,
        attempts: [...(current.attempts || []), paymentAttempt],
      });
      routePickerOpen = false;
    });
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
        <strong>{deal.terms.usdt_amount}</strong><small>{deal.terms.stablecoin || "USDT"}</small>
      </div>
      <button type="button" class="closeButton" aria-label={say("Close window", "Закрыть окно")} on:click={close}>
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
        <img src={deal.terms.stablecoin === "USDC" ? "/icons/assets/usdc.png" : "/icons/assets/usdt.png"} alt="" width="24" height="24" />
        <b>{netAmount}</b>
      </span>
      <span class="rail" aria-hidden="true"><span></span></span>
      <span class="token ever" class:done={payoutDone} class:active={p2pLegActive(deal, "payout")} class:failed={p2pLegFailed(deal, "payout")}>
        <img src="/icons/assets/ever.svg" alt="" width="24" height="24" />
        <b>{deal.terms.ever_amount}</b>
      </span>
    </div>
    <div class="destination" data-testid="otc-stablecoin-destination">
      <span>{say(`${deal.terms.stablecoin || "USDT"} recipient · ${outputNetwork}`, `Получатель ${deal.terms.stablecoin || "USDT"} · ${outputNetwork}`)}</span>
      <code>{deal.terms.usdt_payee}</code>
    </div>

    {#if legs}
      <div class="legs" data-testid="otc-p2p-legs">
        <span class="leg" class:done={myLegDone}>
          <small>{myLegDone ? say("You sent", "Вы отправили") : say("You send", "Вы отправляете")}</small>
          <b>{legs.send.amount} {legs.send.asset}</b>
        </span>
        <span class="leg">
          <small>{say("You receive", "Вы получаете")}</small>
          <b>{legs.receive.amount} {legs.receive.asset}</b>
        </span>
      </div>
    {/if}

    {#if deal.fee_accrued}
      <section class="feeBox" data-testid="otc-p2p-fee">
        <strong>{say(`Seller receives ${netAmount} ${deal.terms.stablecoin || "USDT"}; service fee ${feeAmount} USDT`, `Продавец получает ${netAmount} ${deal.terms.stablecoin || "USDT"}; комиссия сервиса ${feeAmount} USDT`)}</strong>
        {#if feeStatus === "collected"}
          <small>{say(`Payment confirmed on ${outputNetwork}.`, `Платёж подтверждён в сети ${outputNetwork}.`)}</small>
        {:else if isFeePayer && feeAttempt?.state === "prepared"}
          {#if feeAttempt.instructions.source_asset}
            <small>{feeAttempt.instructions.source_amount} {feeAttempt.instructions.source_asset} → {feeAttempt.instructions.minimum_output} {feeAttempt.instructions.destination_asset}</small>
            {#if feeAttempt.instructions.expected_fee}
              <small>{say("Route fee", "Комиссия маршрута")}: {feeAttempt.instructions.expected_fee.amount} {feeAttempt.instructions.expected_fee.asset}</small>
            {/if}
            {#if feeAttempt.instructions.quote_expires_at}
              <small>{say("Quote expires", "Котировка действует до")}: {new Date(feeAttempt.instructions.quote_expires_at).toLocaleString($locale === "ru" ? "ru-RU" : "en-US")}</small>
            {/if}
          {:else}
            <small>{`USDT · ${outputNetwork}`}</small>
          {/if}
          <small>{feeAttempt.instructions.source_asset ? say("Route deposit address", "Адрес депозита маршрута") : say("Fee recipient", "Получатель комиссии")}</small>
          <code>{feeAttempt.instructions.leg.recipient}</code>
          <button type="button" class="confirmButton" disabled={busy || locked} on:click={() => void sendFee()}>
            {say(`Send ${feeAmount} USDT`, `Отправить ${feeAmount} USDT`)}
          </button>
        {:else if canPrepareFee}
          <small>{say(`Paid to the configured ${outputNetwork} address by the agreed fee payer.`, `Комиссия переводится на настроенный адрес в сети ${outputNetwork} согласованным плательщиком.`)}</small>
          <button type="button" class="confirmButton" disabled={busy || locked} on:click={() => void prepareFee()}>
            {#if busy}<span class="spinner" aria-hidden="true"></span>{/if}
            {say("Prepare fee transfer", "Подготовить перевод комиссии")}
          </button>
        {:else if feeStatus === "pending"}
          <small>{say("Waiting for the fee transfer to confirm.", "Ожидаем подтверждения перевода комиссии.")}</small>
        {:else}
          <small>{say("The agreed fee is paid separately by the designated payer.", "Согласованная комиссия оплачивается отдельно назначенным плательщиком.")}</small>
        {/if}
      </section>
    {/if}

    {#if overdue}
      <p class="notice" role="status">{say("The nominal delivery window has passed. You can still complete this transfer.", "Номинальный срок доставки прошёл. Вы всё ещё можете выполнить этот перевод.")}</p>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    {#if routePickerOpen && actionKind === "payment"}
      <section class="routePicker" aria-label={say("Choose payment network", "Выбрать сеть оплаты")} data-testid="otc-payment-network-picker">
        <label>{say("Pay with", "Оплатить активом")}
          <select bind:value={selectedSourceAsset} disabled={busy || locked}>
            {#each compatibleSourceAssets as source}
              <option value={source.asset}>{source.symbol} · {source.network}</option>
            {/each}
          </select>
        </label>
        {#if selectedSource && selectedSource.asset === outputAsset && directPaymentAvailable}
          <p class="routeNote">{say("Same token and network: this payment is sent directly, without conversion.", "Токен и сеть совпадают: платёж отправляется напрямую, без конвертации.")}</p>
        {:else if selectedSource}
          <p class="routeNote">{say(`The quote delivers ${netAmount} ${deal?.terms.stablecoin || "USDT"} to the seller; ${feeAmount} USDT is routed separately to the service-fee address on ${outputNetwork}.`, `Котировка доставит продавцу ${netAmount} ${deal?.terms.stablecoin || "USDT"}; ${feeAmount} USDT отдельно переводится на адрес комиссии в сети ${outputNetwork}.`)}</p>
        {:else}
          <p class="routeNote">{assetLoadError || say("No compatible production payment networks are available.", "Нет доступных production-сетей для оплаты.")}</p>
        {/if}
        <div class="routeActions">
          <button type="button" class="confirmButton" disabled={busy || locked || !selectedSource || (selectedSource.asset === outputAsset && !directPaymentAvailable)} on:click={() => void preparePayment()}>
            {#if busy}<span class="spinner" aria-hidden="true"></span>{/if}
            <span>{selectedSource?.asset === outputAsset && directPaymentAvailable ? say("Send directly", "Отправить напрямую") : say("Get payment quote", "Получить котировку")}</span>
          </button>
          <button type="button" class="quitButton" disabled={busy} on:click={() => { routePickerOpen = false; }}>{say("Back", "Назад")}</button>
        </div>
      </section>
    {/if}

    {#if intentQuote && actionKind === "payment"}
      <section class="routeQuote" aria-label={say("Payment quote", "Котировка оплаты")} data-testid="otc-payment-quote">
        <strong>{say("Payment route", "Маршрут оплаты")}</strong>
        <p>{intentQuote.instructions.source_amount} {intentQuote.instructions.source_asset} → {intentQuote.instructions.minimum_output} {intentQuote.instructions.destination_asset}</p>
        <small>{say("Quoted output", "Расчётное получение")}: {intentQuote.instructions.expected_output} {intentQuote.instructions.destination_asset}</small>
        {#if intentQuote.instructions.expected_fee}
          <small>{say("Route fee", "Комиссия маршрута")}: {intentQuote.instructions.expected_fee.amount} {intentQuote.instructions.expected_fee.asset}</small>
        {/if}
        {#if intentQuote.instructions.slippage_bps !== undefined}
          <small>{say("Maximum slippage", "Максимальное проскальзывание")}: {(intentQuote.instructions.slippage_bps / 100).toFixed(2)}%</small>
        {/if}
        <small>{say("Quote expires", "Котировка действует до")}: {new Date(intentQuote.instructions.quote_expires_at || "").toLocaleString($locale === "ru" ? "ru-RU" : "en-US")}</small>
        {#if intentQuote.instructions.deposit_memo}
          <small>{say("Required payment memo", "Обязательное сообщение платежа")}: <code>{intentQuote.instructions.deposit_memo}</code></small>
        {/if}
        <small class="depositAddress">{say("Deposit address", "Адрес депозита")}: <code>{intentQuote.instructions.deposit_address}</code></small>
        {#if intentQuote.instructions.fee_preflight}
          <strong>{say("Service-fee route estimate", "Расчёт маршрута комиссии")}</strong>
          <p>{intentQuote.instructions.fee_preflight.source_amount} {intentQuote.instructions.fee_preflight.source_asset} → {intentQuote.instructions.fee_preflight.expected_output} {intentQuote.instructions.fee_preflight.destination_asset}</p>
          {#if intentQuote.instructions.fee_preflight.expected_fee}
            <small>{say("Additional route fee estimate", "Оценка дополнительной комиссии маршрута")}: {intentQuote.instructions.fee_preflight.expected_fee.amount} {intentQuote.instructions.fee_preflight.expected_fee.asset}</small>
          {/if}
        {/if}
      </section>
    {/if}
    {#if directPaymentAttempt && actionKind === "payment"}
      <section class="routeQuote" aria-label={say("Direct payment review", "Проверка прямого платежа")} data-testid="otc-direct-payment-review">
        <strong>{say("Direct payment", "Прямой платёж")}</strong>
        <p>{netAmount} {deal.terms.stablecoin || "USDT"} · {outputNetwork}</p>
        <small>{say("Seller receives", "Продавец получает")}: <code>{directPaymentAttempt.instructions.leg.recipient}</code></small>
        {#if directPaymentAttempt.instructions.fee_preflight}
          <strong>{say("Service-fee route estimate", "Расчёт маршрута комиссии")}</strong>
          <p>{directPaymentAttempt.instructions.fee_preflight.source_amount} {directPaymentAttempt.instructions.fee_preflight.source_asset} → {directPaymentAttempt.instructions.fee_preflight.expected_output} {directPaymentAttempt.instructions.fee_preflight.destination_asset}</p>
          {#if directPaymentAttempt.instructions.fee_preflight.expected_fee}
            <small>{say("Additional route fee estimate", "Оценка дополнительной комиссии маршрута")}: {directPaymentAttempt.instructions.fee_preflight.expected_fee.amount} {directPaymentAttempt.instructions.fee_preflight.expected_fee.asset}</small>
          {/if}
        {/if}
      </section>
    {/if}

    <div class="action">
      {#if action}
        <button type="button" class="confirmButton" disabled={busy || locked} data-testid="otc-p2p-confirm" on:click={act}>
          {#if busy}<span class="spinner" aria-hidden="true"></span>{/if}
          <span>{actionLabel}</span>
        </button>
      {:else}
        <p class="waitingAction">{waitingLabel}</p>
      {/if}
      {#if canCancel}
        <button type="button" class="quitButton" disabled={busy || locked} data-testid="otc-p2p-cancel" on:click={cancelDeal}>{say("Cancel deal", "Отменить сделку")}</button>
      {/if}
      <button type="button" class="quitButton" disabled={busy || locked} data-testid="otc-p2p-close" on:click={close}>{say("Close", "Закрыть")}</button>
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
  .destination { display:grid; gap:5px; margin:14px 26px 0; padding:10px 12px; border:1px solid var(--color-border); border-radius:9px; background:var(--color-panel); }
  .destination span { color:var(--color-text-soft); font-size:10px; }
  .destination code { overflow-wrap:anywhere; color:var(--color-text); font-family:var(--font-mono); font-size:10px; line-height:1.5; }
  .legs { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:8px; padding:18px 26px 0; }
  .feeBox { display:grid; gap:8px; margin:14px 26px 0; padding:12px; border:1px solid var(--color-border); border-radius:10px; background:var(--color-panel); }
  .feeBox strong { font-size:11px; }
  .feeBox small { color:var(--color-text-faint); font-size:10px; line-height:1.45; }
  .feeBox code { overflow-wrap:anywhere; color:var(--color-text); font-family:var(--font-mono); font-size:10px; }
  .leg { display:grid; min-width:0; gap:2px; padding:9px 10px; border:1px solid var(--color-border); border-radius:10px; background:var(--color-panel); }
  .leg.done { border-color:color-mix(in srgb, var(--color-good-ink) 38%, var(--color-border)); background:var(--otc-buy-soft); }
  .leg small { color:var(--color-text-faint); font-size:9px; font-weight:650; letter-spacing:.04em; text-transform:uppercase; }
  .leg b { overflow:hidden; font-family:var(--font-mono); font-size:11px; font-weight:650; text-overflow:ellipsis; white-space:nowrap; }
  .notice { margin:12px 26px 0; color:var(--color-text-soft); font-size:11px; line-height:1.45; text-align:center; }
  .error { margin:14px 26px 0; color:var(--color-danger); font-size:11px; line-height:1.45; text-align:center; }
  .action { padding:22px 26px 26px; }
  .routePicker,.routeQuote { display:grid; gap:10px; margin:18px 26px 0; padding:13px; border:1px solid var(--color-border); border-radius:12px; background:var(--color-panel); }
  .routePicker label { display:grid; gap:6px; color:var(--color-text-soft); font-size:11px; }
  .routePicker select,.routePicker input { width:100%; min-height:40px; padding:8px 10px; border:1px solid var(--color-border); border-radius:8px; background:var(--color-paper); color:var(--color-text); font:inherit; }
  .routeNote { margin:0; color:var(--color-text-faint); font-size:11px; line-height:1.45; }
  .routeActions { display:grid; gap:8px; }
  .routeQuote { overflow-wrap:anywhere; }
  .routeQuote strong { font-size:12px; }
  .routeQuote p { margin:0; font-family:var(--font-mono); font-size:11px; }
  .routeQuote small { color:var(--color-text-faint); font-size:10px; line-height:1.4; }
  .routeQuote code { color:var(--color-text); overflow-wrap:anywhere; }
  .routeQuote .depositAddress { margin-top:3px; }
  .confirmButton { display:flex; width:100%; min-height:46px; align-items:center; justify-content:center; gap:8px; border:1px solid var(--color-accent-strong); border-radius:11px; background:var(--color-accent); color:#152016; font-size:13px; font-weight:750; }
  .confirmButton:disabled { opacity:.45; cursor:default; }
  .confirmButton:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  .waitingAction { display:flex; min-height:46px; align-items:center; justify-content:center; margin:0; color:var(--color-text-soft); font-size:12px; line-height:1.45; text-align:center; }
  .quitButton { display:flex; width:100%; min-height:40px; align-items:center; justify-content:center; margin-top:10px; border:1px solid var(--color-border-strong); border-radius:11px; background:transparent; color:var(--color-text-soft); font-size:12px; font-weight:650; }
  .quitButton:hover:not(:disabled) { border-color:var(--color-danger); color:var(--color-danger); }
  .quitButton:disabled { opacity:.45; cursor:default; }
  .quitButton:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  .spinner { width:13px; height:13px; border:2px solid #15201655; border-top-color:#152016; border-radius:50%; animation:spin .8s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }
  @keyframes travel { 0%,100% { transform:translate(-50%,-50%); } 50% { transform:translate(calc(-50% + 13px),-50%); } }
  @media (max-width:420px) { .people, .flow, .legs, .action { padding-right:18px; padding-left:18px; } .dealHead { padding-right:18px; padding-left:18px; } .notice { margin-right:18px; margin-left:18px; } }
  @media (prefers-reduced-motion:reduce) { .flow.active .rail span, .spinner { animation:none; } }
</style>
