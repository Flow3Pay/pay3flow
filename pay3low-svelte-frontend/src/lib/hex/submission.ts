import type { Commitment } from "@hot-labs/kit/core";

export interface Pending { account: string; signed: Commitment; intentHash?: string; transaction?: string }
export interface Relay {
  publish(signed: Commitment): Promise<{status: string; reason?: string; intent_hashes?: string[]}>;
  status(hash: string): Promise<{status: string; reason?: string; data?: {hash?: string}}>;
  verifyTransaction(hash: string): Promise<void>;
}
export class ConfirmedFailure extends Error {}
const key = (account: string) => `pay3flow.hex.pending.v1:${account}`;

export async function withAccountLock<T>(locks: Pick<LockManager,"request"> | undefined, account: string, action: () => Promise<T>): Promise<T> {
  if (!locks) throw new Error("HEX submissions require a current browser with Web Locks over HTTPS");
  return locks.request(`pay3flow.hex:${account}`,{ifAvailable:true},lock => {
    if (!lock) throw new Error("Another tab is checking or submitting an operation for this HEX account");
    return action();
  });
}

/** A saved signed nonce is the only operation retried after a lost response. */
export class Submissions {
  private running = new Map<string,Promise<string | null>>();
  private storage: Pick<Storage,"getItem"|"setItem"|"removeItem">;
  private relay: Relay;
  constructor(storage: Pick<Storage,"getItem"|"setItem"|"removeItem">, relay: Relay) { this.storage = storage; this.relay = relay; }
  pending(account: string): Pending | null {
    const value = this.storage.getItem(key(account));
    if (!value) return null;
    try {
      const record = JSON.parse(value) as Pending;
      if (record.account !== account || !record.signed?.signature || !record.signed.standard) throw new Error();
      return record;
    } catch { throw new Error("The saved HEX submission is unreadable. Restore its original reference before trading."); }
  }
  private save(record: Pending) { this.storage.setItem(key(record.account),JSON.stringify(record)); }
  private clear(account: string) { this.storage.removeItem(key(account)); }
  async submit(account: string, sign: () => Promise<Commitment>): Promise<string | null> {
    if (this.running.has(account) || this.pending(account)) throw new Error("Check the original HEX submission first");
    const operation = (async () => {
      const signed = await sign();
      this.save({account,signed}); // Storage failure prevents publication.
      return this.resume(account,true);
    })();
    this.running.set(account,operation);
    try { return await operation; } finally { this.running.delete(account); }
  }
  async reconcile(account: string): Promise<string | null> {
    const running = this.running.get(account);
    if (running) return running;
    const operation = this.resume(account,false);
    this.running.set(account,operation);
    try { return await operation; } finally { this.running.delete(account); }
  }
  private async resume(account: string, fresh: boolean): Promise<string | null> {
    const record = this.pending(account);
    if (!record) return null;
    if (!record.intentHash && !record.transaction) {
      const result = await this.relay.publish(record.signed);
      if (result.status === "FAILED") {
        // A replay rejection alone cannot establish whether the first request settled.
        if (fresh) this.clear(account);
        throw new Error(result.reason || "HEX submission failed; check the original operation");
      }
      record.intentHash = result.intent_hashes?.[0];
      if (!record.intentHash) throw new Error("HEX returned no intent reference. Check the original submission.");
      this.save(record);
    }
    if (!record.transaction) {
      const status = await this.relay.status(record.intentHash!);
      if (status.status === "FAILED") { this.clear(account); throw new Error(status.reason || "HEX intent failed"); }
      if (status.status !== "SETTLED") return null;
      record.transaction = status.data?.hash;
      if (!record.transaction) throw new Error("HEX settlement reference is unavailable");
      this.save(record);
    }
    try { await this.relay.verifyTransaction(record.transaction!); }
    catch (error) { if (error instanceof ConfirmedFailure) this.clear(account); throw error; }
    this.clear(account);
    return record.transaction!;
  }
}
