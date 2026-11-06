<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { locale } from "$lib/i18n";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { request, type Config, type Session } from "$lib/otc/service";
  import { compareOfferRates, offerDirection, type PublicOffer } from "$lib/otc/public-offers";
  import OtcDeskBook from "$lib/components/otc/OtcDeskBook.svelte";
  import { otcOfferCovers, otcSide, otcTerminalHash } from "./frames";

  export let step: TutorialStep;
  const sessionKey = "pay3flow.otc.session.v1";
  let offers: PublicOffer[] = [], config: Config | null = null;
  let state: "loading" | "ready" | "unavailable" = "loading";
  let busy = false, notice = "", failure = "";
  let timer: ReturnType<typeof setInterval> | undefined;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: side = otcSide(step);
  // The desk book keeps the cheapest side for the direction this step exchanges.
  $: book = offers
    .filter(offer => offerDirection(offer) === side && otcOfferCovers(offer, step))
    .sort((a, b) => compareOfferRates(a.terms.ever_amount, a.terms.usdt_amount, b.terms.ever_amount, b.terms.usdt_amount) * (side === "sell" ? 1 : -1))
    .slice(0, 5);

  async function load() {
    try {
      const [published, settings] = await Promise.all([request<PublicOffer[]>("/offers"), request<Config>("/config")]);
      offers = published; config = settings;
      state = settings.public_offers_available ? "ready" : "unavailable";
    } catch { offers = []; config = null; state = "unavailable"; }
  }
  function signedIn(): Session | null {
    try {
      const saved = JSON.parse(sessionStorage.getItem(sessionKey) || "null") as Session | null;
      return saved?.token && Date.parse(saved.expires_at) > Date.now() ? saved : null;
    } catch { return null; }
  }
  function toTerminal() { location.hash = otcTerminalHash(step).replace(/^#/, ""); }
  async function accept(id: string, destination?: { stablecoin_network: string; stablecoin_asset_id: string | null; stablecoin_address: string }) {
    const owner = signedIn();
    if (!owner) { toTerminal(); return; }
    busy = true; failure = ""; notice = "";
    try {
      await request(`/offers/${id}/accept`, owner.token, destination || {}, crypto.randomUUID());
      notice = say("Offer accepted. Open the terminal to send the first transfer.", "Оффер принят. Откройте терминал, чтобы отправить первый перевод.");
      await tick();
    } catch (reason) { failure = reason instanceof Error ? reason.message : String(reason); }
    busy = false;
  }
  onMount(() => {
    void load();
    timer = setInterval(() => { if (!busy && document.visibilityState === "visible") void load(); }, 20000);
  });
  onDestroy(() => clearInterval(timer));
</script>

<section class="otcDeal" data-testid="guide-otc-card">
  <header class="dealHead">
    <div>
      <span class="dealEyebrow">{side === "sell" ? say("Buy EVER with USDT", "Купить EVER за USDT") : say("Sell EVER for USDT", "Продать EVER за USDT")}</span>
      <strong>{say("Live Pay3Flow offers", "Живые офферы Pay3Flow")}</strong>
    </div>
    <a class="terminalLink" href={otcTerminalHash(step)}>{say("Open OTC terminal", "Открыть терминал OTC")} <span aria-hidden="true">↗</span></a>
  </header>

  {#if state === "loading"}
    <p class="dealNote">{say("Loading the order book…", "Загружаем книгу офферов…")}</p>
  {:else if state === "unavailable"}
    <p class="dealNote">{say("No public offers are open right now. The terminal shows the desk quote instead.", "Публичных офферов сейчас нет. В терминале доступна котировка деска.")}</p>
  {:else}
    <OtcDeskBook publicOffers={book} config={config} listings={[]} selectedId="" onSelect={() => {}} compact showEmpty busy={busy}
      testId="guide-otc-offer" onAccept={accept} />
    {#if !book.length}<p class="dealNote">{say("No single offer covers this amount yet. Open the terminal to request a desk quote.", "Ни один оффер пока не покрывает эту сумму. Откройте терминал, чтобы запросить котировку деска.")}</p>{/if}
    {#if notice}<p class="dealNotice" role="status">{notice}</p>{/if}
    {#if failure}<p class="dealFailure" role="alert">{failure}</p>{/if}
    <p class="dealFoot">{say("Wallet to wallet, without escrow. Check both addresses before sending, and only accept an offer that is still open.", "Расчёт между кошельками без эскроу. Сверьте оба адреса перед переводом и принимайте только активный оффер.")}</p>
  {/if}
</section>

<style>
  .otcDeal {
    --otc-buy: var(--color-good); --otc-sell: var(--color-danger);
    --otc-buy-soft: color-mix(in srgb, var(--color-good) 13%, transparent);
    --otc-sell-soft: color-mix(in srgb, var(--color-danger) 9%, transparent);
    --otc-panel-padding: 14px; --otc-control-height: 44px;
    display: flex; flex-direction: column; width: min(100%, 470px); margin: 0 auto; overflow: visible;
    border: 1px solid var(--color-border); border-radius: 12px; background: var(--color-paper);
    color: var(--color-text); font-size: 12px; line-height: 1.5;
  }
  .dealHead { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px var(--otc-panel-padding); border-bottom: 1px solid var(--color-border); }
  .dealHead > div { display: grid; gap: 3px; min-width: 0; }
  .dealEyebrow { color: var(--color-text-faint); font: 9px var(--font-mono); letter-spacing: .08em; text-transform: uppercase; }
  .dealHead strong { font-size: 12px; font-weight: 650; }
  .terminalLink { flex: 0 0 auto; display: flex; align-items: center; gap: 5px; padding: 7px 10px; border: 1px solid var(--color-border-strong); border-radius: 8px; color: var(--color-accent-text); font-size: 11px; font-weight: 650; text-decoration: none; }
  .terminalLink:hover { background: var(--color-accent-soft); }
  .dealNote, .dealNotice, .dealFailure { margin: 0; padding: 11px var(--otc-panel-padding); color: var(--color-text-soft); font-size: 11px; }
  .dealNotice { color: var(--color-good-ink); }
  .dealFailure { color: var(--color-danger); }
  .dealFoot { margin: 0; padding: 10px var(--otc-panel-padding); border-top: 1px solid var(--color-border); color: var(--color-text-faint); font-size: 10px; }
</style>
