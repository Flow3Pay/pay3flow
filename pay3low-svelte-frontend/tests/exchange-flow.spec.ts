import { expect, test, type Page } from "@playwright/test";

async function mockBackend(page: Page) {
  await page.route("http://localhost:8080/api/**", async (route) => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    const json = (value: unknown, status = 200) =>
      route.fulfill({ status, contentType: "application/json", body: JSON.stringify(value) });

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
    if (url.pathname === "/api/networks") {
      return json([
        { id: "ethereum", name: "Ethereum (ERC-20)", currencies: ["ETH", "USDT", "USDC"] },
        { id: "base", name: "Base", currencies: ["ETH", "USDC"] },
        { id: "tron", name: "TRON (TRC-20)", currencies: ["TRX", "USDT"] },
        { id: "ton", name: "TON", currencies: ["TON", "USDT"] },
        { id: "bitcoin", name: "Bitcoin", currencies: ["BTC"] },
      ]);
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
          id: source === "bybit" ? `masked-${adId}` : null,
          nickname: `${source}-merchant`,
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300,
          completion_rate_30d: 0.99,
        },
        source_url: `https://example.com/${adId}`,
      });
      if (url.searchParams.get("source_fiat") === "USDT") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
        expect(url.searchParams.has("source_payment_method")).toBe(false);
        return json({
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "USDT",
          target_fiat: "RUB",
          source_amount: "125.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            rank: 1,
            asset: "USDT",
            entry_network: "ethereum",
            source_network: "ethereum",
            target_network: null,
            source_fiat: "USDT",
            source_amount: "125.00000000",
            acquired_asset_amount: "125.00000000",
            target_fiat: "RUB",
            target_amount: "11250.00",
            effective_rate: "90.00000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat",
            payment_methods_verified: true,
            entry_offer: null,
            exit_offer: offer("binance", "exit-erc20", "RUB", "USDT"),
            warnings: ["Search estimate only."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "ETH") {
        expect(url.searchParams.get("source_network")).toBe("base");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "ETH",
          target_fiat: "USDT",
          source_amount: "0.03",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            rank: 1,
            asset: "USDT",
            entry_network: "base",
            source_network: "base",
            target_network: "ton",
            source_fiat: "ETH",
            source_amount: "0.030000000000",
            acquired_asset_amount: "80.100000000000",
            target_fiat: "USDT",
            target_amount: "80.100000000000",
            effective_rate: "2670.000000000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto",
            bridge_currency: null,
            market_path: {
              venue: "binance",
              source_pair: "ETHUSDT",
              target_pair: "ETHUSDT",
              source_rate: "2670.000000000000",
              target_rate: "1.000000000000",
              intermediary_amount: "80.100000000000",
            },
            payment_methods_verified: true,
            entry_offer: null,
            exit_offer: null,
            warnings: ["Network availability is not verified."],
          }],
        });
      }
      expect(url.searchParams.get("source_payment_method")).toBe("IDBank");
      expect(url.searchParams.get("target_payment_method")).toBe("Alfa-Bank");
      expect(url.searchParams.get("allow_cross_venue")).toBe("true");
      const routes = Array.from({ length: 12 }, (_, index) => {
        const best = index === 0;
        const asset = index % 2 === 0 ? "USDT" : "USDC";
        const venue = index % 2 === 0 ? "binance" : "bybit";
        return {
          rank: index + 1,
          asset,
          source_fiat: "AMD",
          source_amount: "100000.00",
          acquired_asset_amount: best ? "253.16455696" : "252.52525252",
          target_fiat: "RUB",
          target_amount: best ? "20350.00" : (20120 - index * 20).toFixed(2),
          effective_rate: best ? "0.20350000" : "0.20100000",
          same_venue: true,
          requires_asset_transfer: false,
          transfer_fee_included: true,
          payment_methods_verified: best,
          entry_offer: offer(venue, `entry-${index + 1}`, "AMD", asset),
          exit_offer: offer(venue, `exit-${index + 1}`, "RUB", asset),
          warnings: ["Search estimate only."],
        };
      });
      return json({
        searched_at: "2026-09-19T10:00:00Z",
        source_fiat: "AMD",
        target_fiat: "RUB",
        source_amount: "100000.00",
        assets_searched: ["USDT", "USDC", "BTC", "ETH"],
        can_exchange_to_target: true,
        routes,
      });
    }
    return json({ error: `unmocked ${method} ${url.pathname}` }, 500);
  });
}

