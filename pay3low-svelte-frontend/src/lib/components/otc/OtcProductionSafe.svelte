<script lang="ts">
  import { onMount } from "svelte";
  import { createPublicClient, http, keccak256, type Address } from "viem";
  import { connectWallet } from "$lib/wallet-session";
  import { SAFE_FACTORY, SAFE_DEPLOYMENTS, factoryAbi, safeAbi, safeAppUrl, safeDeployment, safeOwners, safeNetwork, safePlanStorageKey, loadSafePlan, storeSafePlan, type SafeChainId, type SafePlan } from "$lib/otc/production-safe";
  let owners = ["", "", ""], plan: SafePlan | null = null, existing = "", existingUrl = "", busy = false, error = "";
  let chainId: SafeChainId = 1, storageError = false;
  $: network = safeNetwork(chainId);
  $: rpc = createPublicClient({ chain: network.chain, transport: http(network.rpcUrl, { timeout: 20000, retryCount: 1 }) });
  function save(next: SafePlan) { storeSafePlan(localStorage, next); plan = next; }
  function loadNetwork() {
    plan = null; owners = ["", "", ""]; existing = ""; existingUrl = ""; error = ""; storageError = false;
    try {
      plan = loadSafePlan(localStorage, chainId);
      if (plan) owners = [...plan.owners];
    } catch {
      storageError = true;
      error = "Saved Safe details could not be restored. Check your original wallet transaction before continuing.";
    }
  }
  async function run(action: () => Promise<void>) { if (busy) return; busy = true; error = ""; try { await action(); } catch (e) { error = e instanceof Error ? e.message : "Safe request failed"; } finally { busy = false; } }
  async function verifyContracts() {
    if (await rpc.getChainId() !== chainId) throw new Error("Ethereum RPC returned another network.");
    await Promise.all(SAFE_DEPLOYMENTS.map(async ([address, hash]) => {
      const code = await rpc.getCode({ address });
      if (!code || keccak256(code) !== hash) throw new Error("Official Safe contract verification failed.");
    }));
  }
  async function review() {
    if (storageError || plan) return;
    const checked = safeOwners(owners.map(owner => owner.trim()));
    await verifyContracts();
    const code = await rpc.readContract({ address: SAFE_FACTORY, abi: factoryAbi, functionName: "proxyCreationCode" });
    const bytes = crypto.getRandomValues(new Uint8Array(32));
    const salt = BigInt(`0x${Array.from(bytes, byte => byte.toString(16).padStart(2, "0")).join("")}`);
    const deployment = safeDeployment(checked, salt, code);
    save({ chainId, owners: deployment.owners, address: deployment.address, salt: salt.toString(), reference: null, state: "review" });
  }
  async function refresh() {
    if (!plan || storageError || plan.chainId !== chainId) return;
    const current = plan;
    await verifyContracts();
    const creationCode = await rpc.readContract({ address: SAFE_FACTORY, abi: factoryAbi, functionName: "proxyCreationCode" });
    const deployment = safeDeployment(current.owners, BigInt(current.salt), creationCode);
    if (deployment.address !== current.address) throw new Error("Saved Safe configuration differs from its predicted address.");
    const code = await rpc.getCode({ address: current.address });
    if (code && code !== "0x") {
      const [actual, threshold, singleton] = await Promise.all([
        rpc.readContract({ address: current.address, abi: safeAbi, functionName: "getOwners" }),
        rpc.readContract({ address: current.address, abi: safeAbi, functionName: "getThreshold" }),
        rpc.getStorageAt({ address: current.address, slot: "0x0" }),
      ]);
      if (threshold !== 2n || actual.length !== 3 || !actual.every(owner => current.owners.some(expected => expected.toLowerCase() === owner.toLowerCase())) || singleton?.slice(-40).toLowerCase() !== SAFE_DEPLOYMENTS[1][0].slice(2).toLowerCase()) throw new Error("On-chain Safe owners, threshold or singleton differs from the reviewed wallet.");
      save({ ...current, state: "ready" }); return;
    }
    if (current.reference) {
      const receipt = await rpc.getTransactionReceipt({ hash: current.reference });
      if (receipt.status === "reverted") save({ ...current, state: "failed" });
      else throw new Error("Transaction confirmed but Safe is not available. Check the original transaction.");
    } else if (current.state === "unknown") throw new Error("Wallet outcome is unresolved. Check the original transaction in your wallet; do not create another Safe.");
  }
  async function create() {
    if (!plan || storageError || plan.chainId !== chainId || !["review", "failed"].includes(plan.state)) return;
    await verifyContracts();
    const current = plan;
    const code = await rpc.readContract({ address: SAFE_FACTORY, abi: factoryAbi, functionName: "proxyCreationCode" });
    const deployment = safeDeployment(current.owners, BigInt(current.salt), code);
    if (deployment.address !== current.address) throw new Error("Safe configuration changed. Review again.");
    const existingCode = await rpc.getCode({ address: current.address });
    if (existingCode && existingCode !== "0x") { await refresh(); return; }
    const wallet = await connectWallet(network.walletNetwork);
    if (wallet.family !== "evm" || wallet.chainId !== chainId) throw new Error(`Connect your wallet on ${network.name}.`);
    if (!current.owners.some(owner => owner.toLowerCase() === wallet.address.toLowerCase())) throw new Error("Connect one of the three reviewed owner wallets.");
    const { getWalletClient } = await import("@wagmi/core");
    const client = await getWalletClient(wallet.config as Parameters<typeof getWalletClient>[0], { chainId });
    const [activeChainId, accounts] = await Promise.all([client.getChainId(), client.getAddresses()]);
    if (activeChainId !== chainId || !accounts.some(account => account.toLowerCase() === wallet.address.toLowerCase()) || client.account.address.toLowerCase() !== wallet.address.toLowerCase()) throw new Error("Wallet account or network changed. Connect the reviewed owner again.");
    save({ ...current, state: "unknown", reference: null });
    try {
      const reference = await client.sendTransaction({ account: wallet.address as Address, chain: network.chain, to: SAFE_FACTORY, data: deployment.data, value: 0n });
      save({ ...current, state: "submitted", reference });
    } catch (e) {
      // A lost provider response cannot authorize another send. Only explicit rejection can.
      let cause: any = e, rejected = false;
      for (let depth = 0; cause && depth < 8; depth++, cause = cause.cause) if (cause.code === 4001) rejected = true;
      if (rejected) save({ ...current, state: "review", reference: null });
      throw e;
    }
  }
  function reset() {
    if (busy || storageError || (plan && !["review", "ready", "failed"].includes(plan.state))) return;
    try { localStorage.removeItem(safePlanStorageKey(chainId)); plan = null; error = ""; }
    catch { error = "Saved Safe details could not be cleared."; }
  }
  onMount(loadNetwork);
