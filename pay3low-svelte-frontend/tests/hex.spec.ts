import {test,expect,type Page} from "@playwright/test";
import {GNK,USDT,PAIR} from "../src/lib/hex/model";

// Wallet actions and upstream responses are fixtures. No real account signs.
const owner="alice.near",hash="a".repeat(64);
async function fixture(page:Page) {
  const control={outage:false,historyOutage:false,expired:false,quotes:[] as Record<string,string>[],enabled:true};
  await page.emulateMedia({reducedMotion:"reduce"});
  page.on("pageerror",error=>console.error("HEX browser error:",error.message));
  await page.addInitScript(()=>{localStorage.setItem("pay3flow-locale","en");});
  await page.route("**/src/lib/hex/wallet.ts*",route=>route.fulfill({contentType:"text/javascript",body:`
    const state = window.__hexTest = {calls:[],pending:null,reject:false,delayed:false};
    const list=[{account:"alice.near",address:"alice.near",name:"NEAR"},{account:"bob.near",address:"bob.near",name:"NEAR"}];
    let update;
    export const configured=()=>true;
    export const pending=account=>state.pending?.account===account?state.pending:null;
    export async function watch(cb){update=cb;cb(list);return ()=>{};}
    export async function connect(){update(list);return list[0];}
    export async function disconnect(account){update(list.filter(w=>w.account!==account));}
    export async function fund(action,token,account){state.calls.push({kind:action,token,account});}
    export async function profile(){state.calls.push({kind:"profile"});}
    export async function place(account,draft,quote,slippage){
      if(state.reject)throw new Error("Wallet signature rejected");
      state.calls.push({kind:"place",account,draft,slippage});
      if(state.delayed){state.pending={account,signed:{signature:"fixture"},intentHash:"original"};return null;}
      return "fixtureTx";
    }
    export async function cancel(account,hash){state.calls.push({kind:"cancel",account,hash});return "fixtureTx";}
    export async function claim(account){state.calls.push({kind:"claim",account});return "fixtureTx";}
    export async function reconcile(account){state.calls.push({kind:"reconcile",account});state.pending=null;return "fixtureTx";}
  `}));
  await page.route("**/api/hex/**",async route=>{
    const url=new URL(route.request().url()),path=url.pathname.replace("/api/hex","");
    if(control.outage&&path==="/snapshot")return route.fulfill({status:503,json:{error:"HEX unavailable"}});
    if(control.historyOutage&&path==="/history")return route.fulfill({status:503,json:{error:"History unavailable"}});
    let data:unknown;
    if(path==="/config")data={execution_enabled:control.enabled,contract:"orderbook.fi.tg",base_token:GNK,quote_token:USDT};
    else if(path==="/snapshot")data={book:{bids:[{price:"0.00018",order_balance:"18000000",hash,owner:"maker.near"}],asks:[{price:"0.0002",order_balance:"100000000000",hash,owner:"maker.near"}]},fees:{maker_fee_collectors:[],taker_fee_collectors:[{fee:10000,collector:"intents.fi.tg"}]},fetched_at:new Date().toISOString()};
    else if(path==="/account")data={account:url.searchParams.get("account"),balances:["1000000000000","100000000"],pending:{[GNK]:"1000000000"},orders:[{hash,order_balance:"1000000000",order:{side:"sell",price:"0.00025",maker:owner,created_ts:"1778870350271264686",token_pair:PAIR}}]};
    else if(path==="/quote"){
      const draft=route.request().postDataJSON();control.quotes.push(draft);
      data={result:{book_fill_result:{taker_receive_total:draft.side==="buy"?"5000000000":"1000000",taker_unspent:"0",taker_fee_quote:"10000"},taker_remainder:draft.order_type==="limit"?{new_order:draft.side==="buy"?"500000":"500000000"}:null},expires_at:new Date(Date.now()+(control.expired?-1000:60000)).toISOString()};
    }else if(path==="/history")data={transactions:[{type:"sell",timestamp:Math.floor(Date.now()/1000),amount:5,amount_to:1,price:.2,near_trx:"fixtureTx",intents_id:owner,token_id:GNK,token_to:USDT}]};
    else return route.fulfill({status:404,json:{error:"Unknown fixture path"}});
    await route.fulfill({json:data});
  });
  await page.route("**/api/otc/config",route=>route.fulfill({json:{enabled:false,available:false,public_offers_available:false,desk_name:""}}));
  await page.route("**/api/otc/listings",route=>route.fulfill({json:[]}));
  return control;
}
async function open(page:Page,query="") {
  await page.goto(`/#/otc?market=GNK-USDT${query}`);
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state","connected",{timeout:30000});
  await expect(page.locator("#hex-account")).toHaveValue(owner);
}
async function calls(page:Page) {return page.evaluate(()=>(window as any).__hexTest.calls);}

