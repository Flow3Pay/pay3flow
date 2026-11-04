import { test } from "node:test";
import assert from "node:assert/strict";
import { canOpenSwap } from "../src/lib/swap-embed.ts";
import type { RouteCandidate } from "../src/lib/exchange";
import type { TutorialStep } from "../src/lib/route-tutorial";

const route = (execution?: RouteCandidate["execution"]) => ({ route_id: "r", execution } as RouteCandidate);
const step = (provider: string, overrides: Partial<TutorialStep> = {}) => ({ kind: "swap", provider, ...overrides } as TutorialStep);

test("exchange guides never open our swap widget", () => {
  assert.equal(canOpenSwap(step("mexc"), route()), false);
  assert.equal(canOpenSwap(step("bybit"), route()), false);
  assert.equal(canOpenSwap(step("binance"), route()), false);
});

test("a swap opens only when the backend attached a matching executable descriptor", () => {
  const execution = { provider: "near-intents", from_asset: "USDT@ethereum", to_asset: "USDC@ethereum", input_amount: "100", expires_at: "2030-01-01T00:00:00Z", token: "signed" };
  assert.equal(canOpenSwap(step("near-intents", { execution: true }), route(execution)), true);
  assert.equal(canOpenSwap(step("near-intents", { execution: true }), route()), false);
  assert.equal(canOpenSwap(step("near-intents", { execution: true }), route({ ...execution, provider: "cow-swap" })), false);
  assert.equal(canOpenSwap(step("cow-swap", { execution: true }), route({ ...execution, provider: "cow-swap" })), true);
});

test("providers and cycles without an own executable leg stay guides", () => {
  assert.equal(canOpenSwap(step("symbiosis"), route()), false);
  assert.equal(canOpenSwap(step("symbiosis", { execution: false }), route({ provider: "symbiosis", from_asset: "USDT@ethereum", to_asset: "USDC@ethereum", input_amount: "1", expires_at: "2030-01-01T00:00:00Z", token: "t" })), false);
  assert.equal(canOpenSwap(step("cow-swap", { kind: "buy", execution: true }), route({ provider: "cow-swap", from_asset: "USDT@ethereum", to_asset: "USDC@ethereum", input_amount: "1", expires_at: "2030-01-01T00:00:00Z", token: "t" })), false);
});

test("the Pay3Flow OTC desk keeps its own executable swap step", () => {
  assert.equal(canOpenSwap(step("pay3flow-otc"), route()), true);
});

test("each executable cycle leg opens against its own descriptor", () => {
  const multi = {
    route_id: "r",
    execution: { provider: "symbiosis", from_asset: "USDT@ethereum", to_asset: "USDC@ethereum", input_amount: "10", expires_at: "2030-01-01T00:00:00Z", token: "a" },
    executions: [
      { provider: "symbiosis", from_asset: "USDT@ethereum", to_asset: "USDC@ethereum", input_amount: "10", expires_at: "2030-01-01T00:00:00Z", token: "a" },
      { provider: "cow-swap", from_asset: "USDC@ethereum", to_asset: "DAI@ethereum", input_amount: "9.9", expires_at: "2030-01-01T00:00:00Z", token: "b" },
    ],
  } as RouteCandidate;
  assert.equal(canOpenSwap(step("symbiosis", { execution: true, fromAsset: "USDT@ethereum" }), multi), true);
  assert.equal(canOpenSwap(step("cow-swap", { execution: true, fromAsset: "USDC@ethereum" }), multi), true);
  assert.equal(canOpenSwap(step("cow-swap", { execution: true, fromAsset: "USDT@ethereum" }), multi), false);
});
