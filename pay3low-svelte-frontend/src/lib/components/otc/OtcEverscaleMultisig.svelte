<script lang="ts">
  import { onMount } from "svelte";
  import { locale } from "$lib/i18n";
  import { connectEverscale } from "$lib/everscale-wallet";
  import { request, decimal, type Config, type Session } from "$lib/otc/service";
  import { parseAgreement, agreementHash, validateCall, pendingStorageKey, type EverAgreement, type EverReview, type EverOperation, type EverHandoff, type EverPending } from "$lib/otc/everscale-multisig";
  export let config: Config | null = null;
  export let session: Session | null = null;
  export let busy = false;
  export let onConnect: () => Promise<void>;
  export let onBusyChange: (value: boolean) => void = () => {};
  let text = "", expectedHash = "", trade: EverAgreement | null = null, hash = "";
  let review: EverReview | null = null, operation: EverOperation | null = null;
  let pending: EverPending | null = null, reference = "", error = "", notice = "", working = false, loadedActor = "";
  const agreementKey = "pay3flow.otc.ever-agreement.v1";
  $: say = (en: string, ru: string) => $locale === "ru" ? ru : en;
  $: if (session?.actor !== loadedActor && typeof window !== "undefined") {
    loadedActor = session?.actor ?? ""; operation = null; pending = null; reference = "";
    if (loadedActor) {
      try { pending = JSON.parse(localStorage.getItem(pendingStorageKey(loadedActor)) || "null"); reference = pending?.reference ?? ""; }
      catch { error = say("Recovery data could not be read. Verify the original call before signing.", "Не удалось прочитать данные восстановления. Проверьте исходную операцию перед подписью."); }
    }
  }
  async function run(task: () => Promise<void>) {
    if (working || busy) return;
    working = true; onBusyChange(true); error = ""; notice = "";
    try { await task(); } catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { working = false; onBusyChange(false); }
  }
  function savePending(actor: string, value: EverPending | null) {
    if (value) localStorage.setItem(pendingStorageKey(actor), JSON.stringify(value));
    else localStorage.removeItem(pendingStorageKey(actor));
    if (session?.actor === actor) pending = value;
  }
  function edited() { trade = null; review = null; operation = null; hash = ""; error = ""; }
  async function importAgreement() {
    const parsed = parseAgreement(text), calculated = await agreementHash(parsed);
    if (!/^[0-9a-f]{64}$/i.test(expectedHash.trim()) || calculated !== expectedHash.trim().toLowerCase()) throw new Error(say("Terms hash differs from the agreement reviewed with the other parties.", "Хеш не совпадает с соглашением, проверенным с другими участниками."));
    trade = parsed; hash = calculated; review = null; operation = null;
    localStorage.setItem(agreementKey, JSON.stringify({ trade, hash }));
  }
  async function verify() {
    if (!session || !trade) return;
    const owner = session, agreement = trade, expected = hash;
    const result = await request<EverReview>("/multisig/review", owner.token, { trade: agreement, terms_hash: expected });
    if (session?.token !== owner.token || trade !== agreement || hash !== expected) return;
    if (!result.verified || result.terms_hash !== expected || await agreementHash(parseAgreement(JSON.stringify(result.trade))) !== expected) throw new Error("Verifier returned a different agreement.");
    review = result; operation = null;
    if (result.handoff && !pending) savePending(owner.actor, { id: result.handoff.id, terms_hash: result.handoff.terms_hash, reference: null, phase: "signing" });
  }
  async function plan(action: "release" | "refund", transactionId: string | null = null) {
    if (!session || !trade || pending) return;
    const owner = session;
    await verify();
    if (session?.token !== owner.token || !review || review.handoff) return;
    const wallet = await connectEverscale();
    const state = await wallet.provider.request({ method: "getProviderState" });
    const publicKey = state.permissions.accountInteraction?.publicKey?.toLowerCase();
    if (session?.token !== owner.token) return;
    if (state.selectedConnection !== "mainnet" || !publicKey || !review.owners.some(item => item.public_key === publicKey)) throw new Error(say("Connect a buyer, seller or arbitrator EVER Wallet.", "Подключите EVER Wallet покупателя, продавца или арбитра."));
    operation = { public_key: publicKey, action, transaction_id: transactionId };
  }
  async function sign() {
    if (!session || !trade || (!operation && !pending?.request) || (pending && pending.phase !== "preparing")) return;
    const owner = session;
    const body = pending?.request ?? { trade, terms_hash: hash, operation: operation! };
    if (body.terms_hash !== hash) throw new Error("Recover the original agreement before continuing.");
    const wallet = await connectEverscale();
    const state = await wallet.provider.request({ method: "getProviderState" });
    if (session?.token !== owner.token || state.selectedConnection !== "mainnet" || state.permissions.accountInteraction?.publicKey?.toLowerCase() !== body.operation.public_key) throw new Error("Wallet account or network changed. Review again.");
    const marker: EverPending = pending ?? { id: "", terms_hash: hash, reference: null, phase: "preparing", prepare_key: crypto.randomUUID(), request: body };
    if (!marker.prepare_key) throw new Error("Missing original preparation key. Another send is blocked.");
    savePending(owner.actor, marker);
    const prepared = await request<EverHandoff>("/multisig/prepare", owner.token, body, marker.prepare_key);
    if (session?.token !== owner.token) return;
    marker.id = prepared.id; savePending(owner.actor, { ...marker });
    if (prepared.state !== "unknown" || prepared.instructions.request.terms_hash !== body.terms_hash || await agreementHash(parseAgreement(JSON.stringify(prepared.instructions.request.trade))) !== body.terms_hash || prepared.instructions.request.operation.public_key !== body.operation.public_key || prepared.instructions.request.operation.action !== body.operation.action || prepared.instructions.request.operation.transaction_id !== body.operation.transaction_id) throw new Error("Original handoff changed or is already reconciled.");
    validateCall(body.trade, body.operation, prepared.instructions.review.call);
    const finalState = await wallet.provider.request({ method: "getProviderState" });
    if (session?.token !== owner.token || finalState.selectedConnection !== "mainnet" || finalState.permissions.accountInteraction?.publicKey?.toLowerCase() !== body.operation.public_key || finalState.permissions.accountInteraction?.address.toLowerCase() !== wallet.address) throw new Error("Wallet account or network changed. Another send is blocked until reviewed.");
    // Durable before invoking the provider. A timeout or rejection never enables a resend.
    marker.phase = "signing"; savePending(owner.actor, { ...marker }); operation = null;
    const result = await wallet.provider.request({ method: "sendExternalMessage", params: prepared.instructions.review.call });
    const txHash = result.transaction.id.hash;
    if (!/^[0-9a-f]{64}$/i.test(txHash)) throw new Error("Wallet returned no valid transaction hash. Reconcile the original call.");
    marker.reference = txHash.toLowerCase(); savePending(owner.actor, { ...marker });
    if (session?.token === owner.token) { reference = marker.reference; await reconcile(); }
  }
  async function reconcile() {
    if (!session || !pending) return;
    const owner = session, marker = pending;
    if (!marker.id && marker.request && marker.prepare_key) {
      const original = await request<EverHandoff>("/multisig/prepare", owner.token, marker.request, marker.prepare_key);
      marker.id = original.id; savePending(owner.actor, { ...marker });
    }
    if (!marker.id) throw new Error("Original handoff ID is unavailable.");
    if (reference.trim()) {
      marker.reference = reference.trim().toLowerCase(); savePending(owner.actor, { ...marker });
      await request(`/multisig/calls/${marker.id}/reference`, owner.token, { transaction_hash: marker.reference });
    }
    const result = await request<{state:string}>(`/multisig/calls/${marker.id}`, owner.token);
    if (session?.token !== owner.token) return;
    if (["accepted", "failed"].includes(result.state)) {
      savePending(owner.actor, null); operation = null; await verify();
      notice = say(`Original wallet call ${result.state}. Recipient delivery is not yet verified.`, `Исходная операция: ${result.state}. Доставка получателю ещё не подтверждена.`);
    } else notice = say("The original call is unresolved. Another send remains blocked.", "Результат исходной операции неизвестен. Повторная отправка заблокирована.");
  }
  onMount(() => {
    try { const saved = JSON.parse(localStorage.getItem(agreementKey) || "null"); if (saved) { text = JSON.stringify(saved.trade, null, 2); expectedHash = saved.hash; } } catch { /* Explicit review is still required. */ }
    const changed = (event: StorageEvent) => { if (session && event.key === pendingStorageKey(session.actor)) { try { pending = JSON.parse(event.newValue || "null"); } catch { /* Backend still blocks unresolved calls. */ } operation = null; } };
    window.addEventListener("storage", changed); return () => window.removeEventListener("storage", changed);
  });
