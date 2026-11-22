import { expect, test, type Page } from "@playwright/test";

const ever = `0:${"a".repeat(64)}`, eth = `0x${"b".repeat(40)}`;
const sellerEver = `0:${"c".repeat(64)}`, sellerEth = `0x${"d".repeat(40)}`;

type Fixture = { takerEver: string | null; takerUsdt: string | null; acceptError?: string; offerDirection?: "sell" | "buy"; offerNetwork?: string; sessionEver?: boolean };

async function setup(page: Page, options: Fixture) {
  const session = {
    token: "taker-token", actor: "https://relay.example/actors/taker", desk: false, expires_at: "2099-01-01T00:00:00Z",
    signing_wallet: { chain: "everscale", address: ever },
    wallets: [
      ...(options.sessionEver === false ? [] : [{ chain: "everscale", address: ever, verified_at: new Date().toISOString() }]),
      { chain: "ethereum", address: eth, verified_at: new Date().toISOString() },
    ],
  };
  const offer = {
    id: "offer-foreign", seller_ever: sellerEver, seller_avatar: "", state: "open",
    created_at: new Date().toISOString(), expires_at: "2099-01-01T00:00:00Z",
    terms: {
      direction: options.offerDirection ?? "sell", stablecoin: "USDT", ever_amount: "100", usdt_amount: "10",
      usdt_address: sellerEth, stablecoin_network: options.offerNetwork ?? "ethereum", ethereum_chain_id: 1,
    },
  };
  const deals: any[] = [];
  const calls: { path: string; method: string; body: any }[] = [];
  await page.addInitScript(({ session }) => {
    localStorage.setItem("pay3flow-locale", "en");
    if (!sessionStorage.getItem("accept-offer-fixture-initialized")) {
      sessionStorage.setItem("pay3flow.otc.session.v1", JSON.stringify(session));
      sessionStorage.setItem("accept-offer-fixture-initialized", "1");
    }
  }, { session });
  await page.route("**/api/**", async route => {
    const req = route.request(), path = new URL(req.url()).pathname.replace("/api/otc", "");
    const body = req.postDataJSON();
    calls.push({ path, method: req.method(), body });
    let response: any = [], status = 200;
    if (path === "/config") response = {
      enabled: false, available: false, public_offers_available: true, public_offer_directions: ["buy", "sell"],
      p2p_available: true, wallet_login_available: true, wallet_login_chains: ["everscale", "ethereum"],
      demo: false, desk: "", desk_name: "", ever_resource: "", usdt_resource: "", ethereum_chain_id: 1,
    };
    else if (path === "/session" || path === "/session/proof") response = session;
    else if (path === "/session/revoke") response = { revoked: true };
    else if (path === "/listings") response = [];
    else if (path === "/trades") response = { trades: [], rfqs: [] };
    else if (path === "/offers") response = [offer];
    else if (path === "/offers/mine") response = [];
    else if (path === "/offers/history") response = [];
    else if (path === "/balances") response = {
      balances: [
        { chain: "everscale", symbol: "EVER", network: "Everscale", amount: options.takerEver, state: options.takerEver === null ? "unavailable" : "ok" },
        { chain: "wallet", symbol: "USDT", network: "Ethereum", amount: options.takerUsdt, state: options.takerUsdt === null ? "unavailable" : "ok" },
      ],
    };
    else if (path === "/p2p/assets") response = {
      assets: [], destination_assets: [],
      buy_destination_assets: [
        { symbol: "USDT", network: "ethereum", asset_id: "nep141:eth-usdt" },
        { symbol: "USDT", network: "polygon-pos", asset_id: "nep141:pol-usdt" },
        { symbol: "USDT", network: "tron", asset_id: "nep141:trx-usdt" },
      ],
    };
    else if (path === "/p2p/trades") response = { deals };
    else if (path === `/offers/${offer.id}/accept`) {
      if (options.acceptError) { status = 409; response = { error: options.acceptError }; }
      else {
        const deal = {
          id: "deal-1", offer_id: offer.id, state: "payment_pending", role: "taker", counterparty: offer.seller_ever,
          my_leg: "payment", counterparty_leg: "payout", my_ever: ever, counterparty_ever: sellerEver,
          my_confirmed: false, counterparty_confirmed: false, attempts: [],
          terms: { ...offer.terms, usdt_payer: eth, usdt_payee: sellerEth, ever_payer: sellerEver, ever_payee: ever, stablecoin_asset_id: null },
          fee_accrued: false, pay_by: "2099-01-01T00:00:00Z", payout_by: null,
          expires_at: "2099-01-01T00:00:00Z", created_at: new Date().toISOString(),
        };
        deals.unshift(deal); response = deal;
      }
    } else if (path === "/p2p/trades/deal-1") response = { deal: deals.find(item => item.id === "deal-1"), attempts: [] };
    await route.fulfill({ status, json: response });
  });
  await page.goto("/#/otc?market=EVER-USDT&side=sell");
  await expect(page.getByTestId("otc-workspace")).toHaveAttribute("data-state", "connected", { timeout: 20_000 });
  return { calls, offer };
}

