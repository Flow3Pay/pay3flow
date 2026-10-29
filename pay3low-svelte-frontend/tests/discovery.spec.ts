import { test, expect } from "@playwright/test";

test.use({ reducedMotion: "reduce" });

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
  await page.route("**/api/exchange/corridors*",route=>route.fulfill({headers:cors,json:{items:[]}}));
}
function response(overrides = {}) {
  return {search_id:"test",mode:"arbitrage",source_amount:"1000",reporting_currency:"USD",routes:[],routes_found:0,routes_exhaustive:false,complete:false,coverage:{combinations_total:35,combinations_completed:16,combinations_failed:0,combinations_timed_out:0,batch_completed:16},provider_statuses:[],errors:[],next_cursor:"next-page",...overrides};
}
test("SBP works on both selectors and shared discovery selections survive reload",async({page})=>{
  await catalog(page);
  await page.goto("/#/discover?mode=exchange&source_fiat=RUB&target_fiat=RUB&source_amount=1234.56&source_payment_method=ru-sbp&target_payment_method=ru-sbp");
  await expect(page.getByRole("combobox",{name:"Sending method",exact:true})).toHaveValue("ru-sbp", { timeout: 30_000 });
  await expect(page.getByRole("combobox",{name:"Receiving method",exact:true})).toHaveValue("ru-sbp", { timeout: 30_000 });
  await expect(page.getByLabel("Amount per starting currency")).toHaveValue("1234.56", { timeout: 30_000 });
  await page.reload();
  await expect(page.getByRole("combobox",{name:"Sending method",exact:true})).toHaveValue("ru-sbp", { timeout: 30_000 });
  await expect(page).toHaveURL(/#\/discover/);
  await page.getByRole("button",{name:"Arbitrage",exact:true}).click();
  await expect(page.getByText("Return to the same currency, network, payment method and venue.")).toBeVisible();
  await expect(page.getByLabel("Receiving currency",{exact:true})).toHaveCount(0);
});
test("streams partial coverage and continues without dropping existing selections",async({page})=>{
  await catalog(page);
  const messages: Record<string,unknown>[]=[];
  await page.routeWebSocket("**/ws/p2p/discovery", ws => {
    ws.onMessage(data=>{const request=JSON.parse(String(data));messages.push(request);ws.send(JSON.stringify({type:"routes_updated",response:response()}));ws.send(JSON.stringify({type:"search_finished",response:response(request.cursor ? {complete:true,next_cursor:null,coverage:{combinations_total:35,combinations_completed:35,batch_completed:19,combinations_failed:0,combinations_timed_out:0}} : {})}));});
  });
  await page.goto("/#/discover?mode=arbitrage&source_amount=1000");
  await page.getByRole("button",{name:"Find arbitrage",exact:true}).click();
  await expect(page.getByText("16 / 35 combinations searched")).toBeVisible();
  await page.getByRole("button",{name:"Continue search",exact:true}).click();
  await expect(page.getByText("35 / 35 combinations searched")).toBeVisible();
  expect(messages[1].cursor).toBe("next-page");
  expect(messages[1].source_amount).toBe("1000");
});
test("failed search shows an inline retryable error",async({page})=>{
  await catalog(page);
  await page.routeWebSocket("**/ws/p2p/discovery", ws=>ws.onMessage(()=>ws.send(JSON.stringify({type:"search_failed",error:"Selected method is unavailable"}))));
  await page.route("**/api/p2p/discovery?*",route=>route.fulfill({headers:cors,status:400,body:"Selected method is unavailable"}));
  await page.goto("/#/discover?mode=exchange&source_amount=1000");
  await page.getByRole("button",{name:"Find exchanges",exact:true}).click();
  await expect(page.getByRole("alert")).toContainText("Selected method is unavailable");
  await expect(page.getByRole("button",{name:"Find exchanges",exact:true})).toBeEnabled();
});
