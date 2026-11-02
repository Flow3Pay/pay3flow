import { test } from "node:test";
import assert from "node:assert/strict";
import { exactInput, proposalDirection, completedVolume } from "../src/lib/otc/terminal.ts";
import {offerDecimal,offerTotal,offerEver,saleRate,offerUnits,offerFloored,balanceUnits,offerShortfall,MAX_OFFER_EVER_UNITS,compareOfferRates,marketUsdtPerEver,p2pFeeSplit} from "../src/lib/otc/offer-amount.ts";
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
  for(const v of ['0','-1','NaN','1e3','1,2.3']) assert.equal(offerDecimal(v,6),null);
  assert.equal(offerDecimal('1.0000001',6),'1');
  assert.equal(offerDecimal('118426.07318771596',9),'118426.073187715');
  assert.equal(offerFloored('118426.07318771596',9),true);
  assert.equal(offerFloored('118426.073187715',9),false);
  assert.equal(offerUnits('100001',9)! < MAX_OFFER_EVER_UNITS,true);
  assert.equal(offerUnits('1000000000000',9),MAX_OFFER_EVER_UNITS);
  assert.equal(offerUnits('1000000000001',9)! > MAX_OFFER_EVER_UNITS,true);
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

test('market rate converts USD reference prices into an exact twelve-decimal USDT price',()=>{
  assert.equal(marketUsdtPerEver(0.00105027,0.9998),'0.001050480096');
  assert.equal(marketUsdtPerEver(0.00105027,0.9998)!.split('.')[1].length <= 12,true);
  for(const [ever,usdt] of [[undefined,0.9998],[0,0.9998],[-1,0.9998],[NaN,0.9998],[1e19,0.9998],[0.001,undefined],[0.001,0],[0.001,NaN],[0.001,1e19]] as const)
    assert.equal(marketUsdtPerEver(ever,usdt),null);
  assert.deepEqual(offerTotal('100000',marketUsdtPerEver(0.00105027,0.9998)!),{amount:'105.048009',rounded:true});
});

test('the 3% service fee is split exactly from the gross USDT total and rounds up',()=>{
  assert.deepEqual(p2pFeeSplit('11.889'),{basis:'11.889',fee:'0.35667',net:'11.53233'});
  assert.deepEqual(p2pFeeSplit('100'),{basis:'100',fee:'3',net:'97'});
  assert.deepEqual(p2pFeeSplit('0.000001'),{basis:'0.000001',fee:'0.000001',net:'0'});
  assert.equal(p2pFeeSplit('0'),null);
  assert.equal(p2pFeeSplit(''),null);
});

test('a wallet balance is read exactly and a shortfall stays a warning, never a limit',()=>{
  assert.equal(balanceUnits('0',9),0n);
  assert.equal(balanceUnits('378.357',9),378357000000n);
  assert.equal(balanceUnits(' 15812.49 ',6),15812490000n);
  assert.equal(balanceUnits('12',6),12000000n);
  assert.equal(balanceUnits('1.0000000001',9),1000000000n);
  for(const gap of [null,undefined,'','-1','NaN','1e3','1.','.1','0x10','9'.repeat(81)]) assert.equal(balanceUnits(gap as string|null|undefined,6),null);
  assert.equal(offerShortfall(offerUnits('5',9),balanceUnits('1',9)),true);
  assert.equal(offerShortfall(offerUnits('0.5',9),balanceUnits('0',9)),true);
  assert.equal(offerShortfall(offerUnits('1',9),balanceUnits('1',9)),false);
  assert.equal(offerShortfall(offerUnits('118426.073187715',9),balanceUnits('118426.073187715',9)),false);
  assert.equal(offerShortfall(offerUnits('1',9),null),false);
  assert.equal(offerShortfall(null,balanceUnits('5',9)),false);
});
