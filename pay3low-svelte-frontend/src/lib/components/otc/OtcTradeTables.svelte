<script lang="ts">
  import { locale } from "$lib/i18n";
  import { tradeRow } from "$lib/otc/market";
  import { terminalStates } from "$lib/otc/terminal";
  import type { Direction, Trade } from "$lib/otc/service";
  export let trades: Trade[] = [];
  export let selectedId = "";
  export let busy = false;
  export let onSelect: (id: string) => void;
  export let emptyTitle = "";
  export let emptyHint = "";
  type Row = { trade: Trade; row: ReturnType<typeof tradeRow> };
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: rows = trades.map(trade => ({ trade, row: tradeRow(trade) })) as Row[];
  $: buys = rows.filter(item => item.trade.terms.direction === "buy");
  $: sells = rows.filter(item => item.trade.terms.direction === "sell");
  const labels: Record<string, string> = {
    quoted: "Open", booking_pending: "Booking pending", accepted: "Accepted", funded: "Funded",
    payment_confirming: "Confirming", payout_pending: "Payout pending", review: "Review",
    completed: "Filled", refunded: "Refunded", rejected: "Rejected", expired: "Expired", cancelled: "Cancelled",
  };
  function statusLabel(state: string) { return labels[state] ?? state.replaceAll("_", " "); }
  function statusClass(state: string) {
    if (state === "completed") return "filled";
    if (["refunded", "rejected", "expired", "cancelled"].includes(state)) return "closed";
    if (state === "review") return "review";
    return terminalStates.has(state) ? "closed" : "open";
  }
  function datePart(time: number | null) { return time === null ? "—" : new Date(time).toLocaleDateString($locale === "ru" ? "ru-RU" : "en-GB", { day: "2-digit", month: "short", year: "numeric", timeZone: "UTC" }); }
  function timePart(time: number | null) { return time === null ? "" : new Date(time).toLocaleTimeString($locale === "ru" ? "ru-RU" : "en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }); }
  function activate(event: KeyboardEvent, id: string) {
    if (!["Enter", " "].includes(event.key)) return;
    event.preventDefault();
    onSelect(id);
  }
</script>

{#snippet sideTable(title: string, direction: Direction, items: Row[])}
  <section class="sideTable" class:sell={direction === "sell"} aria-label={title}>
    <header><span class="sideDot"></span><h3>{title}</h3><span class="sideCount">{items.length}</span></header>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="tableScroll" tabindex="0" role="region" aria-label={`${title} table`}>
      <table>
        <thead><tr><th>{say("Date / Time", "Дата / время")}</th><th>{say("Pair", "Пара")}</th><th>{say("Type", "Тип")}</th><th>{say("Price", "Цена")} <small>USDT</small></th><th>{say("Amount", "Объём")} <small>EVER</small></th><th>{say("Total", "Всего")} <small>USDT</small></th><th>{say("Status", "Статус")}</th></tr></thead>
        <tbody>
          {#each items as { trade, row } (trade.id)}
            <tr class:selected={trade.id === selectedId} tabindex="0" role="button" aria-label={`${direction === "buy" ? say("Open buy", "Открыть покупку") : say("Open sell", "Открыть продажу")} EVER ${trade.id}`} on:click={() => !busy && onSelect(trade.id)} on:keydown={(event) => activate(event, trade.id)}>
              <td class="dateCell"><span>{datePart(row.time)}</span><small>{timePart(row.time)}</small></td>
              <td class="pairCell">EVER / USDT</td>
              <td><span class="sidePill" class:sell={direction === "sell"}>{direction === "buy" ? say("Buy", "Купить") : say("Sell", "Продать")}</span></td>
              <td class="numberCell">{row.price}</td>
              <td class="numberCell">{row.amount}</td>
              <td class="numberCell">{row.total}</td>
              <td><span class="status" class:filled={statusClass(trade.state) === "filled"} class:closed={statusClass(trade.state) === "closed"} class:review={statusClass(trade.state) === "review"}><i></i>{statusLabel(trade.state)}</span></td>
            </tr>
          {:else}
            <tr class="emptyRow"><td colspan="7"><strong>{emptyTitle || (direction === "buy" ? say("No buy orders yet", "Покупок пока нет") : say("No sell orders yet", "Продаж пока нет"))}</strong>{#if emptyHint}<small>{emptyHint}</small>{/if}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
{/snippet}

<div class="tradeTables">
  {@render sideTable(say("Buy orders", "Ордера на покупку"), "buy", buys)}
  {@render sideTable(say("Sell orders", "Ордера на продажу"), "sell", sells)}
</div>

<style>
  .tradeTables { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 18px; padding: 16px 18px 20px; }
  .sideTable { min-width: 0; }
  .sideTable header { display: flex; align-items: center; gap: 7px; padding: 0 2px 9px; }
  .sideTable h3 { font-size: 12px; font-weight: 700; }
  .sideDot { width: 7px; height: 7px; border-radius: 50%; background: var(--otc-buy); box-shadow: 0 0 0 3px var(--otc-buy-soft); }
  .sideTable.sell .sideDot { background: var(--otc-sell); box-shadow: 0 0 0 3px var(--otc-sell-soft); }
  .sideCount { display: grid; min-width: 17px; height: 17px; place-items: center; margin-left: 2px; padding: 0 4px; border-radius: 4px; background: var(--color-panel); color: var(--color-text-soft); font-family: var(--font-mono); font-size: 10px; }
  .tableScroll { overflow-x: auto; border: 1px solid var(--color-border); border-radius: 7px; }
  .tableScroll:focus-visible { outline: 2px solid var(--color-accent-strong); outline-offset: 2px; }
  table { width: 100%; min-width: 540px; border-collapse: collapse; font-family: var(--font-sans); font-variant-numeric: tabular-nums; font-size: 11px; white-space: nowrap; }
  th { padding: 9px 10px; border-bottom: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 10px; font-weight: 500; text-align: right; }
  th:first-child, th:nth-child(2) { text-align: left; }
  th small { display: block; margin-top: 2px; font-family: var(--font-mono); font-size: 9px; opacity: .7; }
  td { padding: 8px 10px; text-align: right; }
  td:first-child, td:nth-child(2) { text-align: left; }
  tbody tr { cursor: pointer; transition: background .15s ease; }
  tbody tr:hover, tbody tr:focus-visible { background: var(--color-panel); outline: none; }
  tbody tr.selected { background: var(--color-accent-soft); }
  tbody tr + tr td { border-top: 1px solid var(--color-border); }
  .dateCell span, .dateCell small { display: block; }
  .dateCell small { margin-top: 2px; color: var(--color-text-soft); font-family: var(--font-mono); font-size: 9px; }
  .pairCell { color: var(--color-text-soft); font-size: 10px; }
  .numberCell { font-family: var(--font-mono); font-size: 10px; }
  .sidePill { display: inline-block; min-width: 37px; padding: 3px 5px; border-radius: 3px; background: var(--otc-buy-soft); color: var(--otc-buy); font-size: 10px; font-weight: 700; text-align: center; }
  .sidePill.sell { background: var(--otc-sell-soft); color: var(--otc-sell); }
  .status { display: inline-flex; align-items: center; gap: 5px; color: var(--color-text-soft); font-size: 10px; }
  .status i { width: 5px; height: 5px; border-radius: 50%; background: var(--color-text-faint); }
  .status.open { color: var(--otc-buy); }
  .status.open i { background: var(--otc-buy); }
  .status.filled { color: var(--otc-buy); }
  .status.filled i { background: var(--otc-buy); }
  .status.closed { color: var(--color-text-soft); }
  .status.review { color: var(--color-warn); }
  .status.review i { background: var(--color-warn); }
  .emptyRow { cursor: default; }
  .emptyRow td { padding: 28px 12px; text-align: center; white-space: normal; }
  .emptyRow strong { display: block; color: var(--color-text-soft); font-size: 12px; font-weight: 600; }
  .emptyRow small { display: block; max-width: 340px; margin: 5px auto 0; color: var(--color-text-faint); font-size: 11px; line-height: 1.5; }
  @media (max-width: 900px) { .tradeTables { grid-template-columns: 1fr; } }
  @media (max-width: 640px) { .tradeTables { padding: 12px 12px 16px; gap: 14px; } table { min-width: 570px; } }
  @media (prefers-reduced-motion: reduce) { tbody tr { transition: none; } }
</style>
