import { test } from "node:test";
import assert from "node:assert/strict";
import { exactInput, proposalDirection, completedVolume } from "../src/lib/otc/terminal.ts";
import type { Config, Trade, Proposal } from "../src/lib/otc/service.ts";

test("exact input respects selected-chain precision, u128 limits and avoids Number loss", () => {
  assert.equal(exactInput("00100.000001", "buy"), "100.000001");
  assert.equal(exactInput("9007199254740993.123456", "buy"), "9007199254740993.123456");
  assert.equal(exactInput("0.000000001", "sell"), "0.000000001");
  for (const value of ["0", "-1", "1e3", " 1", "1.", ".1", "NaN", "1,000", "1.0000001", "340282366920938463463374607431769"]) assert.equal(exactInput(value, "buy"), null);
  assert.equal(exactInput("1.0000000001", "sell"), null);
});
test("desk direction requires both structured asset identities", () => {
  const config = { ever_resource: "ever", usdt_resource: "usdt" } as Config;
  const proposal = { publishes: { resourceConformsTo: "ever" }, reciprocal: { resourceConformsTo: "usdt" } } as Proposal;
  assert.equal(proposalDirection(proposal, config), "buy");
  assert.equal(proposalDirection({ ...proposal, publishes: { resourceConformsTo: "usdt" }, reciprocal: { resourceConformsTo: "ever" } }, config), "sell");
  assert.equal(proposalDirection({ ...proposal, reciprocal: { resourceConformsTo: "other-usdt" } }, config), null);
  assert.equal(proposalDirection(proposal, null), null);
});
test("completed volume uses accepted USDT leg and excludes pending/refunded trades", () => {
  const trade = (state: string, amount: string) => ({ state, terms: { fee_policy: { basis_units: amount } } }) as Trade;
  assert.equal(completedVolume([trade("completed", "9007199254740993"), trade("completed", "7"), trade("funded", "800000000"), trade("refunded", "900000000")]), "9007199254741000");
});
