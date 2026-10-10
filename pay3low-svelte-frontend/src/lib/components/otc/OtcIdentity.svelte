<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { lockPageScroll } from "$lib/page-scroll-lock";
  import type { Session } from "$lib/otc/service";
  export let session: Session | null;
  export let enabled = false, busy = false;
  export let bound: string[] = [];
  export let error = "";
  export let onClose: () => void;
  export let onLink: (actor: string, credential: string) => Promise<void>;
  export let onVerify: (chain: "ethereum" | "everscale") => void;
  export let onDisconnect: () => void;
  let dialog: HTMLDialogElement, actor = "", credential = "";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  onMount(() => { const previous = document.activeElement as HTMLElement | null; const unlock = lockPageScroll(); dialog.showModal(); return () => { dialog.close(); unlock(); previous?.focus(); }; });
</script>
<dialog bind:this={dialog} aria-labelledby="otc-account-title" on:cancel|preventDefault={onClose}>
  <div class="top"><h2 id="otc-account-title">{say("Actor and wallets", "Аккаунт и кошельки")}</h2><button type="button" aria-label={say("Close", "Закрыть")} on:click={onClose}>×</button></div>
  {#if error}<p role="alert">{error}</p>{/if}
  {#if session}<p class="address">{session.actor}</p><p>{session.desk ? say("Desk operator", "Оператор деска") : say("Customer", "Клиент")}</p><button disabled={busy} on:click={() => onVerify("ethereum")}>{say("Verify Ethereum wallet", "Подтвердить кошелёк Ethereum")}</button><button disabled={busy} on:click={() => onVerify("everscale")}>{say("Verify EVER Wallet", "Подтвердить EVER Wallet")}</button>{#each bound as wallet}<p class="address">{wallet}</p>{/each}<button disabled={busy} on:click={onDisconnect}>{say("Revoke session and relay access", "Завершить сессию и отозвать доступ")}</button>
  {:else}<form on:submit|preventDefault={async () => { const secret = credential; credential = ""; await onLink(actor, secret); }}><label for="otc-actor">{say("Existing LF actor", "Существующий LF actor")}</label><input id="otc-actor" type="url" required bind:value={actor} placeholder="https://relay.example/actors/customer" /><label for="otc-credential">{say("LF credential", "Учётные данные LF")}</label><input id="otc-credential" type="password" required bind:value={credential} autocomplete="off" /><button class="primary" disabled={busy || !enabled}>{say("Link actor", "Подключить аккаунт")}</button></form>{/if}
</dialog>
<style>
dialog { width: min(430px, calc(100% - 32px)); max-height: calc(100dvh - 32px); padding: 24px; margin: auto; overflow: auto; border: 1px solid var(--color-border); border-radius: 18px; background: var(--color-paper); color: var(--color-text); } dialog::backdrop { background: #0a100b99; backdrop-filter: blur(5px); } .top { display: flex; gap: 16px; align-items: center; justify-content: space-between; margin-bottom: 20px; } h2 { font-size: 22px; letter-spacing: -.04em; } .top button { width: 44px; min-width: 44px; margin: 0; font-size: 24px; } form { display: grid; gap: 10px; } label { font-size: 13px; } input { min-width: 0; width: 100%; padding: 12px; border: 1px solid var(--color-border); border-radius: 8px; background: var(--exchange-field-bg); font: inherit; } button { display: block; width: 100%; min-height: 44px; padding: 12px; margin: 10px 0; border: 1px solid var(--color-border); border-radius: 8px; font-size: 13px; } button:disabled { opacity: .4; } button.primary { background: var(--color-accent); color: #152016; } p { margin: 12px 0; font-size: 13px; } .address { overflow-wrap: anywhere; color: var(--color-text-soft); font-size: 12px; }
</style>
