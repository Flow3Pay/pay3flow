<script lang="ts">
  import { onMount } from "svelte";
  import { fade, slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import type { OtcPreview } from "$lib/share-links";
  import { parseOtcHash } from "$lib/otc/link";
  import { request, signInWallet, OtcRequestError, bindWallet, sendAttempt, decimal, rate, type Config, type Session, type Proposal, type Trade, type Rfq, type Attempt, type Direction } from "$lib/otc/service";
  import { exactInput, proposalDirection, terminalStates, completedVolume } from "$lib/otc/terminal";
  import "$lib/otc/terminal.css";
  import OtcProductionSafe from "./OtcProductionSafe.svelte";
  import OtcPanel from "./OtcPanel.svelte";
  import OtcPanelToggle from "./OtcPanelToggle.svelte";
  import OtcTicket from "./OtcTicket.svelte";
  import OtcDeskBook from "./OtcDeskBook.svelte";
  import OtcQuoteChart from "./OtcQuoteChart.svelte";
  import OtcTradeDetails from "./OtcTradeDetails.svelte";
  import OtcDeskConsole from "./OtcDeskConsole.svelte";
  import { otcWalletDialog, openWalletDialog, closeWalletDialog } from "$lib/wallet-dialog";
  import OtcTransferReview from "./OtcTransferReview.svelte";
  export let onPreviewChange: (preview: OtcPreview | null) => void = () => {};
  export let onShareUrlChange: (url: string) => void = () => {};
  let config: Config | null = null, session: Session | null = null;
  let input = "100", direction: Direction = "buy", listings: Proposal[] = [], trades: Trade[] = [], rfqs: Rfq[] = [];
  let selected: Trade | null = null, attempts: Attempt[] = [], receipts: { chain: string; leg: { amount: string }; included_at: string }[] = [], updates: { message: string; at: string }[] = [];
  let dashboard: Record<string, any> | null = null, busy = false, loaded = false, healthy = false, error = "", notice = "", bound: string[] = [];
  let rfqKey = "", listingId = "", bookingUnknown = "", requestingQuote = false;
  let transferReview: { attempt: Attempt; trade: Trade; kind: string } | null = null;
  let handoff: { tradeId: string; kind: string; key: string; phase: string; attemptId?: string } | null = null;
  let bridgeOpen = true, bookOpen = true, activityOpen = true, motionDuration = 320, mounted = false, chartColumn: HTMLDivElement, toggleOffset = 250;
  let selectionVersion = 0, accountVersion = 0, walletProgress = "", accountLocked = false;
  let tab: "trades" | "orders" | "history" = "orders", now = Date.now();
  const sessionKey = "pay3flow.otc.session.v1", layoutKey = "pay3flow.otc.panels.v1";
  $: copy = otcCopy($locale);
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: listing = listings.find(p => p.id === listingId && proposalDirection(p, config) === direction) || listings.find(p => proposalDirection(p, config) === direction);
  $: unresolvedBooking = !!bookingUnknown || trades.some(t => t.state === "booking_pending");
  $: handoffUnknown = !!handoff && handoff.tradeId === selected?.id;
  $: quote = selected?.state === "quoted" && selected.terms.direction === direction && exactInput(input, direction) === exactInput(selected.terms.input, direction) ? selected : null;
  $: needsWallets = !session || !["ethereum", "everscale"].every(chain => bound.some(wallet => wallet.startsWith(`${chain}:`)));
  $: quoteReady = !needsWallets && healthy && !!config?.available && !!listing && !!exactInput(input, direction) && !unresolvedBooking;
  $: if (mounted) otcWalletDialog.set({
    session, bound, busy, error, progress: walletProgress, disconnectDisabled: accountLocked,
    chains: config?.wallet_login_chains ?? ["ethereum", "everscale"],
    enabled: !!(config?.wallet_login_available ?? config?.enabled), requestingQuote, quoteReady,
    onClose: closeIdentity,
    onConnect: chain => run(() => connectAccount(chain)),
    onVerify: chain => run(() => verify(chain)),
    onDisconnect: () => disconnect().catch(e => { error = e instanceof Error ? e.message : "Disconnect failed"; }),
    onRequest: () => { closeWalletDialog(); void run(rfq); },
  });
  $: activeTrades = trades.filter(t => !terminalStates.has(t.state));
  $: history = trades.filter(t => terminalStates.has(t.state));
  $: if (mounted) {
    try { localStorage.setItem(layoutKey, JSON.stringify({ bridgeOpen, bookOpen, activityOpen })); } catch { /* Layout remains usable. */ }
  }
  function panelTransition(node: Element, options: { duration: number }) { return (window.matchMedia("(max-width: 980px)").matches ? slide : fade)(node, { ...options, easing: cubicOut }); }
  function syncLink() {
    // Only the public pair/direction are shared; participant terms never enter previews or URLs.
    const hash = `#/otc?market=EVER-USDT&side=${direction}`;
    if (/^#\/otc(?:[/?]|$)/.test(location.hash)) historyReplace(hash);
    onShareUrlChange(`/${hash}`); onPreviewChange(null);
  }
  function historyReplace(hash: string) { window.history.replaceState(window.history.state, "", `${location.pathname}${location.search}${hash}`); }
  async function run(action: () => Promise<void>, lockAccount = true) {
    if (busy) return; busy = true; accountLocked = lockAccount; error = ""; notice = "";
    try { await action(); } catch (e) { error = e instanceof Error ? e.message : "Request failed"; } finally { busy = false; accountLocked = false; walletProgress = ""; }
  }
  async function refresh() {
    const refreshToken = session?.token;
    try {
      const [route, proposals] = await Promise.all([request<Config>("/config"), request<Proposal[]>("/listings")]);
      if (!mounted) return;
      config = route; listings = proposals; healthy = true; loaded = true;
      if (!session) return;
      const owner = session, previousSelection = selected?.id, previousVersion = selectionVersion;
      const current = await request<Session>("/session", owner.token);
      if (!mounted || session?.token !== owner.token) return;
      retainSession(current);
      const page = await request<{ trades: Trade[]; rfqs: Rfq[] }>("/trades", owner.token);
      if (!mounted || session?.token !== owner.token) return;
      trades = page.trades; rfqs = page.rfqs;
      if (bookingUnknown && trades.some(t => t.id === bookingUnknown && t.state !== "quoted")) { bookingUnknown = ""; sessionStorage.removeItem("pay3flow.otc.booking"); }
      if (previousSelection && selected?.id === previousSelection && selectionVersion === previousVersion) await selectTrade(previousSelection);
      if (owner.desk && session?.token === owner.token) { const data = await request<Record<string, any>>("/desk/dashboard", owner.token); if (session?.token === owner.token) dashboard = data; }
    } catch (e) {
      if (!mounted) return;
      loaded = true;
      if (e instanceof OtcRequestError && e.status === 401) { if (session?.token !== refreshToken) return; clearAccount(); error = say("Session expired. Connect your wallet again.", "Сессия истекла. Подключите кошелёк снова."); return; }
      healthy = false; throw e;
    }
  }
  async function selectTrade(id: string, editTicket = false) {
    if (!session) return;
    const owner = session, version = ++selectionVersion;
    const details = await request<{ trade: Trade; attempts: Attempt[]; customer_updates?: typeof updates[]; evidence?: typeof receipts }>(`/trades/${id}`, owner.token);
    if (!mounted || session?.token !== owner.token || version !== selectionVersion) return;
    selected = details.trade; attempts = details.attempts; updates = details.customer_updates?.flat() || []; receipts = details.evidence || [];
    sessionStorage.setItem("pay3flow.otc.selected", id);
    if (editTicket && selected.state === "quoted") { direction = selected.terms.direction; input = selected.terms.input; rfqKey = ""; syncLink(); }
    if (handoff?.tradeId === id && (terminalStates.has(selected.state) || attempts.some(a => a.kind === handoff?.kind && (!handoff?.attemptId || a.id === handoff.attemptId) && ["failed", "verified"].includes(a.state)) && !attempts.some(a => a.kind === handoff?.kind && !["failed", "verified"].includes(a.state)))) { handoff = null; sessionStorage.removeItem("pay3flow.otc.handoff"); }
  }
  function edited() { selectionVersion++; rfqKey = ""; if (selected?.state === "quoted") { selected = null; attempts = []; receipts = []; updates = []; sessionStorage.removeItem("pay3flow.otc.selected"); } }
  function chooseSide(next: Direction) { if (next !== direction) { direction = next; listingId = ""; edited(); syncLink(); } }
  function selectListing(proposal: Proposal, side: Direction) { chooseSide(side); if (listingId !== proposal.id) edited(); listingId = proposal.id; bridgeOpen = true; document.getElementById("otc-input")?.focus(); }
  function retainSession(current: Session) {
    if (session?.actor !== current.actor && sessionStorage.getItem("pay3flow.otc.recovery.actor") === current.actor) {
      try { handoff = JSON.parse(sessionStorage.getItem("pay3flow.otc.handoff") || "null"); } catch { handoff = null; }
      bookingUnknown = sessionStorage.getItem("pay3flow.otc.booking") || "";
    }
    session = current;
    bound = current.wallets.map(wallet => `${wallet.chain}: ${wallet.address}`);
    sessionStorage.setItem(sessionKey, JSON.stringify(current));
    sessionStorage.removeItem("pay3flow.otc.wallets");
  }
  async function connectAccount(chain: "ethereum" | "everscale") {
    const version = ++accountVersion;
    const current = await signInWallet(chain, message => walletProgress = message);
    if (!mounted || version !== accountVersion) { await request("/session/revoke", current.token, {}); return; }
    clearAccount(false); retainSession(current);
    await refresh();
  }
  async function verify(chain: "ethereum" | "everscale") {
    if (!session) return;
    const owner = session;
    await bindWallet(owner.token, chain, message => walletProgress = message);
    if (!mounted || session?.token !== owner.token) return;
    await refresh();
    if (!mounted || session?.token !== owner.token) return;
    notice = say("Wallet ownership verified.", "Владение кошельком подтверждено.");
  }
  function requestQuote() {
    if (needsWallets) { openIdentity(true); return; }
    void run(rfq);
  }
  async function rfq() {
    if (needsWallets) { openIdentity(true); return; }
    if (!session) return;
    if (!listing || !healthy || !config?.available || unresolvedBooking) return;
    const amount = exactInput(input, direction); if (!amount) throw new Error("Enter a positive exact amount with the asset precision.");
    rfqKey ||= crypto.randomUUID();
    const result = await request<{ id: string }>("/rfqs", session.token, { direction, input: amount, listing_id: listing.id }, rfqKey);
    notice = `Quote requested. Reference: ${result.id}`; tab = "orders"; activityOpen = true; await refresh();
  }
  async function apply() {
    if (!session || !selected || selected.state !== "quoted" || unresolvedBooking || now > Date.parse(selected.terms.quote_by)) return;
    bookingUnknown = selected.id; sessionStorage.setItem("pay3flow.otc.recovery.actor", session.actor); sessionStorage.setItem("pay3flow.otc.booking", bookingUnknown);
    try { await request(`/trades/${selected.id}/apply`, session.token, {}); } finally { await refresh(); }
  }
  async function reconcileBooking() {
    if (!session || !bookingUnknown) return;
    const id = bookingUnknown; await selectTrade(id);
    if (selected?.id === id && selected.state === "quoted") await request(`/trades/${id}/apply`, session.token, {});
    await refresh();
  }
  async function decide(accept: boolean) { if (!session || !selected) return; await request(`/desk/trades/${selected.id}/decision`, session.token, { accept }); await refresh(); }
  async function prepareReview(kind: string) {
    if (!session || !selected || handoffUnknown || attempts.some(a => a.kind === kind && a.state !== "failed")) return;
    const trade = selected; sessionStorage.setItem("pay3flow.otc.recovery.actor", session.actor); handoff = { tradeId: trade.id, kind, key: crypto.randomUUID(), phase: "preparing" };
    sessionStorage.setItem("pay3flow.otc.handoff", JSON.stringify(handoff));
    try {
      const attempt = await request<Attempt>(`/trades/${trade.id}/prepare`, session.token, { kind }, handoff.key);
      handoff = { ...handoff, phase: "review", attemptId: attempt.id }; sessionStorage.setItem("pay3flow.otc.handoff", JSON.stringify(handoff));
      await selectTrade(trade.id); transferReview = { attempt, trade, kind };
    } finally { await refresh(); }
  }
  async function confirmTransfer() {
    if (transferReview?.attempt.instructions.escrow && Date.now() >= Date.parse(transferReview.attempt.instructions.expires_at!) - 1800000) return;
    if (!session || !transferReview || !healthy || config?.demo || transferReview.trade.terms.demo || handoff?.phase !== "review" || (!transferReview.trade.terms.escrow && transferReview.kind === "payment" && Date.now() > Date.parse(transferReview.trade.terms.pay_by))) return;
    const review = transferReview; transferReview = null;
    handoff = { ...handoff, phase: "signing" }; sessionStorage.setItem("pay3flow.otc.handoff", JSON.stringify(handoff));
    try { await sendAttempt(session.token, review.attempt); notice = "Transfer submitted. Waiting for independently verified receipt."; }
    catch (e) { notice = "The saved attempt remains unresolved. Track its original reference; do not send again."; throw e; }
    finally { await refresh(); }
  }
  async function track(attempt: Attempt, reference: string) {
    if (!session) return; const original = reference || sessionStorage.getItem(`pay3flow.otc.reference.${attempt.id}`);
    if (!original) throw new Error("Enter the original transaction reference.");
    await request(`/attempts/${attempt.id}/reference`, session.token, { reference: original }); await refresh();
  }
  function clearAccount(closeDialog = true) {
    if (closeDialog) closeWalletDialog();
    selectionVersion++; accountVersion++; session = null; selected = null; attempts = []; trades = []; rfqs = []; receipts = []; updates = []; bound = []; dashboard = null; transferReview = null; rfqKey = ""; handoff = null; bookingUnknown = "";
    for (const key of [sessionKey, "pay3flow.otc.selected", "pay3flow.otc.wallets"]) sessionStorage.removeItem(key);
  }
  async function disconnect() {
    if (accountLocked) return;
    const owner = session;
    clearAccount();
    if (owner) await request("/session/revoke", owner.token, {});
  }
  function openIdentity(forQuote = false) { requestingQuote = forQuote; openWalletDialog("otc"); }
  function closeIdentity() { requestingQuote = false; accountVersion++; }
  function navigateTab(event: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return; event.preventDefault(); const tabs = ["trades", "orders", "history"] as const;
    tab = tabs[event.key === "Home" ? 0 : event.key === "End" ? 2 : (tabs.indexOf(tab) + (event.key === "ArrowRight" ? 1 : -1) + 3) % 3]; document.getElementById(`otc-${tab}-tab`)?.focus();
  }
  onMount(() => {
    const media = matchMedia("(prefers-reduced-motion: reduce)"); const motionChanged = () => motionDuration = media.matches ? 0 : 320; motionChanged(); media.addEventListener("change", motionChanged);
    const restoreDirection = () => { const shared = parseOtcHash(location.hash); if (shared.side) direction = shared.side; edited(); syncLink(); };
    const shared = parseOtcHash(location.hash); direction = shared.side || "buy"; if (shared.amount && exactInput(shared.amount, direction)) input = shared.amount;
    try { const saved = JSON.parse(localStorage.getItem(layoutKey) || "null"); if (saved) for (const name of ["bridgeOpen", "bookOpen", "activityOpen"] as const) { if (typeof saved[name] === "boolean") { if (name === "bridgeOpen") bridgeOpen = saved[name]; if (name === "bookOpen") bookOpen = saved[name]; if (name === "activityOpen") activityOpen = saved[name]; } } } catch { /* Ignore invalid layout storage. */ }
    let restoreToken = "";
    try {
      const saved = JSON.parse(sessionStorage.getItem(sessionKey) || "null");
      if (typeof saved?.token === "string") restoreToken = saved.token;
      if (saved?.actor && !sessionStorage.getItem("pay3flow.otc.recovery.actor")) sessionStorage.setItem("pay3flow.otc.recovery.actor", saved.actor);
    } catch { /* Ignore malformed saved sessions. */ }

    const observer = new ResizeObserver(() => toggleOffset = Math.max(0, chartColumn.getBoundingClientRect().height / 2 - 21)); observer.observe(chartColumn);
    mounted = true; syncLink(); let stopped = false, polling = false;
    const restoreVersion = accountVersion;
    void run(async () => {
      if (restoreToken) {
        try {
          const restored = await request<Session>("/session", restoreToken);
          if (!mounted || accountVersion !== restoreVersion) return;
          retainSession(restored);
        } catch (e) {
          if (!mounted || accountVersion !== restoreVersion) return;
          if (e instanceof OtcRequestError && e.status === 401) clearAccount(); else throw e;
        }
      }
      await refresh(); const id = sessionStorage.getItem("pay3flow.otc.selected"); if (id && session) await selectTrade(id, true); }, false);
    const timer = setInterval(async () => { if (stopped || busy || polling) return; polling = true; try { await refresh(); } catch (e) { error = e instanceof Error ? e.message : "Status unavailable"; } finally { polling = false; } }, 5000);
    const clock = setInterval(() => now = Date.now(), 1000);
    window.addEventListener("hashchange", restoreDirection); window.addEventListener("popstate", restoreDirection);
    return () => { closeWalletDialog(); otcWalletDialog.set(null); stopped = true; mounted = false; accountVersion++; clearInterval(timer); clearInterval(clock); observer.disconnect(); media.removeEventListener("change", motionChanged); window.removeEventListener("hashchange", restoreDirection); window.removeEventListener("popstate", restoreDirection); };
  });
</script>
<section class="otcWorkspace" data-testid="otc-workspace" data-state={!loaded ? "connecting" : !healthy ? "unavailable" : "connected"}>
  <OtcProductionSafe />
  <div class="pageIntro"><div><h1>{copy.titleLead}<span class="titleAccent">{copy.titleAccent}</span></h1><p>{say("Native EVER on Everscale. USDT on Ethereum. Direct desk settlement.", "Нативный EVER в Everscale. USDT в Ethereum. Прямой обмен с деском.")}</p></div></div>
  <div class="marketStrip"><div class="marketSelector"><img src="/icons/assets/ever.svg" width="38" height="38" alt="" /><div><div class="pairSelect"><strong>EVER / USDT</strong></div><span>Everscale ↔ Ethereum <span class="marketTag">OTC</span></span></div></div><div class="marketStat last"><span>{say("Your selected rate", "Выбранный курс")}</span><strong>{selected ? rate(selected.terms) : "—"} <small>USDT / EVER</small></strong></div><div class="marketStat"><span>{say("Desk", "Деск")}</span><strong>{config?.desk_name || "—"}</strong></div><div class="marketStat secondaryStat"><span>{say("Desk offers", "Предложения деска")}</span><strong>{listings.length}</strong></div><div class="marketStat secondaryStat"><span>{say("Your open trades", "Ваши открытые сделки")}</span><strong>{session ? activeTrades.length : "—"}</strong></div><div class="marketStat volumeStat"><span>{say("Your completed volume", "Ваш завершённый объём")}</span><strong>{session ? decimal(completedVolume(trades), 6) : "—"} <small>USDT</small></strong></div></div>
  <div class="serviceControls">{#if error}<p class="message error" role="alert">{error}</p>{/if}{#if notice}<p class="message" role="status">{notice}</p>{/if}{#if unresolvedBooking}<p class="message" role="status">{say("The original booking is awaiting its decision. Another selection is blocked.", "Ожидается решение по исходной сделке. Другое бронирование заблокировано.")}</p><button disabled={busy || !healthy || !bookingUnknown} on:click={() => run(reconcileBooking)}>{say("Reconcile original booking", "Проверить исходное бронирование")}</button>{/if}</div>
  <div class="otcGrid" class:bridgeClosed={!bridgeOpen} class:bookClosed={!bookOpen} style:--panel-toggle-offset={`${toggleOffset}px`}>
    {#if bridgeOpen}<div class="bridgeColumn" id="otc-bridge-column" transition:panelTransition={{ duration: motionDuration }}><OtcPanel id="otc-bridge" title={say("Exchange", "Обмен")} collapsible={false}><span slot="actions" class="panelMeta">OTC</span><OtcTicket {direction} bind:input {quote} {busy} disabled={!exactInput(input, direction) || unresolvedBooking || (needsWallets && !quote ? !(config?.wallet_login_available ?? config?.enabled) : !healthy || !config?.available || !listing || (!!quote && now > Date.parse(quote.terms.quote_by)))} desk={config?.desk_name || ""} onDirection={chooseSide} onEdit={edited} onRequest={requestQuote} onReview={() => { tab = "orders"; activityOpen = true; document.getElementById("otc-activity-section")?.scrollIntoView({ block: "start", behavior: motionDuration ? "smooth" : "instant" }); }} /></OtcPanel></div>{/if}
    <div class="bridgeToggle"><OtcPanelToggle title={say("Exchange", "Обмен")} controls="otc-bridge-column" bind:expanded={bridgeOpen} /></div>
    <div class="chartColumn" id="otc-chart-column" bind:this={chartColumn}><OtcPanel id="otc-chart" title={copy.chart} collapsible={false}><OtcQuoteChart {trades} /></OtcPanel></div>
    <div class="bookToggle"><OtcPanelToggle title={say("Desk offers", "Предложения деска")} controls="otc-book-column" bind:expanded={bookOpen} direction="right" /></div>
    {#if bookOpen}<div class="bookColumn" id="otc-book-column" transition:panelTransition={{ duration: motionDuration }}><OtcPanel id="otc-book" title={say("Desk offers", "Предложения деска")} collapsible={false}><OtcDeskBook {config} {listings} selectedId={listing?.id || ""} onSelect={selectListing} /></OtcPanel></div>{/if}
  </div>
  <p class="risk">{config?.escrow ? say("Each trade uses a 2-of-3 USDT escrow for four hours from creation, capped at 100,000 EVER. USDT is funded first. Native EVER delivery is separate and is not reversed by a USDT refund.", "Каждая сделка использует эскроу USDT с 2 из 3 подписей на 4 часа с момента создания, до 100 000 EVER. Сначала вносится USDT. Перевод EVER выполняется отдельно и не отменяется возвратом USDT.") : say("Two separate transfers: the customer pays first; the desk delivers or refunds. No atomic swap or automatic rollback.", "Два отдельных перевода: клиент платит первым, деск выполняет выплату или возврат. Атомарного обмена и автоматического отката нет.")}</p>
  <div class="activityToggle"><OtcPanelToggle title={say("Trade activity", "Активность сделок")} controls="otc-activity-section" bind:expanded={activityOpen} vertical /></div>
  {#if activityOpen}<div class="activitySection" id="otc-activity-section" transition:slide={{ duration: motionDuration, easing: cubicOut }}><OtcPanel id="otc-activity" title={say("Trade activity", "Активность сделок")} collapsible={false}><span slot="actions" class="panelMeta">EVER/USDT</span>
    <div class="activityTabs" role="tablist" aria-label={say("Trade activity", "Активность сделок")}>{#each ["trades", "orders", "history"] as item}<button type="button" role="tab" id={`otc-${item}-tab`} aria-selected={tab === item} tabindex={tab === item ? 0 : -1} aria-controls={`otc-${item}-content`} class:active={tab === item} on:keydown={navigateTab} on:click={() => tab = item as typeof tab}>{item === "trades" ? say("Desk offers", "Предложения деска") : item === "orders" ? say("My trades", "Мои сделки") : say("History", "История")}{#if item === "orders" && activeTrades.length}<span class="orderCount">{activeTrades.length}</span>{/if}</button>{/each}</div>
    <div class="activityContent" role="tabpanel" id={`otc-${tab}-content`} aria-labelledby={`otc-${tab}-tab`} tabindex="0">
      {#if tab === "trades"}<OtcDeskBook {config} {listings} selectedId={listing?.id || ""} onSelect={selectListing} />
      {:else}
        <div class="tradeList">{#each tab === "orders" ? activeTrades : history as trade}<button disabled={busy} class:selected={selected?.id === trade.id} on:click={() => run(() => selectTrade(trade.id, true), false)}><strong>{trade.terms.direction === "buy" ? say("Buy", "Купить") : say("Sell", "Продать")} EVER</strong><span>{trade.terms.input} → {trade.terms.output}</span><span>{trade.state.replaceAll("_", " ")}</span></button>{:else}<div class="emptyState"><div class="emptyIcon" aria-hidden="true">≡</div><h3>{say("No trades yet", "Сделок пока нет")}</h3><p>{session ? say("Quotes and accepted trades appear here.", "Здесь появятся котировки и принятые сделки.") : say("Connect your wallet to see private quotes and trades.", "Подключите кошелёк для доступа к приватным котировкам и сделкам.")}</p></div>{/each}</div>
        {#if tab === "orders"}{#each rfqs.filter(r => r.state === "waiting") as r}<p class="waiting">{say("Waiting for desk quote", "Ожидается котировка деска")} · {r.direction} · {r.input} · {r.id}</p>{/each}{/if}
        {#if selected}<div class="tradeDetail">{#if handoffUnknown}<p class="waiting">{say("Saved outgoing attempt is unresolved. Another send is blocked.", "Сохранённая попытка перевода не завершена. Повторная отправка заблокирована.")}</p>{/if}<OtcTradeDetails {selected} {session} {config} {attempts} {receipts} {updates} busy={busy || !healthy} {unresolvedBooking} {handoffUnknown} {now} onApply={() => run(apply)} onDecide={accept => run(() => decide(accept))} onBusyChange={value => { busy = value; accountLocked = value; }} onRefresh={refresh} onTransfer={kind => run(() => prepareReview(kind))} onTrack={(attempt, reference) => run(() => track(attempt, reference))} onRetry={(attempt, evidence) => run(async () => { await request(`/attempts/${attempt.id}/retry`, session!.token, { evidence: { note: evidence } }); await refresh(); })} /></div>{/if}
      {/if}
    </div>
  </OtcPanel></div>{/if}
  {#if session?.desk}<div class="console"><OtcDeskConsole {session} {config} {selected} {rfqs} {dashboard} {busy} {run} onRefresh={refresh} onSelect={selectTrade} /></div>{/if}
</section>

{#if transferReview}<OtcTransferReview trade={transferReview.trade} kind={transferReview.kind} instructions={transferReview.attempt.instructions} disabled={busy || !!config?.demo || transferReview.trade.terms.demo || (transferReview.attempt.instructions.escrow && now >= Date.parse(transferReview.attempt.instructions.expires_at!) - 1800000) || (!transferReview.trade.terms.escrow && transferReview.kind === "payment" && now > Date.parse(transferReview.trade.terms.pay_by))} onClose={() => transferReview = null} onConfirm={() => run(confirmTransfer)} />{/if}

<style>
  .otcWorkspace { --otc-buy: var(--color-good); --otc-sell: var(--color-danger); --otc-buy-soft: color-mix(in srgb, var(--color-good) 13%, transparent); --otc-sell-soft: color-mix(in srgb, var(--color-danger) 9%, transparent); width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter))); margin: 0 auto; padding: 34px 0 20px; }
  .pageIntro { position: relative; display: grid; justify-items: center; gap: 14px; margin: 0 auto 26px; text-align: center; }
  h1 { margin: 0 auto; max-width: 1000px; text-wrap: balance; font-size: clamp(44px, 5.5vw, 72px); font-weight: 650; letter-spacing: -.065em; line-height: .96; }
  .titleAccent { position: relative; z-index: 0; display: inline-block; }
  .titleAccent::after { position: absolute; right: -.05em; bottom: .02em; left: -.04em; z-index: -1; height: .2em; border-radius: 3px; background: var(--color-accent); content: ""; transform: rotate(-1deg); }
  .pageIntro p { max-width: 560px; margin: 12px auto 0; font-size: 16px; line-height: 1.55; color: var(--color-text-soft); }
  .marketStrip { display: flex; align-items: center; justify-content: space-between; gap: 20px; min-height: 88px; padding: 17px 22px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-paper); margin-bottom: 24px; }
  .marketSelector { display: flex; align-items: center; gap: 12px; padding-right: 25px; border-right: 1px solid var(--color-border); }
  .marketSelector > img { border-radius: 50%; }
  .pairSelect { display: flex; align-items: center; gap: 3px; }
  .pairSelect select { appearance: none; border: 0; background: transparent; font-weight: 800; font-size: 14px; cursor: pointer; padding: 3px 0; }
  .pairSelect option { background: var(--color-paper); color: var(--color-text); }
  .pairSelect svg { pointer-events: none; }
  .marketSelector > div > span { display: flex; align-items: center; gap: 8px; color: var(--color-text-soft); font-size: 12px; margin-top: 4px; }
  .marketTag { display: inline-block; background: var(--color-panel); padding: 2px 4px; border-radius: 3px; font-family: var(--font-mono); font-size: 12px; }
  .marketStat { display: grid; gap: 8px; white-space: nowrap; }
  .marketStat > span { font-size: 12px; color: var(--color-text-soft); }
  .marketStat strong { font-family: var(--font-mono); font-weight: 400; font-size: 12px; }
  .marketStat strong small { font-size: 12px; color: var(--color-text-soft); }
  .marketStat.last strong { font-size: 18px; letter-spacing: -.05em; }
  .marketStat strong.positive { color: var(--otc-buy); }
  .otcGrid { display: grid; grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.3fr) 32px minmax(0, .85fr); align-items: start; transition: grid-template-columns .38s cubic-bezier(.22, 1, .36, 1), transform .38s cubic-bezier(.22, 1, .36, 1); }
  .bridgeColumn { grid-column: 1; grid-row: 1; }
  .chartColumn { grid-column: 3; grid-row: 1; }
  .bookColumn { grid-column: 5; grid-row: 1; }
  .bridgeColumn, .chartColumn, .bookColumn { min-width: 0; }
  .bridgeColumn, .bookColumn { overflow: clip; }
  .bridgeToggle, .bookToggle { display: grid; justify-items: center; grid-row: 1; padding-top: var(--panel-toggle-offset); }
  .bridgeToggle { grid-column: 2; }
  .bookToggle { grid-column: 4; }
  .otcGrid.bridgeClosed { grid-template-columns: minmax(0, 0fr) 32px minmax(0, 1.3fr) 32px minmax(0, .85fr); }
  .otcGrid.bookClosed { grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.3fr) 32px minmax(0, 0fr); }
  .otcGrid.bridgeClosed.bookClosed { grid-template-columns: minmax(0, 0fr) 32px minmax(0, 1fr) 32px minmax(0, 0fr); }
  .splitBooks { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 16px; padding: 16px; }
  .buyBook, .sellBook { min-width: 0; }
  .buyBook :global(.panelTitle h2) { color: var(--otc-buy); }
  .sellBook :global(.panelTitle h2) { color: var(--otc-sell); }
  .activityToggle { display: grid; height: 42px; place-items: center; }
  .panelMeta { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 12px; }
  .activitySection { min-width: 0; }
  .activityTabs { display: flex; align-items: center; gap: 23px; padding: 0 20px; border-bottom: 1px solid var(--color-border); }
  .activityTabs button { position: relative; display: flex; align-items: center; gap: 7px; min-height: 48px; color: var(--color-text-soft); font-size: 12px; white-space: nowrap; }
  .activityTabs button.active { font-weight: 800; color: var(--color-text); }
  .activityTabs button.active::after { content: ""; position: absolute; left: 0; right: 0; bottom: -1px; height: 2px; border-radius: 2px; background: var(--color-accent-strong); }
  .orderCount { display: grid; place-items: center; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 4px; background: var(--color-accent-soft); font-family: var(--font-mono); font-size: 12px; }
  .activityViewport { position: relative; overflow: hidden; min-height: 237px; transition: height var(--activity-motion) cubic-bezier(.22, 1, .36, 1); }
  .activityContent { min-height: 237px; }
  .tableScroll { overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; text-align: right; font-family: var(--font-sans); font-variant-numeric: tabular-nums; font-size: 12px; white-space: nowrap; }
  th { font-family: var(--font-sans); font-size: 12px; font-weight: 500; color: var(--color-text-soft); padding: 13px 22px; border-bottom: 1px solid var(--color-border); }
  th small { font-family: var(--font-mono); font-size: 12px; margin-left: 4px; }
  td { padding: 8px 22px; }
  td:first-child, th:first-child, td:nth-child(2), th:nth-child(2) { text-align: left; }
  tbody tr:nth-child(even) { background: var(--color-panel-soft); }
  tbody tr:hover { background: var(--color-panel); }
  .muted { color: var(--color-text-soft); }
  .buyText { color: var(--otc-buy); }
  .sellText { color: var(--otc-sell); }
  .sidePill { display: inline-block; min-width: 57px; padding: 4px 7px; border-radius: 4px; text-align: center; background: var(--otc-buy-soft); color: var(--otc-buy); font-family: var(--font-sans); font-size: 12px; }
  .sidePill.sell { background: var(--otc-sell-soft); color: var(--otc-sell); }
  .statusPill { color: var(--color-text-soft); font-family: var(--font-sans); font-size: 12px; }
  .statusPill.isOpen { color: var(--otc-buy); }
  .cancelOrder { padding: 5px 8px; border: 1px solid var(--color-border); border-radius: 5px; font-family: var(--font-sans); font-size: 12px; }
  .cancelOrder:hover { border-color: var(--otc-sell); color: var(--otc-sell); }
  .emptyState { display: grid; justify-items: center; padding: 28px 16px; text-align: center; }
  .emptyIcon { display: grid; width: 47px; height: 47px; place-items: center; margin-bottom: 12px; border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-panel); color: var(--color-text-soft); }
  .emptyState h3 { font-size: 14px; font-weight: 700; }
  .emptyState p { margin-top: 7px; font-size: 12px; color: var(--color-text-soft); }
  .emptyState button { display: flex; align-items: center; gap: 16px; margin-top: 16px; padding: 9px 13px; border: 1px solid var(--color-border); border-radius: 7px; font-size: 12px; font-weight: 700; }
  .emptyState button:hover { background: var(--color-accent-soft); }
  .orderNotification { position: fixed; bottom: 24px; left: 50%; z-index: 100; display: flex; align-items: center; gap: 12px; width: max-content; max-width: calc(100vw - 32px); padding: 14px 17px; background: var(--color-paper); border: 1px solid var(--color-border); border-radius: 12px; box-shadow: 0 8px 40px #0002; transform: translateX(-50%); font-size: 12px; }
  .orderNotification > span { color: var(--color-accent-text); }
  .orderNotification button { margin-left: 8px; font-size: 18px; }
  [hidden] { display: none !important; }
  @media (max-width: 1200px) and (min-width: 981px) { .otcGrid { grid-template-columns: minmax(0, 1fr) 32px minmax(0, 1.25fr) 32px minmax(0, .9fr); } .marketStrip { padding: 16px; gap: 15px; } .marketSelector { padding-right: 15px; } .secondaryStat { display: none; } }
  @media (max-width: 980px) {
    .otcGrid, .otcGrid.bridgeClosed, .otcGrid.bookClosed, .otcGrid.bridgeClosed.bookClosed { display: flex; width: 100%; flex-direction: column; transform: none; }
    .bridgeColumn, .chartColumn, .bookColumn { width: 100%; }
    .bridgeToggle, .bookToggle { width: 100%; height: 42px; padding: 0; align-items: center; }
    .marketStrip { flex-wrap: wrap; justify-content: flex-start; gap: 16px 28px; }
    .marketSelector { border-right: 0; flex: 1; }
    .last { flex: 1; }
    .marketStat:not(.last) { flex: 1; border-top: 1px solid var(--color-border); padding-top: 12px; }
  }
  @media (max-width: 640px) {
  .otcWorkspace { padding-top: 20px; }
    h1 { font-size: clamp(36px, 9vw, 48px); }
    .pageIntro p { font-size: 14px; }
    .secondaryStat { display: none; }
    .marketStat.last strong { font-size: 16px; }
    .activityTabs { gap: 17px; padding-inline: 14px; overflow-x: auto; }
    th, td { padding-inline: 14px; }
  }
  @media (prefers-reduced-motion: reduce) { .otcGrid { transition: none; } }

.marketStat strong { overflow-wrap: anywhere; white-space: normal; } .risk { margin: 18px 0 0; font-size: 12px; color: var(--color-text-soft); line-height: 1.6; } .tradeList { display: grid; gap: 7px; padding: 18px; } .tradeList button { display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; min-height: 44px; padding: 12px; border: 1px solid var(--color-border); border-radius: 7px; text-align: left; font-size: 12px; } .tradeList button.selected { border-color: var(--color-accent-strong); } .tradeDetail { padding: 0 19px 22px; } .waiting { padding: 12px 19px; overflow-wrap: anywhere; font-size: 12px; color: var(--color-text-soft); } .console { display: grid; gap: 18px; margin-top: 24px; }
</style>
