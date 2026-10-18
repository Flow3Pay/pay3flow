<script lang="ts">
  import { onMount } from "svelte";
  import { locale, t } from "$lib/i18n";
  import type { SwapInput, SwapService, SwapServiceState, PreparedSwap } from "$lib/swap-service";
  import SwapForm from "./SwapForm.svelte";
  export let input: SwapInput;
  export let providerName: string;
  export let networkNames: Record<string, string> = {};
  export let initialOutput: string | undefined = undefined;
  let service: SwapService | undefined;
  let state: SwapServiceState = {};
  let recipient = "";
  let refundAddress = "";
  let manualDeposit = false;
  let depositAddress = "";
  let depositMemo = "";
  let busy = false;
  let loading = true;
  let initialized = false;
  let error = "";
  let prepared: PreparedSwap | undefined;
  let reference = "";
  let status = "";
  let timer: ReturnType<typeof setInterval> | undefined;
  let disposed = false;
  let revision = 0;
  let retrySubmission = false;
  $: [fromSymbol, fromNetwork] = input.fromAsset.split("@");
  $: [toSymbol, toNetwork] = input.toAsset.split("@");
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  function message(cause: unknown) { return copy(cause instanceof Error ? cause.message : "Wallet operation failed"); }
  function changeRecipient() {
    revision += 1;
    prepared = undefined;
    error = "";
    service?.setRecipient(recipient.trim());
  }
  async function connect() {
    if (!service || busy) return;
    busy = true; error = "";
    try { await service.connect(); } catch (cause) { error = message(cause); }
    finally { busy = false; }
  }
  async function prepare() {
    if (!service || busy || !recipient.trim()) return;
    const version = revision;
    busy = true; error = "";
    try {
      const value = await service.prepare();
      if (!Number.isFinite(Date.parse(value.expiresAt))) throw new Error("Invalid provider quote expiry");
      if (!disposed && version === revision) {
        prepared = value;
        if (manualDeposit && state.depositAddress) {
          depositAddress = state.depositAddress;
          depositMemo = state.depositMemo ?? "";
          reference = (await value.submit()).reference;
          status = "submitted";
          timer = setInterval(() => void track(), 4_000);
          void track();
        }
      }
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
  }
  async function track() {
    if (!prepared || !reference || disposed) return;
    try {
      status = await prepared.status(reference);
      if (status !== "submitted" && timer) { clearInterval(timer); timer = undefined; }
    } catch (cause) { error = message(cause); }
  }
  async function submit() {
    if (!prepared || busy || reference) return;
    busy = true; error = "";
    try {
      if (!retrySubmission && Date.parse(prepared.expiresAt) <= Date.now() + 5_000) {
        prepared = await service!.prepare();
        error = copy("The quote expired. Review a fresh quote.");
        return;
      }
      const result = await prepared.submit();
      if (result.approval) {
        prepared = await service!.prepare();
        return;
      }
      reference = result.reference;
      status = "submitted";
      retrySubmission = false;
      timer = setInterval(() => void track(), 4_000);
      void track();
    } catch (cause) { error = message(cause); retrySubmission = true; }
    finally { busy = false; }
  }
  onMount(() => {
    void import("$lib/swap-service").then(({ createSwapService }) => createSwapService(input, value => {
      if (disposed) return;
      if (value.address !== state.address && !retrySubmission && !reference) { revision += 1; prepared = undefined; }
      state = value;
      const first = !initialized;
      initialized = true;
      if (first && service && recipient.trim()) service.setRecipient(recipient.trim());
    })).then(async value => {
      if (disposed) { value.destroy(); return; }
      service = value;
      if (initialized && recipient.trim()) value.setRecipient(recipient.trim());
      loading = false;
      const saved = await value.recover?.();
      if (!disposed && saved) {
        prepared = saved.prepared;
        recipient = saved.recipient;
        reference = saved.reference ?? "";
        depositAddress = saved.depositAddress ?? "";
        depositMemo = saved.depositMemo ?? "";
        manualDeposit = Boolean(depositAddress);
        retrySubmission = !reference;
        if (reference) { status = "submitted"; timer = setInterval(() => void track(), 4_000); void track(); }
      }
    }).catch(cause => { if (!disposed) { error = message(cause); loading = false; } });
    return () => { disposed = true; revision += 1; if (timer) clearInterval(timer); service?.destroy(); };
  });
</script>

<SwapForm provider={input.provider} {providerName} amount={input.amount} {fromSymbol} {toSymbol} {fromNetwork} {toNetwork} output={prepared?.output ?? state.output ?? initialOutput} {networkNames} busy={busy || loading}>
  {#if loading}<p role="status">{copy("Preparing your swap…")}</p>{/if}
  {#if !reference}
    {#if state.address}<p class="address" data-testid="swap-source-wallet">{copy("From wallet")} · {state.address}</p>
    {:else if !manualDeposit && !retrySubmission}<button type="button" class="primary" disabled={!service || !initialized || busy} on:click={connect}>{copy("Connect wallet")}</button>{/if}
    <label>{copy("Send swap output to")}<input aria-label={copy("Swap recipient address")} bind:value={recipient} on:input={changeRecipient} disabled={busy || retrySubmission} placeholder={copy("Recipient on {network}", { network: networkNames[toNetwork] ?? toNetwork })} autocomplete="off" spellcheck="false" /></label>
    {#if state.address && fromNetwork === toNetwork}<button type="button" class="secondary" disabled={busy || retrySubmission} on:click={() => { recipient = state.address!; changeRecipient(); }}>{copy("Connected wallet")}</button>{/if}
    {#if service?.setExternalDeposit && !state.address}
      <button type="button" class="secondary" disabled={!initialized || busy || Boolean(prepared)} on:click={() => { manualDeposit = true; service?.setExternalDeposit?.(refundAddress); }}>{copy("Send from another wallet")}</button>
      {#if manualDeposit}<label>{copy("Refund address")}<input aria-label={copy("Refund address")} bind:value={refundAddress} on:input={() => { revision += 1; prepared = undefined; service?.setExternalDeposit?.(refundAddress); }} disabled={busy || retrySubmission} autocomplete="off" spellcheck="false" /></label>{/if}
    {/if}
    {#if !prepared}<button type="button" class="primary" disabled={!service || busy || !recipient.trim() || !state.ready} on:click={prepare}>{busy ? copy("Preparing…") : copy("Swap {from} for {to}", { from: fromSymbol, to: toSymbol })}</button>
    {:else}
      <p>{copy("Recipient")} · {recipient}</p>
      <p>{copy("Quote expires")} · {new Date(prepared.expiresAt).toLocaleTimeString($locale)}</p>
      {#if manualDeposit && state.depositAddress}
        <p>{copy("Send {amount} {asset} to the deposit address", { amount: input.amount, asset: fromSymbol })}</p>
        <code data-testid="swap-deposit-address">{state.depositAddress}</code>
        {#if state.depositMemo}<p>{copy("Memo")} · <code>{state.depositMemo}</code></p>{/if}
      {/if}
      <button type="button" class="primary" disabled={busy} on:click={submit}>{busy ? copy("Waiting for wallet…") : retrySubmission ? copy("Retry swap") : manualDeposit ? copy("Track swap") : copy("Review and sign")}</button>
    {/if}
  {:else}
    {#if manualDeposit && depositAddress && status === "submitted"}
      <p>{copy("Send {amount} {asset} to the deposit address", { amount: input.amount, asset: fromSymbol })}</p>
      <code data-testid="swap-deposit-address">{depositAddress}</code>
      {#if depositMemo}<p>{copy("Memo")} · <code>{depositMemo}</code></p>{/if}
    {/if}
    <p role="status">{copy(status === "completed" ? "Swap completed. Continue with the next route step." : status === "submitted" ? manualDeposit ? "Deposit address is ready. Pay3Flow tracks receipt and completion automatically." : "Submitted. Pay3Flow is tracking provider completion automatically." : "Execution finished with status: {status}.", { status })}</p>
    <code data-testid="swap-reference">{reference}</code>
  {/if}
  {#if error || state.error}<p class="errorText" role="alert">{error || state.error}</p>{/if}
  <p class="safety">{copy("Pay3Flow never receives your private key. Approval and transfer require confirmation in your wallet.")}</p>
</SwapForm>

<style>
  label { display: grid; gap: 8px; font-size: 12px; color: var(--color-text-soft); }
  input, button { min-height: 44px; border: 1px solid var(--color-border); border-radius: 11px; font: inherit; color: inherit; background: transparent; }
  input { width: 100%; padding: 0 11px; color: var(--color-text); }
  button { padding: 0 12px; cursor: pointer; font-weight: 750; }
  button:disabled { cursor: not-allowed; opacity: .55; }
  .primary { width: 100%; border-color: var(--color-primary); background: var(--color-primary); color: var(--color-text-invert); }
  .secondary { width: fit-content; }
  p, code { margin: 0; overflow-wrap: anywhere; font-size: 12px; line-height: 1.5; }
  .address, .safety { color: var(--color-text-soft); }
  .safety { padding-top: 8px; border-top: 1px solid var(--color-border); }
  .errorText { color: var(--color-danger); }
  @media (max-width: 980px), (pointer: coarse) { input { font-size: 16px; } }
</style>
