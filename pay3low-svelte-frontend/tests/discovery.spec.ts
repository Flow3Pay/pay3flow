import { test, expect } from "@playwright/test";

test.use({ reducedMotion: "reduce" });
test.beforeEach(async({page})=>{await page.addInitScript(()=>localStorage.setItem("pay3flow-locale","en"));});

const cors = { "access-control-allow-origin": "*", "access-control-allow-headers": "*", "access-control-allow-methods": "GET,POST,OPTIONS" };
async function catalog(page: import("@playwright/test").Page) {
  await page.route("**/api/**", route => route.fulfill({headers:cors,json:{}}));
  await page.route("**/api/banks?*", route => route.fulfill({headers:cors,json:{total:3,limit:100,offset:0,items:[
    {id:"one",method_id:"ru-sbp",name:"SBP",display_name:"SBP / СБП",currency:"RUB",country:"RU",role:"both",kind:"bank",color:"#272727",initials:"SBP",popular:true,p2p_query:"SBP",bank_fee_percent:0},
    {id:"two",method_id:"currency-usd",name:"US dollar",display_name:"US dollar",currency:"USD",country:"GLOBAL",role:"both",kind:"currency",color:"#272727",initials:"USD",p2p_query:"USD"},
    {id:"three",method_id:"currency-rub",name:"Russian ruble",display_name:"Russian ruble",currency:"RUB",country:"RU",role:"both",kind:"currency",color:"#272727",initials:"RUB",p2p_query:"RUB"},
  ]}}));
  await page.route("**/api/networks*",route=>route.fulfill({headers:cors,json:[]}));
  await page.route("**/api/providers*",route=>route.fulfill({headers:cors,json:[]}));
  await page.route("**/api/exchange/corridors*",route=>route.fulfill({headers:cors,json:{items:[{id:"rub-rub",source_currency:"RUB",target_currency:"RUB",source_country:"RU",target_country:"RU"}]}}));
}
function response(overrides = {}) {
  return {search_id:"test",mode:"arbitrage",source_amount:"1000",reporting_currency:"USD",routes:[],routes_found:0,routes_exhaustive:false,complete:false,coverage:{combinations_total:35,combinations_completed:16,combinations_failed:0,combinations_timed_out:0,batch_completed:16},provider_statuses:[],errors:[],next_cursor:"next-page",...overrides};
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
test("normal Exchange search streams wildcard coverage and continues with the same selections",async({page})=>{
  await catalog(page);
  const requests:Record<string,unknown>[]=[];
  await page.routeWebSocket("**/ws/p2p/discovery",ws=>ws.onMessage(data=>{const request=JSON.parse(String(data));requests.push(request);ws.send(JSON.stringify({type:"search_finished",response:response({mode:"exchange",...(request.cursor ? {complete:true,next_cursor:null,coverage:{combinations_total:35,combinations_completed:35,batch_completed:19,combinations_failed:0,combinations_timed_out:0}} : {})})}));}));
  await anyExchange(page);
  await page.getByTestId("start-search").click();
  await expect(page.getByText("16 / 35 combinations searched")).toBeVisible();
  expect(requests[0]).toMatchObject({mode:"exchange",source_fiat:"ANY",target_fiat:"ANY",source_amount:"1000"});
  await page.getByRole("button",{name:"Continue search",exact:true}).click();
  await expect(page.getByText("35 / 35 combinations searched")).toBeVisible();
  expect(requests[1].cursor).toBe("next-page");
  await expect(page.getByLabel("Amount to send",{exact:true})).toHaveValue("1000");
  await expect(page).toHaveURL(/#\/swap\/ANY\/ANY/);
  await page.getByLabel("Amount to send",{exact:true}).fill("0.00000001");
  await page.getByTestId("start-search").click();
  await expect.poll(()=>requests.length).toBeGreaterThan(2);
  expect(requests[2].source_amount).toBe("0.00000001");
  expect(requests[2].cursor).toBeUndefined();
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
