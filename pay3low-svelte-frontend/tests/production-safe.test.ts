import test from "node:test";
import assert from "node:assert/strict";
import { decodeFunctionData, zeroAddress } from "viem";
import { SAFE_SINGLETON, SAFE_FALLBACK, safeOwners, safeInitializer, safeDeployment, safeAppUrl, safeAbi, factoryAbi } from "../src/lib/otc/production-safe.ts";
const owners = ["0x1111111111111111111111111111111111111111", "0x2222222222222222222222222222222222222222", "0x3333333333333333333333333333333333333333"];
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
test("Safe links are scoped to Ethereum and reject arbitrary destinations", () => {
  assert.equal(safeAppUrl(owners[0]), `https://app.safe.global/home?safe=eth:${owners[0]}`);
  assert.throws(() => safeAppUrl("javascript:alert(1)"));
  assert.throws(() => safeAppUrl(zeroAddress));
});