for(const side of ["buy","sell"])for(const type of ["market","limit"])test(`${side} GNK ${type} reviews before asking the wallet`,async({page})=>{
  const control=await fixture(page);await open(page,`&side=${side}&type=${type}`);
  await page.locator("#hex-amount").fill("1,000001");
  if(type==="limit")await page.locator("#hex-price").fill("0.205");
  await page.getByRole("button",{name:"Review order",exact:true}).click();
  const dialog=page.getByRole("dialog");await expect(dialog).toContainText("Protocol fee");
  expect(await calls(page)).toHaveLength(0);
  expect(control.quotes[0]).toMatchObject({side,order_type:type,amount:side==="buy"?"1000001":"1000001000"});
  if(type==="market")await expect(dialog).toContainText("Minimum output");
  else {await expect(dialog).toContainText("Resting remainder");expect(control.quotes[0].price).toBe("0.000205");}
  await dialog.getByRole("button",{name:"Confirm in wallet"}).click();
  await expect(dialog).toHaveCount(0);expect((await calls(page))[0]).toMatchObject({kind:"place",account:owner});
});
test("expired quotes are reviewed again and wallet rejection sends no order",async({page})=>{
  const control=await fixture(page);control.expired=true;await open(page);await page.locator("#hex-amount").fill("1");
  await page.getByRole("button",{name:"Review order",exact:true}).click();
  const dialog=page.getByRole("dialog");await expect(dialog).toContainText("Quote expired");control.expired=false;
  await dialog.getByRole("button",{name:"Confirm in wallet"}).click();
  await expect(dialog).toContainText("quote was refreshed");expect(await calls(page)).toHaveLength(0);expect(control.quotes).toHaveLength(2);
  await page.evaluate(()=>(window as any).__hexTest.reject=true);
  await dialog.getByRole("button",{name:"Confirm in wallet"}).click();await expect(dialog).toContainText("Wallet signature rejected");expect(await calls(page)).toHaveLength(0);
});
test("insufficient funds stop before simulation and wallet switching clears review",async({page})=>{
  const control=await fixture(page);await open(page);await page.locator("#hex-amount").fill("101");
  await page.getByRole("button",{name:"Review order",exact:true}).click();await expect(page.getByRole("alert")).toContainText("Insufficient HEX balance");expect(control.quotes).toHaveLength(0);
  await page.locator("#hex-amount").fill("1");await page.getByRole("button",{name:"Review order",exact:true}).click();
  await page.getByRole("dialog").getByRole("button",{name:"Close",exact:true}).click();await page.locator("#hex-account").selectOption("bob.near");
  await expect(page.getByRole("dialog")).toHaveCount(0);expect(await calls(page)).toHaveLength(0);
});
test("pending submission locks orders and reconciles its original account",async({page})=>{
  await fixture(page);await open(page);await page.evaluate(()=>(window as any).__hexTest.delayed=true);
  await page.locator("#hex-amount").fill("1");await page.getByRole("button",{name:"Review order",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"Confirm in wallet"}).click();
  await expect(page.getByRole("button",{name:"Review order",exact:true})).toBeDisabled();
  await page.getByRole("button",{name:"Check original submission"}).click();
  await expect(page.getByRole("button",{name:"Review order",exact:true})).toBeEnabled();
  expect((await calls(page)).map((c:any)=>c.kind)).toEqual(["place","reconcile"]);
});
test("funding, cancellation, and claiming use selected wallet and explicit confirmation",async({page})=>{
  await fixture(page);await open(page);
  const fundRows=page.locator(".fundRow");await fundRows.nth(0).getByRole("button",{name:"Deposit",exact:true}).click();await fundRows.nth(1).getByRole("button",{name:"Withdraw",exact:true}).click();
  await page.getByRole("button",{name:"Cancel",exact:true}).click();expect((await calls(page)).map((c:any)=>c.kind)).toEqual(["deposit","withdraw"]);
  await page.getByRole("dialog").getByRole("button",{name:"Confirm in wallet"}).click();
  await page.getByRole("button",{name:"Claim pending balance",exact:true}).click();await page.getByRole("dialog").getByRole("button",{name:"Confirm in wallet"}).click();
  expect((await calls(page)).map((c:any)=>c.kind)).toEqual(["deposit","withdraw","cancel","claim"]);
});
test("market, direction and exact input survive reload; EVER navigation still works",async({page})=>{
  await fixture(page);await open(page,"&side=sell&type=limit&amount=1.000000001&price=0.205");
  await page.reload();await expect(page.locator("#hex-amount")).toHaveValue("1.000000001");
  await expect(page.locator("#hex-price")).toHaveValue("0.205");await expect(page.locator("#otc-market")).toHaveValue("GNK-USDT");
  await page.locator("#otc-market").selectOption("EVER-USDT");await expect(page.getByTestId("otc-workspace")).toContainText("EVER");
  await page.locator("#otc-market").selectOption("GNK-USDT");await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-market","GNK-USDT");
  await page.setViewportSize({width:320,height:760});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});
