import { get } from "svelte/store";
import { getAnonymousUserId } from "./anonymous-user";
import { prepareSwapExecution, submitRouteExecution, fetchRouteExecution } from "./exchange";
import { wallets, connectWallet, restoreWallets } from "./wallet-session";
import { walletFamily, validateRecipient, prepareWalletAction } from "./wallet-execution";
import type { SwapInput, SwapService, SwapServiceState } from "./swap-service";

export function createBackendSwapService(input: SwapInput, onState: (state: SwapServiceState) => void): SwapService {
  const sourceNetwork = input.fromAsset.split("@")[1], targetNetwork = input.toAsset.split("@")[1];
  const family = walletFamily(sourceNetwork);
  if (!family) throw new Error("Wallet execution is unavailable for this source network");
  let recipient = "";
  let disposed = false;
  const owner = getAnonymousUserId() ?? crypto.randomUUID();
  const storageKey = `pay3flow.swap-submission.${input.routeId}`;
  const emit = () => { if (!disposed) onState({ address: get(wallets)[family]?.address, ready: !!get(wallets)[family] && !!recipient }); };
  const unsubscribe = wallets.subscribe(emit);
  void restoreWallets();
  return {
    async connect() { await connectWallet(sourceNetwork); emit(); },
    setRecipient(address) { recipient = address.trim(); emit(); },
    async recover() {
      const raw = localStorage.getItem(storageKey);
      if (!raw) return null;
      const saved = JSON.parse(raw) as { id: string; owner: string; reference: string; kind: "transaction_hash" | "order_uid" };
      const execution = await fetchRouteExecution(saved.id, saved.owner);
      if (execution.from_asset !== input.fromAsset || execution.to_asset !== input.toAsset || execution.input_amount !== input.amount) return null;
      recipient = execution.recipient;
      return {
        recipient,
        reference: execution.status === "awaiting_signature" ? undefined : saved.reference,
        prepared: {
          output: execution.expected_output ?? "—", expiresAt: execution.quote_expires_at,
          async submit() {
            await submitRouteExecution(saved.id, saved.owner, saved.reference, saved.kind);
            return { reference: saved.reference };
          },
          async status() { return (await fetchRouteExecution(saved.id, saved.owner)).status; },
        },
      };
    },
    async prepare() {
      const wallet = await connectWallet(sourceNetwork);
      await validateRecipient(targetNetwork, recipient);
      const target = recipient;
      const execution = await prepareSwapExecution({
        ...input, anonymousId: owner, sourceAddress: wallet.address, recipient: target, idempotencyKey: crypto.randomUUID(),
      });
      const action = await prepareWalletAction(execution, wallet);
      let pending: { reference: string; kind: "transaction_hash" | "order_uid" } | null = null;
      return {
        output: action.expectedOutput ?? execution.expected_output ?? "—",
        expiresAt: action.expiresAt ?? execution.quote_expires_at,
        async submit() {
          if (!pending) {
            if (disposed || target !== recipient || get(wallets)[family]?.address !== wallet.address) throw new Error("The wallet or recipient changed. Review a fresh quote.");
            const result = await action.submit();
            if (result.kind === "approval_confirmed") return { reference: "", approval: true };
            pending = result;
            try { localStorage.setItem(storageKey, JSON.stringify({ id: execution.id, owner, ...pending })); } catch { /* Tracking remains available in this session. */ }
          }
          await submitRouteExecution(execution.id, owner, pending.reference, pending.kind);
          return { reference: pending.reference };
        },
        async status() { return (await fetchRouteExecution(execution.id, owner)).status; },
      };
    },
    destroy() { disposed = true; unsubscribe(); },
  };
}
