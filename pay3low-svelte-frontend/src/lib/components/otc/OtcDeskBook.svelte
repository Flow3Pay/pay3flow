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
<div class="bookHeader"><span>{say("Direction", "Направление")}</span><span>EVER / USDT</span></div>
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
  .bookHeader { display: flex; flex: 0 0 auto; min-height: 48px; align-items: center; justify-content: space-between; gap: 12px; padding: 12px var(--otc-panel-padding, 20px); border-bottom: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 12px; line-height: 1.5; }
  .bookHeader span:last-child { white-space: nowrap; }
  .bookRows { display: flex; flex: 1; flex-direction: column; min-height: 240px; }
  .bookRow { min-width: 0; padding: 16px var(--otc-panel-padding, 20px); border-bottom: 1px solid var(--color-border); }
  .bookRow.selected { background: var(--otc-buy-soft); }
  .bookRow.sell.selected { background: var(--otc-sell-soft); }
  .bookRow button { display: flex; width: 100%; min-width: 0; min-height: 44px; align-items: center; justify-content: space-between; gap: 12px; font-size: 13px; line-height: 1.5; text-align: left; }
  .bookRow strong { color: var(--otc-buy); font-weight: 650; }
  .bookRow.sell strong { color: var(--otc-sell); }
  .bookRow span { min-width: 0; color: var(--color-text-soft); font-size: 12px; text-align: right; }
  .bookRow p { margin: 4px 0 8px; color: var(--color-text-soft); font-size: 12px; line-height: 1.6; overflow-wrap: anywhere; }
  .bookRow a { display: block; overflow-wrap: anywhere; font-size: 12px; line-height: 1.6; text-decoration: underline; text-underline-offset: 3px; }
  .empty { display: grid; flex: 1; min-height: 240px; align-content: center; justify-items: center; gap: 8px; padding: 32px var(--otc-panel-padding, 20px); text-align: center; }
  .empty > span { display: grid; width: 44px; height: 44px; place-items: center; margin-bottom: 4px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); color: var(--color-text-soft); font-size: 20px; }
  .empty strong { font-size: 14px; font-weight: 650; }
  .empty p { max-width: 280px; color: var(--color-text-soft); font-size: 13px; line-height: 1.6; }
  .bookFooter { display: flex; flex: 0 0 auto; min-height: 48px; align-items: center; padding: 12px var(--otc-panel-padding, 20px); border-top: 1px solid var(--color-border); color: var(--color-text-soft); font-size: 12px; line-height: 1.6; }
</style>