</script>

<details class="safeWallet" data-testid="production-safe">
  <summary>OTC multisig wallet <span>Safe 1.4.1 · {network.name} · 2 of 3</span></summary>
  <div class="content">
    <label>Safe network<select value={chainId} on:change={(event) => { if (busy) return; chainId = Number(event.currentTarget.value) as SafeChainId; loadNetwork(); }} disabled={busy}>
      <option value={1}>Ethereum Mainnet</option><option value={11155111}>Sepolia (testnet)</option>
    </select></label>
    {#if network.faucetUrl}<p>Test network · Use Sepolia ETH only. <a target="_blank" rel="noopener noreferrer" href={network.faucetUrl}>Get test ETH from faucet</a></p>{/if}
    <p>Buyer, seller and arbitrator share a standard Safe. Any two owners approve transfers and refunds in Safe. {chainId === 1 ? "USDT stays in the Safe; EVER delivery is separate." : "This test uses Sepolia ETH; EVER and USDT settlement are outside this test."} This wallet has no automatic expiry or refund.</p>
    {#if !plan}
      <form on:submit|preventDefault={() => run(review)}>
        {#each ["Buyer", "Seller", "Arbitrator"] as role, index}<label>{role} Ethereum address<input required autocomplete="off" spellcheck="false" placeholder="0x…" bind:value={owners[index]} disabled={busy} /></label>{/each}
        <button type="submit" disabled={busy || storageError}>{busy ? "Verifying Safe contracts…" : "Review 2-of-3 Safe"}</button>
      </form>
    {:else}
      <div class="review">
        <strong>{plan.state === "ready" ? "Safe wallet ready" : "Reviewed Safe wallet"}</strong>
        <p>{network.name} · Safe 1.4.1 · 2 of 3 approvals</p>
        {#each plan.owners as owner, index}<p>{["Buyer", "Seller", "Arbitrator"][index]}: <code>{owner}</code></p>{/each}
        <p>Safe address: <code>{plan.address}</code></p>
        {#if plan.reference}<a target="_blank" rel="noopener noreferrer" href={`${network.explorerUrl}/tx/${plan.reference}`}>View original transaction</a>{/if}
        {#if plan.state === "ready"}<a class="action" target="_blank" rel="noopener noreferrer" href={safeAppUrl(plan.address, plan.chainId)}>Open Safe to fund, approve and transfer</a>
        {:else if ["review", "failed"].includes(plan.state)}<p>Your owner wallet pays {chainId === 1 ? "Ethereum" : "Sepolia ETH"} gas. Creating the wallet transfers no tokens.</p><button disabled={busy} on:click={() => run(create)}>{plan.state === "failed" ? "Retry confirmed failed creation" : "Confirm creation in owner wallet"}</button>
        {:else}<p>Creation is pending or unresolved. Keep the original transaction; do not send again.</p>{/if}
        <button disabled={busy} on:click={() => run(refresh)}>Check on-chain status</button>
        {#if ["review", "ready", "failed"].includes(plan.state)}<button disabled={busy} on:click={reset}>{plan.state === "review" ? "Edit owners" : "Set up another wallet"}</button>{/if}
      </div>
    {/if}
    <form class="existing" on:submit|preventDefault={() => { try { existingUrl = safeAppUrl(existing.trim(), chainId); error = ""; } catch (e) { existingUrl = ""; error = (e as Error).message; } }}>
      <label>Already have a Safe? Enter its Ethereum address<input autocomplete="off" placeholder="0x…" bind:value={existing} on:input={() => existingUrl = ""} /></label><button type="submit">Open existing Safe</button>
      {#if existingUrl}<a target="_blank" rel="noopener noreferrer" href={existingUrl}>Continue in Safe</a>{/if}
    </form>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </div>
</details>

<style>
  .safeWallet { margin: 0 0 24px; border: 1px solid var(--color-border); border-radius: 10px; background: var(--color-paper); }
  summary { cursor: pointer; padding: 18px 22px; font-weight: 700; } summary span { display: inline-block; margin-left: 12px; color: var(--color-text-soft); font-size: 12px; font-weight: 400; }
  .content { padding: 0 22px 22px; } p { line-height: 1.6; } form { display: grid; gap: 12px; } label { display: grid; gap: 6px; font-size: 13px; }
  input, select { width: 100%; box-sizing: border-box; min-width: 0; padding: 10px; border: 1px solid var(--color-border); border-radius: 5px; background: var(--color-panel); color: var(--color-text); }
  button, .action { display: inline-block; padding: 10px 14px; margin: 4px 8px 4px 0; border: 1px solid var(--color-border); border-radius: 5px; background: var(--color-panel); color: var(--color-text); cursor: pointer; font-size: 13px; }
  button:disabled { opacity: .5; cursor: wait; } code { overflow-wrap: anywhere; font-size: 12px; } .existing { margin-top: 24px; padding-top: 18px; border-top: 1px solid var(--color-border); } .error { color: #c53b3b; overflow-wrap: anywhere; }
  @media (max-width: 600px) { summary span { display: block; margin: 8px 0 0; } .content { padding: 0 14px 14px; } }
</style>
