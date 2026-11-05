<script lang="ts">
  import { tick } from "svelte";
  import { locale } from "$lib/i18n";
  import type { Stablecoin } from "$lib/otc/public-offers";

  export let selected: Stablecoin = "USDT";
  export let onSelect: (token: Stablecoin) => void;

  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let query = "";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: tokens = ([
    { symbol: "USDT", name: "Tether", icon: "/icons/assets/usdt.png", keywords: "usdt tether" },
    { symbol: "USDC", name: "USD Coin", icon: "/icons/assets/usdc.png", keywords: "usdc usd coin circle" },
  ] as const).filter(token => `${token.symbol} ${token.name} ${token.keywords} ethereum erc20`.toLowerCase().includes(query.trim().toLowerCase()));

  async function open() {
    query = "";
    if (!dialog.open) dialog.showModal();
    await tick();
    input?.focus();
  }
  function choose(token: Stablecoin) {
    dialog.close();
    onSelect(token);
  }
  function close() {
    if (dialog.open) dialog.close();
    query = "";
  }
</script>

<button type="button" class="selectedToken" aria-haspopup="dialog" aria-label={say(`Choose stablecoin, current ${selected}`, `Выбрать стейблкоин, сейчас ${selected}`)} data-testid="otc-stablecoin-select" on:click={open}>
  <span class="assetMain"><span class="assetAvatar"><img src={selected === "USDC" ? "/icons/assets/usdc.png" : "/icons/assets/usdt.png"} alt="" width="28" height="28" /></span><span class="assetText"><strong>{selected}</strong><small>{selected === "USDC" ? "USD Coin" : "Tether"}</small></span></span>
  <span class="assetNetwork"><span class="assetCopy"><strong>Ethereum</strong><small>ERC-20</small></span><span class="chevron" aria-hidden="true">⌄</span></span>
</button>

<dialog bind:this={dialog} class="pickerDialog" aria-labelledby="stablecoin-picker-title" data-testid="otc-stablecoin-picker" on:cancel|preventDefault={close} on:click={(event) => { if (event.target === dialog) close(); }}>
  <div class="pickerHead">
    <h2 id="stablecoin-picker-title">{say("Choose a token", "Выберите токен")}</h2>
    <button type="button" class="close" aria-label={say("Close", "Закрыть")} on:click={close}>×</button>
  </div>
  <label class="search">
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.7" /><path d="m20 20-4-4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg>
    <input bind:this={input} bind:value={query} placeholder={say("Search tokens", "Поиск токенов")} aria-label={say("Search tokens", "Поиск токенов")} />
  </label>
  <div class="options" role="listbox" aria-label={say("ERC-20 stablecoins", "Стейблкоины ERC-20")}>
    {#each tokens as token (token.symbol)}
      <button type="button" role="option" aria-selected={selected === token.symbol} class:active={selected === token.symbol} data-testid={`otc-stablecoin-${token.symbol.toLowerCase()}`} on:click={() => choose(token.symbol)}>
        <img src={token.icon} alt="" width="34" height="34" />
        <span class="tokenName"><strong>{token.symbol}</strong><small>{token.name}</small></span>
        <span class="tokenNetwork">Ethereum · ERC-20</span>
        {#if selected === token.symbol}<span class="check" aria-hidden="true">✓</span>{/if}
      </button>
    {:else}
      <p class="empty">{say("No tokens found.", "Токены не найдены.")}</p>
    {/each}
  </div>
</dialog>

<style>
  .selectedToken { display:flex; width:100%; min-height:44px; align-items:center; justify-content:space-between; gap:8px; padding:0; text-align:left; }
  .assetMain,.assetNetwork { display:flex; min-width:0; align-items:center; gap:8px; }
  .assetAvatar { display:grid; width:30px; height:30px; flex:0 0 auto; place-items:center; border-radius:50%; background:var(--color-panel); }
  .assetAvatar img { width:27px; height:27px; object-fit:contain; }
  .assetText,.assetCopy { display:grid; gap:2px; min-width:0; }
  .assetText strong { color:var(--color-text); font-size:14px; }
  .assetText small,.assetCopy small { color:var(--color-text-faint); font-size:10px; }
  .assetCopy { text-align:right; }
  .assetCopy strong { color:var(--color-text-soft); font-size:11px; font-weight:550; }
  .chevron { color:var(--color-text-faint); font-size:14px; }
  .pickerDialog { width:min(440px,calc(100% - 32px)); max-height:min(540px,calc(100dvh - 32px)); margin:auto; padding:18px; border:1px solid var(--color-border); border-radius:16px; background:var(--color-paper); color:var(--color-text); box-shadow:0 24px 90px #0004; }
  .pickerDialog::backdrop { background:#0a100b99; backdrop-filter:blur(5px); }
  .pickerHead { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  .pickerHead h2 { margin:0; font-size:18px; }
  .close { display:grid; width:36px; height:36px; place-items:center; border-radius:8px; color:var(--color-text-soft); font-size:24px; }
  .close:hover { background:var(--color-panel); }
  .search { display:flex; min-height:44px; align-items:center; gap:9px; margin:16px 0 10px; padding:0 12px; border:1px solid var(--color-border); border-radius:9px; color:var(--color-text-faint); }
  .search input { width:100%; min-width:0; border:0; outline:0; background:transparent; color:var(--color-text); font:inherit; font-size:13px; }
  .options { display:grid; gap:6px; max-height:330px; overflow:auto; }
  .options > button { display:flex; min-height:58px; align-items:center; gap:10px; padding:8px; border:1px solid transparent; border-radius:10px; text-align:left; }
  .options > button:hover,.options > button.active { border-color:var(--color-border); background:var(--color-panel); }
  .options > button img { flex:0 0 auto; border-radius:50%; }
  .tokenName { display:grid; min-width:76px; gap:3px; }
  .tokenName strong { font-size:13px; }
  .tokenName small,.tokenNetwork { color:var(--color-text-faint); font-size:10px; }
  .tokenNetwork { margin-left:auto; }
  .check { color:var(--color-accent-strong); font-weight:800; }
  .empty { padding:18px; color:var(--color-text-soft); text-align:center; font-size:13px; }
  @media(max-width:700px) { .pickerDialog { border-radius:14px; padding:16px; } .tokenNetwork { font-size:9px; } }
</style>
