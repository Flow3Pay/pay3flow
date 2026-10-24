import test from "node:test";
import assert from "node:assert/strict";
import { SAFE_ABI, parseAgreement, agreementHash, validateCall, pendingStorageKey } from "../src/lib/otc/everscale-multisig.ts";

export const agreement = { schema_version: 1, terms: { trade_id: "deal001", network_id: 42,
  buyer: { public_key: "11".repeat(32), address: `0:${"a".repeat(64)}` },
  seller: { public_key: "22".repeat(32), address: `0:${"b".repeat(64)}` },
  arbitrator: "33".repeat(32), deployment_key: "44".repeat(32), amount: "2000000000" },
  wallet_address: `0:${"c".repeat(64)}` };

test("public CLI agreement round trips and hash is canonical", async () => {
  assert.deepEqual(parseAgreement(JSON.stringify(agreement)), agreement);
  const { createHash } = await import("node:crypto");
  const hash = createHash("sha256").update("evernode-otc/v1\0").update(JSON.stringify(agreement)).digest("hex");
  assert.equal(await agreementHash(parseAgreement(JSON.stringify(agreement))), hash);
  assert.equal(await agreementHash(parseAgreement(JSON.stringify({wallet_address:agreement.wallet_address,terms:agreement.terms,schema_version:1}))), hash);
});
test("rejects secret-bearing files, duplicate owners, wrong networks and unsafe amounts", () => {
  for (const modify of [
    (x:any) => x.secret = "never-upload-a-secret",
    (x:any) => x.terms.buyer.secret = "private",
    (x:any) => x.terms.seller.public_key = x.terms.buyer.public_key,
    (x:any) => x.terms.network_id = -1,
    (x:any) => x.terms.amount = 2e9,
    (x:any) => x.terms.amount = "1",
    (x:any) => x.terms.amount = (2n**128n).toString(),
    (x:any) => x.terms.buyer.address = x.wallet_address,
  ]) {
    const value = structuredClone(agreement); modify(value);
    assert.throws(() => parseAgreement(JSON.stringify(value)));
  }
});
test("unsigned call must match independently reviewed release or exact confirmation", () => {
  const trade = parseAgreement(JSON.stringify(agreement));
  const call = {publicKey:trade.terms.buyer.public_key,recipient:trade.wallet_address,
    payload:{abi:SAFE_ABI,method:"submitTransaction",params:{dest:trade.terms.buyer.address,value:trade.terms.amount,bounce:true,allBalance:false,payload:""}}};
  assert.doesNotThrow(() => validateCall(trade, {public_key:call.publicKey,action:"release",transaction_id:null}, call));
  assert.throws(() => validateCall(trade, {public_key:call.publicKey,action:"refund",transaction_id:null}, call));
  assert.throws(() => validateCall(trade, {public_key:call.publicKey,action:"release",transaction_id:null}, {...call,stateInit:"unsafe"}));
  const confirmation = {...call,payload:{...call.payload,method:"confirmTransaction",params:{transactionId:"18446744073709551615"}}};
  assert.doesNotThrow(() => validateCall(trade,{public_key:call.publicKey,action:"refund",transaction_id:"18446744073709551615"},confirmation));
  assert.throws(() => validateCall(trade,{public_key:call.publicKey,action:"refund",transaction_id:"1"},confirmation));
  assert.notEqual(pendingStorageKey("one"),pendingStorageKey("two"));
});
