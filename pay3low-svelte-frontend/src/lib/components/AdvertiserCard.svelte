<script lang="ts">
  import type { P2pOffer, ServiceLink } from "$lib/exchange";
  import { venueIcon } from "$lib/icons";
  import { locale, t } from "$lib/i18n";
  export let offer: P2pOffer | undefined;
  export let label: string;
  export let venueNames: Record<string, string> = {};
  export let serviceLink: ServiceLink | undefined = undefined;
  export let onOpenService: (link: ServiceLink) => void = () => {};
  const VENUE_ICONS: Record<string, string> = { binance: venueIcon("binance"), bybit: venueIcon("bybit"), okx: venueIcon("okx"), bitget: venueIcon("bitget"), rapira: venueIcon("rapira"), whitebird: venueIcon("whitebird"), "cifra-broker": venueIcon("cifra-broker"), bestchange: venueIcon("bestchange"), dzengi: venueIcon("dzengi"), skylabs: venueIcon("skylabs") };
  const venueName = (value?: string) => value ? venueNames[value.toLowerCase()] ?? value : "P2P market";
  $: language = $locale;
  const copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, language);
  const percentage = (value?: number | null) => value == null ? "—" : `${(value * 100).toFixed(1)}%`;
  $: venue = offer ? venueName(offer.source) : "";
  $: directExchange = offer?.advertiser.user_type === "service" || offer?.source.toLowerCase() === "whitebird";
  $: offerIcon = offer ? VENUE_ICONS[offer.source.toLowerCase()] : undefined;
  $: profileUrl = offer?.advertiser_profile_url ?? null;
  $: actionUrl = offer ? profileUrl ?? offer.source_url : "";
  $: actionLabel = offer ? (directExchange ? copy("Open {venue} exchange", { venue }) : profileUrl ? copy("Open {venue} profile", { venue }) : copy("Open {venue} P2P and find {nickname}", { venue, nickname: offer.advertiser.nickname })) : "";
  function fallbackVenueIcon(event: Event, source: string) {
    const image = event.currentTarget as HTMLImageElement;
    image.onerror = null;
    image.src = venueIcon(source);
  }
</script>

