import { test } from "node:test";
import assert from "node:assert/strict";
import { decimalToAtomic, atomicToDecimal, sameWalletAddress } from "../src/lib/wallet-amount.ts";
import { isTronAddress } from "../src/lib/wallet-address.ts";
import { isEverscaleAddress } from "../src/lib/everscale-wallet.ts";

test("token amounts preserve integers beyond JavaScript number precision", () => {
  assert.equal(decimalToAtomic("123456789012345678.123456", 6), 123456789012345678123456n);
  assert.equal(decimalToAtomic("0.000000000000000000000001", 24), 1n);
  assert.equal(atomicToDecimal("123456789012345678123456", 6), "123456789012345678.123456");
  assert.equal(atomicToDecimal("120", 0), "120");
  assert.equal(atomicToDecimal("00100", 2), "1");
});

test("invalid amounts and excess precision cannot silently change the transfer", () => {
  for (const value of ["-1", "1e6", "1.000001", "NaN", "1.2.3", "", ".5"]) assert.throws(() => decimalToAtomic(value, 3), value);
  assert.equal(decimalToAtomic("1.23000", 2), 123n);
  assert.throws(() => decimalToAtomic("1", -1));
});

test("TRON validates the checksum, not just the shape", async () => {
  assert.equal(await isTronAddress("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t"), true);
  assert.equal(await isTronAddress("TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6u"), false);
  assert.equal(await isTronAddress("0x0000000000000000000000000000000000000001"), false);
});

test("hex wallet identities ignore letter case while NEAR and TRON do not", () => {
  assert.equal(sameWalletAddress("evm", "0xAbC", "0xabc"), true);
  assert.equal(sameWalletAddress("everscale", `0:${"A".repeat(64)}`, `0:${"a".repeat(64)}`), true);
  assert.equal(sameWalletAddress("tron", "TR7N", "Tr7n"), false);
  assert.equal(sameWalletAddress("near", "alice.near", "ALICE.NEAR"), false);
});

test("Everscale addresses require a supported workchain and a complete hash", () => {
  for (const workchain of ["0", "-1"]) assert.equal(isEverscaleAddress(`${workchain}:${"A".repeat(64)}`), true);
  for (const address of ["0:broken", `0:${"a".repeat(63)}`, `0:${"g".repeat(64)}`, `2:${"a".repeat(64)}`, `0x${"a".repeat(40)}`, "alice.near"]) assert.equal(isEverscaleAddress(address), false, address);
});
