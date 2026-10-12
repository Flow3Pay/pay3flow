import { test } from "node:test";
import assert from "node:assert/strict";
import { aggregateTrends, layoutWall, observeRoutes, trendSwapHref } from "../src/lib/trends/model.ts";
import { emptyWallSlots } from "../src/lib/trends/background.ts";
import type { RouteCandidate } from "../src/lib/exchange.ts";

const now = Date.UTC(2026, 9, 10);
function route(overrides: Partial<RouteCandidate> = {}): RouteCandidate {
  return { route_id: "a", status: "complete", source_amount_minor: 10000, source_currency: "RUB", target_currency: "BTC", entry_asset: "BTC", entry_network: "bitcoin", target_network: "bitcoin", spread_bps: 0, legs: [{ kind: "entry", from: "RUB", to: "BTC", provider: "bestchange", status: "found" }], ...overrides };
}

test("search frequency counts a combination once while results count every route", () => {
  const event = observeRoutes([route(), route({ route_id: "b" }), route({ status: "partial" })], now);
  assert.equal(event.blocks.length, 1);
  const [item] = aggregateTrends([event, { ...event, at: now - 1000 }], "1d", "searches", now);
  assert.equal(item.searches, 2);
  assert.equal(item.appearances, 4);
  assert.equal(item.pairs[0].count, 4);
  assert.equal(item.history.reduce((a, b) => a + b, 0), 2);
});

test("a cycle counts repeated visits once and preserves separate networks", () => {
  const event = observeRoutes([route({ route_provider: "near-intents", route_path: ["BTC@bitcoin", "USDT@tron", "BTC@bitcoin", "USDT@ethereum", "BTC@bitcoin"] })], now);
  assert.equal(event.blocks.length, 4);
  assert.equal(event.blocks.find((block) => block.asset === "BTC")?.appearances, 1);
  assert.deepEqual(event.blocks.filter((block) => block.asset === "USDT").map((block) => block.network), ["tron", "ethereum"]);
});

test("periods exclude old and future searches and ranking follows the chosen metric", () => {
  const old = observeRoutes([route()], now - 2 * 86400000);
  const recent = observeRoutes([route({ entry_asset: "USDT", entry_network: "tron", target_currency: "USDT", target_network: "tron" })], now);
  assert.equal(aggregateTrends([old, recent, { ...old, at: now + 1000 }], "1d", "searches", now).length, 1);
  assert.equal(aggregateTrends([old, recent], "1w", "searches", now).length, 2);
  const manyResults = observeRoutes(Array.from({ length: 5 }, () => route()), now);
  const items = aggregateTrends([manyResults, recent, recent], "1d", "searches", now);
  assert.equal(items[0].asset, "USDT");
  assert.equal(aggregateTrends([manyResults, recent, recent], "1d", "appearances", now)[0].asset, "BTC");
  const layout = layoutWall(items, 800);
  assert.ok(layout.every((cell, index) => !index || cell.row >= layout[index - 1].row));
});

test("route links retain canonical network IDs and the observed exchange pair", () => {
  const item = aggregateTrends([observeRoutes([route({ source_currency: "BTC", source_network: "bitcoin", target_currency: "USDT", target_network: "tron" })], now)], "1d", "searches", now).find((item) => item.asset === "USDT")!;
  const href = trendSwapHref(item);
  assert.ok(href.startsWith("/#/swap/BTC/USDT?"));
  assert.ok(href.includes("toNetwork=tron"));
  assert.ok(href.includes("sources=bestchange"));
});

test("source blocks retain their network without inventing a platform", () => {
  const event = observeRoutes([route({ source_currency: "BTC", source_network: "bitcoin", target_currency: "USDT", target_network: "tron" })], now);
  const items = aggregateTrends([event], "1d", "searches", now);
  const source = items.find((item) => item.asset === "BTC")!;
  assert.equal(source.provider, "");
  assert.equal(source.network, "bitcoin");
  assert.equal(items.find((item) => item.asset === "USDT")?.provider, "bestchange");
  const params = new URLSearchParams(trendSwapHref(source).split("?")[1]);
  assert.equal(params.has("sources"), false);
  assert.equal(params.get("fromNetwork"), "bitcoin");
});

test("saved blocks preserve both venue-specific and network-only combinations", () => {
  const items = aggregateTrends([{ at: now, blocks: [
    { asset: "USDT", network: "tron", provider: "", appearances: 1, source: "RUB", target: "USDT" },
    { asset: "USDT", network: "tron", provider: "bybit", appearances: 2, source: "RUB", target: "USDT" },
    { asset: "USDT", network: "tron", provider: "binance", appearances: 3, source: "RUB", target: "USDT" },
  ] }], "1d", "searches", now);
  assert.deepEqual(new Set(items.map((item) => item.provider)), new Set(["", "bybit", "binance"]));
  assert.ok(items.every((item) => item.searches === 1));
  const sizes = Object.fromEntries(items.map((item) => [item.id, item.provider ? 180 : 90]));
  const cells = layoutWall(items, 393, sizes);
  assert.equal(cells.find((cell) => !cell.item.provider)?.width, 90);
  for (const cell of cells) assert.ok(cell.left + cell.width <= 377);
  for (let index = 1; index < cells.length; index++) {
    if (cells[index].row === cells[index - 1].row) assert.ok(cells[index].left >= cells[index - 1].left + cells[index - 1].width);
  }
});


test("empty courses fit occupied widths and fill the available interval completely", () => {
  const [item] = aggregateTrends([observeRoutes([route()], now)], "1d", "searches", now);
  const cells = [{ item, row: 0, left: 100, width: 83, rank: 1 }, { item, row: 0, left: 500, width: 207, rank: 2 }];
  const slots = emptyWallSlots(cells, [83, 207], -500, 1200, -100, 100, 44);
  for (const slot of slots) {
    assert.ok(slot.width > 0);
    assert.equal(slot.height, 44);
    assert.ok(cells.every((cell) => slot.left + slot.width <= cell.left || slot.left >= cell.left + cell.width));
  }
  const gap = slots.filter((slot) => slot.left >= 189 && slot.left < 494);
  assert.equal(gap[0].left, 189);
  assert.equal(gap.at(-1)!.left + gap.at(-1)!.width, 494);
  assert.ok(gap.every((slot, index) => !index || slot.left === gap[index - 1].left + gap[index - 1].width + 6));
});

test("panning does not change the coordinates of existing empty slots", () => {
  const [item] = aggregateTrends([observeRoutes([route()], now)], "1d", "searches", now);
  const cells = [{ item, row: 1, left: 120, width: 91, rank: 1 }];
  const before = emptyWallSlots(cells, [91, 192, 223], -500, 1400, 0, 200, 41);
  const after = emptyWallSlots(cells, [91, 192, 223], -700, 1600, 0, 200, 41);
  assert.ok(before.every((slot) => after.some((other) => other.left === slot.left && other.top === slot.top && other.width === slot.width)));
});
