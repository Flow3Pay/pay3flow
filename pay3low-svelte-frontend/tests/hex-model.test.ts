import assert from "node:assert/strict";
import test from "node:test";
import {marketFromHash} from "../src/lib/otc/model.ts";
import { GNK,USDT,PAIR,units,decimal,contractPrice,humanPrice,makeDraft,levels,minimumOutput,refundedInput,orderMessage,cancelMessage,settledTrades,isQuoteCurrent,type Quote,type HistoryEntry } from "../src/lib/hex/model.ts";

test("GNK and USDT atomic amounts retain precision beyond JavaScript integers", () => {
  assert.equal(units("9007199254.740993001",9),9007199254740993001n);
  assert.equal(decimal(9007199254740993001n,9),"9007199254.740993001");
  assert.equal(makeDraft("buy","market","001,000001","").amount,"1000001");
  assert.equal(makeDraft("sell","market","0.000000001","").amount,"1");
  for (const input of ["1e3","-1","NaN","0","1.0000001","1".repeat(65)]) assert.throws(()=>units(input,6));
});
test("GNK links resolve without introducing a synthetic market price",()=>{
  assert.equal(marketFromHash("#/otc?market=GNK-USDT").base,"GNK");
  assert.equal(marketFromHash("#/otc?market=GNK-USDT").price,0);
  assert.equal(marketFromHash("#/otc").base,"EVER");
});
test("contract price scaling is exact in both directions", () => {
  assert.equal(contractPrice("0.205"),"0.000205");
  assert.equal(humanPrice("0.000204999987873602401745"),"0.204999987873602401745");
  assert.equal(contractPrice(humanPrice("0.000204999987873602401745")),"0.000204999987873602401745");
});
test("book bids hold USDT and asks hold GNK; matching prices aggregate exactly", () => {
  const raw=(balance:string,price="0.0002")=>({price,order_balance:balance,hash:"a".repeat(64)});
  assert.deepEqual(levels([raw("1000000"),raw("3000000")],"buy"),[{price:"0.2",amount:"20",total:"4",depth:4}]);
  assert.deepEqual(levels([raw("5000000000")],"sell"),[{price:"0.2",amount:"5",total:"1",depth:1}]);
  assert.deepEqual(levels([raw("1000000","0.0001"),raw("1000000","0.0002")],"buy").map(l=>l.price),["0.2","0.1"]);
});
const quote: Quote={result:{book_fill_result:{taker_receive_total:"4829751505",taker_unspent:"0",taker_fee_quote:"9901"}},expires_at:"2099-01-01T00:00:00Z"};
test("resting input is not displayed as a simultaneous refund",()=>{
  const resting:Quote={...quote,result:{book_fill_result:{taker_receive_total:"0",taker_unspent:"1000000",taker_fee_quote:"0"},taker_remainder:{new_order:"1000000"}}};
  assert.equal(refundedInput(resting),"0");
  assert.equal(refundedInput({...resting,result:{...resting.result,taker_remainder:null}}),"1000000");
  assert.equal(refundedInput({...resting,result:{...resting.result,taker_remainder:{new_order:"800000",refund:"200000"}}}),"200000");
});
test("signed market payload enforces exact minimum output with no referral fee", () => {
  assert.equal(minimumOutput("4829751505","0.5"),"4805602747");
  assert.equal(minimumOutput("4829751505","0"),"4829751505");
  for (const value of ["100","-1","1e-2"]) assert.throws(()=>minimumOutput("1000",value));
  const msg=orderMessage(makeDraft("buy","market","1",""),quote,"0.5");
  assert.deepEqual(msg,{token_pair:PAIR,order_type:"market",refs:[],min_amount_out:"4805602747"});
  assert.deepEqual(orderMessage(makeDraft("sell","limit","1","0.205"),quote,"0.5"),{token_pair:{base:GNK,quote:USDT},order_type:"limit",refs:[],price:"0.000205"});
});
test("cancellation uses current HEX v1 hash and token pair shape", () => {
  assert.deepEqual(cancelMessage("a".repeat(64)),{action:"close_order",data:{hash:"a".repeat(64),token_pair:PAIR}});
  assert.throws(()=>cancelMessage("other-market/hash"));
});
test("charts exclude order publications and cancellations; quote expiry is strict", () => {
  const entry={timestamp:100,amount:1,amount_to:0.2,price:0.2,near_trx:"hash",intents_id:"alice.near",token_id:GNK,token_to:USDT};
  const data=["create_buy","create_sell","cancel","buy","sell"].map(type=>({...entry,type})) as HistoryEntry[];
  assert.deepEqual(settledTrades(data).map(t=>t.type),["buy","sell"]);
  assert.ok(isQuoteCurrent(quote));
  assert.equal(isQuoteCurrent({...quote,expires_at:"2020-01-01"}),false);
  assert.equal(isQuoteCurrent({...quote,expires_at:"invalid"}),false);
});
