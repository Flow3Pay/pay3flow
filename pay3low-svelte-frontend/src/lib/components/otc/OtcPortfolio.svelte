<script lang="ts">
  import { apiUrl } from "$lib/api";
  import { locale } from "$lib/i18n";
  import { trimDecimal } from "$lib/otc/market";
  import type { WalletBalance } from "$lib/otc/service";

  export let balances: WalletBalance[] = [];
  export let connected = false;
  export let onWallets: () => void = () => {};

  interface Row { symbol: string; network: string; icon: string; amount: string | null }
  interface Value { price_usd: number | null; value_usd: number | null }

  let values = new Map<string, Value>();
  let failed = false;

  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: localeTag = $locale === "ru" ? "ru-RU" : "en-US";
  $: rows = balances.map<Row>(entry => ({
    symbol: entry.symbol,
    network: entry.network,
    icon: entry.symbol === "EVER" ? "/icons/assets/ever.svg" : "/icons/assets/usdt.png",
    amount: entry.state === "ok" ? entry.amount : null,
  }));
  $: priced = rows.filter(row => row.amount && Number(row.amount) > 0).map(row => ({ asset: row.symbol, amount: row.amount as string }));
  $: signature = priced.map(item => `${item.asset}:${item.amount}`).join("|");

  $: usd = (row: Row) => {
    if (!row.amount) return "—";
    if (row.symbol === "USDT") return money(Number(row.amount));
    const value = values.get(row.symbol)?.value_usd;
    return value === undefined || value === null ? "—" : money(value);
  };
  $: quantity = (row: Row) => row.amount ? `${trimDecimal(row.amount, 4)} ${row.symbol}` : "—";
  $: everRow = rows.find(row => row.symbol === "EVER") || null;

  $: if (signature) void load(); else values = new Map();

  async function load() {
    try {
      const response = await fetch(apiUrl("/api/market-values"), {
        method: "POST", headers: { "Content-Type": "application/json" }, cache: "no-store",
        body: JSON.stringify({ items: priced.slice(0, 4) }),
      });
      if (!response.ok) throw new Error("values unavailable");
      const data = await response.json() as { values?: (Value & { asset: string })[] };
      values = new Map((data.values ?? []).map(item => [item.asset, { price_usd: item.price_usd, value_usd: item.value_usd }]));
      failed = false;
    } catch { values = new Map(); failed = true; }
  }

  function money(value: number): string {
    return `$${value.toLocaleString(localeTag, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
  }
</script>
<div class="portfolio">
  <div class="cardHead">
    <h2>{say("My Portfolio", "Мой портфель")}</h2>
    <button type="button" class="walletIcon" aria-label={say("Wallet connections", "Подключённые кошельки")} on:click={onWallets}><img src="/icons/ui/wallet-connect.png" width="20" height="20" alt="" /></button>
  </div>
  <div class="assetRows">
    {#each rows as row (row.symbol)}
      <div class="assetRow" class:active={row.symbol === "EVER"}>
        <img class="assetIcon" src={row.icon} width="38" height="38" alt="" />
        <div class="assetNames"><strong>{row.symbol}</strong><span>{row.network}</span></div>
        <div class="assetAmounts"><span>{usd(row)}</span><b>{quantity(row)}</b></div>
      </div>
    {:else}
      <div class="assetRow empty"><div class="assetNames"><strong>{say("No wallets yet", "Кошельков пока нет")}</strong><span>{connected ? say("Verification lasts a week.", "Подтверждение действует неделю.") : say("Connect a wallet to load balances.", "Подключите кошелёк, чтобы увидеть баланс.")}</span></div></div>
    {/each}
  </div>
  <div class="available">
    <span>{say("Available EVER", "Доступно EVER")}</span>
    <strong>{everRow?.amount ? quantity(everRow) : "—"}{#if !connected}<small>{say("connect to load", "подключите кошелёк")}</small>{/if}</strong>
  </div>
  {#if failed && priced.length}<p class="valueNote">{say("USD values are unavailable right now.", "Долларовая оценка сейчас недоступна.")}</p>{/if}
</div>

<style>
  .portfolio { display:flex; flex-direction:column; gap:16px; padding:20px; border:1px solid var(--color-border); border-radius:var(--otc-card-radius,10px); background:var(--color-paper); }
  .cardHead { display:flex; align-items:center; justify-content:space-between; gap:12px; }
  h2 { margin:0; font-size:19px; font-weight:700; letter-spacing:-.02em; }
  .walletIcon { display:grid; width:44px; height:44px; flex:0 0 auto; place-items:center; border-radius:50%; background:var(--color-accent-soft, var(--color-accent)); }
  .walletIcon:hover { filter:brightness(1.04); }
  .assetRows { display:grid; gap:12px; }
  .assetRow { display:grid; grid-template-columns:38px minmax(0,1fr) auto; align-items:center; gap:12px; min-width:0; padding:14px; border:1px solid transparent; border-radius:16px; background:var(--color-panel); }
  .assetRow.active { border-color:var(--color-accent-strong); background:var(--color-accent-soft, color-mix(in srgb, var(--color-accent) 22%, transparent)); }
  .assetRow.empty { grid-template-columns:minmax(0,1fr); }
  .assetIcon { flex:0 0 38px; border-radius:50%; }
  .assetNames { display:grid; min-width:0; gap:2px; }
  .assetNames strong { font-size:16px; font-weight:700; letter-spacing:-.01em; }
  .assetNames span { overflow:hidden; color:var(--color-text-soft); font-size:12px; text-overflow:ellipsis; white-space:nowrap; }
  .assetAmounts { display:grid; min-width:0; gap:2px; text-align:right; }
  .assetAmounts span { color:var(--color-text-soft); font-family:var(--font-mono); font-size:12px; font-variant-numeric:tabular-nums; }
  .assetAmounts b { overflow:hidden; font-family:var(--font-mono); font-size:14px; font-weight:600; font-variant-numeric:tabular-nums; text-overflow:ellipsis; white-space:nowrap; }
  .available { display:flex; align-items:baseline; justify-content:space-between; gap:12px; padding-top:16px; border-top:1px solid var(--color-border); }
  .available > span { color:var(--color-text-soft); font-size:13px; }
  .available strong { display:flex; align-items:baseline; gap:8px; font-family:var(--font-mono); font-size:20px; font-weight:600; letter-spacing:-.03em; font-variant-numeric:tabular-nums; }
  .available small { color:var(--color-text-soft); font-family:var(--font-sans); font-size:11px; font-weight:400; letter-spacing:0; }
  .valueNote { margin:0; color:var(--color-text-soft); font-size:11px; }
  button:focus-visible { outline:var(--focus-ring-width) solid var(--color-focus); outline-offset:2px; }
  @media (max-width:640px) { .portfolio { padding:16px; } .assetRow { grid-template-columns:32px minmax(0,1fr); grid-template-rows:auto auto; row-gap:8px; } .assetIcon { flex-basis:32px; width:32px; height:32px; } .assetAmounts { grid-column:1/-1; justify-items:end; text-align:right; } }
</style>
