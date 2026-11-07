<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { getBalances, request, otcP2pNetwork, walletNetwork, type Session, type Direction, type WalletBalance } from "$lib/otc/service";
  import { connectWallet, wallets } from "$lib/wallet-session";
  import { sameWalletAddress } from "$lib/wallet-amount";
  import { isValidWalletAddress } from "$lib/wallet-address";
  import { chainIdForNetwork, walletFamily } from "$lib/wallet-execution";
  import { fetchMarketPrices } from "$lib/exchange";
  import { saleRate, offerDecimal, offerTotal, offerEver, offerUnits, offerFloored, balanceUnits, offerShortfall, marketUsdtPerEver, marketUsdPerEver, MAX_OFFER_EVER_UNITS, MAX_OFFER_USDT_UNITS, type PublicOffer as Offer, type OfferHistoryPoint, type SaleTicketState, type Stablecoin } from "$lib/otc/public-offers";
  export let session: Session | null;
  export let busy = false;
  export let onConnect: () => void;
  export let enabled = false;
  export let direction: Direction = "sell";
  export let posting = false;
  export let onPublished: () => void = () => {};
  let suggestionKey = "";
  let offers: Offer[] = [], mine: Offer[] = [], history: OfferHistoryPoint[] = [], error = "";
  let amountSource: "ever" | "usdt" = "ever";
  let stablecoin: Stablecoin = "USDT";
  let priceInput = "", priceMode = false, loadedDirection: Direction = "sell";
  let marketRate = "", marketUsdRate = "", marketPrice = true;
  let everAmount = "", usdtAmount = "", receivingAddress = "", key = "";
  let stablecoinNetwork = "ethereum";
  let stablecoinAssets: Array<{ asset: string; symbol: Stablecoin; network: string; asset_id: string }> = [];
  let linkingWallet = false, reviewing = false, mounted = false, generation = 0, loadedActor = "";
  let marketAbort: AbortController | null = null;
  let balances: WalletBalance[] = [], balanceGeneration = 0;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: seller = session?.wallets?.find(wallet => wallet.chain === "everscale")?.address || "";
  $: draftKey = session ? `pay3flow.otc.sell-draft.v1:${session.actor}` : "";
  $: if (mounted && (session?.actor || "") !== loadedActor) {
    const previousActor = loadedActor;      loadedActor = session?.actor || ""; loadedDirection = direction; mine = []; reviewing = false; error = "";
    if (previousActor || !loadedActor) { everAmount = ""; usdtAmount = ""; stablecoin = "USDT"; receivingAddress = ""; key = ""; priceInput = ""; priceMode = false; marketPrice = true; }
    try {
      const draft = JSON.parse(localStorage.getItem(draftKey) || "null");
      if (draft && (draft.direction || "sell") === direction && [draft.ever_amount, draft.usdt_amount, draft.usdt_address, draft.key].every(value => typeof value === "string")) {
        everAmount = draft.ever_amount; usdtAmount = draft.usdt_amount; receivingAddress = draft.usdt_address; key = draft.key; priceMode = !!draft.price_mode; priceInput = typeof draft.price_input === "string" ? draft.price_input : ""; amountSource = draft.amount_source === "usdt" ? "usdt" : "ever";
        stablecoin = draft.stablecoin === "USDC" ? "USDC" : "USDT";
        stablecoinNetwork = typeof draft.stablecoin_network === "string" ? draft.stablecoin_network : "ethereum";
        if (typeof draft.market_price === "boolean") marketPrice = draft.market_price;
        else marketPrice = true;
      }
    } catch { /* An unreadable draft is not submitted. */ }
    if (marketPrice && marketRate) applyMarketRate(marketRate);
    void refresh();
    void refreshBalances();
  }
  $: askingRate = saleRate(everAmount,usdtAmount);
  // Conversion hints only — amounts stay independent of the price field.
  $: hintRate = (priceMode && offerDecimal(priceInput, 12)) ? priceInput : marketRate;
  $: usdtApprox = hintRate ? offerTotal(everAmount, hintRate)?.amount || "" : "";
  $: everApprox = hintRate ? offerEver(usdtAmount, hintRate)?.amount || "" : "";
  $: price = priceMode ? priceInput : askingRate === "—" ? "" : askingRate;
  $: roundingAsset = null;
  $: priceRounded = false;
  $: amountFloored = offerFloored(everAmount,9) ? "EVER" as const : offerFloored(usdtAmount,6) ? "USDT" as const : null;
  $: everUnits = offerUnits(everAmount,9);
  $: usdtUnits = offerUnits(usdtAmount,6);
  /** Only the leg the trader pays from their own wallet can be short: EVER when selling, USDT when buying. */
  $: fundedLeg = direction === "sell" ? "ever" as const : "usdt" as const;
  // Backend balance reads cover Everscale + Ethereum only. Other stablecoin networks
  // must not inherit the Ethereum USDT warning.
  $: ethereumWallet = session?.wallets.find(wallet => wallet.chain === "ethereum")?.address || "";
  $: fundedBalance = fundedLeg === "ever"
    ? balances.find(balance => balance.symbol === "EVER" && balance.state === "ok")?.amount || ""
    : stablecoinNetwork === "ethereum"
      && !!ethereumWallet
      && sameWalletAddress("evm", receivingAddress.trim(), ethereumWallet)
      ? balances.find(balance => balance.symbol === stablecoin && balance.chain === "ethereum" && balance.state === "ok")?.amount || ""
      : "";
  $: shortfall = offerShortfall(
    fundedLeg === "ever" ? everUnits : usdtUnits,
    balanceUnits(fundedBalance, fundedLeg === "ever" ? 9 : 6),
  ) ? fundedLeg : null;
  $: blockingReason = priceMode && !offerDecimal(priceInput,12)
    ? say("Enter a positive price.", "Укажите положительный курс.")
    : everUnits === null
    ? say("Enter a positive EVER amount.", "Укажите положительную сумму EVER.")
    : everUnits > MAX_OFFER_EVER_UNITS
    ? say("One offer can list at most 1,000,000,000,000 EVER.", "Одно предложение может содержать не более 1 000 000 000 000 EVER.")
    : usdtUnits === null
    ? say(`Enter a positive ${stablecoin} total.`, `Укажите положительную сумму ${stablecoin}.`)
    : usdtUnits > MAX_OFFER_USDT_UNITS
    ? say(`One offer can total at most 1,000,000 ${stablecoin}.`, `Общая сумма одного предложения не может превышать 1 000 000 ${stablecoin}.`)
    : !isValidWalletAddress(stablecoinNetwork, receivingAddress)
      ? say(`Enter a valid receiving ${stablecoin} address for ${networkLabel(stablecoinNetwork)}.`, `Укажите корректный адрес получения ${stablecoin} в сети ${networkLabel(stablecoinNetwork)}.`)
      : "";
  const networkLabels: Record<string, string> = { ethereum: "Ethereum", sei: "Sei", "bnb-smart-chain": "BNB Smart Chain", "polygon-pos": "Polygon", "avalanche-c": "Avalanche", "arbitrum-one": "Arbitrum One", base: "Base", optimism: "Optimism", gnosis: "Gnosis", near: "NEAR", tron: "TRON", solana: "Solana", ton: "TON", sui: "Sui", aptos: "Aptos" };
  const networkLabel = (network: string) => networkLabels[network] ?? network;
  $: valid = !blockingReason;
  $: if (direction !== loadedDirection) resetDirection(direction);
  function resetDirection(next: Direction) { loadedDirection = next; amountSource = next === "buy" ? "usdt" : "ever"; stablecoinNetwork = "ethereum"; receivingAddress = ""; reviewing = false; key = ""; suggestionKey = ""; }
  function recipientSuggestion(network: string): string {
    const family = walletFamily(network);
    if (!family) return "";
    if (family === "evm") {
      const expected = network === "ethereum" ? otcP2pNetwork().chainId : chainIdForNetwork(network);
      if (network === "ethereum" || network === otcP2pNetwork().name) {
        const verified = session?.wallets.find(wallet => wallet.chain === "ethereum")?.address || "";
        if (verified) return verified;
      }
      return $wallets.evm?.family === "evm" && expected != null && $wallets.evm.chainId === expected ? $wallets.evm.address : "";
    }
    return $wallets[family]?.address || "";
  }
  $: if (mounted) suggestRecipient(stablecoinNetwork, recipientSuggestion(stablecoinNetwork), session?.actor || "");
  function suggestRecipient(network: string, address: string, actor: string) {
    const next = `${actor}:${network}:${address}`;
    if (next === suggestionKey) return;
    suggestionKey = next;
    if (address && !receivingAddress) { receivingAddress = address; edited(); }
  }    function edited() { reviewing = false; key = ""; }
  async function useNetworkWallet() {
    if (posting || busy || linkingWallet) return;
    const family = walletFamily(stablecoinNetwork);
    if (!family) {
      error = say(`Connect a ${networkLabel(stablecoinNetwork)} wallet from the wallet menu.`, `Подключите кошелёк ${networkLabel(stablecoinNetwork)} через меню кошельков.`);
      return;
    }
    const actor = session?.actor; linkingWallet = true; error = "";
    try {
      const target = stablecoinNetwork === "ethereum" ? walletNetwork("ethereum", otcP2pNetwork()) : stablecoinNetwork;
      const wallet = await connectWallet(target);
      if (session?.actor !== actor) return;
      if (wallet.family !== family) throw new Error(say(`Select a ${networkLabel(stablecoinNetwork)} wallet.`, `Выберите кошелёк ${networkLabel(stablecoinNetwork)}.`));
      if (wallet.family === "evm") {
        const expected = stablecoinNetwork === "ethereum" ? otcP2pNetwork().chainId : chainIdForNetwork(stablecoinNetwork);
        if (expected == null || wallet.chainId !== expected) {
          throw new Error(say(`Select ${networkLabel(stablecoinNetwork)}.`, `Выберите сеть ${networkLabel(stablecoinNetwork)}.`));
        }
      }
      receivingAddress = wallet.address; edited();
    } catch (e) { if (session?.actor === actor) error = e instanceof Error ? e.message : "Connection failed"; }
    finally { linkingWallet = false; }
  }
  async function refresh() {
    if (!enabled) return;
    const owner = session, revision = ++generation;
    try {
      const [publicOffers, privateOffers, publicationHistory] = await Promise.allSettled([
        request<Offer[]>("/offers"), owner ? request<Offer[]>("/offers/mine", owner.token) : Promise.resolve([]),
        request<OfferHistoryPoint[]>("/offers/history"),
      ]);
      if (!mounted || revision !== generation) return;
      if (publicOffers.status === "fulfilled") offers = publicOffers.value;
      if (privateOffers.status === "fulfilled" && session?.token === owner?.token) mine = privateOffers.value;
      if (publicationHistory.status === "fulfilled") history = publicationHistory.value;
      const failed = [publicOffers, privateOffers, publicationHistory].find(result => result.status === "rejected");
      if (failed?.status === "rejected") error = failed.reason instanceof Error ? failed.reason.message : "Request failed";
    } catch (e) {
      if (mounted && revision === generation) { error = e instanceof Error ? e.message : "Request failed"; }
    }
  }
  /** Display-only warning feed: a wallet that cannot be read simply leaves nothing to warn about. */
  async function refreshBalances() {
    const owner = session, revision = ++balanceGeneration;
    if (!enabled || !owner) { balances = []; return; }
    try {
      const list = await getBalances(owner.token);
      if (!mounted || revision !== balanceGeneration || session?.token !== owner.token) return;
      balances = list;
    } catch (error) {
      if (revision !== balanceGeneration) return;
      balances = [];
      console.warn("Wallet balances unavailable", error);
    }
  }
  async function publish() {
    if (!enabled || !session || !seller || posting || busy || linkingWallet || !valid) return;
    if (!reviewing) { reviewing = true; error = ""; return; }
    key ||= crypto.randomUUID();
    const selectedAsset = stablecoinAssets.find(item => item.symbol === stablecoin && item.network === stablecoinNetwork);
    if (stablecoinNetwork !== "ethereum" && !selectedAsset) {
      error = say("That network is no longer available. Select it again.", "Эта сеть больше недоступна. Выберите её снова.");
      return;
    }
    const body = { direction, stablecoin, ever_amount: offerDecimal(everAmount,9)!, usdt_amount: offerDecimal(usdtAmount,6)!, usdt_address: receivingAddress.trim(), stablecoin_network: stablecoinNetwork, stablecoin_asset_id: selectedAsset?.asset_id };
    try {
      localStorage.setItem(draftKey, JSON.stringify({ ...body, key, price_mode: priceMode, price_input: priceInput, amount_source: amountSource, market_price: marketPrice }));
    } catch { error = say("Enable browser storage to publish and safely retry an offer.", "Разрешите хранение данных в браузере для безопасной публикации и повторной проверки."); return; }
    posting = true; error = "";
    const owner = session;
    try {
      const published = await request<Offer>("/offers", owner.token, body, key);
      if (session?.token !== owner.token) return;
      generation++;
      mine = [published, ...mine.filter(offer => offer.id !== published.id)];
      localStorage.removeItem(draftKey); key = ""; reviewing = false;
      if (published.state !== "open" || Date.parse(published.expires_at) <= Date.now()) {
        error = say("This saved offer is already cancelled or expired. Review again to publish a new offer.", "Сохранённое предложение уже отменено или истекло. Проверьте условия ещё раз, чтобы опубликовать новое.");
        await refresh();
        return;
      }
      offers = [published, ...offers.filter(offer => offer.id !== published.id)];
      onPublished();
      await refresh();
    } catch (e) { if (session?.token === owner.token) error = e instanceof Error ? e.message : "Request failed"; }
    finally { posting = false; }
  }
  async function cancel(id: string) {
    if (!session || posting || busy) return;
    const owner = session; posting = true; error = "";
    try {
      const cancelled = await request<Offer>(`/offers/${id}/cancel`, owner.token, {});
      if (session?.token !== owner.token) return;
      generation++;
      offers = offers.filter(offer => offer.id !== id);
      mine = mine.map(offer => offer.id === id ? cancelled : offer);
      await refresh();
    }
    catch (e) { if (session?.token === owner.token) error = e instanceof Error ? e.message : "Request failed"; }
    finally { posting = false; }
  }
  async function refreshMarket(signal: AbortSignal) {
    try {
      const prices = await fetchMarketPrices(signal);
      if (signal.aborted) return;
      marketRate = marketUsdtPerEver(prices.prices.EVER, stablecoin === "USDC" ? (prices.prices.USDC ?? 1) : prices.prices.USDT) || "";
      marketUsdRate = marketUsdPerEver(prices.prices.EVER) || "";
      if (!marketRate) marketPrice = false;
      else if (marketPrice) applyMarketRate(marketRate);
    } catch (error) {
      if (signal.aborted) return;
      // A missing feed must not interrupt offer publication; the price stays editable.
      marketRate = "";
      marketUsdRate = "";
      marketPrice = false;
      console.warn("Market prices unavailable", error);
    }
  }
  function applyMarketRate(rate: string) {
    priceInput = rate;
    priceMode = true;
  }
  function useMarketPrice(checked: boolean) {
    marketPrice = checked;
    if (checked && marketRate) { applyMarketRate(marketRate); edited(); }
  }
  onMount(() => {
    mounted = true;
    void request<{ destination_assets: typeof stablecoinAssets }>("/p2p/assets")
      .then(result => { stablecoinAssets = result.destination_assets || []; })
      .catch(() => { stablecoinAssets = []; });
    const timer = setInterval(() => { if (enabled && !posting && !busy) void refresh(); }, 5000);
    const controller = new AbortController();
    marketAbort = controller;
    void refreshMarket(controller.signal);
    const marketTimer = setInterval(() => { if (enabled && !posting && !busy) void refreshMarket(controller.signal); }, 60_000);
    const balanceTimer = setInterval(() => { if (enabled && session && !posting && !busy) void refreshBalances(); }, 20_000);
    return () => { mounted = false; generation++; clearInterval(timer); clearInterval(marketTimer); clearInterval(balanceTimer); controller.abort(); marketAbort = null; };
  });

  let started = false;
  $: if (mounted && enabled && !started) { started = true; void refresh(); void refreshBalances(); }
  function changePrice(value: string) { priceInput = value; priceMode = true; edited(); }
  function changeEver(value: string) {
    everAmount = value; amountSource = "ever";
    if (!priceMode && marketPrice && marketRate) { priceInput = marketRate; priceMode = true; }
    edited();
  }
  function changeUsdt(value: string) {
    usdtAmount = value; amountSource = "usdt";
    if (!priceMode && marketPrice && marketRate) { priceInput = marketRate; priceMode = true; }
    edited();
  }
  function changeStablecoin(value: Stablecoin) { stablecoin = value; stablecoinNetwork = "ethereum"; receivingAddress = ""; suggestionKey = ""; edited(); if (marketAbort) void refreshMarket(marketAbort.signal); }
  function changeAddress(value: string) { receivingAddress = value; edited(); }
  function changeStablecoinNetwork(value: string) {
    if (stablecoinNetwork === value) return;
    stablecoinNetwork = value;
    receivingAddress = "";
    suggestionKey = "";
    edited();
  }
  $: ticket = {
    direction, stablecoin, price, priceRounded, roundingAsset, amountFloored, shortfall, fundedBalance, blockingReason, everAmount, usdtAmount, receivingAddress, stablecoinNetwork, supportedStablecoinAssets: stablecoinAssets.map(({ symbol, network }) => ({ symbol, network })), seller, askingRate, valid, reviewing, linkingWallet,
    marketPrice, marketRate, marketUsdRate, usdtApprox, everApprox, error,
    onPrice: changePrice,
    onEver: changeEver,
    onUsdt: changeUsdt,
    onStablecoin: changeStablecoin,
    onAddress: changeAddress,
    onStablecoinNetwork: changeStablecoinNetwork,
    onUseWallet: useNetworkWallet,
    onMarketPrice: useMarketPrice,
    onRequest: () => seller ? void publish() : onConnect(),
    onCloseReview: () => { reviewing = false; },
  } satisfies SaleTicketState;
</script>

<slot {ticket} {offers} {mine} {history} offerError={error} onCancel={cancel} />
