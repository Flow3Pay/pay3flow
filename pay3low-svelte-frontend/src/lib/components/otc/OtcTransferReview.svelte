<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import { type Trade, type Attempt, decimal } from "$lib/otc/service";
  export let trade: Trade;
  export let kind: string;
  export let instructions: Attempt["instructions"];
  export let onClose: () => void;
  export let onConfirm: () => void;
  export let disabled = false;
  let dialog: HTMLDialogElement;
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: leg = instructions.leg;
  $: asset = leg.chain === "ethereum" ? "USDT" : "EVER";
  onMount(() => { const previous = document.activeElement as HTMLElement | null; const unlock = lockPageScroll(); dialog.showModal(); return () => { dialog.close(); unlock(); previous?.focus(); }; });
</script>
<dialog bind:this={dialog} aria-labelledby="otc-transfer-title" on:cancel|preventDefault={onClose}>
  <div class="top"><h2 id="otc-transfer-title">{say("Review transfer", "Проверить перевод")}</h2><button aria-label={say("Close", "Закрыть")} on:click={onClose}>×</button></div>
  <p>{kind} · {leg.chain === "ethereum" ? "Ethereum" : "Everscale"}</p>
  <dl><dt>{say("Net amount", "Сумма к получению")}</dt><dd>{decimal(leg.amount, leg.chain === "ethereum" ? 6 : 9)} {asset}</dd><dt>{say("Sender", "Отправитель")}</dt><dd>{leg.sender}</dd><dt>{say("Recipient", "Получатель")}</dt><dd>{leg.recipient}</dd><dt>{say("Network costs", "Сетевые расходы")}</dt><dd>{trade.terms.network_costs}</dd></dl>
  <p>{say("The saved transfer is sent once. An unknown result requires reconciliation.", "Сохранённый перевод отправляется один раз. Неизвестный результат требует сверки.")}</p>
  <button class="primary" {disabled} on:click={onConfirm}>{say("Confirm in wallet", "Подтвердить в кошельке")} ↗</button><button on:click={onClose}>{say("Close and track saved transfer", "Закрыть и отслеживать сохранённый перевод")}</button>
</dialog>
<style>
dialog { width: min(460px, calc(100% - 32px)); max-height: calc(100dvh - 32px); margin: auto; padding: 24px; overflow: auto; border: 1px solid var(--color-border); border-radius: 18px; background: var(--color-paper); color: var(--color-text); } dialog::backdrop { background: #0a100b99; backdrop-filter: blur(5px); } .top { display: flex; justify-content: space-between; align-items: center; gap: 16px; } h2 { font-size: 23px; letter-spacing: -.04em; } .top button { width: 44px; min-width: 44px; font-size: 24px; } p { margin: 14px 0; color: var(--color-text-soft); font-size: 13px; line-height: 1.6; } dl { display: grid; gap: 8px; padding: 16px 0; font-size: 13px; } dt { color: var(--color-text-soft); } dd { margin: 0 0 8px; overflow-wrap: anywhere; } button { display: block; width: 100%; min-height: 44px; padding: 12px; border: 1px solid var(--color-border); border-radius: 8px; margin-top: 8px; font-size: 13px; } button.primary { background: var(--color-accent); color: #152016; } button:disabled { opacity: .4; }
</style>
