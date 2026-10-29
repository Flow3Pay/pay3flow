import { test, expect } from "@playwright/test";

test.use({ reducedMotion: "reduce" });
test.beforeEach(async({page})=>{await page.addInitScript(()=>localStorage.setItem("pay3flow-locale","en"));});

const cors = { "access-control-allow-origin": "*", "access-control-allow-headers": "*", "access-control-allow-methods": "GET,POST,OPTIONS" };
async function catalog(page: import("@playwright/test").Page) {
  await page.route("**/api/**", route => route.fulfill({headers:cors,json:{}}));
  await page.route("**/api/banks?*", route => route.fulfill({headers:cors,json:{total:4,limit:100,offset:0,items:[
    {id:"one",method_id:"ru-sbp",name:"SBP",display_name:"SBP / СБП",currency:"RUB",country:"RU",role:"both",kind:"bank",color:"#272727",initials:"SBP",popular:true,p2p_query:"SBP",bank_fee_percent:0},
    {id:"two",method_id:"currency-usd",name:"US dollar",display_name:"US dollar",currency:"USD",country:"GLOBAL",role:"both",kind:"currency",color:"#272727",initials:"USD",p2p_query:"USD"},
    {id:"four",method_id:"am-idbank",name:"IDBank",display_name:"IDBank",currency:"AMD",country:"AM",role:"both",kind:"bank",color:"#21a366",initials:"ID",p2p_query:"IDBank",bank_fee_percent:0.75},
    {id:"three",method_id:"currency-rub",name:"Russian ruble",display_name:"Russian ruble",currency:"RUB",country:"RU",role:"both",kind:"currency",color:"#272727",initials:"RUB",p2p_query:"RUB"},
  ]}}));
  await page.route("**/api/networks*",route=>route.fulfill({headers:cors,json:[]}));
  await page.route("**/api/providers*",route=>route.fulfill({headers:cors,json:[]}));
  await page.route("**/api/exchange/corridors*",route=>route.fulfill({headers:cors,json:{items:[{id:"rub-rub",source_currency:"RUB",target_currency:"RUB",source_country:"RU",target_country:"RU"}]}}));
}
function response(overrides = {}) {
  return {searched_at:new Date().toISOString(),search_id:"test",mode:"exchange",source_amount:"1000",reporting_currency:"USD",routes:[],routes_found:0,routes_exhaustive:false,complete:false,coverage:{combinations_total:35,combinations_completed:16,combinations_failed:0,combinations_timed_out:0,batch_completed:16},provider_statuses:[],errors:[],next_cursor:"next-page",...overrides};
}

