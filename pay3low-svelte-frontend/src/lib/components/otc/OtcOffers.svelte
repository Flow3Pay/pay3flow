<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { getBalances, request, otcP2pNetwork, walletNetwork, type Session, type Direction, type WalletBalance } from "$lib/otc/service";
  import { connectWallet, wallets } from "$lib/wallet-session";
  import { sameWalletAddress } from "$lib/wallet-amount";
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
  let offers: Offer[] = [], mine: Offer[] = [], history: OfferHistoryPoint[] = [], error = "", message = "";
  let amountSource: "ever" | "usdt" = "ever";
  let stablecoin: Stablecoin = "USDT";
  let priceInput = "", priceMode = false, loadedDirection: Direction = "sell";
  let marketRate = "", marketUsdRate = "", marketPrice = false;
  let everAmount = "", usdtAmount = "", receivingAddress = "", key = "";
  let stablecoinNetwork = "ethereum";
  let stablecoinAssets: Array<{ asset: string; symbol: Stablecoin; network: string; asset_id: string }> = [];
  let linkingEvm = false, reviewing = false, mounted = false, generation = 0, loadedActor = "";
  let balances: WalletBalance[] = [], balanceGeneration = 0;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: seller = session?.wallets?.find(wallet => wallet.chain === "everscale")?.address || "";
  $: draftKey = session ? `pay3flow.otc.sell-draft.v1:${session.actor}` : "";
  $: if (mounted && (session?.actor || "") !== loadedActor) {
    const previousActor = loadedActor;
    loadedActor = session?.actor || ""; loadedDirection = direction; mine = []; reviewing = false; error = ""; message = "";
    if (previousActor || !loadedActor) { everAmount = ""; usdtAmount = ""; stablecoin = "USDT"; receivingAddress = ""; key = ""; priceInput = ""; priceMode = false; marketPrice = false; }
    try {
      const draft = JSON.parse(localStorage.getItem(draftKey) || "null");
      if (draft && (draft.direction || "sell") === direction && [draft.ever_amount, draft.usdt_amount, draft.usdt_address, draft.key].every(value => typeof value === "string")) {
        everAmount = draft.ever_amount; usdtAmount = draft.usdt_amount; receivingAddress = draft.usdt_address; key = draft.key; priceMode = !!draft.price_mode; priceInput = typeof draft.price_input === "string" ? draft.price_input : ""; amountSource = draft.amount_source === "usdt" ? "usdt" : "ever";
        stablecoin = draft.stablecoin === "USDC" ? "USDC" : "USDT";
        stablecoinNetwork = typeof draft.stablecoin_network === "string" ? draft.stablecoin_network : "ethereum";
        if (typeof draft.market_price === "boolean") marketPrice = draft.market_price;
      }
    } catch { /* An unreadable draft is not submitted. */ }
    if (marketPrice && marketRate) applyMarketRate(marketRate);
    void refresh();
    void refreshBalances();
  }
  $: askingRate = saleRate(everAmount,usdtAmount);
  $: marketApprox = marketRate ? offerTotal(everAmount, marketRate)?.amount || "" : "";
  $: price = priceMode ? priceInput : askingRate === "—" ? "" : askingRate;
  $: roundingAsset = priceMode ? amountSource === "ever" ? offerTotal(everAmount,priceInput)?.rounded ? "USDT" as const : null : offerEver(usdtAmount,priceInput)?.rounded ? "EVER" as const : null : null;
  $: priceRounded = !!roundingAsset;
  $: amountFloored = offerFloored(everAmount,9) ? "EVER" as const : offerFloored(usdtAmount,6) ? "USDT" as const : null;
  $: everUnits = offerUnits(everAmount,9);
  $: usdtUnits = offerUnits(usdtAmount,6);
  /** Only the leg the trader pays from their own wallet can be short: EVER when selling, USDT when buying. */
  $: fundedLeg = direction === "sell" ? "ever" as const : "usdt" as const;
  // The reported USDT balance belongs to the verified wallet, so a hand-typed paying address is not ours to judge.
  $: fundedBalance = fundedLeg === "ever"
    ? balances.find(balance => balance.symbol === "EVER" && balance.state === "ok")?.amount || ""
    : sameWalletAddress("evm", receivingAddress.trim(), session?.wallets.find(wallet => wallet.chain === "ethereum")?.address || "")
      ? balances.find(balance => balance.symbol === stablecoin && balance.state === "ok")?.amount || ""
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
    : stablecoinNetwork === "ethereum"
      ? !/^0x[0-9a-fA-F]{40}$/.test(receivingAddress.trim()) || /^0x0{40}$/i.test(receivingAddress.trim())
        ? say("Enter your Ethereum wallet address, or use the wallet button.", "Укажите адрес Ethereum-кошелька или нажмите кнопку кошелька.") : ""
      : stablecoinNetwork === "near"
        ? !/^(?=.{2,64}$)[a-z0-9]+(?:[._-][a-z0-9]+)*$/.test(receivingAddress.trim())
          ? say("Enter a valid NEAR account ID.", "Укажите корректный NEAR account ID.") : ""
        : stablecoinNetwork === "tron"
          ? !/^T[1-9A-HJ-NP-Za-km-z]{33}$/.test(receivingAddress.trim())
            ? say("Enter a valid TRON address.", "Укажите корректный TRON-адрес.") : ""
          : !/^0x[0-9a-fA-F]{40}$/.test(receivingAddress.trim())
            ? say("Enter a valid receiving address.", "Укажите корректный адрес получения.") : "";
  $: valid = !blockingReason;
  $: stablecoinNetworks = direction === "buy"
    ? [{ network: "ethereum", assetId: "" }]
    : [
        { network: "ethereum", assetId: "" },
        ...stablecoinAssets
          .filter(item => item.symbol === stablecoin && item.network !== "ethereum")
          .map(item => ({ network: item.network, assetId: item.asset_id })),
      ].filter((item, index, list) => list.findIndex(candidate => candidate.network === item.network) === index);
  $: if (direction !== loadedDirection) resetDirection(direction);
  function resetDirection(next: Direction) { loadedDirection = next; amountSource = next === "buy" ? "usdt" : "ever"; stablecoinNetwork = "ethereum"; receivingAddress = ""; reviewing = false; key = ""; message = ""; }
  $: if (mounted) suggestEthereum(
    session?.wallets.find(w => w.chain === "ethereum")?.address || ($wallets.evm?.family === "evm" && $wallets.evm.chainId === otcP2pNetwork().chainId ? $wallets.evm.address : ""),
    session?.actor || "",
  );
  function suggestEthereum(address: string, actor: string) {
    const next=`${actor}:${address}`;
    if (next === suggestionKey) return;
    suggestionKey=next;
    if (address && !receivingAddress) { receivingAddress=address; edited(); }
  }
  function edited() { reviewing = false; key = ""; message = ""; }
  async function useEthereumWallet() {
    if (posting || busy || linkingEvm) return;
    const actor = session?.actor; linkingEvm = true; error = "";
    try {
      const wallet = await connectWallet(walletNetwork("ethereum", otcP2pNetwork()));
      if (session?.actor !== actor) return;
      if (wallet.family !== "evm" || wallet.chainId !== otcP2pNetwork().chainId) throw new Error(say(`Select ${otcP2pNetwork().label}.`, `Выберите сеть ${otcP2pNetwork().label}.`));
      receivingAddress = wallet.address; edited();
    } catch (e) { if (session?.actor === actor) error = e instanceof Error ? e.message : "Connection failed"; }
    finally { linkingEvm = false; }
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
    if (!enabled || !session || !seller || posting || busy || linkingEvm || !valid) return;
    if (!reviewing) { reviewing = true; error = ""; return; }
    key ||= crypto.randomUUID();
    const selectedAsset = stablecoinAssets.find(item => item.symbol === stablecoin && item.network === stablecoinNetwork);
    if (stablecoinNetwork !== "ethereum" && !selectedAsset) {
      error = say("That receiving network is no longer available. Select it again.", "Эта сеть получения больше недоступна. Выберите её снова.");
      return;
    }
    const body = { direction, stablecoin, ever_amount: offerDecimal(everAmount,9)!, usdt_amount: offerDecimal(usdtAmount,6)!, usdt_address: receivingAddress.trim(), stablecoin_network: stablecoinNetwork, stablecoin_asset_id: selectedAsset?.asset_id };
    try {
      localStorage.setItem(draftKey, JSON.stringify({ ...body, key, price_mode: priceMode, price_input: priceInput, amount_source: amountSource, market_price: marketPrice }));
    } catch { error = say("Enable browser storage to publish and safely retry an offer.", "Разрешите хранение данных в браузере для безопасной публикации и повторной проверки."); return; }
    posting = true; error = ""; message = "";
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
      message = direction === "buy" ? say("Your EVER buy offer is published.", "Ваше предложение о покупке EVER опубликовано.") : say("Your EVER sale offer is published.", "Ваше предложение о продаже EVER опубликовано.");
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
    priceInput = rate; priceMode = true;
    if (amountSource === "ever") usdtAmount = offerTotal(everAmount, rate)?.amount || usdtAmount;
    else everAmount = offerEver(usdtAmount, rate)?.amount || everAmount;
  }
  function useMarketPrice(checked: boolean) {
    marketPrice = checked;
    if (checked && marketRate) { applyMarketRate(marketRate); edited(); }
  }
  onMount(() => {
    mounted = true;
    void request<{ assets: typeof stablecoinAssets }>("/p2p/assets")
      .then(result => { stablecoinAssets = result.assets || []; })
      .catch(() => { stablecoinAssets = []; });
    const timer = setInterval(() => { if (enabled && !posting && !busy) void refresh(); }, 5000);
    const controller = new AbortController();
    void refreshMarket(controller.signal);
    const marketTimer = setInterval(() => { if (enabled && !posting && !busy) void refreshMarket(controller.signal); }, 60_000);
    const balanceTimer = setInterval(() => { if (enabled && session && !posting && !busy) void refreshBalances(); }, 20_000);
    return () => { mounted = false; generation++; clearInterval(timer); clearInterval(marketTimer); clearInterval(balanceTimer); controller.abort(); };
  });

  let started = false;
  $: if (mounted && enabled && !started) { started = true; void refresh(); void refreshBalances(); }
  function changePrice(value: string) { priceInput = value; priceMode = true; if (amountSource === "ever") usdtAmount = offerTotal(everAmount,priceInput)?.amount || ""; else everAmount = offerEver(usdtAmount,priceInput)?.amount || ""; edited(); }
  function changeEver(value: string) { everAmount = value; amountSource = "ever"; if (priceMode) usdtAmount = offerTotal(everAmount,priceInput)?.amount || ""; edited(); }
  function changeUsdt(value: string) { usdtAmount = value; amountSource = "usdt"; if (priceMode) everAmount = offerEver(usdtAmount,priceInput)?.amount || ""; edited(); }
  function changeStablecoin(value: Stablecoin) { stablecoin = value; stablecoinNetwork = "ethereum"; receivingAddress = ""; edited(); }
  function changeAddress(value: string) { receivingAddress = value; edited(); }
  function changeStablecoinNetwork(value: string) {
    if (stablecoinNetwork === value) return;
    stablecoinNetwork = value;
    receivingAddress = "";
    edited();
  }
  $: ticket = {
    direction, stablecoin, price, priceRounded, roundingAsset, amountFloored, shortfall, fundedBalance, blockingReason, everAmount, usdtAmount, receivingAddress, stablecoinNetwork, stablecoinNetworks, seller, askingRate, valid, reviewing, linkingEvm,
    marketPrice, marketRate, marketUsdRate, marketApprox, error,
    onPrice: changePrice,
    onEver: changeEver,
    onUsdt: changeUsdt,
    onStablecoin: changeStablecoin,
    onAddress: changeAddress,
    onStablecoinNetwork: changeStablecoinNetwork,
    onUseEthereum: useEthereumWallet,
    onMarketPrice: useMarketPrice,
    onRequest: () => seller ? void publish() : onConnect(),
    onCloseReview: () => { reviewing = false; },
  } satisfies SaleTicketState;
</script>

<slot {ticket} {offers} {mine} {history} {message} offerError={error} onCancel={cancel} />
