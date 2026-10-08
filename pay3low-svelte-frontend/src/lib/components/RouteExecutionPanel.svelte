<script lang="ts">
  import "@near-wallet-selector/modal-ui/styles.css";
  import { onDestroy, onMount } from "svelte";
  import { getAnonymousUserId } from "$lib/anonymous-user";
  import { createRouteExecution, fetchRouteExecution, submitRouteExecution, type RouteCandidate, type RouteExecution } from "$lib/exchange";
  import { locale, t } from "$lib/i18n";
  import { connectForNetwork, hasExecutionFunds, prepareWalletAction, walletFamily, type ConnectedWallet, type PreparedWalletAction } from "$lib/wallet-execution";

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
  let ownerId = "";
  const STORAGE_PREFIX = "pay3flow.route-execution.";

  $: descriptor = route.execution;
  $: sourceNetwork = descriptor?.from_asset.split("@", 2)[1]?.toLowerCase() ?? "";
  $: destinationNetwork = descriptor?.to_asset.split("@", 2)[1]?.toLowerCase() ?? "";
  $: sourceSupported = Boolean(walletFamily(sourceNetwork));
  $: recipient = recipientMode === "connected" ? recipientWallet?.address ?? "" : manualRecipient.trim();
  $: copy = (key: string, params: Record<string, string | number> = {}) => t(key, params, $locale);

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

  function storageKey(): string {
    return `${STORAGE_PREFIX}${route.route_id}`;
  }

  function rememberExecution(value: RouteExecution) {
    try {
      localStorage.setItem(storageKey(), JSON.stringify({ id: value.id, owner: executionOwner() }));
    } catch {
      // Persistence is optional in private browsing mode.
    }
  }

  function forgetExecution() {
    try {
      localStorage.removeItem(storageKey());
    } catch {
      // Persistence is optional in private browsing mode.
    }
  }

  function startStatusPolling() {
    if (statusTimer) clearInterval(statusTimer);
    statusTimer = setInterval(() => void updateStatus(), 4_000);
  }

  async function connectSource() {
    if (!descriptor || !sourceSupported) return;
    busy = true;
    error = "";
    notice = "";
    try {
      const connected = await connectForNetwork(sourceNetwork);
      const retained = execution && execution.source_address.toLowerCase() === connected.address.toLowerCase();
      sourceWallet = connected;
      if (retained) {
        await updateFunds();
        autoPrompt = true;
        if (fundsReady) {
          autoPrompt = false;
          await signAndSubmit();
        } else if (!fundTimer) {
          fundTimer = setInterval(() => void updateFunds(), 5_000);
        }
      } else {
        execution = null;
        preparedAction = null;
        fundsReady = false;
        forgetExecution();
        const recipientFamily = walletFamily(destinationNetwork);
        if (recipientFamily === connected.family) {
          recipientWallet = connected;
          recipientMode = "connected";
          await beginAutomaticSwap();
        } else {
          recipientWallet = null;
          recipientMode = "manual";
        }
      }
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Wallet connection failed";
    } finally {
      busy = false;
    }
  }

  async function chooseConnectedRecipient() {
    recipientMode = "connected";
    busy = true;
    error = "";
    notice = "";
    try {
      if (sourceWallet && walletFamily(destinationNetwork) === sourceWallet.family) {
        recipientWallet = sourceWallet;
      } else {
        recipientWallet = await connectForNetwork(destinationNetwork);
      }
      execution = null;
      preparedAction = null;
      forgetExecution();
      await beginAutomaticSwap();
    } catch (cause) {
      recipientWallet = null;
      error = cause instanceof Error ? cause.message : "Recipient wallet connection failed";
    } finally {
      busy = false;
    }
  }

  function chooseManualRecipient() {
    recipientMode = "manual";
    recipientWallet = null;
    manualRecipient = "";
    execution = null;
    preparedAction = null;
    forgetExecution();
    error = "";
    notice = "";
  }

  function recipientChanged() {
    execution = null;
    preparedAction = null;
    forgetExecution();
    if (sourceWallet && recipient) void beginAutomaticSwap();
  }

  async function beginAutomaticSwap() {
    if (!descriptor || !sourceWallet || !recipient) return;
    autoPrompt = true;
    await prepare();
    if (execution && fundsReady) {
      autoPrompt = false;
      await signAndSubmit();
    }
  }

  async function prepare() {
    if (!descriptor || !sourceWallet || !recipient) return;
    busy = true;
    error = "";
    notice = "";
    stopTimers();
    try {
      execution = await createRouteExecution({
        anonymousId: executionOwner(),
        routeToken: descriptor.token,
        sourceAddress: sourceWallet.address,
        recipient,
        refundTo: sourceWallet.address,
        amount: descriptor.input_amount,
        slippageBps: 100,
        idempotencyKey: crypto.randomUUID(),
      });
      rememberExecution(execution);
      preparedAction = await prepareWalletAction(execution, sourceWallet);
      if (preparedAction.expectedOutput) execution.expected_output = preparedAction.expectedOutput;
      execution.expected_fee = preparedAction.expectedFee;
      if (preparedAction.expiresAt) execution.quote_expires_at = preparedAction.expiresAt;
      await updateFunds();
      if (!fundsReady) fundTimer = setInterval(() => void updateFunds(), 5_000);
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Executable quote failed";
    } finally {
      busy = false;
    }
  }

  async function updateFunds() {
    if (!execution || !sourceWallet) return;
    try {
      fundsReady = await hasExecutionFunds(execution, sourceWallet);
      if (fundsReady && fundTimer) {
        clearInterval(fundTimer);
        fundTimer = null;
      }
      if (fundsReady && autoPrompt && !busy) {
        autoPrompt = false;
        void signAndSubmit();
      }
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Balance check failed";
    }
  }

  async function signAndSubmit() {
    if (!execution || !sourceWallet || !descriptor) return;
    busy = true;
    error = "";
    notice = "";
    try {
      const refreshBuffer = execution.provider === "symbiosis" ? 25_000 : 30_000;
      if (Date.parse(execution.quote_expires_at) <= Date.now() + refreshBuffer) {
        await prepare();
        if (!execution || !fundsReady) return;
      }
      if (!preparedAction) {
        preparedAction = await prepareWalletAction(execution, sourceWallet);
        if (preparedAction.expectedOutput) execution.expected_output = preparedAction.expectedOutput;
        execution.expected_fee = preparedAction.expectedFee;
        if (preparedAction.expiresAt) execution.quote_expires_at = preparedAction.expiresAt;
      }
      const submitted = await preparedAction.submit();
      if (submitted.kind === "approval_confirmed") {
        await prepare();
        notice = copy("Approval confirmed. Review and sign the refreshed swap transaction.");
        return;
      }
      execution = await submitRouteExecution(execution.id, executionOwner(), submitted.reference, submitted.kind);
      preparedAction = null;
      rememberExecution(execution);
      startStatusPolling();
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Wallet rejected or failed to submit the operation";
    } finally {
      busy = false;
    }
  }

  async function updateStatus() {
    if (!execution || execution.status !== "submitted") return;
    try {
      execution = await fetchRouteExecution(execution.id, executionOwner());
      rememberExecution(execution);
      if (["completed", "failed", "cancelled", "expired", "refunded", "stuck"].includes(execution.status) && statusTimer) {
        clearInterval(statusTimer);
        statusTimer = null;
      }
    } catch (cause) {
      error = cause instanceof Error ? cause.message : "Unable to refresh execution status";
    }
  }

  onMount(() => {
    try {
      const raw = localStorage.getItem(storageKey());
      if (!raw) return;
      const saved = JSON.parse(raw) as { id?: string; owner?: string };
      if (!saved.id || !saved.owner) return;
      ownerId = saved.owner;
      void fetchRouteExecution(saved.id, ownerId)
        .then((restored) => {
          if (restored.route_id !== route.route_id) return;
          execution = restored;
          preparedAction = null;
          recipientMode = "manual";
          manualRecipient = restored.recipient;
          if (restored.status === "submitted") startStatusPolling();
        })
        .catch(() => forgetExecution());
    } catch {
      forgetExecution();
    }
  });

  function stopTimers() {
    if (fundTimer) clearInterval(fundTimer);
    if (statusTimer) clearInterval(statusTimer);
    fundTimer = null;
    statusTimer = null;
  }

  onDestroy(stopTimers);
