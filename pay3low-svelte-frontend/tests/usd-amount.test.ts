import { test } from "node:test";
import assert from "node:assert/strict";
import {
  decimalNumber,
  formatTokenMarketValue,
  formatUsdAmount,
  formatUsdMarketValue,
  sanitizeDecimalInput,
  tokenAmountForUsd,
  usdForTokenAmount,
} from "../src/lib/usd-amount.ts";
import { everFromUsd, marketUsdPerEver, offerTotal } from "../src/lib/otc/offer-amount.ts";

test("USD input keeps one decimal separator and ignores stray characters", () => {
  assert.equal(sanitizeDecimalInput("1 234,56"), "1234.56");
  assert.equal(sanitizeDecimalInput("$1.2.3"), "12.3");
  assert.equal(sanitizeDecimalInput("abc"), "");
  assert.equal(sanitizeDecimalInput("0.5"), "0.5");
  assert.equal(decimalNumber("1,25"), 1.25);
  assert.equal(decimalNumber(""), null);
});

test("token and USD values convert in both directions", () => {
  assert.equal(usdForTokenAmount("100", 0.5), 50);
  assert.equal(usdForTokenAmount("", 0.5), null);
  assert.equal(usdForTokenAmount("100", 0), null);
  assert.equal(tokenAmountForUsd("100", 0.5), "200");
  assert.equal(tokenAmountForUsd("99.85", 1), "99.85");
  assert.equal(tokenAmountForUsd("", 1), "");
});

test("display strings match the money panel slots", () => {
  assert.equal(formatUsdAmount(99.85), "99.85");
  assert.equal(formatUsdAmount(null), "");
  assert.equal(formatUsdMarketValue(0.001), "<$0.01");
  assert.equal(formatUsdMarketValue(99.85), "$99.85");
  assert.equal(formatTokenMarketValue(99.85, "USDT"), "99.85 USDT");
  assert.equal(formatTokenMarketValue(95169.7, "EVER"), "95,169.7 EVER");
  assert.equal(formatTokenMarketValue(null, "EVER"), "");
});

test("OTC USD amount converts into EVER with exact reference prices", () => {
  const usdPerEver = marketUsdPerEver(0.00105027);
  assert.equal(usdPerEver, "0.00105027");
  assert.deepEqual(everFromUsd("100", usdPerEver!), { amount: "95213.611737934", rounded: true });
  assert.equal(everFromUsd("0", usdPerEver!), null);
  assert.equal(marketUsdPerEver(undefined), null);
  assert.deepEqual(offerTotal("95213.611737934", usdPerEver!), { amount: "99.999999", rounded: true });
});
