import { expect, Page, test } from "@playwright/test";

async function mockBackend(page: Page) {
  await page.route("http://localhost:8080/api/**", async (route) => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    const json = (value: unknown, status = 200) =>
      route.fulfill({ status, contentType: "application/json", body: JSON.stringify(value) });

    if (url.pathname === "/api/auth/login") return json({ error: "not found" }, 404);
    if (url.pathname === "/api/auth/register") return json({ token: "e2e-token" });
    if (url.pathname === "/api/exchange/corridors") {
      return json({
        terms_version: "2026-09-19",
        items: [{
          id: "00000000-0000-4000-8000-000000000010",
          source_country: "AM",
          source_currency: "AMD",
          target_country: "RU",
          target_currency: "RUB",
          min_amount_minor: 1000,
          max_amount_minor: null,
          daily_limit_minor: 5_000_000_000,
          metadata: {},
        }],
      });
    }
    if (url.pathname === "/api/p2p/routes") {
      const offer = (source: string, adId: string, fiat: string, asset: string) => ({
        source,
        ad_id: adId,
        fiat,
        asset,
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          nickname: `${source}-merchant`,
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300,
          completion_rate_30d: 0.99,
        },
        source_url: `https://example.com/${adId}`,
      });
      return json({
        searched_at: "2026-09-19T10:00:00Z",
        source_fiat: "AMD",
        target_fiat: "RUB",
        source_amount: "100000.00",
        assets_searched: ["USDT", "USDC", "BTC", "ETH"],
        can_exchange_to_target: true,
        routes: [
          {
            rank: 1,
            asset: "USDT",
            source_fiat: "AMD",
            source_amount: "100000.00",
            acquired_asset_amount: "253.16455696",
            target_fiat: "RUB",
            target_amount: "20350.00",
            effective_rate: "0.20350000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            entry_offer: offer("binance", "entry-1", "AMD", "USDT"),
            exit_offer: offer("binance", "exit-1", "RUB", "USDT"),
            warnings: ["Search estimate only."],
          },
          {
            rank: 2,
            asset: "USDC",
            source_fiat: "AMD",
            source_amount: "100000.00",
            acquired_asset_amount: "252.52525252",
            target_fiat: "RUB",
            target_amount: "20100.00",
            effective_rate: "0.20100000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            entry_offer: offer("bybit", "entry-2", "AMD", "USDC"),
            exit_offer: offer("bybit", "exit-2", "RUB", "USDC"),
            warnings: ["Search estimate only."],
          },
        ],
      });
    }
    return json({ error: `unmocked ${method} ${url.pathname}` }, 500);
  });
}

test("login modal → real P2P route search → select estimate", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/");

  await expect(page.getByRole("dialog", { name: "Sign in to Pay3Flow" })).toBeVisible();
  await page.getByRole("button", { name: "Sign in", exact: true }).last().click();
  await expect(page.getByTestId("auth-form")).toBeHidden();

  await expect(page.getByLabel("Send from")).toHaveValue("AM:AMD");
  await expect(page.getByLabel("Send to")).toHaveValue("RU:RUB");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(2);
  await expect(page.getByText("20,350 RUB")).toBeVisible();
  await page.getByTestId("complete-route").first().click();
  await expect(page.getByTestId("selected-route")).toBeVisible();
  await expect(page.getByText(/No trade or reservation has been placed/)).toBeVisible();
});
