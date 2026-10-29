import test from "node:test";
import assert from "node:assert/strict";
import { asExchangeResponse, mergeDiscoveryRoutes, routeExpired, discoveryLink, type DiscoveryQuery, type DiscoveryResponse, type DiscoveryRoute } from "../src/lib/discovery.ts";
const now = Date.now();
function row(id: string, profit: number | null, overrides: Partial<DiscoveryRoute> = {}): DiscoveryRoute {
  return { route_id:id, net_profit_usd:profit, return_percent:1, value_received_per_input:1.01, missing_costs:[], observed_at:new Date(now).toISOString(), ...overrides } as DiscoveryRoute;
}
test("USD profits rank across different currencies without comparing their nominal outputs", () => {
  const rows = mergeDiscoveryRoutes([], [row("rub",1,{target_amount:"100000"}), row("usd",8,{target_amount:"1008"}), row("unknown",null)],"arbitrage",now);
  assert.deepEqual(rows.map(r=>r.route_id),["usd","rub","unknown"]);
});
test("continuation replaces duplicate routes and removes expired quotes", () => {
  const expired=row("expired",100,{quote_expires_at:new Date(now-1).toISOString()});
  const rows=mergeDiscoveryRoutes([row("one",1),expired],[row("one",4),row("two",2)],"arbitrage",now);
  assert.deepEqual(rows.map(r=>r.route_id),["one","two"]);assert.equal(rows[0].net_profit_usd,4);
  assert.equal(routeExpired(row("old",1,{observed_at:new Date(now-31000).toISOString()}),now),true);
});
test("exchange comparison uses value ratio and keeps incomplete costs behind complete quotes", () => {
  const rows=mergeDiscoveryRoutes([],[row("unknown",null,{value_received_per_input:2,missing_costs:["provider_fee"]}),row("better",null,{value_received_per_input:1.1}),row("worse",null,{value_received_per_input:1})],"exchange",now);
  assert.deepEqual(rows.map(r=>r.route_id),["better","worse","unknown"]);
});
test("shared links preserve selections but omit continuation tokens", () => {
  const link=discoveryLink({mode:"arbitrage",source_fiat:"RUB",target_fiat:"RUB",source_payment_method:"ru-sbp",source_amount:"1234.56",max_actions:4,allow_cross_venue:true,exchange_mode:"all",cursor:"private"});
  assert.match(link,/source_payment_method=ru-sbp/);assert.match(link,/source_amount=1234.56/);assert.doesNotMatch(link,/cursor/);
});

test("wildcard snapshots feed the existing Exchange response without losing route endpoints or ranking", () => {
  const query = {mode:"exchange",source_fiat:"ANY",target_fiat:"ANY"} as DiscoveryQuery;
  const routes = [row("usd",null,{asset:"USDT",target_fiat:"USD",target_amount:"12"}), row("rub",null,{asset:"USDT",target_fiat:"RUB",target_amount:"99999"})];
  const result = asExchangeResponse(query,{search_id:"test",searched_at:"2026-10-29T00:00:00Z",source_amount:"1000",routes_found:2,routes_exhaustive:false,provider_statuses:[]} as unknown as DiscoveryResponse,routes);
  assert.deepEqual(result.routes.map(route => route.route_id),["usd","rub"]);
  assert.deepEqual(result.assets_searched,["USDT"]);
  assert.equal(result.source_fiat,"ANY");
  assert.equal(result.routes[0].target_fiat,"USD");
  assert.equal(result.routes_found,2);
});
