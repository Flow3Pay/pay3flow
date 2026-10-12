import { test, expect } from "@playwright/test";
import type { RouteCandidate } from "../src/lib/exchange";
import { buildRouteTutorial } from "../src/lib/route-tutorial";
import { routeWorkflowSteps } from "../src/lib/workflow-steps";

const route: RouteCandidate = {
  route_id: "fiat-spot", status: "complete", route_kind: "fiat_to_crypto",
  source_currency: "UAH", source_amount: "10000", source_amount_minor: 1000000,
  acquired_asset_amount: "250", entry_asset: "USDT", entry_network: "internal",
  target_currency: "SUI", target_network: "sui", target_amount: "124.875",
  spread_bps: 0, route_path: ["UAH", "USDT", "SUI@sui"],
  market_path: { venue: "bybit", source_pair: "SUIUSDT", target_pair: "SUIUSDT", source_rate: "0.5", target_rate: "1", intermediary_amount: "124.875" },
  entry_offer_snapshot: {
    source: "bybit", ad_id: "listing", fiat: "UAH", asset: "USDT", price: "40",
    available_asset: "1000", min_fiat: "100", max_fiat: "100000",
    payment_methods: ["Monobank"], pay_time_limit_minutes: 15,
    advertiser: { id: "seller", nickname: "Seller", user_type: "merchant", is_merchant: true, is_verified: true, completed_orders_30d: 100, completion_rate_30d: 99 },
    source_url: "https://www.bybit.com/fiat/trade/otc/",
  },
  legs: [
    { kind: "entry", from: "UAH", to: "USDT", provider: "bybit", status: "found" },
    { kind: "exit", from: "USDT", to: "SUI", provider: "bybit", status: "found" },
  ],
};
const copy = (key: string, params: Record<string, string | number> = {}) => key.replace(/\{(\w+)\}/g, (match, name) => String(params[name] ?? match));

test("fiat plus Spot guide buys the intermediate asset before spending that balance", () => {
  const steps = buildRouteTutorial(route, { bybit: "Bybit" }, {}, { sui: "Sui" }, copy);
  expect(steps.map(step => [step.id, step.kind, step.from, step.to])).toEqual([
    ["entry", "buy", "UAH", "USDT"], ["market_source", "swap", "USDT", "SUI"],
  ]);
  expect(steps[0].amount).toBe("10,000 UAH");
  expect(steps[1].amount).toBe("250 USDT");
  expect(steps[1].url).toBe("https://www.bybit.com/trade/spot/SUI/USDT");
  expect(steps[1].targetNetwork).toBe("Sui");
  expect(steps[1].frames.some(frame => frame.text.includes("250 USDT"))).toBe(true);
  expect(routeWorkflowSteps(route)).toEqual([
    { currency: "UAH", network: undefined, provider: undefined, paymentMethod: undefined, paymentMethodIcon: undefined },
    { currency: "USDT", network: undefined, provider: "bybit" },
    { currency: "SUI", network: "sui", provider: "bybit" },
  ]);
});

test("a pure crypto Spot guide continues to spend the original source amount", () => {
  const cryptoRoute: RouteCandidate = { ...route, route_kind: "crypto_to_crypto", source_currency: "USDT", source_amount: "300", source_amount_minor: 30000, entry_offer_snapshot: undefined, route_path: ["USDT", "SUI@sui"] };
  const steps = buildRouteTutorial(cryptoRoute, {}, {}, {}, copy);
  expect(steps).toHaveLength(1);
  expect(steps[0].amount).toBe("300 USDT");
  expect(steps[0].from).toBe("USDT");
});
