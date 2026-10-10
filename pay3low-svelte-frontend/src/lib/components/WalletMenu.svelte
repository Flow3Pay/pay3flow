<script lang="ts">
  import "@near-wallet-selector/modal-ui/styles.css";
  import { onMount, tick } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { env } from "$env/dynamic/public";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { locale, t } from "$lib/i18n";
  import { assetIcon } from "$lib/icons";
  import { wallets, connectWallet, disconnectFamily, restoreWallets } from "$lib/wallet-session";
  import type { WalletFamily } from "$lib/wallet-execution";

  const choices: { family: WalletFamily; network: string; label: string; icon: string; keywords: string }[] = [
    { family: "evm", network: "ethereum", label: "Ethereum", icon: "/icons/assets/eth.webp", keywords: "ETH EVM эфир этериум" },
    { family: "near", network: "near", label: "NEAR", icon: assetIcon("near"), keywords: "NEAR Protocol нир" },
    { family: "tron", network: "tron", label: "TRON", icon: "/icons/assets/trx.webp", keywords: "TRX трон" },
    { family: "everscale", network: "everscale", label: "Everscale", icon: "/icons/assets/ever.svg", keywords: "EVER эверскейл" },
  ];
  let open = false;
  let query = "";
  let searchInput: HTMLInputElement;
  let busy: WalletFamily | null = null;
  let error = "";
  let dialog: HTMLDivElement;
  let handoff = false;
  let motionDuration = 160;
  let toggle: HTMLButtonElement;
  $: count = Object.keys($wallets).length;
  $: terms = query.normalize("NFKC").toLowerCase().trim().split(/\s+/).filter(Boolean);
  $: visibleChoices = choices.filter(choice => {
    const text = `${choice.label} ${choice.network} ${choice.family} ${choice.keywords} ${$wallets[choice.family]?.address ?? ''}`.toLowerCase();
    return terms.every(term => text.includes(term));
  });
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  onMount(() => {
    void restoreWallets();
    const media = matchMedia("(prefers-reduced-motion: reduce)");
    const updateMotion = () => motionDuration = media.matches ? 0 : 160;
    updateMotion(); media.addEventListener("change", updateMotion);
    return () => media.removeEventListener("change", updateMotion);
  });

  function portal(node: HTMLDivElement) {
    const previousFocus = document.activeElement as HTMLElement | null;
    document.body.appendChild(node);
    const shell = document.querySelector<HTMLElement>(".appShell");
    const wasInert = shell?.inert ?? false;
    if (shell) shell.inert = true;
    const unlock = lockPageScroll();
    void tick().then(() => searchInput?.focus());
    return { destroy() {
      if (shell) shell.inert = wasInert;
      unlock(); node.remove(); previousFocus?.focus();
    } };
  }
  async function focusChoice(family: WalletFamily) {
    await tick();
    if (open) (dialog.querySelector<HTMLButtonElement>(`[data-family="${family}"] button`) ?? searchInput)?.focus();
  }
  function clearSearch() { query = ""; searchInput?.focus(); }
  function keydown(event: KeyboardEvent) {
    if (!open || handoff) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); open = false; return; }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      const results = [...dialog.querySelectorAll<HTMLButtonElement>(".connectionButton:not(:disabled)")];
      const index = results.findIndex(button => button === document.activeElement);
      if (document.activeElement === searchInput && event.key === "ArrowDown" && results.length) {
        event.preventDefault(); results[0].focus();
      } else if (index >= 0) {
        event.preventDefault();
        (event.key === "ArrowUp" ? results[index - 1] ?? searchInput : results[index + 1] ?? results[index]).focus();
      }
      return;
    }
    if (event.key !== "Tab") return;
    const items = [...dialog.querySelectorAll<HTMLElement>("input:not(:disabled), button:not(:disabled)")];
    const first = items[0], last = items.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }

  async function connect(family: WalletFamily, network: string) {
    if (busy) return;
    busy = family;
    error = "";
    // Selector UIs mount outside our dialog. Let them receive focus and input
    // while keeping the underlying application locked until they return.
    handoff = family === "near" || (family === "evm" && Boolean(env.PUBLIC_REOWN_PROJECT_ID?.trim()));
    try { await connectWallet(network); }
    catch (cause) { error = cause instanceof Error ? copy(cause.message) : copy("Wallet connection failed"); }
    finally { busy = null; handoff = false; void focusChoice(family); }
  }
  async function disconnect(family: WalletFamily) {
    if (busy) return;
    busy = family;
    error = "";
    try { await disconnectFamily(family); }
    catch (cause) { error = cause instanceof Error ? cause.message : copy("Wallet disconnect failed"); }
    finally { busy = null; void focusChoice(family); }
  }