</script>

{#if descriptor}
  <section class="execution" data-testid="route-wallet-execution">
    <div class="executionTitle"><strong>{copy("Execute with wallet")}</strong><span>{copy("Non-custodial")}</span></div>
    {#if !sourceSupported}
      <p class="muted">{copy("Embedded execution does not yet support a wallet for {network}. Open the provider manually for this route.", { network: sourceNetwork })}</p>
    {:else}
      <div class="walletRow">
        <button type="button" class="walletButton" disabled={busy} on:click={connectSource}>
          {sourceWallet ? copy("Source: {address}", { address: shortAddress(sourceWallet.address) }) : copy("Connect {network} wallet", { network: sourceNetwork })}
        </button>
      </div>
      {#if sourceWallet}
        <fieldset>
          <legend>{copy("Send swap output to")}</legend>
          <div class="choiceRow">
            <button type="button" class:active={recipientMode === "connected"} disabled={busy || !walletFamily(destinationNetwork)} on:click={chooseConnectedRecipient}>{copy("Connected wallet")}</button>
            <button type="button" class:active={recipientMode === "manual"} disabled={busy} on:click={chooseManualRecipient}>{copy("Another address")}</button>
          </div>
          {#if recipientMode === "connected" && recipientWallet}<p class="address">{shortAddress(recipientWallet.address)} · {destinationNetwork}</p>{/if}
          {#if recipientMode === "manual"}<input bind:value={manualRecipient} on:change={recipientChanged} autocomplete="off" spellcheck="false" placeholder={copy("Recipient on {network}", { network: destinationNetwork })} aria-label={copy("Swap recipient address")} />{/if}
        </fieldset>
      {/if}
      {#if sourceWallet && recipient && !execution}
        <button type="button" class="primary" disabled={busy} on:click={prepare}>{busy ? copy("Preparing…") : copy("Prepare live transaction")}</button>
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
        {#if execution.status === "awaiting_signature"}
          {#if !sourceWallet}
            <p class="muted">{copy("Reconnect the source wallet to continue this prepared swap.")}</p>
          {:else if fundsReady}
            <p class="ready">{copy("Funds detected. Review the amount and recipient, then confirm in your wallet.")}</p>
            <button type="button" class="primary" disabled={busy} on:click={signAndSubmit}>{busy ? copy("Waiting for wallet…") : copy("Review and sign")}</button>
          {:else}
            <p class="muted">{copy("Waiting until the wallet contains at least {amount} {asset}. Balance is checked automatically.", { amount: execution.input_amount, asset: execution.from_asset })}</p>
            <button type="button" class="secondary" disabled={busy || !sourceWallet} on:click={updateFunds}>{copy("Check now")}</button>
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
