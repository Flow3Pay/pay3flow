<script lang="ts">
  import { assetIcon, networkIcon, venueIcon } from "$lib/icons";
  import { fiatFlagUrl } from "$lib/currency-flags";
  import { FIAT_CURRENCIES } from "$lib/workflow-steps";
  import { locale, t } from "$lib/i18n";
  export let asset: string;
  export let network = "";
  export let networkLabel = network;
  export let provider = "";
  export let providerLabel = provider;
  export let iconUrl: string | undefined = undefined;
  export let paymentMethod = "";
  export let paymentMethodIcon = "";
  export let fill = false;
  $: flag = fiatFlagUrl(asset);
  $: icon = flag ?? (FIAT_CURRENCIES.has(asset.toUpperCase()) ? assetIcon(asset) : iconUrl ?? assetIcon(asset));
</script>

<span class="workflowControl" class:fill>
  <span class="workflowAsset"><span class="workflowIcon" aria-hidden="true"><img class:fiatFlag={Boolean(flag)} src={icon} alt="" width="18" height="18" loading="lazy" decoding="async" /></span><strong>{asset.toUpperCase()}</strong></span>
  {#if network}<span class="workflowNetwork" aria-label={t("Network: {network}", { network: networkLabel }, $locale)} title={networkLabel}><span class="workflowNetworkIcon" aria-hidden="true"><img src={networkIcon(network)} alt="" width="18" height="18" loading="lazy" decoding="async" /></span></span>{/if}
  {#if paymentMethod}<span class="workflowNetwork workflowPayment" aria-label={t("Payment method: {method}", { method: paymentMethod }, $locale)} title={paymentMethod}><span class="workflowNetworkIcon" aria-hidden="true">{#if paymentMethodIcon}<img src={paymentMethodIcon} alt="" width="18" height="18" loading="lazy" decoding="async" />{:else}<span class="paymentInitials">{paymentMethod.slice(0, 2).toUpperCase()}</span>{/if}</span></span>{/if}
  {#if provider}<span class="workflowVenue" title={providerLabel}><span class="workflowVenueIcon" aria-hidden="true"><img src={venueIcon(provider)} alt="" width="18" height="18" loading="lazy" decoding="async" /></span><strong>{providerLabel}</strong></span>{/if}
</span>

<style>
.workflowAsset, .workflowVenue { display: inline-flex; min-width: 0; align-items: center; }
.workflowControl {
  display: inline-flex;
  min-width: 0;
  max-width: 100%;
  align-items: stretch;
  overflow: hidden;
  border: 1px solid #d7dcd3;
  border-radius: 5px;
  background: #e7ebe2;
}

.workflowAsset {
  flex: 0 0 auto;
  gap: 6px;
  padding: 5px 8px 5px 6px;
  color: var(--color-text);
}

.workflowAsset strong {
  font-size: 12px;
  font-weight: 850;
}

.workflowNetwork {
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  padding: 5px 7px;
  border-left: 1px solid #d7dcd3;
}

.workflowNetworkIcon {
  display: inline-grid;
  width: 18px;
  height: 18px;
  flex: 0 0 auto;
  place-items: center;
  overflow: hidden;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.08);
}

.workflowNetworkIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.workflowIcon,
.workflowVenueIcon {
  display: inline-grid;
  flex: 0 0 auto;
  place-items: center;
  overflow: hidden;
  border-radius: 50%;
}

.workflowIcon {
  width: 18px;
  height: 18px;
  background: rgba(255, 255, 255, 0.08);
}

.workflowVenueIcon {
  width: 18px;
  height: 18px;
}

.workflowIcon img,
.workflowVenueIcon img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.workflowIcon img.fiatFlag { object-fit: cover; }

.workflowVenue {
  flex: 0 1 auto;
  gap: 6px;
  padding: 5px 8px 5px 7px;
  border-left: 1px solid #d7dcd3;
  color: var(--color-text);
}

.workflowVenue strong {
  overflow: hidden;
  line-height: 1.5;
  padding-block: 1px;
  font-size: 12px;
  font-weight: 850;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fill { width: 100%; height: 100%; }
.paymentInitials { font-size: 9px; font-weight: 800; line-height: 1; }
:global(html[data-theme="dark"]) .workflowControl {
  border-color: #3b3b3b;
  background: #2a2a2a;
}

:global(html[data-theme="dark"]) .workflowNetwork,
:global(html[data-theme="dark"]) .workflowVenue {
  border-color: #3b3b3b;
}


@media (max-width: 640px) {
  .workflowAsset {
    gap: 3px;
    padding: 4px 5px 4px 4px;
  }

  .workflowNetwork {
    padding: 4px 3px;
  }

  .workflowVenue {
    gap: 3px;
    padding: 4px 5px 4px 4px;
  }

  .workflowAsset strong,
  .workflowVenue strong {
    font-size: 12px;
  }

  .workflowIcon,
  .workflowNetworkIcon,
  .workflowVenueIcon {
    width: 12px;
    height: 12px;
  }

}
@media (max-width: 980px), (pointer: coarse) {
  .workflowControl { flex: 1; }
  .workflowControl .workflowVenue { min-width: 0; margin-left: auto; }
  .workflowControl .workflowVenue strong { max-width: 110px; }
  .workflowControl .workflowIcon, .workflowControl .workflowNetworkIcon, .workflowControl .workflowVenueIcon { width: 16px; height: 16px; }
}
</style>