async function openOffer(page: Page) {
  await page.locator(".bookColumn").getByTestId("otc-offer-take").first().click();
  const dialog = page.getByRole("dialog", { name: "Offer details", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
}

test("a taker without USDT is warned before accepting and cannot book an unpayable deal", async ({ page }) => {
  await setup(page, { takerEver: "2", takerUsdt: "0" });
  const dialog = await openOffer(page);
  await expect(dialog.locator(".addressRow").nth(1)).toContainText("USDT receiving wallet · Ethereum");
  const warning = dialog.getByTestId("otc-accept-shortfall");
  await expect(warning).toContainText("Not enough USDT");
  await expect(warning).toContainText("0 USDT available on Ethereum");
  await expect(warning).toContainText("9.7 USDT needed");
  await expect(dialog.getByTestId("otc-accept-offer")).toBeDisabled();
});

test("accepting an offer opens the deal window for the taker", async ({ page }) => {
  const f = await setup(page, { takerEver: "2", takerUsdt: "500" });
  const dialog = await openOffer(page);
  await expect(dialog.getByTestId("otc-accept-shortfall")).toHaveCount(0);
  await dialog.getByTestId("otc-accept-offer").click();
  await expect(page.getByTestId("otc-p2p-deal-id")).toContainText("deal-1");
  await expect(page.getByTestId("otc-p2p-deal-id")).toBeVisible();
  expect(f.calls.find(call => call.path === "/offers/offer-foreign/accept")?.body).toEqual({ taker_ever: ever });
  await expect(page.getByTestId("otc-error")).toHaveCount(0);
});

test("a sell offer accepts a typed EVER address when no EVER wallet is available", async ({ page }) => {
  const f = await setup(page, { takerEver: null, takerUsdt: "500", sessionEver: false });
  const dialog = await openOffer(page);
  const address = dialog.getByTestId("otc-ever-payout-address");
  await expect(address).toHaveValue("");
  await expect(dialog.getByTestId("otc-accept-offer")).toBeDisabled();
  await address.fill(`0:${"f".repeat(64)}`);
  await expect(dialog.getByTestId("otc-accept-offer")).toBeEnabled();
  await dialog.getByTestId("otc-accept-offer").click();
  expect(f.calls.find(call => call.path === "/offers/offer-foreign/accept")?.body).toEqual({ taker_ever: `0:${"f".repeat(64)}` });
});

test("a buy offer names the seller's actual stablecoin network and warns about the EVER leg", async ({ page }) => {
  await setup(page, { takerEver: "2", takerUsdt: "500", offerDirection: "buy", offerNetwork: "polygon-pos" });
  const dialog = await openOffer(page);
  await expect(dialog.locator(".addressRow").nth(1)).toContainText("USDT paying wallet · Polygon");
  const warning = dialog.getByTestId("otc-accept-shortfall");
  await expect(warning).toContainText("Not enough EVER");
  await expect(warning).toContainText("100 EVER needed");
  await expect(dialog.getByTestId("otc-accept-offer")).toBeDisabled();
});

test("the receiving network is a styled picker with short names and a prefilled verified address", async ({ page }) => {
  await setup(page, { takerEver: "500", takerUsdt: "500", offerDirection: "buy" });
  const dialog = await openOffer(page);
  const choices = dialog.getByTestId("otc-buy-destination-network");
  await expect(choices).toHaveCount(3);
  await expect(choices.nth(0)).toHaveText("Ethereum");
  await expect(choices.nth(1)).toHaveText("Polygon");
  await expect(choices.nth(2)).toHaveText("TRON");
  await expect(choices.nth(0)).toHaveAttribute("aria-pressed", "true");
  expect(await choices.nth(0).locator("img").getAttribute("src")).toContain("eth");
  const address = dialog.getByTestId("otc-buy-destination-address");
  await expect(address).toHaveValue(eth);
  await expect(dialog.getByTestId("otc-accept-offer")).toBeEnabled();
  await choices.nth(1).click();
  await expect(choices.nth(1)).toHaveAttribute("aria-pressed", "true");
  await expect(address).toHaveValue(eth);
  await choices.nth(2).click();
  await expect(address).toHaveValue("");
  await expect(dialog.getByTestId("otc-accept-offer")).toBeDisabled();
  await address.fill(`0x${"e".repeat(40)}`);
  await expect(dialog.getByRole("alert")).toContainText("Enter a valid USDT address for TRON");
  await expect(dialog.getByTestId("otc-accept-offer")).toBeDisabled();
  await address.fill(`T${"1".repeat(33)}`);
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await expect(dialog.getByTestId("otc-accept-offer")).toBeEnabled();
});

test("a rejected acceptance reports why instead of closing silently", async ({ page }) => {
  await setup(page, { takerEver: "2", takerUsdt: "500", acceptError: "offer is no longer open" });
  const dialog = await openOffer(page);
  await dialog.getByTestId("otc-accept-offer").click();
  await expect(page.getByTestId("otc-error")).toHaveText("offer is no longer open");
  await expect(page.getByTestId("otc-p2p-deal-id")).toHaveCount(0);
});
