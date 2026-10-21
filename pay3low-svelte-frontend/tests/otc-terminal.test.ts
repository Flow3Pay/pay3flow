import { test } from "node:test";
import assert from "node:assert/strict";
import { exactInput, proposalDirection, completedVolume } from "../src/lib/otc/terminal.ts";
import {offerDecimal,offerTotal,offerEver,saleRate,validOfferAmount,compareOfferRates} from "../src/lib/otc/offer-amount.ts";
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

test('offer decimal entry normalizes locale punctuation without losing exact precision',()=>{
  assert.equal(offerDecimal(' 001,2500000 ',6),'1.25');
  assert.equal(offerDecimal('.5',9),'0.5');
  assert.equal(offerDecimal('1.',9),'1');
  for(const v of ['0','-1','NaN','1e3','1,2.3','1.0000001']) assert.equal(offerDecimal(v,6),null);
  assert.equal(validOfferAmount('100001',9,100_000_000_000_000n),false);
});
test('entered prices calculate exact executable USDT totals and disclose discarded fractions',()=>{
  assert.deepEqual(offerTotal('100','0,11889'),{amount:'11.889',rounded:false});
  assert.deepEqual(offerTotal('1','0.123456789012'),{amount:'0.123456',rounded:true});
  assert.deepEqual(offerTotal('0.000000001','0.001'),{amount:'0',rounded:true});
  assert.equal(offerTotal('1','0'),null);
  assert.equal(saleRate('100','11,889'),'0.11889');
  assert.equal(saleRate('100','0.11889'),'0.0011889');
});

test('buy offers calculate EVER from the entered USDT budget with exact rounding',()=>{
  assert.deepEqual(offerEver('11,889','0.11889'),{amount:'100',rounded:false});
  assert.deepEqual(offerEver('100','0.11889'),{amount:'841.113634452',rounded:true});
  assert.equal(offerEver('0','0.1'),null);
});


test('book prices sort by exact ratios, including ratios with identical displayed decimals',()=>{
  assert.equal(compareOfferRates('10','2.05','100','20.5'),0);
  assert.equal(compareOfferRates('10','2.05','10','3'),-1);
  assert.equal(compareOfferRates('99999.999999999','0.000001','100000','0.000001'),1);
  assert.deepEqual(offerTotal('10','0.205'),{amount:'2.05',rounded:false});
  assert.equal(saleRate('10','2.05'),'0.205');
});
