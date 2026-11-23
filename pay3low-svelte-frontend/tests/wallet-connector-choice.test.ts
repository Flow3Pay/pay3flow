import assert from "node:assert/strict";
import test from "node:test";
import { selectWalletConnector, walletConnectorId } from "../src/lib/wallet-connector-choice.ts";

test("wallet connector selection auto-selects only an unambiguous choice", () => {
  const only = [{ id: "meteor:0", name: "Meteor" }];
  assert.equal(selectWalletConnector(only), only[0]);
  assert.equal(selectWalletConnector([]), undefined);
  assert.equal(selectWalletConnector([...only, { id: "here:1", name: "Here" }]), undefined);
});

test("wallet connector selection honors an explicit id and rejects stale ids", () => {
  const choices = [
    { id: "meteor:0", name: "Meteor" },
    { id: "here:1", name: "Here" },
  ];
  assert.equal(selectWalletConnector(choices, "here:1"), choices[1]);
  assert.equal(selectWalletConnector(choices, "removed:0"), undefined);
});

test("connector ids are stable from wallet identity rather than display locale", () => {
  assert.equal(walletConnectorId("Trust Wallet", 0), "trust-wallet:0");
  assert.equal(walletConnectorId("Wallet!", 2), "wallet-:2");
  assert.equal(walletConnectorId("   ", 0), "wallet:0");
});