</script>

<svelte:window on:keydown={keydown} />
<div class="walletMenu">
  <button class="walletToggle" type="button" bind:this={toggle} aria-label={count ? copy("Wallets ({count})", { count }) : copy("Connect wallet")} aria-haspopup="dialog" aria-expanded={open} aria-controls="wallet-connections" on:click={() => { error = ""; query = ""; open = true; }}>
    <img src="/icons/ui/wallet-connect.png" width="18" height="18" alt="" aria-hidden="true" />
    <span>{count ? copy("Wallets ({count})", { count }) : copy("Connect wallet")}</span>
  </button>
</div>
{#if open}
  <div class="walletBackdrop" class:handoff aria-hidden={handoff ? "true" : undefined} use:portal transition:fade={{ duration: motionDuration }} role="presentation" on:mousedown={(event) => { if (event.target === event.currentTarget) open = false; }}>
    <div class="walletConnections" id="wallet-connections" bind:this={dialog} role="dialog" aria-modal="true" aria-labelledby="wallet-connections-title" transition:fly={{ y: 8, duration: motionDuration }}>
      <div class="walletHeading">
        <label class="walletSearch"><svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.7" /><path d="m20 20-4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg><input type="search" bind:this={searchInput} bind:value={query} disabled={Boolean(busy)} placeholder={copy("Search networks or currencies")} aria-label={copy("Search wallets")} aria-controls="wallet-search-results" autocomplete="off" spellcheck="false" on:input={() => error = ""} /></label>
        {#if query}<button class="clearSearch" type="button" disabled={Boolean(busy)} aria-label={copy("Clear wallet search")} on:click={clearSearch}>×</button>{/if}
        <button class="closeButton" type="button" aria-label={copy("Close wallet dialog")} on:click={() => open = false}><svg width="19" height="19" viewBox="0 0 24 24" fill="none" aria-hidden="true"><path d="m6 6 12 12M18 6 6 18" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" /></svg></button>
      </div>
      <h2 id="wallet-connections-title">{copy("Wallet connections")}</h2>
      <div class="walletList" id="wallet-search-results">
        {#each visibleChoices as choice (choice.family)}
          {@const wallet = $wallets[choice.family]}
          <div class="walletChoice" data-family={choice.family} aria-busy={busy === choice.family}>
            <img class="walletAvatar" src={choice.icon} alt="" width="40" height="40" />
            <div class="walletIdentity"><strong>{choice.label}</strong>{#if wallet}<span class="address" title={wallet.address}>{wallet.address.length > 20 ? `${wallet.address.slice(0, 7)}…${wallet.address.slice(-6)}` : wallet.address}</span>{/if}</div>
            {#if wallet}
              <button class="connectionButton" type="button" disabled={Boolean(busy)} aria-label={copy("Disconnect {network} wallet", { network: choice.label })} on:click={() => disconnect(choice.family)}>{copy("Disconnect")}</button>
            {:else}
              <button class="connectionButton" type="button" disabled={Boolean(busy)} aria-label={busy === choice.family ? copy("Connecting…") : copy("Connect {network} wallet", { network: choice.label })} on:click={() => connect(choice.family, choice.network)}>{busy === choice.family ? copy("Connecting…") : copy("Connect")}</button>
            {/if}
          </div>
        {/each}
        {#if !visibleChoices.length}<div class="emptySearch" role="status"><strong>{copy("No wallets found")}</strong><span>{copy("Try a different search.")}</span></div>{/if}
      </div>
      {#if error}<p role="alert">{error}</p>{/if}
    </div>
  </div>
{/if}

<style>
  .walletMenu { position: relative; }
  button { min-height: 36px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); color: var(--color-text); font: inherit; cursor: pointer; transition: background .16s ease; }
  button:hover { background: var(--color-accent-soft); }
  button:disabled { opacity: .6; cursor: wait; }
  .walletToggle { display: flex; align-items: center; gap: 7px; padding: 0 10px; white-space: nowrap; font-size: 12px; font-weight: 750; }
  .walletToggle img { width: 18px; height: 18px; flex: 0 0 auto; object-fit: contain; filter: brightness(0); }
  :global(html[data-theme="dark"]) .walletToggle img { filter: brightness(0) invert(1); }
  .walletBackdrop { position: fixed; inset: 0; z-index: 2000; display: grid; place-items: center; padding: 20px; background: rgba(8, 11, 8, .6); }
  .walletBackdrop.handoff { visibility: hidden; pointer-events: none; }
  .walletConnections { width: min(460px, 100%); max-height: calc(100dvh - 40px); overflow-y: auto; padding: 24px; border: 1px solid var(--color-border); border-radius: 20px; background: var(--color-paper); color: var(--color-text); box-shadow: 0 24px 80px rgba(0, 0, 0, .18); }
  .walletHeading { display: flex; align-items: center; gap: 8px; margin-bottom: 18px; }
  .walletSearch { display: flex; align-items: center; gap: 10px; min-width: 0; min-height: 44px; flex: 1; color: var(--color-text-soft); border-bottom: 2px solid var(--color-border); }
  .walletSearch:focus-within { color: var(--color-accent-text); border-bottom-color: var(--color-accent); }
  .walletSearch svg { flex: 0 0 auto; }
  .walletSearch input { width: 100%; min-width: 0; padding: 10px 0; border: 0; outline: 0; background: transparent; color: var(--color-text); font: inherit; font-size: 14px; }
  .walletSearch input::placeholder { color: var(--color-text-soft); opacity: 1; }
  .walletSearch input::-webkit-search-cancel-button { display: none; }
  .clearSearch { flex: 0 0 32px; padding: 0; font-size: 22px; }
  h2 { min-width: 0; overflow-wrap: anywhere; margin: 0 0 12px; color: var(--color-text-soft); font-size: 12px; font-weight: 500; }
  .closeButton { display: grid; flex: 0 0 40px; height: 40px; place-items: center; border-radius: 10px; }
  .walletList { display: grid; gap: 10px; }
  .walletChoice { display: flex; gap: 12px; padding: 14px; align-items: center; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); }
  .walletAvatar { display: block; width: 40px; height: 40px; flex: 0 0 auto; border-radius: 50%; object-fit: contain; }
  .walletIdentity { display: grid; gap: 5px; min-width: 0; }
  strong { font-size: 14px; font-weight: 750; }
  .connectionButton { flex: 0 0 auto; margin-left: auto; padding: 7px 12px; background: var(--color-paper); font-size: 12px; font-weight: 750; }
  .address { font-family: var(--font-mono); font-size: 11px; color: var(--color-text-soft); overflow-wrap: anywhere; }
  .emptySearch { display: grid; gap: 8px; padding: 30px 12px; text-align: center; color: var(--color-text-soft); font-size: 12px; }
  p { margin: 16px 0 0; color: var(--color-danger); font-size: 12px; line-height: 1.5; overflow-wrap: anywhere; }
  @media (max-width: 640px), (pointer: coarse) { button { min-height: 44px; } }
  @media (max-width: 640px) {
    .walletToggle span { display: none; }
    .walletToggle { width: 44px; justify-content: center; }
  }
  @media (max-width: 480px) {
    .walletSearch input { font-size: 16px; }
    .walletBackdrop { padding: 16px; }
    .walletConnections { padding: 18px; max-height: calc(100dvh - 32px); }
    .walletChoice { flex-wrap: wrap; padding: 12px; gap: 10px; }
    .connectionButton { flex-basis: 100%; margin-left: 0; }
  }
  @media (prefers-reduced-motion: reduce) { button { transition: none; } }
</style>