test("unconfigured execution keeps live market visible and orders disabled",async({page})=>{
  const control=await fixture(page);control.enabled=false;await open(page);
  await expect(page.getByText("HEX trading and funding are not enabled",{exact:false})).toBeVisible();
  await expect(page.getByRole("button",{name:"Review order",exact:true})).toBeDisabled();await expect(page.getByTestId("otc-workspace").getByRole("button",{name:"Connect wallet",exact:true})).toBeDisabled();
  await expect(page.locator(".book .level")).toHaveCount(2);
});
test("local HOT wallet bundle loads in the browser with the Gonka connector",async({page})=>{
  await fixture(page);await open(page);
  const exports=await page.evaluate(async()=>{
    const sdk=await import(/* @vite-ignore */ "/vendor/hex-hot-kit.js");
    return {kit:typeof sdk.HotKit,cosmos:typeof sdk.cosmos,connectors:sdk.defaultConnectors.length,gnk:sdk.OmniToken.GONKA};
  });
  expect(exports).toEqual({kit:"function",cosmos:"function",connectors:6,gnk:GNK});
});
test("header wallet control uses the active HEX connector",async({page})=>{
  await fixture(page);await open(page);
  await page.getByRole("banner").getByRole("button",{name:"Wallets (2)",exact:true}).click();
  expect(await calls(page)).toEqual([{kind:"profile"}]);
});
test("market outages block orders while history outages remain explicit",async({page})=>{
  const control=await fixture(page);await page.clock.install();await open(page);
  await page.locator("#hex-amount").fill("1");control.outage=true;control.historyOutage=true;
  await page.clock.fastForward(6000);
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state","unavailable");
  await expect(page.getByRole("button",{name:"Review order",exact:true})).toBeDisabled();
  control.outage=false;await page.clock.fastForward(6000);
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state","connected");
  await expect(page.getByText("Account history is unavailable.",{exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"Review order",exact:true})).toBeEnabled();
});
