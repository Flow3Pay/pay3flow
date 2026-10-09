<script lang="ts">
  import "@near-wallet-selector/modal-ui/styles.css";
  import { onMount } from "svelte";
  import { locale, t } from "$lib/i18n";
  import { wallets, connectWallet, disconnectFamily, restoreWallets } from "$lib/wallet-session";
  import type { WalletFamily } from "$lib/wallet-execution";

  const choices: { family: WalletFamily; network: string; label: string }[] = [
    { family: "evm", network: "ethereum", label: "Ethereum" },
    { family: "near", network: "near", label: "NEAR" },
    { family: "tron", network: "tron", label: "TRON" },
  ];
  let open = false;
  let busy: WalletFamily | null = null;
  let error = "";
  let root: HTMLDivElement;
  let toggle: HTMLButtonElement;
  $: count = Object.keys($wallets).length;
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  onMount(() => { void restoreWallets(); });

  async function connect(family: WalletFamily, network: string) {
    if (busy) return;
    busy = family;
    error = "";
    try { await connectWallet(network); }
    catch (cause) { error = cause instanceof Error ? cause.message : copy("Wallet connection failed"); }
    finally { busy = null; }
  }
  async function disconnect(family: WalletFamily) {
    if (busy) return;
    busy = family;
    error = "";
    try { await disconnectFamily(family); }
    catch (cause) { error = cause instanceof Error ? cause.message : copy("Wallet disconnect failed"); }
    finally { busy = null; }
  }
  function outside(event: MouseEvent) { if (open && !event.composedPath().includes(root)) open = false; }
  function keydown(event: KeyboardEvent) { if (open && event.key === "Escape") { open = false; toggle.focus(); } }
</script>

<svelte:window on:click={outside} on:keydown={keydown} />
<div class="walletMenu" bind:this={root}>
  <button class="walletToggle" type="button" bind:this={toggle} aria-label={count ? copy("Wallets ({count})", { count }) : copy("Connect wallet")} aria-expanded={open} aria-controls="wallet-connections" on:click={() => open = !open}>
    <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><path d="M4 5h14v3M4 5v14h17V8H4V5Zm12 7h5v4h-5v-4Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" /></svg>
    <span>{count ? copy("Wallets ({count})", { count }) : copy("Connect wallet")}</span>
  </button>
  {#if open}
    <div class="walletConnections" id="wallet-connections" role="group" aria-label={copy("Wallet connections")}>
      {#each choices as choice}
        {@const wallet = $wallets[choice.family]}
        <div class="walletChoice">
          <strong>{choice.label}</strong>
          {#if wallet}
            <span class="address" title={wallet.address}>{wallet.address.length > 20 ? `${wallet.address.slice(0, 7)}…${wallet.address.slice(-6)}` : wallet.address}</span>
            <button type="button" disabled={Boolean(busy)} aria-label={copy("Disconnect {network} wallet", { network: choice.label })} on:click={() => disconnect(choice.family)}>{copy("Disconnect")}</button>
          {:else}
            <button type="button" disabled={Boolean(busy)} on:click={() => connect(choice.family, choice.network)}>{busy === choice.family ? copy("Connecting…") : copy("Connect {network} wallet", { network: choice.label })}</button>
          {/if}
        </div>
      {/each}
      {#if error}<p role="alert">{error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .walletMenu { position: relative; }
  button { min-height: 36px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); color: var(--color-text); font: inherit; cursor: pointer; }
  button:disabled { opacity: .6; cursor: wait; }
  .walletToggle { display: flex; align-items: center; gap: 7px; padding: 0 10px; white-space: nowrap; font-size: 12px; font-weight: 750; }
  .walletConnections { position: absolute; top: calc(100% + 10px); right: 0; width: min(360px, calc(100vw - 32px)); padding: 14px; border: 1px solid var(--color-border); border-radius: 14px; background: var(--color-paper); box-shadow: 0 12px 40px #0002; }
  .walletChoice { display: grid; grid-template-columns: auto 1fr; gap: 8px; padding: 10px 0; align-items: center; }
  .walletChoice button { grid-column: 1 / -1; padding: 6px 10px; }
  .address { justify-self: end; font-size: 12px; color: var(--color-text-soft); overflow-wrap: anywhere; }
  p { color: var(--color-danger); font-size: 12px; overflow-wrap: anywhere; }
  @media (max-width: 640px), (pointer: coarse) {
    button { min-height: 44px; }
  }
  @media (max-width: 640px) {
    .walletConnections { position: fixed; top: 80px; right: 16px; }
    .walletToggle span { display: none; }
    .walletToggle { width: 44px; justify-content: center; }
  }
</style>
