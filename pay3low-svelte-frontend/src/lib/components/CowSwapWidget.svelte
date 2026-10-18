<script lang="ts">
  import { onMount } from "svelte";
  import type { CowSwapWidgetHandler, CowSwapWidgetParams, EthereumProvider, SupportedChainId, TradeType } from "@cowprotocol/widget-lib";
  import type { RouteCandidate } from "$lib/exchange";
  import type { TutorialStep } from "$lib/route-tutorial";
  import { chainIdForNetwork } from "$lib/wallet-execution";
  import { connectWallet, restoreWallets, wallets } from "$lib/wallet-session";
  import { locale, t } from "$lib/i18n";

  export let route: RouteCandidate;
  export let step: TutorialStep;
  let container: HTMLDivElement;
  let widget: CowSwapWidgetHandler | undefined;
  let ready = false;
  let busy = false;
  let error = "";
  let disposed = false;
  let providerRevision = 0;
  let theme: "light" | "dark" = "light";
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: paths = route.route_path?.filter(asset => asset.includes("@")) ?? [];
  $: sourceNetwork = route.execution?.from_asset.split("@")[1] ?? paths[0]?.split("@")[1] ?? route.source_network ?? "ethereum";
  $: targetNetwork = route.execution?.to_asset.split("@")[1] ?? paths.at(-1)?.split("@")[1] ?? route.target_network ?? sourceNetwork;
  const extraChains: Record<string, number> = { linea: 59144, plasma: 9745, ink: 57073, bitcoin: 1000000000, solana: 1000000001 };
  $: sharedWalletSupported = Boolean(chainIdForNetwork(sourceNetwork));
  $: chainId = chainIdForNetwork(sourceNetwork) ?? extraChains[sourceNetwork];
  $: targetChainId = chainIdForNetwork(targetNetwork) ?? extraChains[targetNetwork];
  $: amount = route.execution?.input_amount ?? (route.entry_offer_snapshot ? route.acquired_asset_amount : route.source_amount);
  $: tokenUrl = (step.url ?? route.route_provider_url ?? "").match(/#\/\d+\/swap\/([^/?#]+)\/([^/?#]+)/);
  $: params = {
    appCode: "Pay3Flow",
    chainId: (chainId ?? 1) as SupportedChainId,
    targetChainId: targetChainId ?? undefined,
    sell: { asset: tokenUrl ? decodeURIComponent(tokenUrl[1]) : step.from, amount },
    buy: { asset: tokenUrl ? decodeURIComponent(tokenUrl[2]) : step.to },
    tradeType: "swap" as TradeType,
    enabledTradeTypes: ["swap" as TradeType],
    standaloneMode: !sharedWalletSupported,
    disableSwitchingTokens: true,
    disableInfiniteApprove: true,
    theme,
    locale: $locale,
    rootStyle: { width: "100%", height: "var(--dynamicHeight, 640px)", minHeight: "520px", borderRadius: "24px" },
  } satisfies CowSwapWidgetParams;
  $: if (widget) widget.updateParams(params);
  $: if (widget) void updateProvider($wallets.evm);

  async function updateProvider(wallet: typeof $wallets.evm) {
    const version = ++providerRevision;
    if (!sharedWalletSupported || !wallet || wallet.family !== "evm") { widget?.updateProvider(undefined); return; }
    try {
      const { getAccount } = await import("@wagmi/core");
      const account = getAccount(wallet.config as Parameters<typeof getAccount>[0]);
      const provider = await account.connector?.getProvider();
      if (version === providerRevision && !disposed) widget?.updateProvider(provider as EthereumProvider | undefined);
    } catch (cause) { if (!disposed) error = cause instanceof Error ? cause.message : copy("Wallet operation failed"); }
  }
  async function connect() {
    busy = true;
    error = "";
    try { await connectWallet(sourceNetwork); }
    catch (cause) { error = cause instanceof Error ? cause.message : copy("Wallet operation failed"); }
    finally { busy = false; }
  }
  onMount(() => {
    theme = document.documentElement.dataset.theme === "dark" ? "dark" : "light";
    const observer = new MutationObserver(() => theme = document.documentElement.dataset.theme === "dark" ? "dark" : "light");
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    void restoreWallets();
    void import("@cowprotocol/widget-lib").then(({ createCowSwapWidget }) => {
      if (disposed || !chainId) return;
      widget = createCowSwapWidget(container, { params, onReady: () => ready = true, onLoadingError: () => error = copy("Could not load CoW Swap. Try again.") });
      widget.iframe.title = "CoW Swap";
    }).catch(cause => { if (!disposed) error = cause instanceof Error ? cause.message : copy("Could not load CoW Swap. Try again."); });
    return () => { disposed = true; providerRevision += 1; observer.disconnect(); widget?.destroy(); };
  });
</script>

<section class="cowSwap" data-testid="cow-live-widget" aria-label="CoW Swap">
  <div class="walletBar">
    <strong>CoW Swap</strong>
    {#if sharedWalletSupported}<button type="button" disabled={busy} on:click={connect}>{busy ? copy("Waiting for wallet…") : $wallets.evm ? `${$wallets.evm.address.slice(0, 6)}…${$wallets.evm.address.slice(-4)}` : copy("Connect wallet")}</button>{/if}
  </div>
  {#if !chainId}<p>{copy("Embedded execution does not yet support a wallet for {network}. Open the provider manually for this route.", { network: sourceNetwork })}</p>{:else if !ready && !error}<p role="status">{copy("Loading CoW Swap…")}</p>{/if}
  <div bind:this={container} class="widgetContainer"></div>
  {#if error}<p role="alert">{error}</p>{/if}
  <a href={step.url ?? "https://swap.cow.fi/"} target="_blank" rel="noopener noreferrer">{copy("Open {venue}", { venue: "CoW Swap" })} ↗</a>
</section>

<style>
  .cowSwap { width: min(100%, 470px); margin: 28px auto; }
  .walletBar { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 12px; color: var(--color-text); }
  .walletBar strong { font-size: 15px; }
  button { min-height: 44px; padding: 10px 16px; border: 0; border-radius: 24px; background: #004591; color: #65d9ff; font-size: 13px; font-weight: 600; }
  button:disabled { opacity: .6; }
  .widgetContainer { min-height: 520px; }
  /* Match CoW's default document color scheme so the iframe canvas stays transparent. */
  .widgetContainer :global(iframe) { color-scheme: normal; }
  p, a { display: block; font-size: 12px; line-height: 1.6; color: var(--color-text-soft); }
  p[role="alert"] { color: var(--color-danger); }
  a { text-align: center; margin-top: 14px; }
</style>