test("public P2P route search → open step-by-step instructions", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/");

  const backgroundPattern = await expect
    .poll(() => page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage))
    .not.toBe("none")
    .then(() =>
      page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage),
    );
  await page.reload();
  await expect
    .poll(async () => {
      const nextPattern = await page
        .locator(".appShell")
        .evaluate((element) => getComputedStyle(element, "::before").backgroundImage);
      return nextPattern !== "none" && nextPattern !== backgroundPattern;
    })
    .toBe(true);

  await expect(page.getByTestId("auth-form")).toHaveCount(0);
  await expect(page.getByLabel("Amount to send")).toHaveValue("0");

  const amountInput = page.getByLabel("Amount to send");
  await amountInput.press("End");
  await amountInput.type("123");
  await expect(amountInput).toHaveValue("123");
  await amountInput.fill("12б5");
  await expect(amountInput).toHaveValue("12,5");
  await amountInput.fill("12ю5");
  await expect(amountInput).toHaveValue("12.5");
  await amountInput.fill("0");

  const swapDirection = page.getByRole("button", { name: "Swap sender and recipient" });
  await swapDirection.click();
  await expect(page.getByRole("button", { name: "Select sending bank: Sberbank" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Select recipient bank: Ameriabank" })).toBeVisible();
  await swapDirection.click();
  await expect(page.getByRole("button", { name: "Select sending bank: Ameriabank" })).toBeVisible();

  await page.getByRole("button", { name: "Route refresh settings" }).click();
  await page.getByRole("button", { name: "30s" }).click();

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await expect(sourcePicker).toBeVisible();
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();
  await expect(page.getByRole("button", { name: "Select sending bank: IDBank" })).toBeVisible();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await expect(targetPicker).toBeVisible();
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();
  await expect(page.getByRole("button", { name: "Select recipient bank: Alfa-Bank" })).toBeVisible();

  await amountInput.fill("100000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(12);
  await expect(page.getByTestId("complete-route").first()).toContainText("20350 RUB");
  await expect(page.getByTestId("complete-route").first().locator(".workflow")).toHaveAttribute(
    "aria-label",
    "AMD → USDT Tether (Binance) → RUB (Binance)",
  );
  const routeGroups = page.getByTestId("route-groups");
  const scrollMetrics = await routeGroups.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: getComputedStyle(element).overflowY,
  }));
  expect(scrollMetrics.overflowY).toBe("auto");
  expect(scrollMetrics.scrollHeight).toBeGreaterThan(scrollMetrics.clientHeight);
  await routeGroups.evaluate((element) => element.scrollTo({ top: element.scrollHeight }));
  await expect(page.getByTestId("complete-route").last()).toBeVisible();
  await routeGroups.evaluate((element) => element.scrollTo({ top: 0 }));
  await page.getByTestId("complete-route").first().locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions).toBeVisible();
  await expect(instructions.getByText("Buy USDT for 100,000 AMD")).toBeVisible();
  const offerLinks = instructions.getByRole("link", { name: /Open Binance P2P and find binance-merchant/ });
  await expect(offerLinks.first()).toHaveAttribute(
    "href",
    "https://example.com/entry-1",
  );
  await expect(offerLinks).toHaveCount(2);
  await instructions.getByRole("button", { name: "Close instructions" }).click();
  await expect(instructions).toBeHidden();

  await swapDirection.click();
  await expect(amountInput).toHaveValue("20350");
});

test("cryptocurrency search binds the selected asset to its network", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const picker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await picker.getByLabel("Search banks and payment methods").fill("USDT ERC20");

  const ethereumUsdt = picker.getByRole("option", { name: /Tether USDT · Ethereum \(ERC-20\)/ });
  await expect(ethereumUsdt).toHaveCount(1);
  await ethereumUsdt.click();

  const selectedAsset = page.getByRole("button", { name: "Select sending asset: Tether" });
  await expect(selectedAsset).toContainText("USDT · Ethereum (ERC-20)");

  await page.getByLabel("Amount to send").fill("125");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDT Tether · Ethereum (ERC-20) (Binance) → RUB",
  );
});

test("crypto route keeps distinct source and target networks", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("ETH Base");
  await sourcePicker.getByRole("option", { name: /Ethereum ETH · Base/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("USDT TON");
  await targetPicker.getByRole("option", { name: /Tether USDT · TON/ }).click();

  await page.getByLabel("Amount to send").fill("0.03");
  await page.getByTestId("start-search").click();

  await expect(page.getByTestId("complete-route")).toContainText(
    "ETH Ether · Base → USDT Tether · TON (Binance)",
  );
});

test("same asset on different networks reports unavailable bridge provider", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("USDT TRC20");
  await sourcePicker.getByRole("option", { name: /Tether USDT · TRON \(TRC-20\)/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("USDT TON");
  await targetPicker.getByRole("option", { name: /Tether USDT · TON/ }).click();

  await page.getByLabel("Amount to send").fill("125");
  await page.getByTestId("start-search").click();

  await expect(page.getByRole("alert")).toContainText(
    "No live bridge provider is configured for USDT: TRON (TRC-20) → TON",
  );
});