{#if !offer}
  <div class="counterparty missing"><span class="counterpartyLabel">{label}</span><strong>{copy("Advertiser details unavailable")}</strong></div>
{:else}
  <div class="counterparty">
    <div class="counterpartyIdentity">
      <span class:directExchangeAvatar={directExchange} class="counterpartyAvatar" aria-hidden="true">
        {#if directExchange && offerIcon}
          <img class="avatarLogo" src={offerIcon} alt="" width="42" height="42" loading="lazy" decoding="async" on:error={(event) => fallbackVenueIcon(event, offer?.source ?? "")} />
        {:else}
          <span class="avatarInitial">{offer.advertiser.nickname.trim().charAt(0).toUpperCase() || "?"}</span>
          {#if offerIcon}<span class="avatarVenue"><img src={offerIcon} alt="" width="16" height="16" loading="lazy" decoding="async" on:error={(event) => fallbackVenueIcon(event, offer?.source ?? "")} /></span>{/if}
        {/if}
      </span>
      <div class="counterpartyIdentityCopy"><div class="counterpartyTopline"><span class="counterpartyLabel">{label}</span><span class={directExchange || profileUrl ? "profileBadge" : "manualBadge"}>{directExchange ? copy("Direct exchange") : profileUrl ? copy("User profile") : copy("Find by nickname")}</span></div><strong class="advertiser">{offer.advertiser.nickname}</strong><span class="venueLine">{venue} · {directExchange ? copy("Exchange service") : offer.advertiser.is_merchant ? copy("Merchant") : copy("Advertiser")}</span></div>
    </div>
    <div class="metrics">{#if !directExchange}<span><b>{percentage(offer.advertiser.completion_rate_30d)}</b> {copy("completion")}</span><span><b>{offer.advertiser.completed_orders_30d ?? "—"}</b> {copy("orders / 30d")}</span>{/if}<span><b>{offer.price} {offer.fiat}</b> {copy("rate")}</span></div>
    <span class="paymentLine">{directExchange ? copy("Settlement") : copy("Payment")}: {offer.payment_methods.length ? offer.payment_methods.join(", ") : copy("confirm on provider")}</span>
    {#if serviceLink}<button type="button" class="profileLink" on:click={() => onOpenService(serviceLink!)}>{actionLabel} <span>↗</span></button>{:else}<a href={actionUrl} target="_blank" rel="noreferrer noopener" class="profileLink">{actionLabel} <span>↗</span></a>{/if}
    {#if !directExchange && !profileUrl}<small class="adHint">{copy("Match the nickname and ad ID {id} before opening an order.", { id: offer.ad_id })}</small>{/if}
  </div>
{/if}

<style>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 1200;
  display: grid;
  place-items: center;
  padding: 20px;
  background: rgba(15, 17, 14, 0.68);
  backdrop-filter: blur(18px) saturate(120%);
  -webkit-backdrop-filter: blur(18px) saturate(120%);
  animation: fadeIn 0.2s ease;
}

.modal {
  width: min(100%, 560px);
  max-height: min(720px, calc(100vh - 40px));
  overflow-y: auto;

  padding: 28px;
  border: 1px solid rgba(255, 255, 255, 0.72);
  border-radius: 28px;
  background: rgba(250, 250, 246, 0.98);
  box-shadow: none;
  color: var(--color-text);
  animation: modalIn 0.3s cubic-bezier(0.22, 1, 0.36, 1);
}
.header {
  display: flex;
  justify-content: space-between;
  gap: 20px;
}

.eyebrow {
  color: var(--color-violet);
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.1em;
  text-transform: uppercase;
}

.header h2 {
  margin: 8px 0 7px;
  font-size: 27px;
  letter-spacing: -0.055em;
  line-height: 1.05;
}

.header p {
  margin: 0;
  color: var(--color-text-soft);
  font-size: 12px;
}

.header p strong {
  color: var(--color-text);
}

.closeButton {
  width: 36px;
  height: 36px;
  flex: 0 0 auto;
  border: 1px solid var(--color-border);
  border-radius: 12px;
  color: var(--color-text-soft);
  font-size: 24px;
  line-height: 1;
}

.closeButton:hover {
  background: var(--color-accent-soft);
  color: var(--color-text);
}

.workflow {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 25px;
}

.step {
  display: grid;
  grid-template-columns: 34px 1fr;
  gap: 13px;
  padding: 15px;
  border: 1px solid var(--color-border);
  border-radius: 17px;
  background: rgba(255, 255, 255, 0.72);
}

.stepNumber {
  display: grid;
  width: 30px;
  height: 30px;
  place-items: center;
  border-radius: 10px;
  background: var(--color-primary);
  color: var(--color-accent);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 800;
}

.step strong {
  display: block;
  font-size: 13px;
  line-height: 1.35;
}

.step p {
  margin: 6px 0 10px;
  color: var(--color-text-soft);
  font-size: 12px;
  line-height: 1.5;
}

.counterparty {
  margin-top: 12px;
  padding: 12px;
  border: 1px solid rgba(111, 83, 190, 0.2);
  border-radius: 14px;
  background: rgba(255, 255, 255, 0.78);
}

.counterpartyTopline {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.counterpartyIdentity {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.counterpartyIdentityCopy {
  min-width: 0;
  flex: 1;
}

.counterpartyAvatar {
  position: relative;
  display: grid;
  width: 42px;
  height: 42px;
  flex: 0 0 42px;
  place-items: center;
  border: 1px solid #cfe0bf;
  border-radius: 50%;
  background: linear-gradient(145deg, #d9f28c, #8dbd2b);
  color: #30430b;
  font-size: 16px;
  font-weight: 850;
  box-shadow: none;
}

.counterpartyAvatar.directExchangeAvatar {
  overflow: hidden;
  border-color: var(--color-border);
  background: #fff;
}

.avatarLogo {
  display: block;
  width: 100%;
  height: 100%;
  padding: 4px;
  object-fit: contain;
}

.avatarVenue {
  position: absolute;
  right: -3px;
  bottom: -2px;
  display: grid;
  width: 16px;
  height: 16px;
  place-items: center;
  overflow: hidden;
  border: 2px solid #f4f8f1;
  border-radius: 50%;
  background: #fff;
}

.avatarVenue img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.counterpartyLabel {
  color: var(--color-violet);
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.profileBadge,
.manualBadge {
  padding: 4px 7px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 850;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.profileBadge {
  background: rgba(181, 224, 58, 0.2);
  color: var(--color-accent-text);
}

.manualBadge {
  background: rgba(240, 166, 89, 0.16);
  color: var(--color-warn);
}

.advertiser {
  display: block;
  margin-top: 9px;
  font-size: 16px;
  letter-spacing: -0.03em;
}

.counterpartyIdentityCopy .advertiser {
  margin-top: 6px;
}

.venueLine,
.paymentLine,
.adHint {
  display: block;
  color: var(--color-text-faint);
  font-size: 12px;
  line-height: 1.45;
}

.metrics {
  display: flex;
  flex-wrap: wrap;
  gap: 6px 12px;
  margin: 10px 0 7px;
  color: var(--color-text-soft);
  font-family: var(--font-mono);
  font-size: 12px;
}

.metrics b {
  color: var(--color-text);
}

.profileLink {
  display: inline-block;
  margin-top: 10px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--color-accent-text);
  font-size: 12px;
  font-weight: 850;
  text-decoration: none;
}

.profileLink:hover {
  text-decoration: underline;
}

.profileLink span {
  margin-left: 3px;
}

.adHint {
  margin-top: 6px;
  font-size: 12px;
}

.missing {
  color: var(--color-text-soft);
}

.warning {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 16px;
  padding: 12px 14px;
  border: 1px solid rgba(240, 166, 89, 0.28);
  border-radius: 14px;
  background: rgba(240, 166, 89, 0.1);
  color: #75501f;
  font-size: 12px;
  line-height: 1.45;
}

.warning strong {
  font-size: 12px;
  text-transform: uppercase;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes modalIn {
  from { opacity: 0; transform: translateY(16px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.backdrop {
  background: rgba(38, 57, 37, 0.28);
  backdrop-filter: blur(9px);
  -webkit-backdrop-filter: blur(9px);
}

.modal {
  padding: 24px;
  border-color: var(--color-border-strong);
  border-radius: 16px;
  background: rgba(255, 255, 255, 0.98);
  box-shadow: none;
}

.eyebrow,
.counterpartyLabel {
  color: var(--color-accent-text);
}

.closeButton {
  border-color: var(--color-border);
  border-radius: 9px;
}

.step {
  border-radius: 10px;
  background: #fbfcfa;
  transition: border-color 0.14s ease, background 0.14s ease, transform 0.14s ease;
}

.step:hover {
  border-color: #b7cead;
  background: #fff;
  transform: translateX(2px);
}

.counterparty {
  border-color: #d4e4c5;
  background: #f4f8f1;
}

.stepNumber {
  border-radius: 8px;
  background: var(--color-primary);
  color: var(--color-accent);
}

@media (max-width: 560px) {
  .backdrop {
    padding: 12px;
  }

  .modal {
    max-height: calc(100vh - 24px);
    padding: 22px 18px;
    border-radius: 23px;
  }

  .header h2 {
    font-size: 23px;
  }
}

:global(html[data-theme="dark"]) .modal {
  border-color: var(--color-border-strong);
  background: rgba(25, 25, 25, 0.99);
  box-shadow: none;
}

:global(html[data-theme="dark"]) .counterparty {
  border-color: #3b3b3b;
  background: #222222;
}

:global(html[data-theme="dark"]) .step:hover {
  border-color: #555555;
  background: #2a2a2a;
}

:global(html[data-theme="dark"]) .avatarVenue {
  border-color: #222222;
  background: #ffffff;
}

:global(html[data-theme="dark"]) .profileBadge {
  color: var(--color-accent);
}

</style>
