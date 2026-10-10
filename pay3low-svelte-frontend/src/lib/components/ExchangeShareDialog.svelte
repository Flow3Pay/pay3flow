<script lang="ts">
  import { tick } from 'svelte';
  import { locale, t } from '$lib/i18n';
  import { lockPageScroll } from '$lib/page-scroll-lock';
  import { exchangeShareImage, type ExchangeShareState } from '$lib/exchange-share';

  export let state: ExchangeShareState | null = null;
  export let imageUrl: string | null = null;
  export let shareTitle = "Share exchange";
  export let shareDescription = "Send this link to open the same exchange settings.";
  export let linkLabel = "Exchange link";
  export let previewAlt = "";
  export let url: string;
  export let onClose: () => void;
  let dialog: HTMLDivElement;
  let linkInput: HTMLInputElement;
  $: previewImage = imageUrl ?? (state ? exchangeShareImage(new URL(url).origin, state.source, state.target, new URL(url).searchParams) : "");
  let copied = false;
  let copyFailed = false;

  function portal(node: HTMLElement) {
    const previousFocus = document.activeElement as HTMLElement | null;
    document.body.appendChild(node);
    const shell = document.querySelector<HTMLElement>('.appShell');
    const wasInert = shell?.inert ?? false;
    if (shell) shell.inert = true;
    const unlock = lockPageScroll();
    void tick().then(() => dialog.querySelector<HTMLButtonElement>('button')?.focus());
    return { destroy() {
      if (shell) shell.inert = wasInert;
      unlock(); node.remove(); previousFocus?.focus();
    } };
  }
  async function copyLink() {
    try {
      await navigator.clipboard.writeText(url);
      copied = true; copyFailed = false;
    } catch {
      copyFailed = true;
      linkInput.focus(); linkInput.select();
    }
  }
  function keydown(event: KeyboardEvent) {
    if (event.key === 'Escape') { event.preventDefault(); onClose(); }
    if (event.key !== 'Tab') return;
    const items = [...dialog.querySelectorAll<HTMLElement>('button, input')];
    const first = items[0], last = items.at(-1);
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
  }
</script>

<svelte:window on:keydown={keydown} />
<div class="shareBackdrop" use:portal role="presentation" on:mousedown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
  <div class="shareDialog" bind:this={dialog} role="dialog" aria-modal="true" aria-labelledby="exchange-share-title" aria-describedby="exchange-share-description">
    <div class="shareHeading"><h2 id="exchange-share-title">{t(shareTitle, {}, $locale)}</h2><button type="button" class="closeButton" aria-label={t('Close share dialog', {}, $locale)} on:click={onClose}>×</button></div>
    <p id="exchange-share-description">{t(shareDescription, {}, $locale)}</p>
    <img class="sharePreview" src={previewImage} alt={previewAlt || `${state?.source} → ${state?.target} · Pay3Flow Bridge`} width="1200" height="630" />
    <label for="exchange-share-link">{t(linkLabel, {}, $locale)}</label>
    <div class="shareLinkRow"><input id="exchange-share-link" bind:this={linkInput} readonly value={url} on:focus={(event) => event.currentTarget.select()} /><button type="button" class="copyButton" on:click={copyLink}>{t(copied ? 'Link copied' : 'Copy link', {}, $locale)}</button></div>
    <p class="copyStatus" aria-live="polite">{copyFailed ? t('Select and copy the link manually.', {}, $locale) : copied ? t('Link copied', {}, $locale) : ''}</p>
  </div>
</div>

<style>
  .shareBackdrop { position: fixed; inset: 0; z-index: 2000; display: grid; place-items: center; padding: 20px; background: rgba(8, 11, 8, .6); }
  .shareDialog { width: min(500px, 100%); max-height: calc(100dvh - 40px); overflow-y: auto; padding: 24px; border: 1px solid var(--color-border); border-radius: 20px; color: var(--color-text); background: var(--color-paper); box-shadow: 0 24px 80px rgba(0, 0, 0, .18); }
  .shareHeading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 0; font-size: 23px; }
  .closeButton { display: grid; flex: 0 0 40px; height: 40px; place-items: center; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-panel); font-size: 26px; }
  p { margin: 12px 0 20px; color: var(--color-text-soft); font-size: 14px; line-height: 1.5; }
  .sharePreview { display: block; width: 100%; height: auto; margin-bottom: 22px; border: 1px solid var(--color-border); border-radius: 14px; background: #101110; }
  label { display: block; margin-bottom: 8px; font-size: 13px; font-weight: 700; }
  .shareLinkRow { display: flex; gap: 8px; }
  input { width: 100%; min-width: 0; padding: 12px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-panel); color: var(--color-text); font-size: 13px; }
  .copyButton { flex-shrink: 0; padding: 10px 14px; border: 1px solid var(--color-border); border-radius: 9px; background: var(--color-accent-soft); font-weight: 700; }
  button:hover { background: var(--color-accent-soft); }
  .copyStatus { min-height: 21px; margin: 10px 0 0; font-size: 13px; }
  @media (max-width: 480px) { .shareDialog { padding: 16px; } .shareLinkRow { flex-direction: column; } }
</style>