async function anyExchange(page: import("@playwright/test").Page) {
  await page.goto("/#/swap/ANY/ANY?from=any-ANY&to=any-ANY");
  await expect(page.getByRole("button",{name:/^Select sending bank: Any method/})).toBeVisible();
  await page.getByLabel("Amount to send",{exact:true}).fill("1000");
}
test("Any currency is selected inside Exchange and survives reload without mode buttons",async({page})=>{
  await catalog(page);
  await page.goto("/#/swap/RUB/RUB?from=ru-sbp&to=ru-sbp");
  await expect(page.getByRole("button",{name:/^Select sending bank: SBP/})).toBeVisible();
  const sbpLogo = page.getByRole("button",{name:/^Select sending bank: SBP/}).locator(".methodAvatar img");
  await expect(sbpLogo).toHaveAttribute("src","/icons/assets/sbp.svg");
  await expect.poll(() => sbpLogo.evaluate((img: HTMLImageElement) => img.complete && img.naturalWidth > 0)).toBe(true);
  await expect(page.getByRole("button",{name:"Any / any",exact:true})).toHaveCount(0);
  await expect(page.getByRole("button",{name:"Arbitrage",exact:true})).toHaveCount(0);
  await page.getByRole("button",{name:/^Select sending bank:/}).click();
  await page.getByRole("option",{name:/^Any currency/}).click();
  await page.getByRole("button",{name:/^Select recipient bank:/}).click();
  await page.getByRole("option",{name:/^Any currency/}).click();
  await expect(page.getByRole("button",{name:"Select sending currency: ANY",exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"Select recipient currency: ANY",exact:true})).toBeVisible();
  await expect(page).toHaveURL(/#\/swap\/ANY\/ANY/);
  await page.reload();
  await expect(page.getByRole("button",{name:"Select sending currency: ANY",exact:true})).toBeVisible();
  await expect(page.getByRole("button",{name:"Select recipient currency: ANY",exact:true})).toBeVisible();
});
test("existing currency picker supports any method while preserving explicit SBP on the other side",async({page})=>{
  await catalog(page);
  const requests:Record<string,unknown>[]=[];
  await page.routeWebSocket("**/ws/p2p/discovery",ws=>ws.onMessage(data=>{requests.push(JSON.parse(String(data)));ws.send(JSON.stringify({type:"search_finished",response:response({mode:"exchange",complete:true,next_cursor:null})}));}));
  await page.goto("/#/swap/RUB/RUB?from=ru-sbp&to=ru-sbp");
  await page.getByRole("button",{name:"Select sending currency: RUB",exact:true}).click();
  await page.getByRole("option",{name:/^USD /}).click();
  await expect(page.getByRole("button",{name:/^Select sending bank: Any method/})).toBeVisible();
  await expect(page.getByRole("button",{name:/^Select recipient bank: SBP/})).toBeVisible();
  await page.getByLabel("Amount to send",{exact:true}).fill("1000");
  await page.getByTestId("start-search").click();
  await expect.poll(()=>requests.length).toBeGreaterThan(0);
  expect(requests[0]).toMatchObject({mode:"exchange",source_fiat:"USD",target_fiat:"RUB",target_payment_method:"ru-sbp",source_amount:"1000"});
  expect(requests[0].source_payment_method).toBeUndefined();
});
function quote(id: string, targetCurrency: string, output: string, valueRatio: number) {
  const endpoint = (currency: string) => ({currency,network:null,method_id:currency === "RUB" ? "ru-sbp" : currency === "AMD" ? "am-idbank" : "currency-usd",method:currency === "RUB" ? "SBP / СБП" : currency === "AMD" ? "IDBank" : "US dollar",venue:"binance",verified:true});
  return {route_id:id,asset:"USDT",entry_network:null,source_fiat:"RUB",source_amount:"1000",target_fiat:targetCurrency,target_amount:output,acquired_asset_amount:"10",route_kind:"fiat_to_fiat",route_provider:"binance",route_provider_url:"https://p2p.binance.com",route_path:["RUB","USDT",targetCurrency],source_settlement:endpoint("RUB"),target_settlement:endpoint(targetCurrency),value_received_per_input:valueRatio,missing_costs:[],warnings:[],observed_at:new Date().toISOString(),quote_expires_at:new Date(Date.now()+120000).toISOString(),payment_methods_verified:true};
}
test("wildcards reuse Exchange cards, normalized ranking, refresh and instructions",async({page})=>{
  await catalog(page);
  const requests:Record<string,unknown>[]=[];
  await page.routeWebSocket("**/ws/p2p/discovery",ws=>ws.onMessage(data=>{
    const request=JSON.parse(String(data));requests.push(request);
    ws.send(JSON.stringify({type:"search_finished",response:response({routes:[quote("best-usd","USD","12",1.1),quote("worse-rub","RUB","99999",1)],routes_found:2,...(request.cursor ? {complete:true,next_cursor:null} : {})})}));
  }));
  await anyExchange(page);
  await page.getByTestId("start-search").click();
  const results=page.getByRole("complementary",{name:"Found routes"});
  await expect(results.getByTestId("complete-route")).toHaveCount(2);
  await expect(results.getByTestId("complete-route").first()).toContainText("12 USD");
  await expect(results.getByTestId("complete-route").nth(1)).toContainText("9.09% less");
  await expect(results.getByTestId("best-route-badge")).toHaveCount(1);
  await expect(results.getByRole("button",{name:"Like this route",exact:true})).toHaveCount(2);
  await expect(page.getByLabel("Amount to receive",{exact:true})).toHaveValue("12");
  await expect(page.getByLabel("Amount to receive",{exact:true})).toBeEnabled();
  await expect(page.getByRole("button",{name:"Continue search",exact:true})).toHaveCount(0);
  expect(requests[0]).toMatchObject({mode:"exchange",source_fiat:"ANY",target_fiat:"ANY",source_amount:"1000"});
  await page.getByRole("button",{name:"Refresh routes now",exact:true}).click();
  await expect.poll(()=>requests.length).toBe(2);
  expect(requests[1].cursor).toBe("next-page");
  await expect(results.getByTestId("complete-route")).toHaveCount(2);
  await page.getByLabel("Amount to send",{exact:true}).fill("0.00000001");
  await page.getByTestId("start-search").click();
  await expect.poll(()=>requests.length).toBe(3);
  expect(requests[2].source_amount).toBe("0.00000001");
  expect(requests[2].cursor).toBeUndefined();
  await page.waitForTimeout(800);
  expect(requests).toHaveLength(3);
  await page.getByTestId("start-search").click();
  await expect(page).toHaveURL(/#\/guide\/RUB\/USD/);
  const params=new URLSearchParams(page.url().split("?")[1]);
  expect(params.get("from")).toBe("ru-sbp");
  expect(params.get("to")).toBe("currency-usd");
});
test("wildcards reuse the existing loading and empty states",async({page})=>{
  await catalog(page);
  let finish: (()=>void) | undefined;
  await page.routeWebSocket("**/ws/p2p/discovery",ws=>ws.onMessage(()=>{finish=()=>ws.send(JSON.stringify({type:"search_finished",response:response({complete:true,next_cursor:null})}));}));
  await anyExchange(page);
  await page.getByTestId("start-search").click();
  const results=page.getByRole("complementary",{name:"Found routes"});
  await expect(results).toHaveAttribute("aria-busy","true");
  await expect.poll(()=>Boolean(finish)).toBe(true);
  finish!();
  await expect(results).toHaveAttribute("aria-busy","false");
  await expect(results.getByText("No routes found",{exact:true})).toBeVisible();
  await expect(results.getByText("No compatible live offers were found for this amount and payment method.")).toBeVisible();
});
test("failed wildcard search shows a retryable error inside Exchange",async({page})=>{
  await catalog(page);
  await page.routeWebSocket("**/ws/p2p/discovery",ws=>ws.onMessage(()=>ws.send(JSON.stringify({type:"search_failed",error:"Selected method is unavailable"}))));
  await page.route("**/api/p2p/discovery?*",route=>route.fulfill({headers:cors,status:400,body:"Selected method is unavailable"}));
  await anyExchange(page);
  await page.getByTestId("start-search").click();
  await expect(page.getByRole("alert").filter({hasText:"Selected method is unavailable"})).toBeVisible();
  await expect(page.getByTestId("start-search")).toBeEnabled();
});

test("an empty timed-out batch stays incomplete and refresh resumes it", async ({page}) => {
  await catalog(page);
  const requests:Record<string,unknown>[]=[];
  await page.routeWebSocket("**/ws/p2p/discovery", ws=>ws.onMessage(data=>{
    const request=JSON.parse(String(data));requests.push(request);
    ws.send(JSON.stringify({type:"search_finished",response:request.cursor ? response({complete:false,routes:[quote("idbank-estimate","USD","12",1.1)],routes_found:1}) : response({complete:true,coverage:{combinations_total:1,combinations_completed:1,combinations_partial:1,combinations_timed_out:1,combinations_failed:0,batch_completed:0}})}));
  }));
  await anyExchange(page);
  await page.getByTestId("start-search").click();
  const results=page.getByRole("complementary",{name:"Found routes"});
  await expect(results.getByText("Search incomplete",{exact:true})).toBeVisible();
  await expect(results.getByText("No routes found",{exact:true})).toHaveCount(0);
  await page.getByRole("button",{name:"Refresh routes now",exact:true}).click();
  await expect(results.getByTestId("complete-route")).toHaveCount(1);
  expect(requests[1].cursor).toBe("next-page");
});

test("the shared ANY to AMD IDBank search displays an unverified direct estimate in Exchange",async({page})=>{
  await catalog(page);
  const requests:Record<string,unknown>[]=[];
  await page.routeWebSocket("**/ws/p2p/discovery",ws=>ws.onMessage(data=>{
    requests.push(JSON.parse(String(data)));
    const route=quote("idpay-rub-amd","AMD","4290",1.01);
    route.target_settlement.verified=false;
    ws.send(JSON.stringify({type:"search_finished",response:response({routes:[{...route,asset:"AMD",route_provider:"id-pay",route_path:["RUB","AMD"],payment_methods_verified:false,warnings:["Provider has not verified the selected IDBank payment method; confirm recipient eligibility before sending."]}],routes_found:1})}));
  }));
  await page.goto("/#/swap/ANY/AMD?amount=1000&from=any-ANY&to=am-idbank");
  await expect(page.getByRole("button",{name:"Select recipient bank: IDBank",exact:true})).toBeVisible();
  await page.getByLabel("Amount to send",{exact:true}).fill("1000");
  await page.getByTestId("start-search").click();
  const card=page.getByTestId("complete-route").first();
  await expect(card).toContainText("4290 AMD");
  expect(requests[0]).toMatchObject({source_fiat:"ANY",target_fiat:"AMD",source_amount:"1000",target_payment_method:"am-idbank"});
  await expect(page.getByLabel("Amount to receive",{exact:true})).toHaveValue("4290");
  await page.getByTestId("start-search").click();
  await expect(page).toHaveURL(/#\/guide\/RUB\/AMD/);
  expect(new URLSearchParams(page.url().split("?")[1]).get("to")).toBe("am-idbank");
});
