<script lang="ts">
  import { locale } from "$lib/i18n";
  import { tradeRow } from "$lib/otc/market";
  import { terminalStates } from "$lib/otc/terminal";
  import { offerDirection, offerOwner, saleRate, type PublicOffer } from "$lib/otc/public-offers";
  import type { Direction, Trade } from "$lib/otc/service";
  export let trades: Trade[] = [];
  export let offers: PublicOffer[] = [];
  export let selectedId = "";
  export let busy = false;
  export let onSelect: (id: string) => void;
  export let onOfferSelect: (offer: PublicOffer) => void = () => {};
  export let emptyTitle = "";
  export let emptyHint = "";
  type Item = {
    key: string;
    kind: "trade" | "offer";
    direction: Direction;
    time: number | null;
    price: string;
    amount: string;
    total: string;
    state: string;
    id: string;
    trade?: Trade;
    offer?: PublicOffer;
    owner?: string;
  };
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: numberLocale = $locale === "ru" ? "ru-RU" : "en-US";
  $: tradeItems = trades.map(trade => {
    const row = tradeRow(trade);
    return { key: `trade:${trade.id}`, kind: "trade", direction: trade.terms.direction, time: row.time, price: row.price, amount: row.amount, total: row.total, state: trade.state, id: trade.id, trade } as Item;
  });
  $: offerItems = offers.map(offer => ({
    key: `offer:${offer.id}`,
    kind: "offer",
    direction: offerDirection(offer),
    time: offer.created_at ? Date.parse(offer.created_at) : null,
    price: saleRate(offer.terms.ever_amount, offer.terms.usdt_amount),
    amount: offer.terms.ever_amount,
    total: offer.terms.usdt_amount,
    state: offer.state,
    id: offer.id,
    offer,
    owner: offerOwner(offer),
  }) as Item);
  $: items = [...tradeItems, ...offerItems].sort((a, b) => (b.time ?? 0) - (a.time ?? 0));
  $: buys = items.filter(item => item.direction === "buy");
  $: sells = items.filter(item => item.direction === "sell");
  const labels: Record<string, string> = {
    open: "Open", quoted: "Open", booking_pending: "Booking pending", accepted: "Accepted", funded: "Funded",
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
  function formatPrice(value: string, loc: string): string {
    const number = Number(value);
    if (!Number.isFinite(number)) return "—";
    return number.toLocaleString(loc, { maximumSignificantDigits: 8 });
  }
  function formatEver(value: string, loc: string): string {
    const [whole, fraction = ""] = value.split(".");
    const grouped = BigInt(whole || "0").toLocaleString(loc);
    const tail = fraction.replace(/0+$/, "");
    return tail ? `${grouped}.${tail}` : grouped;
  }
  function datePart(time: number | null) { return time === null ? "—" : new Date(time).toLocaleDateString($locale === "ru" ? "ru-RU" : "en-GB", { day: "2-digit", month: "short", year: "numeric", timeZone: "UTC" }); }
  function timePart(time: number | null) { return time === null ? "" : new Date(time).toLocaleTimeString($locale === "ru" ? "ru-RU" : "en-GB", { hour: "2-digit", minute: "2-digit", timeZone: "UTC" }); }
  function open(item: Item) {
    if (item.kind === "offer" && item.offer) onOfferSelect(item.offer);
    else onSelect(item.id);
  }
  function activate(event: KeyboardEvent, item: Item) {
    if (!["Enter", " "].includes(event.key)) return;
    event.preventDefault();
    open(item);
  }
</script>

{#snippet sideTable(title: string, direction: Direction, side: "bids" | "asks", items: Item[])}
  <section class="obTable" data-side={side} aria-label={title}>
    <div class="tableScroll">
      <table>
        <thead><tr>
          <th>{title}</th>
          <th>{say("Price", "Цена")}<small>USDT</small></th>
          <th>{say("Amount", "Объём")}<small>EVER</small></th>
          <th>{say("Total", "Всего")}<small>USDT</small></th>
        </tr></thead>
        <tbody>
          {#each items as item (item.key)}
            <tr class:selected={item.kind === "trade" && item.id === selectedId} tabindex="0" role="button" data-kind={item.kind} aria-label={`${direction === "buy" ? say("Open buy", "Открыть покупку") : say("Open sell", "Открыть продажу")} ${item.kind === "offer" ? say("offer", "оффер") : "EVER"} ${item.id}`} title={item.kind === "offer" && item.owner ? item.owner : `${datePart(item.time)} ${timePart(item.time)}`} on:click={() => !busy && open(item)} on:keydown={(event) => activate(event, item)}>
              <td class="obWhen"><span class="status {statusClass(item.state)}"><i></i>{statusLabel(item.state)}</span><small>{datePart(item.time)} · {timePart(item.time)}</small></td>
              <td class="obPrice"><i aria-hidden="true">$</i><b>{formatPrice(item.price, numberLocale)}</b></td>
              <td class="obAmount">{formatEver(item.amount, numberLocale)}</td>
              <td class="obTotal"><i aria-hidden="true">$</i>{formatEver(item.total, numberLocale)}</td>
            </tr>
          {:else}
            <tr class="emptyRow"><td colspan="4"><strong>{emptyTitle || (direction === "buy" ? say("No buy orders yet", "Покупок пока нет") : say("No sell orders yet", "Продаж пока нет"))}</strong>{#if emptyHint}<small>{emptyHint}</small>{/if}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
  </section>
{/snippet}

<div class="tradeTables">
  <!-- Sellers come first so the list reads ask-then-bid; buyers take the left side, as in the book. -->
  {@render sideTable(say("Sell orders", "Ордера на продажу"), "sell", "asks", sells)}
  {@render sideTable(say("Buy orders", "Ордера на покупку"), "buy", "bids", buys)}
</div>

<style>
  .tradeTables { display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); gap:20px; padding:0 var(--otc-panel-padding,20px) 16px; }
  .obTable { min-width:0; }
  .obTable[data-side="bids"] { order:-1; }
  .tableScroll { overflow-x:auto; }
  table { width:100%; min-width:290px; border-collapse:separate; border-spacing:0 4px; font-family:var(--font-mono); font-variant-numeric:tabular-nums; font-size:12px; }
  th { padding:0 12px 6px; color:var(--color-text-soft); font-size:11px; font-weight:500; line-height:1.35; text-align:right; white-space:nowrap; }
  th:first-child { width:34%; text-align:left; }
  th small { display:block; color:var(--color-text-faint); font-size:9px; }
  tbody td { padding:8px 12px; border-top:1px solid var(--color-border); border-bottom:1px solid var(--color-border); background:var(--otc-row-sell-bg); color:var(--color-text); text-align:right; white-space:nowrap; }
  .obTable[data-side="bids"] tbody td { background:var(--otc-row-buy-bg); }
  tbody td:first-child { border-left:1px solid var(--color-border); border-radius:10px 0 0 10px; text-align:left; }
  tbody td:last-child { border-right:1px solid var(--color-border); border-radius:0 10px 10px 0; }
  tbody tr { cursor:pointer; }
  tbody tr:hover td, tbody tr:focus-visible td { background:var(--color-panel); }
  tbody tr:focus-visible { outline:none; }
  tbody tr.selected td { background:var(--color-accent-soft); }
  .obWhen { display:grid; gap:2px; justify-content:start; justify-items:start; }
  .obWhen small { color:var(--color-text-faint); font-size:9px; }
  .obPrice { overflow:hidden; text-overflow:ellipsis; color:var(--color-good-ink); }
  .obTable[data-side="bids"] .obPrice { color:var(--color-danger); }
  .obPrice i, .obTotal i { font-style:normal; color:var(--color-text); opacity:.62; }
  .obPrice b { font-weight:650; }
  .obAmount,.obTotal { overflow:hidden; text-overflow:ellipsis; }
  .status { display:inline-flex; align-items:center; gap:5px; color:var(--color-text-soft); font-size:10px; }
  .status i { width:5px; height:5px; border-radius:50%; background:var(--color-text-faint); }
  .status.open, .status.filled { color:var(--color-good-ink); }
  .status.open i, .status.filled i { background:var(--otc-buy); }
  .status.closed { color:var(--color-text-soft); }
  .status.review { color:var(--color-warn); }
  .status.review i { background:var(--color-warn); }
  .emptyRow { cursor:default; }
  .emptyRow td { border-style:dashed; border-radius:10px; background:transparent; color:var(--color-text-faint); text-align:left; white-space:normal; }
  .emptyRow strong { display:block; color:var(--color-text-soft); font-size:12px; font-weight:600; }
  .emptyRow small { display:block; margin-top:5px; font-family:var(--font-sans); font-size:11px; line-height:1.5; }
  @media (max-width:900px) { .tradeTables { grid-template-columns:minmax(0,1fr); } }
  @media (max-width:520px) { .tradeTables { padding-right:12px; padding-left:12px; } table { font-size:10.5px; } th,tbody td { padding-right:8px; padding-left:8px; } }
  @media (prefers-reduced-motion:reduce) { tbody td { transition:none; } }
</style>
