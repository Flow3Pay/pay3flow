<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { request, otcEvmNetwork, walletNetwork, type Session, type Direction } from "$lib/otc/service";
  import { connectWallet, wallets } from "$lib/wallet-session";
  import { saleRate, offerDecimal, offerTotal, offerEver, validOfferAmount, type PublicOffer as Offer, type OfferHistoryPoint, type SaleTicketState } from "$lib/otc/public-offers";
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
  let priceInput = "", priceMode = false, loadedDirection: Direction = "sell";
  let everAmount = "", usdtAmount = "", receivingAddress = "", key = "";
  let linkingEvm = false, reviewing = false, mounted = false, generation = 0, loadedActor = "";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: seller = session?.wallets?.find(wallet => wallet.chain === "everscale")?.address || "";
  $: draftKey = session ? `pay3flow.otc.sell-draft.v1:${session.actor}` : "";
  $: if (mounted && (session?.actor || "") !== loadedActor) {
    const previousActor = loadedActor;
    loadedActor = session?.actor || ""; loadedDirection = direction; mine = []; reviewing = false; error = ""; message = "";
    if (previousActor || !loadedActor) { everAmount = ""; usdtAmount = ""; receivingAddress = ""; key = ""; priceInput = ""; priceMode = false; }
    try {
      const draft = JSON.parse(localStorage.getItem(draftKey) || "null");
      if (draft && (draft.direction || "sell") === direction && [draft.ever_amount, draft.usdt_amount, draft.usdt_address, draft.key].every(value => typeof value === "string")) {
        everAmount = draft.ever_amount; usdtAmount = draft.usdt_amount; receivingAddress = draft.usdt_address; key = draft.key; priceMode = !!draft.price_mode; priceInput = typeof draft.price_input === "string" ? draft.price_input : ""; amountSource = draft.amount_source === "usdt" ? "usdt" : "ever";
      }
    } catch { /* An unreadable draft is not submitted. */ }
    void refresh();
  }
  $: askingRate = saleRate(everAmount,usdtAmount);
  $: price = priceMode ? priceInput : askingRate === "—" ? "" : askingRate;
  $: roundingAsset = priceMode ? amountSource === "ever" ? offerTotal(everAmount,priceInput)?.rounded ? "USDT" as const : null : offerEver(usdtAmount,priceInput)?.rounded ? "EVER" as const : null : null;
  $: priceRounded = !!roundingAsset;
  $: blockingReason = priceMode && !offerDecimal(priceInput,12)
    ? say("Enter a positive price.", "Укажите положительный курс.")
    : !validOfferAmount(everAmount,9,100_000_000_000_000n)
    ? say("Enter a valid EVER amount.", "Укажите корректную сумму EVER.")
    : !validOfferAmount(usdtAmount,6,1_000_000_000_000n)
    ? say("Enter a valid USDT total.", "Укажите корректную сумму USDT.")
    : !/^0x[0-9a-fA-F]{40}$/.test(receivingAddress.trim()) || /^0x0{40}$/.test(receivingAddress.trim())
    ? say("Enter your Ethereum wallet address, or use the wallet button.", "Укажите адрес Ethereum-кошелька или нажмите кнопку кошелька.") : "";
  $: valid = !blockingReason;
  $: if (direction !== loadedDirection) resetDirection(direction);
  function resetDirection(next: Direction) { loadedDirection = next; amountSource = next === "buy" ? "usdt" : "ever"; reviewing = false; key = ""; message = ""; }
  $: if (mounted) suggestEthereum(
    session?.wallets.find(w => w.chain === "ethereum")?.address || ($wallets.evm?.family === "evm" && $wallets.evm.chainId === otcEvmNetwork().chainId ? $wallets.evm.address : ""),
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
      const wallet = await connectWallet(walletNetwork("ethereum"));
      if (session?.actor !== actor) return;
      if (wallet.family !== "evm" || wallet.chainId !== otcEvmNetwork().chainId) throw new Error(say(`Select ${otcEvmNetwork().label}.`, `Выберите сеть ${otcEvmNetwork().label}.`));
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
  async function publish() {
    if (!enabled || !session || !seller || posting || busy || linkingEvm || !valid) return;
    if (!reviewing) { reviewing = true; error = ""; return; }
    key ||= crypto.randomUUID();
    const body = { direction, ever_amount: offerDecimal(everAmount,9)!, usdt_amount: offerDecimal(usdtAmount,6)!, usdt_address: receivingAddress.trim() };
    try {
      localStorage.setItem(draftKey, JSON.stringify({ ...body, key, price_mode: priceMode, price_input: priceInput, amount_source: amountSource }));
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
  onMount(() => {
    mounted = true;
    const timer = setInterval(() => { if (enabled && !posting && !busy) void refresh(); }, 5000);
    return () => { mounted = false; generation++; clearInterval(timer); };
  });

  let started = false;
  $: if (mounted && enabled && !started) { started = true; void refresh(); }
  function changePrice(value: string) { priceInput = value; priceMode = true; if (amountSource === "ever") usdtAmount = offerTotal(everAmount,priceInput)?.amount || ""; else everAmount = offerEver(usdtAmount,priceInput)?.amount || ""; edited(); }
  function changeEver(value: string) { everAmount = value; amountSource = "ever"; if (priceMode) usdtAmount = offerTotal(everAmount,priceInput)?.amount || ""; edited(); }
  function changeUsdt(value: string) { usdtAmount = value; amountSource = "usdt"; if (priceMode) everAmount = offerEver(usdtAmount,priceInput)?.amount || ""; edited(); }
  function changeAddress(value: string) { receivingAddress = value; edited(); }
  $: ticket = {
    direction, price, priceRounded, roundingAsset, blockingReason, everAmount, usdtAmount, receivingAddress, seller, askingRate, valid, reviewing, linkingEvm,
    onPrice: changePrice,
    onEver: changeEver,
    onUsdt: changeUsdt,
    onAddress: changeAddress,
    onUseEthereum: useEthereumWallet,
    onRequest: () => seller ? void publish() : onConnect(),
  } satisfies SaleTicketState;
</script>

<slot {ticket} {offers} {mine} {history} {message} offerError={error} onCancel={cancel} />
