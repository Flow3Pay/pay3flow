<script lang="ts">
  import { onMount, tick } from "svelte";
  import { locale } from "$lib/i18n";
  import { otcCopy } from "$lib/otc/copy";
  import type { OtcPreview } from "$lib/share-links";
  import { parseOtcHash } from "$lib/otc/link";
  import { request, signInWallet, OtcRequestError, isTransientReaderOutage, bindWallet, sendAttempt, getBalances, decimal, rate, setOtcNetwork, type Config, type Session, type Proposal, type Trade, type Rfq, type Attempt, type Direction, type WalletBalance } from "$lib/otc/service";
  import type { PublicOffer } from "$lib/otc/public-offers";
  import { exactInput, proposalDirection, terminalStates, completedVolume } from "$lib/otc/terminal";
  import "$lib/otc/terminal.css";
  import OtcActivityContent from "./OtcActivityContent.svelte";
  import OtcTicket from "./OtcTicket.svelte";
  import OtcDeskBook from "./OtcDeskBook.svelte";
  import OtcMarketChart from "./OtcMarketChart.svelte";
  import OtcTradeDetails from "./OtcTradeDetails.svelte";
  import OtcDeskConsole from "./OtcDeskConsole.svelte";
  import { otcWalletDialog, openWalletDialog, closeWalletDialog } from "$lib/wallet-dialog";
  import { disconnectFamily } from "$lib/wallet-session";
  import OtcTransferReview from "./OtcTransferReview.svelte";
  import OtcOffers from "./OtcOffers.svelte";
  import OtcTerminal from "./OtcTerminal.svelte";
  import OtcDealPanel from "./OtcDealPanel.svelte";
  import OtcTradeTables from "./OtcTradeTables.svelte";
  import ScrambleHeadline from "../ScrambleHeadline.svelte";
  export let onPreviewChange: (preview: OtcPreview | null) => void = () => {};
  export let onShareUrlChange: (url: string) => void = () => {};
  export let introCompleted = true;
  let config: Config | null = null, session: Session | null = null;
  let input = "100", direction: Direction = "buy", listings: Proposal[] = [], trades: Trade[] = [], rfqs: Rfq[] = [];
  let selected: Trade | null = null, attempts: Attempt[] = [], receipts: { chain: string; leg: { amount: string }; included_at: string }[] = [], updates: { message: string; at: string }[] = [];
  let dashboard: Record<string, any> | null = null, busy = false, loaded = false, healthy = false, error = "", notice = "", bound: string[] = [];
  let rfqKey = "", listingId = "", bookingUnknown = "", requestingQuote = false;
  let transferReview: { attempt: Attempt; trade: Trade; kind: string } | null = null;
  let handoff: { tradeId: string; kind: string; key: string; phase: string; attemptId?: string } | null = null;
  let motionDuration = 320, mounted = false;
  let selectionVersion = 0, accountVersion = 0, walletProgress = "", accountLocked = false;
  let tab: "trades" | "orders" | "history" = "trades", now = Date.now();
  let offersBusy = false, balances: WalletBalance[] = [];
  let offerBook: OtcDeskBook;
  let dealPanel: OtcDealPanel | null = null;
  $: saleEnabled = !!config?.public_offers_available && (config.public_offer_directions ?? ["sell"]).includes(direction) && !listingId && !quote;
  $: bookTitle = config?.public_offers_available ? say("Orderbook", "Стакан") : say("Desk offers", "Предложения деска");
  const sessionKey = "pay3flow.otc.session.v1";
  $: copy = otcCopy($locale);
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: listing = listings.find(p => p.id === listingId && proposalDirection(p, config) === direction) || listings.find(p => proposalDirection(p, config) === direction);
  $: unresolvedBooking = !!bookingUnknown || trades.some(t => t.state === "booking_pending");
  $: handoffUnknown = !!handoff && handoff.tradeId === selected?.id;
  $: quote = selected?.state === "quoted" && selected.terms.direction === direction && exactInput(input, direction) === exactInput(selected.terms.input, direction) ? selected : null;
  $: needsWallets = !session || !["ethereum", "everscale"].every(chain => bound.some(wallet => wallet.startsWith(`${chain}:`)));
  $: quoteReady = !saleEnabled && !needsWallets && healthy && !!config?.available && !!listing && !!exactInput(input, direction) && !unresolvedBooking;
  $: missingWallets = (["ethereum", "everscale"] as const).filter(chain => !bound.some(wallet => wallet.startsWith(`${chain}:`)));
  $: quoteStatus = saleEnabled ? (bound.some(wallet => wallet.startsWith("everscale:")) ? say("EVER ownership verified. You can publish an offer.", "Владение EVER-кошельком подтверждено. Можно опубликовать предложение.") : say("Verify your EVER wallet to publish an offer.", "Подтвердите EVER-кошелёк для публикации предложения."))
    : !loaded ? say("Checking OTC availability…", "Проверяем доступность OTC…")
    : !healthy ? say("OTC is unavailable. Reconnecting automatically.", "OTC недоступен. Соединение восстановится автоматически.")
    : !config?.available ? say("OTC quotes are currently paused. Connecting a wallet does not enable trading.", "Котировки OTC сейчас отключены. Подключение кошелька не включает торговлю.")
    : missingWallets.some(chain => !(config?.wallet_login_chains ?? ["ethereum", "everscale"]).includes(chain)) ? say("A required wallet network is unavailable on this server. Quotes cannot be requested yet.", "Подключение одной из необходимых сетей недоступно на сервере. Запросить котировку пока нельзя.")
    : needsWallets ? say(`Verify ${missingWallets.map(chain => chain === "ethereum" ? "Ethereum" : "Everscale").join(" + ")} wallet ownership to request a quote.`, `Подтвердите владение кошельком ${missingWallets.map(chain => chain === "ethereum" ? "Ethereum" : "Everscale").join(" + ")}, чтобы запросить котировку.`)
    : unresolvedBooking ? say("Reconcile the pending booking before requesting another quote.", "Проверьте незавершённое бронирование перед новым запросом.")
    : !exactInput(input, direction) ? say("Enter a valid amount to request a quote.", "Укажите корректную сумму для запроса котировки.")
    : !listing ? say("No desk offers are available for this direction yet.", "Для этого направления пока нет предложений деска.") : "";
  $: if (mounted) otcWalletDialog.set({
    session, bound, busy, error, progress: walletProgress, disconnectDisabled: accountLocked || offersBusy,
    chains: config?.wallet_login_chains ?? ["ethereum", "everscale"],
    enabled: !!(config?.wallet_login_available ?? config?.enabled), requestingQuote, quoteReady, quoteStatus,
    onClose: closeIdentity,
    onConnect: chain => run(() => connectAccount(chain)),
    onVerify: chain => run(() => verify(chain)),
    onDisconnect: () => disconnect().catch(e => { error = e instanceof Error ? e.message : "Disconnect failed"; }),
    onRequest: () => { closeWalletDialog(); void run(rfq); },
  });
  $: activeTrades = trades.filter(t => !terminalStates.has(t.state));
  $: history = trades.filter(t => terminalStates.has(t.state));
  function activityIndicator(node: HTMLDivElement, _key: string) {
    const measure = () => {
      const active = node.querySelector<HTMLElement>('[aria-selected="true"]');
      if (!active) return;
      const box = node.getBoundingClientRect(), rect = active.getBoundingClientRect();
      node.style.setProperty("--active-tab-left", `${rect.left - box.left}px`);
      node.style.setProperty("--active-tab-width", `${rect.width}px`);
    };
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    node.querySelectorAll("button").forEach(button => observer.observe(button));
    measure();
    return { update: () => { void tick().then(measure); }, destroy: () => observer.disconnect() };
  }
  function syncLink() {
    // Only the public pair/direction are shared; participant terms never enter previews or URLs.
    const hash = `#/otc?market=EVER-USDT&side=${direction}`;
    if (/^#\/otc(?:[/?]|$)/.test(location.hash)) historyReplace(hash);
    onShareUrlChange(`/${hash}`); onPreviewChange(null);
  }
  function historyReplace(hash: string) { window.history.replaceState(window.history.state, "", `${location.pathname}${location.search}${hash}`); }
  const chainLabel = (chain: "ethereum" | "everscale") => (chain === "ethereum" ? "Ethereum" : "Everscale");
  async function run(action: () => Promise<void>, lockAccount = true) {
    if (busy || offersBusy) return; busy = true; accountLocked = lockAccount; error = ""; notice = "";
    try { await action(); } catch (e) {
      // 403 is only ever a lapsed wallet verification: re-read the session so the expired wallet stops showing as connected.
      if (e instanceof OtcRequestError && e.status === 403) {
        await refresh();
        const days = config?.wallet_verification_days ?? 7, names = missingWallets.map(chainLabel).join(" + ");
        error = missingWallets.length
          ? say(`Wallet verification lasts ${days} days and has lapsed. Confirm ${names} again.`, `Подтверждение кошелька действует ${days} дней и истекло. Подтвердите ${names} заново.`)
          : say("Wallet verification has lapsed. Connect your wallet again to continue.", "Подтверждение кошелька истекло. Подключите кошелёк заново, чтобы продолжить.");
      } else error = e instanceof Error ? e.message : "Request failed";
    } finally { busy = false; accountLocked = false; walletProgress = ""; }
  }
  async function refresh() {
    const refreshToken = session?.token;
    try {
      const [route, proposals] = await Promise.all([request<Config>("/config"), request<Proposal[]>("/listings")]);
      if (!mounted) return;
      config = route; listings = proposals; healthy = true; loaded = true;
      try { setOtcNetwork(route); void dealPanel?.refresh(); } catch (e) { error = e instanceof Error ? e.message : "Unsupported OTC network"; }
      if (!session) return;
      const owner = session, previousSelection = selected?.id, previousVersion = selectionVersion;
      const current = await request<Session>("/session", owner.token);
      if (!mounted || session?.token !== owner.token) return;
      retainSession(current);
      const page = await request<{ trades: Trade[]; rfqs: Rfq[] }>("/trades", owner.token);
      if (!mounted || session?.token !== owner.token) return;
      trades = page.trades; rfqs = page.rfqs;
      if (!balances.length) void pollBalances();
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
  async function pollBalances() {
    const owner = session;
    if (!owner) { balances = []; return; }
    try { const result = await getBalances(owner.token); if (session?.token === owner.token) balances = result; } catch { balances = []; }
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
  function selectListing(proposal: Proposal, side: Direction) { chooseSide(side); if (listingId !== proposal.id) edited(); listingId = proposal.id; document.getElementById("otc-input")?.focus(); }
  function openOffer(offer: PublicOffer) { void tick().then(() => offerBook?.openOffer(offer)); }
  async function acceptOffer(id: string) {
    const owner = session;
    tab = "orders";
    if (!owner) { error = say("Connect your wallet to accept an offer.", "Подключите кошелёк, чтобы принять оффер."); return; }
    await run(async () => {
      await request(`/offers/${id}/accept`, owner.token, {}, crypto.randomUUID());
      notice = say("Offer accepted. Send the first transfer to start settlement.", "Оффер принят. Отправьте первый перевод, чтобы начать расчёт.");
      await tick();
      void dealPanel?.refresh();
    });
    await refresh();
  }
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
    notice = `Quote requested. Reference: ${result.id}`; tab = "orders"; await refresh();
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
    if (accountLocked || offersBusy) return;
    const owner = session;
    clearAccount();
    const results = await Promise.allSettled([
      disconnectFamily("evm"),
      disconnectFamily("everscale"),
      ...(owner ? [request("/session/revoke", owner.token, {})] : []),
    ]);
    const failed = results.find(result => result.status === "rejected");
    if (failed?.status === "rejected") throw failed.reason;
  }
  function openIdentity(forQuote = false) { requestingQuote = forQuote; openWalletDialog("otc"); }
  function closeIdentity() { requestingQuote = false; accountVersion++; }
  function navigateTab(event: KeyboardEvent) {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return; event.preventDefault(); const tabs = ["trades", "orders", "history"] as const;
    tab = tabs[event.key === "Home" ? 0 : event.key === "End" ? 2 : (tabs.indexOf(tab) + (event.key === "ArrowRight" ? 1 : -1) + 3) % 3]; document.getElementById(`otc-${tab}-tab`)?.focus();
  }
  onMount(() => {
    const media = matchMedia("(prefers-reduced-motion: reduce)"); const motionChanged = () => motionDuration = media.matches ? 0 : 320; motionChanged(); media.addEventListener("change", motionChanged);
    const restoreDirection = () => { const shared = parseOtcHash(location.hash); if (shared.marketId === "GNK-USDT") return; if (shared.side) direction = shared.side; edited(); syncLink(); };
    const shared = parseOtcHash(location.hash); direction = shared.side || "buy"; if (shared.amount && exactInput(shared.amount, direction)) input = shared.amount;
    let restoreToken = "";
    try {
      const saved = JSON.parse(sessionStorage.getItem(sessionKey) || "null");
      if (typeof saved?.token === "string") restoreToken = saved.token;
      if (saved?.actor && !sessionStorage.getItem("pay3flow.otc.recovery.actor")) sessionStorage.setItem("pay3flow.otc.recovery.actor", saved.actor);
    } catch { /* Ignore malformed saved sessions. */ }

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
      await refresh(); const id = sessionStorage.getItem("pay3flow.otc.selected"); if (id && session) { tab = "orders"; await selectTrade(id, true); } }, false);
    const timer = setInterval(async () => { if (stopped || busy || polling) return; polling = true; try { await refresh(); } catch (e) { error = e instanceof Error ? e.message : "Status unavailable"; } finally { polling = false; } }, 5000);
    // Display-only chain reads: they can wait for the RPCs instead of the 5-second desk poll.
    const walletTimer = setInterval(() => void pollBalances(), 30000);
    const clock = setInterval(() => now = Date.now(), 1000);
    window.addEventListener("hashchange", restoreDirection); window.addEventListener("popstate", restoreDirection);
    return () => { closeWalletDialog(); otcWalletDialog.set(null); stopped = true; mounted = false; accountVersion++; clearInterval(timer); clearInterval(walletTimer); clearInterval(clock); media.removeEventListener("change", motionChanged); window.removeEventListener("hashchange", restoreDirection); window.removeEventListener("popstate", restoreDirection); };
  });
</script>
<OtcOffers enabled={!!config?.public_offers_available} {direction} {session} {busy} bind:posting={offersBusy} onConnect={() => run(() => session ? verify("everscale") : connectAccount("everscale"))} onPublished={() => { tab = "trades"; }} let:ticket let:offers let:mine let:history={offerHistory} let:message let:offerError let:onCancel>
<section class="otcWorkspace" data-testid="otc-workspace" data-state={!loaded ? "connecting" : !healthy ? "unavailable" : "connected"}>
  <div class="pageIntro" class:localeLong={$locale !== "en"}><ScrambleHeadline phrases={copy.heroPhrases} active={introCompleted} /><p>{config?.public_offers_available ? say("Native EVER on Everscale. USDT on Ethereum. Your OTC offers.", "Нативный EVER в Everscale. USDT в Ethereum. Ваши предложения OTC.") : say("Native EVER on Everscale. USDT on Ethereum. Direct desk settlement.", "Нативный EVER в Everscale. USDT в Ethereum. Прямой обмен с деском.")}</p></div>
  <OtcTerminal chartPanel={false} bookPanel={false} selectedRate={saleEnabled ? ticket.askingRate : selected ? rate(selected.terms) : "—"} rateLabel={saleEnabled ? say("Your offer price", "Цена вашего оффера") : ""} {bookTitle} stats={[
    {label:say("Desk","Деск"),value:config?.desk_name || "—"},
    {label:config?.public_offers_available ? say("Open offers","Открытые офферы") : say("Desk offers","Предложения деска"),value:String(listings.length + offers.length),secondary:true},
    {label:config?.public_offers_available ? say("Your open orders","Ваши открытые ордера") : say("Your open trades","Ваши открытые сделки"),value:session ? String(activeTrades.length + mine.filter(o => o.state === "open").length) : "—",secondary:true},
    {label:say("Your completed volume","Ваш завершённый объём"),value:session ? decimal(completedVolume(trades),6) : "—",unit:"USDT"},
  ]}>
  <div slot="status" class="serviceControls">{#if offerError && !isTransientReaderOutage(offerError)}<p class="message error" role="alert">{offerError}</p>{/if}{#if message}<p class="message" role="status">{message}</p>{/if}{#if walletProgress}<p class="message" role="status">{walletProgress}</p>{/if}{#if error && !isTransientReaderOutage(error)}<p class="message error" role="alert">{error}</p>{/if}{#if notice}<p class="message" role="status">{notice}</p>{/if}{#if unresolvedBooking}<p class="message" role="status">{say("The original booking is awaiting its decision. Another selection is blocked.", "Ожидается решение по исходной сделке. Другое бронирование заблокировано.")}</p><button disabled={busy || !healthy || !bookingUnknown} on:click={() => run(reconcileBooking)}>{say("Reconcile original booking", "Проверить исходное бронирование")}</button>{/if}</div>
    <svelte:fragment slot="ticket"><OtcTicket sale={saleEnabled ? ticket : null} {direction} bind:input {quote} {balances} busy={busy || offersBusy} disabled={!exactInput(input, direction) || unresolvedBooking || (needsWallets && !quote ? !(config?.wallet_login_available ?? config?.enabled) : !healthy || !config?.available || !listing || (!!quote && now > Date.parse(quote.terms.quote_by)))} status={quote || saleEnabled ? "" : quoteStatus} onDirection={chooseSide} onEdit={edited} onRequest={requestQuote} onReview={() => { tab = "orders"; document.getElementById("otc-book-column")?.scrollIntoView({ block: "start", behavior: motionDuration ? "smooth" : "instant" }); }} /></svelte:fragment>
    <OtcMarketChart slot="chart" {trades} publicOffers={config?.public_offers_available && (saleEnabled || !trades.length) ? offerHistory : null} />
    <svelte:fragment slot="book">
      <section class="activityCard" id="otc-activity" aria-label={say("Trade activity", "Активность сделок")}>
        <div class="activityTabs" role="tablist" aria-label={say("Trade activity", "Активность сделок")} use:activityIndicator={`${tab}:${$locale}:${activeTrades.length}`} style:--activity-motion={`${motionDuration}ms`}>{#each ["trades", "orders", "history"] as item}<button type="button" role="tab" id={`otc-${item}-tab`} aria-selected={tab === item} tabindex={tab === item ? 0 : -1} aria-controls={`otc-${item}-content`} class:active={tab === item} on:keydown={navigateTab} on:click={() => tab = item as typeof tab}>{item === "trades" ? config?.public_offers_available ? say("Orderbook", "Стакан") : say("Desk offers", "Предложения деска") : item === "orders" ? config?.public_offers_available ? say("My orders", "Мои заявки") : say("My trades", "Мои сделки") : say("History", "История")}{#if item === "orders" && activeTrades.length + mine.filter(o => o.state === "open").length}<span class="orderCount">{activeTrades.length + mine.filter(o => o.state === "open").length}</span>{/if}</button>{/each}<span class="tabIndicator" aria-hidden="true"></span></div>
        <OtcActivityContent activeKey={tab} duration={motionDuration}>
        <div class="activityContent" role="tabpanel" id={`otc-${tab}-content`} aria-labelledby={`otc-${tab}-tab`} tabindex="0">
          {#if tab === "trades"}
            <OtcDeskBook publicOffers={offers} ownIds={mine.map(o => o.id)} onCancel={onCancel} onAccept={acceptOffer} busy={busy || offersBusy} {config} {listings} selectedId={listing?.id || ""} onSelect={selectListing} />
          {:else}
            {#if tab === "orders"}<OtcDealPanel bind:this={dealPanel} {config} {session} locked={busy || offersBusy} {now} onNotice={message => notice = message} onError={message => error = message} onBusyChange={value => busy = value} />{/if}
            <OtcTradeTables trades={tab === "orders" ? activeTrades : history} offers={tab === "orders" ? mine.filter(o => o.state === "open") : mine.filter(o => o.state !== "open")} selectedId={selected?.id || ""} busy={busy || offersBusy} onSelect={id => run(() => selectTrade(id, true), false)} onOfferSelect={openOffer} emptyTitle={tab === "orders" ? say("No open orders", "Открытых ордеров нет") : say("No history yet", "Истории пока нет")} emptyHint={session ? say("Quotes, offers and accepted trades appear here.", "Здесь появятся котировки, офферы и принятые сделки.") : say("Connect your wallet to see private quotes and trades.", "Подключите кошелёк для доступа к приватным котировкам и сделкам.")} />
            {#if tab === "orders"}{#each rfqs.filter(r => r.state === "waiting") as r}<p class="waiting">{say("Waiting for desk quote", "Ожидается котировка деска")} · {r.direction} · {r.input} · {r.id}</p>{/each}{/if}
            {#if selected}<div class="tradeDetail">{#if handoffUnknown}<p class="waiting">{say("Saved outgoing attempt is unresolved. Another send is blocked.", "Сохранённая попытка перевода не завершена. Повторная отправка заблокирована.")}</p>{/if}<OtcTradeDetails {selected} {session} {config} {attempts} {receipts} {updates} busy={busy || !healthy} {unresolvedBooking} {handoffUnknown} {now} onApply={() => run(apply)} onDecide={accept => run(() => decide(accept))} onBusyChange={value => { busy = value; accountLocked = value; }} onRefresh={refresh} onTransfer={kind => run(() => prepareReview(kind))} onTrack={(attempt, reference) => run(() => track(attempt, reference))} onRetry={(attempt, evidence) => run(async () => { await request(`/attempts/${attempt.id}/retry`, session!.token, { evidence: { note: evidence } }); await refresh(); })} /></div>{/if}
          {/if}
        </div>
        </OtcActivityContent>
      </section>
    </svelte:fragment>
  </OtcTerminal>
  <p class="risk">{config?.p2p_available ? say("Accepting an offer creates two direct transfers between the wallets of both parties. There is no escrow and no automatic rollback: whoever sends first relies on the counterparty delivering afterwards.", "Принятие оффера создаёт два прямых перевода между кошельками сторон. Эскроу и автоматического отката нет: кто отправляет первым, полагается на то, что контрагент выполнит свой перевод следом.") : config?.public_offers_available && !config.available ? say("Publication does not move or reserve funds. Buyer acceptance and settlement through this site are not available yet.", "Публикация не переводит и не резервирует средства. Принятие предложения покупателем и расчёты через сайт пока недоступны.") : config?.escrow ? say("Each trade uses a 2-of-3 USDT escrow for four hours from creation, capped at 100,000 EVER. USDT is funded first. Native EVER delivery is separate and is not reversed by a USDT refund.", "Каждая сделка использует эскроу USDT с 2 из 3 подписей на 4 часа с момента создания, до 100 000 EVER. Сначала вносится USDT. Перевод EVER выполняется отдельно и не отменяется возвратом USDT.") : say("Two separate transfers: the customer pays first; the desk delivers or refunds. No atomic swap or automatic rollback.", "Два отдельных перевода: клиент платит первым, деск выполняет выплату или возврат. Атомарного обмена и автоматического отката нет.")}</p>
  {#if session?.desk}<div class="console"><OtcDeskConsole {session} {config} {selected} {rfqs} {dashboard} {busy} {run} onRefresh={refresh} onSelect={selectTrade} /></div>{/if}
</section>
<OtcDeskBook bind:this={offerBook} dialogOnly publicOffers={offers} ownIds={mine.map(o => o.id)} onCancel={onCancel} onAccept={acceptOffer} onSelect={() => {}} busy={busy || offersBusy} {config} />
</OtcOffers>

{#if transferReview}<OtcTransferReview trade={transferReview.trade} kind={transferReview.kind} instructions={transferReview.attempt.instructions} disabled={busy || !!config?.demo || transferReview.trade.terms.demo || (transferReview.attempt.instructions.escrow && now >= Date.parse(transferReview.attempt.instructions.expires_at!) - 1800000) || (!transferReview.trade.terms.escrow && transferReview.kind === "payment" && now > Date.parse(transferReview.trade.terms.pay_by))} onClose={() => transferReview = null} onConfirm={() => run(confirmTransfer)} />{/if}

<style>
  .otcWorkspace {
    --otc-buy: var(--color-good); --otc-sell: var(--color-danger);
    --otc-buy-soft: color-mix(in srgb, var(--color-good) 13%, transparent);
    --otc-sell-soft: color-mix(in srgb, var(--color-danger) 9%, transparent);
    --otc-panel-padding: 20px; --otc-control-height: 44px;
    width: min(var(--layout-width), calc(100% - 2 * var(--page-gutter)));
    margin: 0 auto; padding: 32px 0 24px; font-size: 13px; line-height: 1.5;
  }
  /* The startup headline docks onto this one, so both share the same metrics. */
  .pageIntro { width: min(900px, 100%); margin: 0 auto 32px; text-align: center; }
  .pageIntro h1 { margin: 0; color: var(--color-text); font-size: clamp(44px, 5.5vw, 72px); font-weight: 650; letter-spacing: -.065em; line-height: .96; }
  .pageIntro.localeLong h1 { font-size: clamp(42px, 5vw, 68px); }
  .titleAccent { position: relative; z-index: 0; display: inline-block; margin-inline-start: .06em; }
  .titleAccent::after { position: absolute; right: -.05em; bottom: .02em; left: -.04em; z-index: -1; height: .2em; border-radius: 3px; background: var(--color-accent); content: ""; transform: rotate(-1deg); }
  .pageIntro p { max-width: 560px; margin: 12px auto 0; color: var(--color-text-soft); font-size: 14px; line-height: 1.6; }
  .risk { margin: 16px 0 0; color: var(--color-text-soft); font-size: 12px; line-height: 1.6; }
  .panelMeta { color: var(--color-text-faint); font-family: var(--font-mono); font-size: 12px; }
  .activityCard { display: flex; width: 100%; min-width: 0; flex-direction: column; border: 1px solid var(--color-border-strong); border-radius: var(--otc-card-radius, 24px); background: var(--exchange-card-bg); overflow: hidden; scroll-margin-top: 24px; }
  .activityTabs { position: relative; display: flex; flex: 0 0 auto; align-items: stretch; gap: 4px; margin: 16px var(--otc-panel-padding) 8px; padding: 4px; border-radius: 12px; background: var(--color-panel); box-shadow: inset 0 0 0 1px var(--color-border); }
  .activityTabs button { position: relative; z-index: 1; display: flex; flex: 1 1 0; min-width: 0; min-height: 38px; align-items: center; justify-content: center; gap: 8px; padding: 6px 10px; border-radius: 9px; color: var(--color-text-soft); font-size: 13px; line-height: 1.35; text-align: center; transition: color var(--activity-motion) ease; }
  .activityTabs button:hover { color: var(--color-text); }
  .activityTabs button.active { color: var(--color-text); font-weight: 650; }
  .tabIndicator { position: absolute; z-index: 0; top: 4px; left: 0; width: var(--active-tab-width, 0px); height: calc(100% - 8px); border-radius: 9px; background: var(--color-paper); box-shadow: 0 1px 4px rgb(0 0 0 / .08); transform: translateX(var(--active-tab-left, 0px)); transition: transform var(--activity-motion) cubic-bezier(.22, 1, .36, 1), width var(--activity-motion) cubic-bezier(.22, 1, .36, 1); pointer-events: none; }
  .orderCount { display: grid; flex: 0 0 auto; min-width: 20px; height: 20px; padding: 0 4px; place-items: center; border-radius: 4px; background: var(--color-accent-soft); font-family: var(--font-mono); font-size: 11px; }
  .activityContent { min-height: 240px; }
  .activityContent:focus-visible { outline-offset: -3px; }
  .tradeList { display: grid; gap: 8px; padding: var(--otc-panel-padding); }
  .tradeList:has(.emptyState) { padding: 0; }
  .tradeList button { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1fr); min-width: 0; min-height: 48px; align-items: center; gap: 16px; padding: 12px 16px; border: 1px solid var(--color-border); border-radius: 8px; text-align: left; font-size: 13px; line-height: 1.5; }
  .tradeList button.selected { border-color: var(--color-accent-strong); background: var(--color-panel-soft); }
  .tradeList button > * { min-width: 0; overflow-wrap: anywhere; }
  .tradeList button span:last-child { color: var(--color-text-soft); text-align: right; }
  .tradeDetail { min-width: 0; padding: 0 var(--otc-panel-padding) var(--otc-panel-padding); }
  .waiting { padding: 12px var(--otc-panel-padding); overflow-wrap: anywhere; color: var(--color-text-soft); font-size: 13px; }
  .emptyState { display: grid; min-height: 240px; align-content: center; justify-items: center; gap: 8px; padding: 32px var(--otc-panel-padding); text-align: center; }
  .emptyIcon { display: grid; width: 44px; height: 44px; place-items: center; margin-bottom: 4px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); color: var(--color-text-soft); font-size: 20px; }
  .emptyState h3 { font-size: 14px; font-weight: 650; }
  .emptyState p { max-width: 420px; color: var(--color-text-soft); font-size: 13px; line-height: 1.6; }
  .console { display: grid; gap: 24px; margin-top: 24px; }
  @media (max-width: 980px) {
  }
  @media (max-width: 640px) {
    .otcWorkspace { --otc-panel-padding: 16px; padding-top: 24px; }
    .pageIntro { margin-bottom: 24px; }
    .pageIntro h1 { font-size: clamp(30px, 8vw, 44px); }
    .pageIntro p { font-size: 13px; }
    .activityTabs { margin: 12px 12px 4px; gap: 2px; }
    .activityTabs button { min-height: 44px; padding: 6px; font-size: 12px; }
    .tradeList button { grid-template-columns: minmax(0, 1fr) auto; gap: 8px 12px; }
    .tradeList button span:last-child { grid-column: 1 / -1; text-align: left; }
  }
  @media (prefers-reduced-motion: reduce) { .activityTabs button, .tabIndicator { transition: none; } }
</style>
