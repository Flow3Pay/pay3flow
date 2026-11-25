import assert from "node:assert/strict";
import test from "node:test";
import { balanceInputValue, walletToken, type WalletToken } from "../src/lib/token-catalog-model.ts";

const tokens: WalletToken[] = [
  { network: "ethereum", symbol: "ETH", decimals: 18, contract_address: null },
  { network: "ethereum", symbol: "USDT", decimals: 6, contract_address: "0xdac17f958d2ee523a2206206994597c13d831ec7" },
  { network: "tron", symbol: "USDT", decimals: 6, contract_address: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t" },
  { network: "tron", symbol: "TRX", decimals: 6, contract_address: "" },
];

test("the token catalog resolves the selected network and ticker only", () => {
  assert.equal(walletToken(tokens, "tron", "usdt")?.contract_address, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
  assert.equal(walletToken(tokens, "ethereum", "USDT")?.contract_address, "0xdac17f958d2ee523a2206206994597c13d831ec7");
  assert.equal(walletToken(tokens, "ethereum", "TRX"), null);
  assert.equal(walletToken(tokens, "solana", "USDT"), null);
});

test("a native placeholder never wins over a real contract", () => {
  const ambiguous: WalletToken[] = [
    { network: "tron", symbol: "USDT", decimals: 6, contract_address: null },
    { network: "tron", symbol: "USDT", decimals: 6, contract_address: "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t" },
  ];
  assert.equal(walletToken(ambiguous, "tron", "USDT")?.contract_address, "TR7NHqjeKQxGTCi8q8ZY4pL8otSzgjLj6t");
});

test("balance input values keep the token precision without trailing zeros", () => {
  assert.equal(balanceInputValue("12.500000", 6), "12.5");
  assert.equal(balanceInputValue("0.000000001234567890", 18), "0.00000000123456789");
  assert.equal(balanceInputValue("3507999439.618519000000000000", 6), "3507999439.618519");
  assert.equal(balanceInputValue("0.000000", 6), null);
  assert.equal(balanceInputValue(null, 6), null);
});
