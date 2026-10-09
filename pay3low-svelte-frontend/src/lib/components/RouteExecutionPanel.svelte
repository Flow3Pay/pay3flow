<script lang="ts">
  import "@near-wallet-selector/modal-ui/styles.css";
  import { onDestroy, onMount } from "svelte";
  import { getAnonymousUserId } from "$lib/anonymous-user";
  import { createRouteExecution, fetchRouteExecution, submitRouteExecution, type RouteCandidate, type RouteExecution } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";
  import { hasExecutionFunds, prepareWalletAction, validateRecipient, walletFamily, type ConnectedWallet, type PreparedWalletAction } from "$lib/wallet-execution";
  import { wallets, connectWallet, restoreWallets } from "$lib/wallet-session";

  export let route: RouteCandidate;
  let sourceWallet: ConnectedWallet | null = null;
  let recipientWallet: ConnectedWallet | null = null;
  let recipientMode: "connected" | "manual" | null = null;
  let manualRecipient = "";
  let execution: RouteExecution | null = null;
  let preparedAction: PreparedWalletAction | null = null;
  let fundsReady = false;
  let autoPrompt = false;
  let busy = false;
  let error = "";
  let notice = "";
  let fundTimer: ReturnType<typeof setInterval> | null = null;
  let statusTimer: ReturnType<typeof setInterval> | null = null;
  let checkingFunds = false;
  let checkingStatus = false;
  let ownerId = "";
  let revision = 0;
  let mounted = false;
  let disposed = false;
  let previousIdentity = "";
  let pendingSubmission: { reference: string; kind: "transaction_hash" | "order_uid" } | null = null;
  let quoteRequestKey: string | null = null;
  const STORAGE_PREFIX = "pay3flow.route-execution.";

  $: descriptor = route.execution;
  $: sourceNetwork = descriptor?.from_asset.split("@", 2)[1]?.toLowerCase() ?? "";
  $: destinationNetwork = descriptor?.to_asset.split("@", 2)[1]?.toLowerCase() ?? "";
  $: sourceFamily = walletFamily(sourceNetwork);
  $: destinationFamily = walletFamily(destinationNetwork);
  $: sourceSupported = Boolean(sourceFamily);
  $: sourceWallet = sourceFamily ? $wallets[sourceFamily] ?? null : null;
  $: recipientWallet = recipientMode === "connected" && destinationFamily ? $wallets[destinationFamily] ?? null : null;
  $: recipient = recipientMode === "connected" ? recipientWallet?.address ?? "" : manualRecipient.trim();
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);
  $: identity = `${route.route_id}:${sourceWallet?.address ?? ""}:${sourceWallet?.family === "evm" ? sourceWallet.chainId : ""}:${recipient}`;
  $: if (mounted && identity !== previousIdentity) {
    previousIdentity = identity;
    revision += 1;
    autoPrompt = false;
    fundsReady = false;
    preparedAction = null;
    quoteRequestKey = null;
    if (fundTimer) clearInterval(fundTimer);
    fundTimer = null;

  }

  function shortAddress(value: string) {
    return value.length > 18 ? `${value.slice(0, 8)}…${value.slice(-6)}` : value;
  }
  function quoteExpiry(value: string) {
    return new Date(value).toLocaleTimeString($locale, { hour: "2-digit", minute: "2-digit" });
  }
  function executionOwner(): string {
    if (!ownerId) ownerId = getAnonymousUserId() ?? crypto.randomUUID();
    return ownerId;
  }
  function storageKey(): string { return `${STORAGE_PREFIX}${route.route_id}`; }
  function rememberExecution(value: RouteExecution) {
    try { localStorage.setItem(storageKey(), JSON.stringify({ id: value.id, owner: executionOwner(), submission: pendingSubmission })); } catch { /* Optional persistence. */ }
  }
  function forgetExecution() {
    try { localStorage.removeItem(storageKey()); } catch { /* Optional persistence. */ }
  }
  function startStatusPolling() {
    if (statusTimer) clearInterval(statusTimer);
    if (!disposed) statusTimer = setInterval(() => void updateStatus(), 4_000);
  }
  function message(cause: unknown) { return cause instanceof Error ? cause.message : "Wallet operation failed"; }

  async function connectSource() {
    if (!descriptor || !sourceSupported || busy || pendingSubmission || execution?.status === "submitted") return;
    busy = true;
    error = "";
    try {
      const connected = await connectWallet(sourceNetwork);
      if (disposed) return;
      sourceWallet = connected;
      if (destinationFamily === connected.family || (destinationFamily && $wallets[destinationFamily])) recipientMode = "connected";
      else recipientMode = "manual";
      // Let shared session and recipient changes invalidate the old preparation.
      const { tick } = await import("svelte");
      await tick();
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
    if (sourceWallet && recipient && !error && !disposed) await beginAutomaticSwap();
  }

  async function chooseConnectedRecipient() {
    if (busy || !destinationFamily || pendingSubmission || execution?.status === "submitted") return;
    busy = true;
    error = "";
    try {
      // Connecting a destination EVM wallet must not switch the source chain.
      if (!$wallets[destinationFamily]) await connectWallet(destinationNetwork);
      recipientMode = "connected";
      const { tick } = await import("svelte");
      await tick();
    } catch (cause) { error = message(cause); }
    finally { busy = false; }
    if (!error && !disposed) await beginAutomaticSwap();
  }
  function chooseManualRecipient() {
    if (busy || pendingSubmission || execution?.status === "submitted") return;
    recipientMode = "manual";
    manualRecipient = "";
    execution = null;
    preparedAction = null;
    autoPrompt = false;
    forgetExecution();
    error = "";
    notice = "";
  }
  function recipientChanged() {
    if (sourceWallet && recipient && !busy) void beginAutomaticSwap();
  }

  async function prepareQuote(version: number): Promise<boolean> {
    if (!descriptor || !sourceWallet || !recipient || pendingSubmission || execution?.status === "submitted") return false;
    const wallet = sourceWallet;
    await validateRecipient(sourceNetwork, wallet.address);
    await validateRecipient(destinationNetwork, recipient);
    quoteRequestKey ??= crypto.randomUUID();
    const value = await createRouteExecution({
      anonymousId: executionOwner(), routeToken: descriptor.token,
      sourceAddress: wallet.address, recipient, refundTo: wallet.address,
      amount: descriptor.input_amount, slippageBps: 100, idempotencyKey: quoteRequestKey,
    });
    if (version !== revision || disposed) return false;
    execution = value;
    rememberExecution(value);
    const prepared = await prepareWalletAction(value, wallet);
    if (version !== revision || disposed) return false;
    preparedAction = prepared;
    if (prepared.expectedOutput) value.expected_output = prepared.expectedOutput;
    if (prepared.expectedFee) value.expected_fee = prepared.expectedFee;
    if (prepared.expiresAt) value.quote_expires_at = prepared.expiresAt;
    fundsReady = await hasExecutionFunds(value, wallet);
    return version === revision && !disposed;
  }

  async function beginAutomaticSwap() {
    if (busy || pendingSubmission || execution?.status === "submitted" || !sourceWallet || !recipient || disposed) return;
    const { tick } = await import("svelte");
    await tick();
    if (busy || disposed) return;
    busy = true;
    error = "";
    notice = "";
    quoteRequestKey = null;
    autoPrompt = true;
    const version = revision;
    try {
      if (fundTimer) clearInterval(fundTimer);
      fundTimer = null;
      if (!await prepareQuote(version)) { autoPrompt = false; return; }
      if (fundsReady) await signPrepared(version);
      else fundTimer = setInterval(() => void updateFunds(), 5_000);
    } catch (cause) { error = message(cause); autoPrompt = false; }
    finally { busy = false; }
  }

  async function updateFunds() {
    if (!execution || !sourceWallet || checkingFunds || busy || disposed || execution.status !== "awaiting_signature") return;
    checkingFunds = true;
    const version = revision;
    try {
      const ready = await hasExecutionFunds(execution, sourceWallet);
      if (version !== revision || disposed) return;
      fundsReady = ready;
      if (ready && fundTimer) { clearInterval(fundTimer); fundTimer = null; }
      if (ready && autoPrompt) await signAndSubmit();
    } catch (cause) { error = message(cause); autoPrompt = false; }
    finally { checkingFunds = false; }
  }

  async function recordSubmission() {
    if (!execution || !pendingSubmission) return;
    execution = await submitRouteExecution(execution.id, executionOwner(), pendingSubmission.reference, pendingSubmission.kind);
    pendingSubmission = null;
    rememberExecution(execution);
    startStatusPolling();
  }

  async function signPrepared(version: number) {
    autoPrompt = false;
    for (let approvals = 0; approvals < 4; approvals += 1) {
      if (!execution || !sourceWallet || version !== revision || disposed) return;
      if (Date.parse(execution.quote_expires_at) <= Date.now() + 5_000) {
        quoteRequestKey = null;
        if (!await prepareQuote(version) || !fundsReady) return;
      }
      if (!preparedAction) preparedAction = await prepareWalletAction(execution, sourceWallet);
      if (version !== revision || disposed) return;
      const signingExecution = execution;
      // The callback is already signed, even if the account changes meanwhile.
      const submitted = await preparedAction.submit();
      if (submitted.kind === "approval_confirmed") {
        quoteRequestKey = null;
        preparedAction = null;
        notice = copy("Approval confirmed. Review and sign the refreshed swap transaction.");
        if (version !== revision || disposed || !await prepareQuote(version) || !fundsReady) return;
        continue;
      }
      execution = signingExecution;
      pendingSubmission = submitted;
      preparedAction = null;
      rememberExecution(signingExecution);
      await recordSubmission();
      return;
    }
    throw new Error("Approval did not make the token available. Retry the swap.");
  }

  async function signAndSubmit() {
    if (busy || disposed) return;
    busy = true;
    error = "";
    try {
      if (pendingSubmission) await recordSubmission();
      else await signPrepared(revision);
    } catch (cause) { error = message(cause); autoPrompt = false; }
    finally { busy = false; }
  }

  async function updateStatus() {
    if (!execution || execution.status !== "submitted" || checkingStatus || disposed) return;
    checkingStatus = true;
    try {
      execution = await fetchRouteExecution(execution.id, executionOwner());
      rememberExecution(execution);
      if (execution.status !== "submitted" && statusTimer) { clearInterval(statusTimer); statusTimer = null; }
    } catch (cause) { error = message(cause); }
    finally { checkingStatus = false; }
  }

  onMount(() => {
    mounted = true;
    previousIdentity = identity;
    void restoreWallets();
    void (async () => {
      try {
        const raw = localStorage.getItem(storageKey());
        if (!raw) return;
        const saved = JSON.parse(raw) as { id?: string; owner?: string; submission?: typeof pendingSubmission };
        if (!saved.id || !saved.owner) return;
        busy = true;
        ownerId = saved.owner;
        const restored = await fetchRouteExecution(saved.id, ownerId);
        if (disposed || restored.route_id !== route.route_id) return;
        execution = restored;
        manualRecipient = restored.recipient;
        recipientMode = "manual";
        pendingSubmission = saved.submission ?? null;
        const callback = new URL(window.location.href);
        const redirectExecution = callback.searchParams.get("pay3flow_execution");
        const hash = callback.searchParams.get("transactionHashes");
        if (redirectExecution === restored.id && hash && /^[1-9A-HJ-NP-Za-km-z]{32,64}$/.test(hash)) {
          pendingSubmission = { reference: hash, kind: "transaction_hash" };
          rememberExecution(restored);
        }
        if (redirectExecution === restored.id) {
          if (callback.searchParams.has("errorCode")) error = callback.searchParams.get("errorMessage") ?? "NEAR wallet cancelled the transaction";
          for (const key of ["pay3flow_execution", "transactionHashes", "errorCode", "errorMessage"]) callback.searchParams.delete(key);
          history.replaceState(history.state, "", callback);
        }
        if (restored.status === "submitted") { pendingSubmission = null; startStatusPolling(); }
        else if (pendingSubmission) await recordSubmission();
      } catch (cause) { error = message(cause); }
      finally { busy = false; }
    })();
  });
  onDestroy(() => {
    disposed = true;
    revision += 1;
    if (fundTimer) clearInterval(fundTimer);
    if (statusTimer) clearInterval(statusTimer);
  });
</script>

{#if descriptor}
  <section class="execution" data-testid="route-wallet-execution">
    <div class="executionTitle"><strong>{copy("Execute with wallet")}</strong><span>{copy("Non-custodial")}</span></div>
    {#if !sourceSupported}
      <p class="muted">{copy("Embedded execution does not yet support a wallet for {network}. Open the provider manually for this route.", { network: sourceNetwork })}</p>
    {:else}
      <div class="walletRow">
        <button type="button" class="walletButton" disabled={busy || Boolean(pendingSubmission) || execution?.status === "submitted"} on:click={connectSource}>
          {sourceWallet ? copy("Swap from {address}", { address: shortAddress(sourceWallet.address) }) : copy("Connect {network} wallet", { network: sourceNetwork })}
        </button>
      </div>
      {#if sourceWallet}
        <fieldset>
          <legend>{copy("Send swap output to")}</legend>
          <div class="choiceRow">
            <button type="button" class:active={recipientMode === "connected"} disabled={busy || Boolean(pendingSubmission) || execution?.status === "submitted" || !walletFamily(destinationNetwork)} on:click={chooseConnectedRecipient}>{copy("Connected wallet")}</button>
            <button type="button" class:active={recipientMode === "manual"} disabled={busy || Boolean(pendingSubmission) || execution?.status === "submitted"} on:click={chooseManualRecipient}>{copy("Another address")}</button>
          </div>
          {#if recipientMode === "connected" && recipientWallet}<p class="address">{shortAddress(recipientWallet.address)} · {destinationNetwork}</p>{/if}
          {#if recipientMode === "manual"}<input disabled={busy || Boolean(pendingSubmission) || execution?.status === "submitted"} bind:value={manualRecipient} on:change={recipientChanged} autocomplete="off" spellcheck="false" placeholder={copy("Recipient on {network}", { network: destinationNetwork })} aria-label={copy("Swap recipient address")} />{/if}
        </fieldset>
      {/if}
      {#if sourceWallet && recipient && !execution}
        <button type="button" class="primary" disabled={busy} on:click={beginAutomaticSwap}>{busy ? copy("Preparing…") : copy("Prepare live transaction")}</button>
      {/if}
      {#if execution}
        <div class="review">
          <span><small>{copy("Amount")}</small><strong>{execution.input_amount} {execution.from_asset}</strong></span>
          {#if execution.expected_output}<span><small>{copy("Expected output")}</small><strong>{execution.expected_output} {execution.to_asset}</strong></span>{/if}
          {#if execution.expected_fee}<span><small>{copy("Provider fee")}</small><strong>{execution.expected_fee.amount} {execution.expected_fee.asset}</strong></span>{/if}
          <span><small>{copy("Quote expires")}</small><strong>{quoteExpiry(execution.quote_expires_at)}</strong></span>
          <span><small>{copy("Recipient")}</small><strong>{shortAddress(execution.recipient)}</strong></span>
          <span><small>{copy("Status")}</small><strong>{execution.status.replaceAll("_", " ")}</strong></span>
        </div>
        {#if pendingSubmission}
          <p class="ready">{copy("Transaction sent. Retry tracking without signing again.")}</p>
          <button type="button" class="primary" disabled={busy} on:click={signAndSubmit}>{copy("Retry tracking")}</button>
        {:else if execution.status === "awaiting_signature"}
          {#if !sourceWallet}
            <p class="muted">{copy("Reconnect the source wallet to continue this prepared swap.")}</p>
          {:else if fundsReady}
            <p class="ready">{copy("Funds detected. Review the amount and recipient, then confirm in your wallet.")}</p>
            <button type="button" class="primary" disabled={busy} on:click={signAndSubmit}>{busy ? copy("Waiting for wallet…") : copy("Review and sign")}</button>
          {:else}
            <p class="muted">{copy("Waiting until the wallet contains at least {amount} {asset}. Balance is checked automatically.", { amount: execution.input_amount, asset: execution.from_asset })}</p>
            <button type="button" class="secondary" disabled={busy || !sourceWallet} on:click={beginAutomaticSwap}>{copy("Retry swap")}</button>
          {/if}
        {:else if execution.status === "submitted"}
          <p class="ready">{copy("Submitted. Pay3Flow is tracking provider completion automatically.")}</p>
        {:else if execution.status === "completed"}
          <p class="success">{copy("Swap completed. Continue with the next route step.")}</p>
        {:else}
          <p class="errorText">{copy("Execution finished with status: {status}.", { status: execution.status.replaceAll("_", " ") })}</p>
        {/if}
      {/if}
      {#if error}<p class="errorText" role="alert">{error}</p>{/if}
      {#if notice}<p class="ready" role="status">{notice}</p>{/if}
      <p class="safety">{copy("Pay3Flow never receives your private key. Approval and transfer require confirmation in your wallet.")}</p>
    {/if}
  </section>
{/if}

<style>
  .execution { display: grid; gap: 12px; margin-top: 14px; padding: 14px; border: 1px solid var(--color-border); border-radius: 16px; background: var(--color-panel); }
  .executionTitle, .walletRow, .choiceRow { display: flex; align-items: center; gap: 8px; }
  .executionTitle { justify-content: space-between; }
  .executionTitle span { padding: 4px 8px; border-radius: 999px; background: var(--color-accent-soft); color: var(--color-accent-text); font-size: 12px; font-weight: 800; text-transform: uppercase; }
  fieldset { display: grid; gap: 9px; margin: 0; padding: 0; border: 0; }
  legend, small { color: var(--color-text-soft); font-size: 12px; }
  button, input { min-height: 40px; border: 1px solid var(--color-border); border-radius: 11px; font: inherit; }
  button { padding: 0 12px; cursor: pointer; background: transparent; color: inherit; font-weight: 750; }
  button:disabled { cursor: not-allowed; opacity: .55; }
  .walletButton, .primary { width: 100%; }
  .primary { border-color: var(--color-primary); background: var(--color-primary); color: var(--color-text-invert); }
  .secondary { width: fit-content; }
  .choiceRow button { flex: 1; }
  .choiceRow button.active { border-color: var(--color-accent-text); border-width: var(--border-highlight-width); background: var(--color-accent-soft); }
  input { width: 100%; padding: 0 11px; background: transparent; color: inherit; }
  .review { display: grid; gap: 7px; padding: 10px; border-radius: 12px; background: rgba(127,127,127,.08); }
  .review span { display: flex; justify-content: space-between; gap: 12px; }
  .review strong { overflow-wrap: anywhere; text-align: right; font-size: 12px; }
  p { margin: 0; line-height: 1.45; }
  .muted, .safety, .address { color: var(--color-text-soft); font-size: 12px; }
  .ready { color: var(--color-accent-text); font-size: 12px; }
  .success { color: var(--color-good); font-weight: 700; }
  .errorText { color: var(--color-danger); font-size: 12px; }
  .safety { padding-top: 4px; border-top: 1px solid var(--color-border); }

  @media (max-width: 980px), (pointer: coarse) { button, input { min-height: 44px; } input { font-size: 16px; } }
</style>
