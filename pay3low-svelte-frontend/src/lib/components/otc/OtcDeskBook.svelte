<script lang="ts">
  import { locale } from "$lib/i18n";
  import { proposalDirection } from "$lib/otc/terminal";
  import type { Config, Direction, Proposal } from "$lib/otc/service";
  export let config: Config | null;
  export let listings: Proposal[] = [];
  export let selectedId = "";
  export let onSelect: (proposal: Proposal, direction: Direction) => void;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: offers = listings.map(proposal => ({ proposal, direction: proposalDirection(proposal, config) })).filter(item => item.direction);
</script>
<div class="bookHeader"><span>{say("Desk offers", "Предложения деска")}</span><span>EVER / USDT</span></div>
<div class="bookRows">
  {#each offers as { proposal, direction }}
    <div class="bookRow" class:sell={direction === "sell"} class:selected={proposal.id === selectedId}>
      <button type="button" on:click={() => onSelect(proposal, direction!)}><strong>{direction === "buy" ? say("Buy EVER", "Купить EVER") : say("Sell EVER", "Продать EVER")}</strong><span>{say("Quote on request", "Котировка по запросу")}</span></button>
      <p>{direction === "buy" ? "USDT · Ethereum → EVER · Everscale" : "EVER · Everscale → USDT · Ethereum"}</p>
      <a href={proposal.id} target="_blank" rel="noreferrer">{proposal.name || say("Desk Proposal", "Предложение деска")}</a>
    </div>
  {:else}<div class="empty"><span aria-hidden="true">≡</span><strong>{say("No desk offers", "Нет предложений деска")}</strong><p>{say("Published desk Proposals appear here when the route is configured.", "Опубликованные предложения появятся после настройки деска.")}</p></div>{/each}
</div>
<p class="bookFooter">{say("Exact private quote · customer pays first", "Приватная котировка · клиент платит первым")}</p>
<style>
.bookHeader { display: flex; justify-content: space-between; gap: 12px; padding: 12px 17px; border-bottom: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 11px; }
.bookRows { min-height: 330px; } .bookRow { padding: 14px 17px; border-bottom: 1px solid var(--color-border); } .bookRow.selected { background: var(--otc-buy-soft); } .bookRow.sell.selected { background: var(--otc-sell-soft); }
.bookRow button { display: flex; width: 100%; min-height: 44px; justify-content: space-between; align-items: center; gap: 10px; text-align: left; font-size: 12px; } .bookRow strong { color: var(--otc-buy); } .bookRow.sell strong { color: var(--otc-sell); } .bookRow span, .bookRow p { color: var(--color-text-soft); font-size: 11px; } .bookRow p { line-height: 1.6; margin-bottom: 7px; } .bookRow a { display: block; overflow-wrap: anywhere; font-size: 11px; text-decoration: underline; }
.empty { display: grid; gap: 10px; padding: 80px 20px; text-align: center; justify-items: center; font-size: 13px; } .empty > span { font-size: 28px; color: var(--color-text-faint); } .empty p { max-width: 240px; color: var(--color-text-soft); line-height: 1.6; font-size: 12px; } .bookFooter { padding: 14px 17px; border-top: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 11px; line-height: 1.5; }
</style>
