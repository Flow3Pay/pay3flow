import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { decodeFunctionData, keccak256, zeroAddress } from "viem";
import { SAFE_DEPLOYMENTS, SAFE_SINGLETON, SAFE_FALLBACK, safeOwners, safeInitializer, safeDeployment, safeAppUrl, safeAbi, factoryAbi, safeNetwork, safePlanStorageKey, loadSafePlan, storeSafePlan, type SafePlan, type SafeChainId } from "../src/lib/otc/production-safe.ts";
const owners = ["0x1111111111111111111111111111111111111111", "0x2222222222222222222222222222222222222222", "0x3333333333333333333333333333333333333333"];
test("browser fixtures preserve the official Safe 1.4.1 runtime hashes", () => {
  const fixture = JSON.parse(readFileSync(new URL("./fixtures/safe-1.4.1.json", import.meta.url), "utf8"));
  assert.equal(fixture.chainId, 11155111);
  for (const [address, hash] of SAFE_DEPLOYMENTS) assert.equal(keccak256(fixture.contracts[address.toLowerCase()]), hash);
});
test("OTC Safe requires three independent nonzero owners", () => {
  assert.equal(safeOwners(owners).length, 3);
  for (const invalid of [owners.slice(0, 2), [...owners, owners[0]], [owners[0], owners[0], owners[2]], [zeroAddress, owners[1], owners[2]], ["invalid", owners[1], owners[2]]]) assert.throws(() => safeOwners(invalid));
});
test("production Safe setup has two approvals and no delegatecall or payment", () => {
  const setup = decodeFunctionData({ abi: safeAbi, data: safeInitializer(owners) });
  assert.equal(setup.functionName, "setup");
  assert.deepEqual(setup.args, [owners, 2n, zeroAddress, "0x", SAFE_FALLBACK, zeroAddress, 0n, zeroAddress]);
});
test("factory call binds reviewed owners, singleton and unique creation salt", () => {
  const first = safeDeployment(owners, 1n, "0x60006000");
  const call = decodeFunctionData({ abi: factoryAbi, data: first.data });
  assert.deepEqual(call.args, [SAFE_SINGLETON, safeInitializer(owners), 1n]);
  assert.notEqual(first.address, safeDeployment(owners, 2n, "0x60006000").address);
  assert.notEqual(first.address, safeDeployment([...owners].reverse(), 1n, "0x60006000").address);
  assert.throws(() => safeDeployment(owners, -1n, "0x60006000"));
  assert.throws(() => safeDeployment(owners, 2n ** 256n, "0x60006000"));
});
test("Safe links require an explicit supported network and reject arbitrary destinations", () => {
  assert.equal(safeAppUrl(owners[0], 1), `https://app.safe.global/home?safe=eth:${owners[0]}`);
  assert.equal(safeAppUrl(owners[0], 11155111), `https://app.safe.global/home?safe=sep:${owners[0]}`);
  for (const chainId of [1, 11155111] as const) {
    assert.throws(() => safeAppUrl("javascript:alert(1)", chainId));
    assert.throws(() => safeAppUrl(zeroAddress, chainId));
  }
  assert.throws(() => safeAppUrl(owners[0], 100 as SafeChainId));
  assert.throws(() => safeAppUrl(owners[0], "__proto__" as unknown as SafeChainId));
  assert.throws(() => safeAppUrl(owners[0], undefined as unknown as SafeChainId));
});
test("Safe network settings bind RPC, explorer, wallet network and chain", () => {
  for (const [chainId, rpc, explorer, prefix, wallet] of [
    [1, "https://ethereum-rpc.publicnode.com", "https://etherscan.io", "eth", "ethereum"],
    [11155111, "https://ethereum-sepolia-rpc.publicnode.com", "https://sepolia.etherscan.io", "sep", "sepolia"],
  ] as const) {
    const config = safeNetwork(chainId);
    assert.equal(config.chain.id, chainId);
    assert.equal(config.rpcUrl, rpc);
    assert.equal(config.explorerUrl, explorer);
    assert.equal(config.safePrefix, prefix);
    assert.equal(config.walletNetwork, wallet);
  }
  assert.equal(safeNetwork(1).faucetUrl, null);
  assert.equal(new URL(safeNetwork(11155111).faucetUrl!).protocol, "https:");
});

function storage() {
  const values = new Map<string, string>();
  return { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); }, removeItem: (key: string) => { values.delete(key); } };
}
const legacyKey = "pay3flow.otc.production-safe.v1";
function plan(chainId: SafeChainId, state: SafePlan["state"] = "submitted"): SafePlan {
  return { chainId, owners: safeOwners(owners), salt: "1", address: safeDeployment(owners, 1n, "0x60006000").address, reference: `0x${"ab".repeat(32)}`, state };
}
for (const state of ["review", "unknown", "submitted", "ready", "failed"] as const) test(`legacy ${state} plan migrates to Mainnet without losing transaction or state`, () => {
  const db = storage(), { chainId, ...legacy } = plan(1, state);
  db.setItem(legacyKey, JSON.stringify(legacy));
  assert.equal(loadSafePlan(db, 11155111), null);
  assert.notEqual(db.getItem(legacyKey), null);
  assert.deepEqual(loadSafePlan(db, 1), { chainId, ...legacy });
  assert.equal(db.getItem(legacyKey), null);
  assert.deepEqual(loadSafePlan(db, 1), { chainId, ...legacy });
});
test("separate network records retain pending operations even at identical Safe addresses", () => {
  const db = storage(), mainnet = plan(1, "unknown"), sepolia = plan(11155111);
  storeSafePlan(db, mainnet); storeSafePlan(db, sepolia);
  assert.deepEqual(loadSafePlan(db, 1), mainnet);
  assert.deepEqual(loadSafePlan(db, 11155111), sepolia);
  db.removeItem(safePlanStorageKey(11155111));
  assert.equal(loadSafePlan(db, 11155111), null);
  assert.deepEqual(loadSafePlan(db, 1), mainnet);
});
test("migration preserves the original record if persistence fails", () => {
  const db = storage(), { chainId, ...legacy } = plan(1);
  db.setItem(legacyKey, JSON.stringify(legacy));
  assert.throws(() => loadSafePlan({ ...db, setItem: () => { throw new Error("Storage unavailable"); } }, 1));
  assert.equal(db.getItem(legacyKey), JSON.stringify(legacy));
});
test("invalid or mismatched network records are rejected without discarding pending evidence", () => {
  for (const invalid of [
    { ...plan(1), chainId: 11155111 }, { ...plan(1), chainId: undefined }, { ...plan(1), chainId: null },
    { ...plan(1), salt: "-1" }, { ...plan(1), salt: (2n ** 256n).toString() },
    { ...plan(1), reference: "0x1234" }, { ...plan(1), reference: null },
    { ...plan(1), owners: [owners[0], owners[0], owners[1]] }, { ...plan(1), state: "other" },
  ]) {
    const db = storage(), raw = JSON.stringify(invalid);
    db.setItem(safePlanStorageKey(1), raw);
    assert.throws(() => loadSafePlan(db, 1));
    assert.equal(db.getItem(safePlanStorageKey(1)), raw);
  }
});
