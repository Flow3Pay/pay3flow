<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import type { OtcPreview } from "$lib/share-links";
  import { parseOtcHash } from "$lib/otc/link";
  import { venueWalletMenu } from "$lib/wallet-dialog";
  import { request } from "$lib/hex/api";
  import { GNK, USDT, CONTRACT, decimal, levels, makeDraft, humanPrice, minimumOutput, refundedInput, isQuoteCurrent, settledTrades, chartPoints,
    type Config, type Snapshot, type Account, type HistoryEntry, type Side, type OrderType, type Draft, type Quote, type Order } from "$lib/hex/model";
  import * as wallet from "$lib/hex/wallet";
  import OtcTerminal from "./OtcTerminal.svelte";
  import OtcPanel from "./OtcPanel.svelte";
  import "$lib/otc/terminal.css";
  export let onPreviewChange: (preview: OtcPreview | null) => void = () => {};
  export let onShareUrlChange: (url: string) => void = () => {};
  let config: Config | null = null, snapshot: Snapshot | null = null, accountData: Account | null = null;
  let wallets: wallet.Wallet[] = [], owner = "", history: HistoryEntry[] = [], personalHistory: HistoryEntry[] = [];
  let side: Side = "buy", type: OrderType = "market", amount = "", price = "", slippage = "0.5";
  let error = "", notice = "", historyError = "", personalHistoryError = "", accountError = "", healthy = false, loaded = false, busy = false;
  let pending: wallet.Pending | null = null, mounted = false, generation = 0, now = Date.now(), bridgeOpen = true, bookOpen = true;
  let tab: "orders" | "history" = "orders", range = "1M", moreOrders = true, moreHistory = true;
  let review: { draft: Draft; quote: Quote; account: string; slippage: string; minimum: string } | null = null;
  let confirmAction: { kind: "cancel" | "claim"; order?: Order } | null = null;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: bids = levels(snapshot?.book.bids ?? [], "buy");
  $: asks = levels(snapshot?.book.asks ?? [], "sell");
  $: execution = !!config?.execution_enabled && wallet.configured();
  $: balance = accountData?.balances[side === "buy" ? 1 : 0] ?? "0";
  $: claimable = [GNK,USDT].some(token => BigInt(accountData?.pending[token] ?? "0") > 0n);
  $: filteredHistory = settledTrades(history).filter(t => now - t.timestamp * 1000 <= ({"1D":86400000,"7D":604800000,"1M":2592000000}[range] ?? Infinity));
  $: points = chartPoints(filteredHistory);
  $: latest = filteredHistory.at(-1);
  $: canReview = execution && healthy && !!owner && !!accountData && !pending && !busy && !!amount && (type !== "limit" || !!price);
  $: if (mounted) venueWalletMenu.set({ count:wallets.length, disabled:busy || !execution, onOpen:() => { void run(owner ? wallet.profile : connect); } });
  function syncLink() {
    if (!mounted) return;
    const query = new URLSearchParams({market:"GNK-USDT",side,type});
    if (/^\d+(?:[.,]\d+)?$/.test(amount) && amount.length <= 64) query.set("amount",amount.replace(",","."));
    if (type === "limit" && /^\d+(?:[.,]\d+)?$/.test(price) && price.length <= 64) query.set("price",price.replace(",","."));
    const hash = `#/otc?${query}`;
    window.history.replaceState(window.history.state,"",`${location.pathname}${location.search}${hash}`);
    onShareUrlChange(`/${hash}`);
  }
  function edit() { review = null; notice = ""; syncLink(); }
  function chooseSide(value: Side) { side = value; edit(); }
  function pickPrice(value: string, next: Side) { side = next; type = "limit"; price = value; edit(); }
  async function run(action: () => Promise<void>) {
    if (busy) return;
    busy = true; error = ""; notice = "";
    try { await action(); }
    catch (e) { if (mounted) error = e instanceof Error ? e.message : String(e); }
    finally { if (mounted) { busy = false; pending = owner ? wallet.pending(owner) : null; } }
  }
  async function loadMarket(signal?: AbortSignal) {
    try {
      const [cfg,data] = await Promise.all([request<Config>("/config",undefined,signal),request<Snapshot>("/snapshot",undefined,signal)]);
      if (!mounted) return;
      if (cfg.contract !== CONTRACT || cfg.base_token !== GNK || cfg.quote_token !== USDT) throw new Error("HEX market configuration differs from the reviewed GNK market");
      config = cfg; snapshot = data; healthy = true; loaded = true;
    } catch (e) { if (mounted) { healthy = false; loaded = true; error = e instanceof Error ? e.message : String(e); } }
  }
  async function loadHistory(signal?: AbortSignal) {
    try {
      const page = await request<{transactions:HistoryEntry[]}>("/history",undefined,signal);
      if (mounted) { history = page.transactions; historyError = ""; }
    } catch { if (mounted) historyError = say("HEX history is unavailable. Retrying automatically.","История HEX недоступна. Повторяем запрос автоматически."); }
  }
  async function loadAccount(append = false) {
    const current = owner, version = generation;
    if (!current) return;
    const query = new URLSearchParams({account:current,skip:String(append ? accountData?.orders.length ?? 0 : 0)});
    try {
      const data = await request<Account>(`/account?${query}`);
      if (!mounted || version !== generation || current !== owner) return;
      moreOrders = data.orders.length === 100;
      accountData = { ...data, orders: append ? [...accountData?.orders ?? [], ...data.orders] : data.orders };
      accountError = "";
    } catch (e) {
      if (mounted && version === generation) { accountData = null; accountError = e instanceof Error ? e.message : String(e); }
    }
  }
  async function loadPersonalHistory(append = false) {
    const current = owner, version = generation;
    if (!current) return;
    try {
      const query = new URLSearchParams({account:current,skip:String(append ? personalHistory.length : 0)});
      const data = await request<{transactions:HistoryEntry[]}>(`/history?${query}`);
      if (!mounted || version !== generation || current !== owner) return;
      personalHistory = append ? [...personalHistory,...data.transactions] : data.transactions;
      personalHistoryError = "";
      moreHistory = data.transactions.length === 100;
    } catch { if (mounted && version === generation) personalHistoryError = say("Account history is unavailable.","История аккаунта недоступна."); }
  }
  function selectAccount(account: string) {
    if (account === owner) return;
    generation++; owner = account; review = null; confirmAction = null; accountData = null; personalHistory = []; accountError = ""; personalHistoryError = "";
    pending = owner ? wallet.pending(owner) : null;
    if (owner) void Promise.all([loadAccount(),loadPersonalHistory()]);
  }
  async function connect() {
    const result = await wallet.connect(); if (mounted) selectAccount(result.account);
  }
  async function getReview() {
    const draft = makeDraft(side,type,amount,price), current = owner, version = generation;
    await loadAccount();
    if (!mounted || current !== owner || version !== generation) return;
    if (!accountData) throw new Error(say("Refresh the wallet balance before trading.","Обновите баланс кошелька перед сделкой."));
    if (BigInt(draft.amount) > BigInt(accountData.balances[draft.side === "buy" ? 1 : 0])) throw new Error(say("Insufficient HEX balance. Deposit funds first.","Недостаточный баланс HEX. Сначала внесите средства."));
    const quote = await request<Quote>("/quote",draft);
    if (!mounted || current !== owner || version !== generation) return;
    const minimum = draft.order_type === "market" ? minimumOutput(quote.result.book_fill_result.taker_receive_total,slippage) : "0";
    if (BigInt(quote.result.book_fill_result.taker_receive_total) === 0n && BigInt(quote.result.taker_remainder?.new_order ?? "0") === 0n) throw new Error(say("No executable liquidity or resting order for this amount.","Для этой суммы нет доступной ликвидности или возможности разместить заявку."));
    review = {draft,quote,account:current,slippage,minimum};
  }
  async function confirmOrder() {
    const original = review;
    if (!original || original.account !== owner || !execution || !healthy || pending) return;
    if (!isQuoteCurrent(original.quote)) {
      await getReview();
      notice = say("The quote was refreshed. Review the updated terms and confirm again.","Котировка обновлена. Проверьте новые условия и подтвердите снова."); return;
    }
    const hash = await wallet.place(original.account,original.draft,original.quote,original.slippage);
    if (!mounted) return;
    review = null; notice = hash ? say("Transaction confirmed.","Транзакция подтверждена.") : say("Submitted. Awaiting settlement.","Отправлено. Ожидаем расчёта.");
    await Promise.all([loadAccount(),loadPersonalHistory(),loadMarket(),loadHistory()]);
  }
  async function checkPending() {
    const account = owner;
    const hash = await wallet.reconcile(account);
    if (!mounted || account !== owner) return;
    pending = wallet.pending(account);
    notice = hash ? say(`Transaction confirmed: ${hash}`,`Транзакция подтверждена: ${hash}`) : say("Awaiting HEX settlement.","Ожидаем расчёта HEX.");
    await Promise.all([loadAccount(),loadPersonalHistory(),loadMarket(),loadHistory()]);
  }
  async function executeAction() {
    const action = confirmAction, account = owner;
    if (!action || !execution || !healthy || pending) return;
    if (action.kind === "cancel" && action.order?.order.maker !== account) throw new Error("This order belongs to another wallet");
    const hash = action.kind === "cancel" ? await wallet.cancel(account,action.order!.hash) : await wallet.claim(account);
    if (!mounted || account !== owner) return;
    confirmAction = null; notice = hash ? say("Transaction confirmed.","Транзакция подтверждена.") : say("Submitted. Awaiting settlement.","Отправлено. Ожидаем расчёта.");
    await Promise.all([loadAccount(),loadPersonalHistory(),loadMarket()]);
  }
  async function funding(action: "deposit" | "withdraw", token: string) {
    review = null; await wallet.fund(action,token,owner);
    if (mounted) await loadAccount();
  }
  function modal(node: HTMLDialogElement) {
    const active = document.activeElement as HTMLElement | null;
    node.showModal(); return { destroy() { active?.focus(); } };
  }
  function explorer(hash: string) { return `https://nearblocks.io/txns/${encodeURIComponent(hash)}`; }
  function number(value: string | number) { return Number(value).toLocaleString($locale === "ru" ? "ru-RU" : "en-US",{maximumFractionDigits:9}); }
  onMount(() => {
    mounted = true;
    const sync = () => {
      const shared = parseOtcHash(location.hash);
      if (shared.marketId !== "GNK-USDT") return;
      side = shared.side ?? "buy"; type = shared.type ?? "market"; amount = shared.amount ?? ""; price = shared.price ?? ""; review = null; syncLink();
    };
    sync(); const abort = new AbortController();
    void loadMarket(abort.signal); void loadHistory(abort.signal);
    let unwatch = () => {}, polling = false;
    if (wallet.configured()) void wallet.watch(list => {
      if (!mounted) return;
      wallets = list;
      if (!list.some(w => w.account === owner)) selectAccount(list[0]?.account ?? "");
    }).then(off => { if (mounted) unwatch = off; else off(); }).catch(e => { if (mounted) error = String(e); });
    const timer = setInterval(async () => {
      if (!mounted || polling || busy || document.visibilityState !== "visible") return;
      polling = true;
      try {
        await Promise.all([loadMarket(abort.signal),loadHistory(abort.signal),loadAccount(),loadPersonalHistory()]);
        if (owner) { pending = wallet.pending(owner); if (pending?.intentHash) await checkPending(); }
      } catch (e) { if (mounted) error = e instanceof Error ? e.message : String(e); }
      finally { polling = false; }
    },5000);
    const clock = setInterval(() => now = Date.now(),1000);
    window.addEventListener("hashchange",sync); window.addEventListener("popstate",sync);
    return () => { mounted = false; generation++; abort.abort(); unwatch(); venueWalletMenu.set(null); clearInterval(timer); clearInterval(clock); window.removeEventListener("hashchange",sync); window.removeEventListener("popstate",sync); onPreviewChange(null); };
  });
  $: if (mounted) onPreviewChange({closes:settledTrades(history).slice(-64).map(t=>t.price!),bids:bids.slice(0,6).map(l=>({price:Number(l.price),amount:Number(l.amount)})),asks:asks.slice(0,6).map(l=>({price:Number(l.price),amount:Number(l.amount)})),capturedAt:Date.now()});