</script>

<details class="everMultisig" data-testid="everscale-multisig">
  <summary>{say("Native EVER OTC multisig", "OTC мультиподпись EVER")} <span>Everscale · 2 / 3</span></summary>
  <div class="content">
    <p>{say("Buyer, seller and arbitrator use their own keys. Any two owners can release EVER to the buyer or refund the seller.", "Покупатель, продавец и арбитр используют свои ключи. Любые два участника могут перевести EVER покупателю или вернуть продавцу.")}</p>
    <p class="muted">{say("Use the public agreement from evernode-rs. Create and deploy the wallet in the CLI, then import it here. There is no automatic expiry, automatic refund or atomic EVER/USDT swap. This workspace does not complete a desk order.", "Используйте публичное соглашение из evernode-rs. Создайте и разверните кошелёк в CLI, затем импортируйте его сюда. Автоматического срока действия, возврата и атомарного обмена EVER/USDT нет. Эта панель не завершает заявку деска.")}</p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    {#if notice}<p role="status">{notice}</p>{/if}
    <label for="ever-agreement">{say("Public OTC agreement JSON", "Публичное OTC соглашение JSON")}</label>
    <textarea id="ever-agreement" rows="5" bind:value={text} on:input={edited} disabled={working || busy} spellcheck="false" placeholder="Public trade.json from evernode-rs"></textarea>
    <label for="ever-terms-hash">{say("Independently reviewed terms hash", "Независимо проверенный хеш условий")}</label>
    <input id="ever-terms-hash" bind:value={expectedHash} on:input={edited} disabled={working || busy} spellcheck="false" autocomplete="off" />
    <button disabled={working || busy || !text || !expectedHash} on:click={() => run(importAgreement)}>{say("Review public agreement", "Проверить публичное соглашение")}</button>
    {#if trade}
      <dl><dt>{say("Agreement", "Соглашение")}</dt><dd>{trade.terms.trade_id}</dd><dt>{say("Multisig wallet", "Мультисиг кошелёк")}</dt><dd>{trade.wallet_address}</dd><dt>{say("Fixed amount", "Фиксированная сумма")}</dt><dd>{decimal(trade.terms.amount, 9)} EVER</dd><dt>{say("Release to buyer", "Перевод покупателю")}</dt><dd>{trade.terms.buyer.address}</dd><dt>{say("Refund to seller", "Возврат продавцу")}</dt><dd>{trade.terms.seller.address}</dd><dt>{say("Buyer public key", "Публичный ключ покупателя")}</dt><dd>{trade.terms.buyer.public_key}</dd><dt>{say("Seller public key", "Публичный ключ продавца")}</dt><dd>{trade.terms.seller.public_key}</dd><dt>{say("Arbitrator public key", "Публичный ключ арбитра")}</dt><dd>{trade.terms.arbitrator}</dd><dt>{say("Terms hash", "Хеш условий")}</dt><dd>{hash}</dd></dl>
      {#if !config?.everscale_multisig_available}<p role="status">{say("Native EVER multisig signing is not configured on this server. Public agreement review is available.", "Подпись мультисиг EVER на сервере не настроена. Проверка публичного соглашения доступна.")}</p>
      {:else if !session}<button disabled={working || busy} on:click={() => run(onConnect)}>{say("Sign in with EVER Wallet", "Войти через EVER Wallet")}</button>
      {:else}<button disabled={working || busy} on:click={() => run(verify)}>{say("Verify on Everscale", "Проверить в Everscale")}</button>{/if}
    {/if}
    {#if review}
      <p>{say("Verified SafeMultisig code, three owners, two approvals and network 42.", "Проверены код SafeMultisig, три владельца, две подписи и сеть 42.")} {say("Balance", "Баланс")}: {decimal(review.balance_nanotokens, 9)} EVER.</p>
      {#if !pending && review.transactions.length === 0}
        <button disabled={working || busy} on:click={() => run(() => plan("release"))}>{say("Review release", "Проверить перевод")}</button>
        <button disabled={working || busy} on:click={() => run(() => plan("refund"))}>{say("Review refund", "Проверить возврат")}</button>
      {/if}
      {#each review.transactions as transaction}
        <div class="transaction"><strong>{transaction.action ?? say("Unrecognized transfer — do not confirm", "Неизвестный перевод — не подтверждайте")}</strong><p>{say("Transaction ID", "ID транзакции")}: {transaction.id}</p><p>{transaction.destination} · {transaction.amount} nanotokens</p>
        {#if transaction.action && !pending}<button disabled={working || busy} on:click={() => run(() => plan(transaction.action!, transaction.id))}>{say("Review second approval", "Проверить вторую подпись")}</button>{/if}</div>
      {/each}
    {/if}
    {#if operation && trade}
      <div class="review"><strong>{operation.transaction_id ? say("Second approval", "Вторая подпись") : say("New proposal", "Новое предложение")} · {operation.action}</strong><p>{decimal(trade.terms.amount, 9)} EVER → {trade.terms[operation.action === "release" ? "buyer" : "seller"].address}</p>{#if operation.transaction_id}<p>ID: {operation.transaction_id}</p>{/if}<p>{say("Network fees are paid from the multisig balance. Two owners using another tool can authorize different spending.", "Комиссии сети оплачиваются из баланса мультисиг кошелька. Два владельца могут разрешить другие переводы через иной инструмент.")}</p><button disabled={working || busy || !!pending} on:click={() => run(sign)}>{say("Confirm in EVER Wallet", "Подтвердить в EVER Wallet")}</button><button disabled={working || busy} on:click={() => operation = null}>{say("Cancel review", "Отменить проверку")}</button></div>
    {/if}
    {#if pending}
      <div class="review"><p role="status">{say("Original handoff pending or unresolved. Another send is blocked.", "Исходная операция ожидается или её результат неизвестен. Повторная отправка заблокирована.")}</p><p class="address">{pending.id || pending.prepare_key}</p>
      {#if pending.phase === "preparing" && pending.request && pending.terms_hash === hash}<button disabled={working || busy || !session || !config?.everscale_multisig_available} on:click={() => run(sign)}>{say("Continue original wallet review", "Продолжить исходное подтверждение")}</button>{/if}
      <label for="ever-original-hash">{say("Original Everscale transaction hash", "Хеш исходной транзакции Everscale")}</label><input id="ever-original-hash" bind:value={reference} spellcheck="false" />
      <button disabled={working || busy || !session || !config?.everscale_multisig_available} on:click={() => run(reconcile)}>{say("Reconcile original call", "Проверить исходную операцию")}</button></div>
    {/if}
  </div>
</details>
<style>
  .everMultisig { margin: 0 0 12px; border: 1px solid var(--color-border); border-radius: 14px; background: var(--color-paper); }
  summary { padding: 16px 20px; cursor: pointer; font-weight: 600; } summary span { display: inline-block; margin-left: 10px; font-size: 12px; color: var(--color-text-soft); }
  .content { padding: 0 20px 20px; max-width: 850px; } p { font-size: 13px; line-height: 1.6; margin: 10px 0; overflow-wrap: anywhere; } .muted { color: var(--color-text-soft); }
  label { display: block; font-size: 13px; margin: 14px 0 6px; } input, textarea { box-sizing: border-box; width: 100%; min-width: 0; padding: 11px; border: 1px solid var(--color-border); border-radius: 8px; background: var(--exchange-field-bg); color: var(--color-text); font: inherit; font-size: 12px; } textarea { resize: vertical; }
  button { min-height: 44px; padding: 10px 15px; margin: 10px 8px 0 0; border: 1px solid var(--color-border); border-radius: 8px; font-size: 13px; } button:disabled { opacity: .45; cursor: not-allowed; }
  dl { display: grid; grid-template-columns: minmax(110px, 1fr) minmax(0, 3fr); gap: 8px 12px; margin: 15px 0; font-size: 12px; } dt { color: var(--color-text-soft); } dd { margin: 0; overflow-wrap: anywhere; font-family: monospace; } .review, .transaction { padding: 14px; margin-top: 15px; border: 1px solid var(--color-border); border-radius: 10px; } .error { color: var(--color-danger, #b34242); }
  @media (max-width: 500px) { .content { padding: 0 12px 14px; } summary { padding: 14px 12px; } dl { grid-template-columns: 1fr; gap: 4px; } dd { margin-bottom: 8px; } button { width: 100%; margin-right: 0; } }
</style>