</script>

<section class="otcWorkspace gonka" data-testid="otc-workspace" data-market="GNK-USDT" data-state={!loaded ? "connecting" : healthy ? "connected" : "unavailable"}>
  <div class="intro"><h1>Gonka <span>OTC</span></h1><p>{say("Trade GNK / USDT on HEX. Your wallet signs every action.","Торгуйте GNK / USDT на HEX. Каждое действие подписывает ваш кошелёк.")}</p></div>
  <OtcTerminal baseAsset="GNK" marketIcon="/icons/assets/gnk.jpg" networkLabel="Gonka · HOT Omni Balance" venueLabel="HEX" bind:bridgeOpen bind:bookOpen selectedRate={asks[0]?.price ?? "—"} rateLabel={say("Best ask","Лучшая продажа")} bookTitle={say("HEX orderbook","Стакан HEX")} stats={[
    {label:say("Best bid","Лучшая покупка"),value:bids[0]?.price ?? "—"},
    {label:say("Open orders","Открытые ордера"),value:owner && accountData ? String(accountData.orders.length) : "—",secondary:true},
    {label:say("HEX balance","Баланс HEX"),value:accountData ? decimal(accountData.balances[0],9) : "—",unit:"GNK"},
    {label:say("HEX balance","Баланс HEX"),value:accountData ? decimal(accountData.balances[1],6) : "—",unit:"USDT"},
  ]}>
    <div slot="status" class="serviceControls status">
      {#if error}<p class="message error" role="alert">{error}</p>{/if}
      {#if notice}<p class="message" role="status">{notice}</p>{/if}
      {#if loaded && !healthy}<p role="status">{say("Market data is unavailable. Trading is paused while reconnecting.","Рыночные данные недоступны. Торговля приостановлена до восстановления связи.")}</p>{/if}
      {#if loaded && !execution}<p class="message">{say("HEX trading and funding are not enabled on this server. Live market data is available.","Торговля и переводы HEX на этом сервере не включены. Рыночные данные доступны.")}</p>{/if}
      <div class="walletActions">
        {#if wallets.length}<label for="hex-account">{say("Trading wallet","Торговый кошелёк")}</label><select id="hex-account" value={owner} disabled={busy} on:change={e => selectAccount(e.currentTarget.value)}>{#each wallets as w}<option value={w.account}>{w.name} · {w.address}</option>{/each}</select>{/if}
        <button disabled={busy || !execution} on:click={() => run(connect)}>{say("Connect wallet","Подключить кошелёк")}</button>
        {#if owner}<button disabled={busy} on:click={() => run(() => wallet.disconnect(owner))}>{say("Disconnect","Отключить")}</button><button disabled={busy} on:click={() => run(wallet.profile)}>{say("Wallet & transfers","Кошелёк и переводы")}</button>{/if}
      </div>
      {#if owner}<p class="address">{say("Omni account","Аккаунт Omni")}: {owner}</p>{/if}
      {#if accountError}<p class="message error" role="alert">{accountError}</p>{/if}
      {#if personalHistoryError}<p class="message error" role="alert">{personalHistoryError}</p>{/if}
      {#if pending}<div class="message" role="status">{say("An original submission is awaiting confirmation. New orders are locked.","Ожидаем подтверждения исходной заявки. Новые ордера заблокированы.")} {#if pending.transaction}<a target="_blank" rel="noreferrer" href={explorer(pending.transaction)}>{say("View transaction","Транзакция")}</a>{/if}<button disabled={busy} on:click={() => run(checkPending)}>{say("Check original submission","Проверить исходную заявку")}</button></div>{/if}
    </div>
    <div slot="ticket" class="serviceControls card ticket">
      <div class="sides"><button class:active={side === "buy"} aria-pressed={side === "buy"} disabled={busy} on:click={() => chooseSide("buy")}>{say("Buy GNK","Купить GNK")}</button><button class:active={side === "sell"} aria-pressed={side === "sell"} disabled={busy} on:click={() => chooseSide("sell")}>{say("Sell GNK","Продать GNK")}</button></div>
      <label for="hex-order-type">{say("Order type","Тип ордера")}</label><select id="hex-order-type" bind:value={type} disabled={busy} on:change={edit}><option value="market">{say("Market","Рыночный")}</option><option value="limit">{say("Limit","Лимитный")}</option></select>
      {#if type === "limit"}<label for="hex-price">{say("Limit price · USDT / GNK","Лимитная цена · USDT / GNK")}</label><input id="hex-price" inputmode="decimal" bind:value={price} disabled={busy} on:input={edit} />{/if}
      <label for="hex-amount">{say("You spend","Вы отправляете")} · {side === "buy" ? "USDT" : "GNK"}</label><input id="hex-amount" inputmode="decimal" bind:value={amount} disabled={busy} on:input={edit} />
      <p class="muted">{say("Available","Доступно")}: {accountData ? decimal(balance,side === "buy" ? 6 : 9) : "—"} {side === "buy" ? "USDT" : "GNK"}</p>
      {#if type === "market"}<label for="hex-slippage">{say("Maximum slippage · %","Максимальное проскальзывание · %")}</label><input id="hex-slippage" inputmode="decimal" bind:value={slippage} disabled={busy} on:input={edit} />{/if}
      <button class="primary" disabled={!canReview} on:click={() => run(getReview)}>{busy ? say("Please wait…","Подождите…") : say("Review order","Проверить ордер")}</button>
      <p class="muted">{say("Protocol fees and any unfilled remainder appear in the review.","Комиссия протокола и неисполненный остаток указаны при проверке.")}</p>
      <div class="funding"><strong>{say("Fund your HEX balance","Пополнить баланс HEX")}</strong>{#each [{symbol:"GNK",token:GNK},{symbol:"USDT",token:USDT}] as asset}<div class="fundRow"><span>{asset.symbol}</span><button disabled={busy || !owner || !execution || !!pending} on:click={() => run(() => funding("deposit",asset.token))}>{say("Deposit","Внести")}</button><button disabled={busy || !owner || !execution || !!pending} on:click={() => run(() => funding("withdraw",asset.token))}>{say("Withdraw","Вывести")}</button></div>{/each}<p class="muted">{say("Choose a supported network in the HOT bridge. Pending transfers remain available in Wallet & transfers.","Выберите поддерживаемую сеть в мосте HOT. Незавершённые переводы доступны в разделе «Кошелёк и переводы».")}</p></div>
      {#if claimable}<p>{say("Claimable","К получению")}: {decimal(accountData?.pending[GNK] ?? "0",9)} GNK · {decimal(accountData?.pending[USDT] ?? "0",6)} USDT</p><button disabled={busy || !execution || !healthy || !!pending} on:click={() => confirmAction = {kind:"claim"}}>{say("Claim pending balance","Получить доступный остаток")}</button>{/if}
    </div>
    <div slot="chart" class="liveChart">
      <div class="ranges" role="group" aria-label={say("Chart range","Период графика")}>{#each ["1D","7D","1M","ALL"] as item}<button aria-pressed={range === item} class:active={range === item} on:click={() => range = item}>{item}</button>{/each}</div>
      {#if historyError}<p role="status">{historyError}</p>{:else if points}<svg viewBox="0 0 600 250" role="img" aria-label={say("Confirmed GNK / USDT trades on HEX","Подтверждённые сделки GNK / USDT на HEX")}><title>{say("Confirmed HEX fills","Подтверждённые сделки HEX")}</title><path d="M20 40H580M20 130H580M20 220H580" stroke="var(--color-border)" fill="none"/><polyline points={points} stroke="var(--color-good)" stroke-width="2" fill="none" /></svg><p>{say("Latest fill","Последняя сделка")}: {number(latest?.price ?? 0)} USDT / GNK · {filteredHistory.length} {say("confirmed fills in the latest history page","подтверждённых сделок на последней странице истории")}</p>{:else}<div class="empty">{say("No confirmed fills in this period.","За этот период нет подтверждённых сделок.")}</div>{/if}
    </div>
    <div slot="book" class="book"><div class="bookHead"><span>{say("Price · USDT","Цена · USDT")}</span><span>GNK</span><span>USDT</span></div>{#each [{name:say("Asks","Продажа"),side:"sell",rows:asks},{name:say("Bids","Покупка"),side:"buy",rows:bids}] as book}<h3 class:sell={book.side === "sell"}>{book.name}</h3>{#each book.rows.slice(0,12) as level}<button class="level" class:sell={book.side === "sell"} title={`${level.price} USDT / GNK`} style={`--depth:${100 * level.depth / (book.rows.at(-1)?.depth || 1)}%`} disabled={busy} on:click={() => pickPrice(level.price,book.side === "sell" ? "buy" : "sell")}><span>{number(level.price)}</span><span>{number(level.amount)}</span><span>{number(level.total)}</span></button>{:else}<p class="empty">{say("No open orders","Открытых ордеров нет")}</p>{/each}{/each}</div>
  </OtcTerminal>
  <div class="activity"><OtcPanel title={say("HEX activity","Активность HEX")} id="hex-activity" collapsible={false}>
    <div class="activityTabs"><button aria-pressed={tab === "orders"} class:active={tab === "orders"} on:click={() => tab = "orders"}>{say("My orders","Мои ордера")}</button><button aria-pressed={tab === "history"} class:active={tab === "history"} on:click={() => tab = "history"}>{say("My history","Моя история")}</button></div>
    <div class="tableWrap serviceControls">{#if !owner}<p class="empty">{say("Connect a wallet to see its orders and fills.","Подключите кошелёк для просмотра ордеров и сделок.")}</p>{:else if tab === "orders"}<table><thead><tr><th>{say("Side","Сторона")}</th><th>USDT / GNK</th><th>{say("Remaining","Остаток")}</th><th>{say("Action","Действие")}</th></tr></thead><tbody>{#each accountData?.orders ?? [] as order}<tr><td>{order.order.side === "buy" ? say("Buy","Покупка") : say("Sell","Продажа")}</td><td>{humanPrice(order.order.price)}</td><td>{decimal(order.order_balance,order.order.side === "buy" ? 6 : 9)} {order.order.side === "buy" ? "USDT" : "GNK"}</td><td><button disabled={busy || !execution || !healthy || !!pending || order.order.maker !== owner} on:click={() => confirmAction = {kind:"cancel",order}}>{say("Cancel","Отменить")}</button></td></tr>{:else}<tr><td colspan="4">{say("No open orders","Открытых ордеров нет")}</td></tr>{/each}</tbody></table>{#if moreOrders && accountData?.orders.length}<button disabled={busy} on:click={() => run(() => loadAccount(true))}>{say("Load more orders","Ещё ордера")}</button>{/if}{:else}<table><thead><tr><th>{say("Time","Время")}</th><th>{say("Action","Действие")}</th><th>{say("Amount","Сумма")}</th><th>{say("Transaction","Транзакция")}</th></tr></thead><tbody>{#each personalHistory as item}<tr><td>{new Date(item.timestamp*1000).toLocaleString()}</td><td>{item.type}</td><td>{number(item.amount)} {item.type === "buy" || item.type === "create_buy" ? "USDT" : item.type === "cancel" ? "" : "GNK"}{#if item.amount_to !== null} → {number(item.amount_to)} {item.type === "buy" ? "GNK" : "USDT"}{/if}</td><td><a href={explorer(item.near_trx)} target="_blank" rel="noreferrer">{say("View","Открыть")} ↗</a></td></tr>{:else}<tr><td colspan="4">{say("No history available","История пока недоступна")}</td></tr>{/each}</tbody></table>{#if moreHistory && personalHistory.length}<button disabled={busy} on:click={() => run(() => loadPersonalHistory(true))}>{say("Load older history","Более ранняя история")}</button>{/if}{/if}</div>
  </OtcPanel></div>
</section>

{#if review || confirmAction}
<dialog use:modal on:cancel|preventDefault={() => { if (!busy) { review = null; confirmAction = null; } }} aria-labelledby="hex-review-title" class="hexReview">
  <h2 id="hex-review-title">{review ? say("Review HEX order","Проверьте ордер HEX") : confirmAction?.kind === "cancel" ? say("Cancel HEX order","Отменить ордер HEX") : say("Claim pending balance","Получить доступный остаток")}</h2>
  {#if review}
    <dl><dt>{say("Order","Ордер")}</dt><dd>{review.draft.side} · {review.draft.order_type}</dd><dt>{say("Omni account","Аккаунт Omni")}</dt><dd>{review.account}</dd><dt>{say("Maximum spend","Максимальный расход")}</dt><dd>{decimal(review.draft.amount,review.draft.side === "buy" ? 6 : 9)} {review.draft.side === "buy" ? "USDT" : "GNK"}</dd>
    {#if review.draft.price}<dt>{say("Limit price","Лимитная цена")}</dt><dd>{humanPrice(review.draft.price)} USDT / GNK</dd>{/if}
    <dt>{say("Immediate output","Сразу получаете")}</dt><dd>{decimal(review.quote.result.book_fill_result.taker_receive_total,review.draft.side === "buy" ? 9 : 6)} {review.draft.side === "buy" ? "GNK" : "USDT"}</dd>
    {#if review.draft.order_type === "market"}<dt>{say("Minimum output","Минимум к получению")}</dt><dd>{decimal(review.minimum,review.draft.side === "buy" ? 9 : 6)} {review.draft.side === "buy" ? "GNK" : "USDT"}</dd>{/if}
    <dt>{say("Protocol fee","Комиссия протокола")}</dt><dd>{decimal(review.quote.result.book_fill_result.taker_fee_quote,6)} USDT</dd>
    <dt>{say("Resting remainder","Остаток в ордере")}</dt><dd>{decimal(review.quote.result.taker_remainder?.new_order ?? "0",review.draft.side === "buy" ? 6 : 9)} {review.draft.side === "buy" ? "USDT" : "GNK"}</dd>
    <dt>{say("Unspent / refunded","Не потрачено / возврат")}</dt><dd>{decimal(refundedInput(review.quote),review.draft.side === "buy" ? 6 : 9)} {review.draft.side === "buy" ? "USDT" : "GNK"}</dd></dl>
    {#if !isQuoteCurrent(review.quote,now)}<p role="status">{say("Quote expired. Confirm to refresh it before signing.","Котировка истекла. Подтвердите для обновления перед подписью.")}</p>{/if}
  {:else if confirmAction?.order}<p>{humanPrice(confirmAction.order.order.price)} USDT / GNK</p><p class="address">{confirmAction.order.hash}</p><p>{say("Remaining funds return to your HEX balance.","Оставшиеся средства вернутся на ваш баланс HEX.")}</p>
  {:else}<p>{say("Return claimable GNK and USDT to this Omni account.","Получить доступные GNK и USDT на этот аккаунт Omni.")}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}{#if notice}<p role="status">{notice}</p>{/if}
  <div class="reviewActions"><button disabled={busy} on:click={() => { review = null; confirmAction = null; }}>{say("Close","Закрыть")}</button><button class="primary" disabled={busy || !execution || !healthy || !!pending} on:click={() => run(review ? confirmOrder : executeAction)}>{busy ? say("Awaiting wallet…","Ожидаем кошелёк…") : say("Confirm in wallet","Подтвердить в кошельке")}</button></div>
</dialog>
{/if}

<style>
  .gonka { --otc-buy:var(--color-good); --otc-sell:var(--color-danger); --otc-panel-padding:20px; width:min(var(--layout-width),calc(100% - 2 * var(--page-gutter))); margin:0 auto; padding:32px 0; font-size:13px; line-height:1.5; }
  .intro { text-align:center; margin-bottom:24px; } h1 { font-size:clamp(40px,5vw,64px); line-height:1.06; letter-spacing:-.06em; margin:0 0 12px; } h1 span { text-decoration:underline; text-decoration-color:var(--color-accent); } .intro p { color:var(--color-text-soft); }
  .walletActions { display:flex; align-items:center; flex-wrap:wrap; gap:8px; margin-bottom:16px; } .walletActions select { width:auto; max-width:100%; } .status { margin-bottom:16px; } .ticket { padding:20px; } .funding { border-top:1px solid var(--color-border); margin-top:24px; padding-top:16px; } .fundRow { display:flex; align-items:center; gap:8px; margin-top:8px; } .fundRow span { flex:1; }
  .liveChart { width:100%; padding:16px; } .liveChart svg { width:100%; min-height:230px; } .liveChart p { color:var(--color-text-soft); } .ranges { display:flex; gap:8px; } .ranges button,.activityTabs button { min-height:44px; padding:8px 12px; border:1px solid transparent; border-radius:6px; cursor:pointer; } .active { border-color:var(--color-border-strong)!important; background:var(--color-panel); } .empty { padding:24px; text-align:center; color:var(--color-text-soft); }
  .book { width:100%; } .bookHead,.level { display:grid; grid-template-columns:repeat(3,minmax(0,1fr)); gap:8px; text-align:right; padding:10px 12px; font-variant-numeric:tabular-nums; font-size:11px; overflow-wrap:anywhere; } .bookHead { color:var(--color-text-soft); border-bottom:1px solid var(--color-border); } .bookHead span:first-child,.level span:first-child { text-align:left; } .book h3 { padding:8px 12px; margin:0; font-size:12px; color:var(--color-good); } .book h3.sell,.level.sell span:first-child { color:var(--color-danger); } .level { width:100%; min-height:44px; cursor:pointer; border-bottom:1px solid var(--color-border); background:linear-gradient(to left,color-mix(in srgb,var(--color-good) 9%,transparent) var(--depth),transparent var(--depth)); } .level.sell { background:linear-gradient(to left,color-mix(in srgb,var(--color-danger) 9%,transparent) var(--depth),transparent var(--depth)); } .level span:first-child { color:var(--color-good); }
  .activity { margin-top:24px; } .activityTabs { padding:8px 16px; display:flex; gap:8px; border-bottom:1px solid var(--color-border); } .tableWrap { overflow:auto; padding:16px; } table { width:100%; border-collapse:collapse; font-size:12px; } th,td { text-align:left; padding:12px 8px; border-bottom:1px solid var(--color-border); overflow-wrap:anywhere; max-width:220px; } th { color:var(--color-text-soft); } .address { overflow-wrap:anywhere; }
  .hexReview { width:min(580px,calc(100% - 32px)); max-height:85vh; overflow:auto; padding:24px; border:1px solid var(--color-border); border-radius:16px; background:var(--color-paper); color:var(--color-text); } .hexReview::backdrop { background:#0009; } .hexReview h2 { margin:0 0 20px; font-size:22px; } .hexReview dl { display:grid; grid-template-columns:1fr 1.2fr; gap:12px; font-size:13px; } .hexReview dt { color:var(--color-text-soft); } .hexReview dd { margin:0; overflow-wrap:anywhere; } .reviewActions { display:flex; gap:12px; margin-top:24px; } .reviewActions button { flex:1; min-height:44px; padding:12px; border:1px solid var(--color-border); border-radius:8px; cursor:pointer; } .reviewActions .primary { background:var(--color-accent); color:#152016; } button:disabled { opacity:.45; cursor:default; }
  @media(max-width:640px) { .walletActions select { width:100%; } .hexReview dl { grid-template-columns:1fr; gap:4px; } .hexReview dd { margin-bottom:8px; } .bookHead,.level { font-size:12px; } }
</style>
