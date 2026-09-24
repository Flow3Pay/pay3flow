import { expect, test, type Locator, type Page } from "@playwright/test";

async function expectNumberedTimeline(instructions: Locator, numbers: string[]) {
  const steps = instructions.getByTestId("instruction-step");
  await expect(steps).toHaveCount(numbers.length);
  await expect(steps.locator(".stepNumber")).toHaveText(numbers);
  const markerShape = await steps.locator(".stepNumber").first().evaluate((element) => {
    const rect = element.getBoundingClientRect();
    return { width: rect.width, height: rect.height, radius: getComputedStyle(element).borderRadius };
  });
  expect(Math.abs(markerShape.width - markerShape.height)).toBeLessThan(0.1);
  expect(markerShape.radius).toBe("50%");
}

async function openApp(page: Page) {
  await page.goto("/");
  await expect
    .poll(() => page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage))
    .not.toBe("none");
}

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
    if (url.pathname === "/api/providers") {
      return json([
        { slug: "binance", name: "Binance", side: "sell", source_url: "https://p2p.binance.com", currencies: ["AMD", "RUB"], banks: [], searchable: true },
        { slug: "bybit", name: "Bybit", side: "sell", source_url: "https://www.bybit.com/fiat/trade/otc", currencies: ["AMD", "RUB"], banks: [], searchable: true },
        { slug: "whitebird", name: "Whitebird Sell", side: "sell", source_url: "https://whitebird.io", currencies: ["BYN", "USD", "EUR", "RUB"], banks: [], searchable: false },
      ]);
    }
    if (url.pathname === "/api/service-executions/open" && method === "POST") {
      return json({
        execution_id: "00000000-0000-4000-8000-000000000301",
        newly_recorded: true,
        redirect_url: "about:blank",
        service: { id: "00000000-0000-4000-8000-000000000202", slug: "bybit", display_name: "Bybit", executions_total: 6201, likes_total: 850, dislikes_total: 40 },
      });
    }
    if (url.pathname === "/api/services/00000000-0000-4000-8000-000000000202/vote" && method === "PUT") {
      return json({ id: "00000000-0000-4000-8000-000000000202", slug: "bybit", display_name: "Bybit", executions_total: 6201, likes_total: 851, dislikes_total: 40, viewer_vote: "like" });
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
          user_type: "merchant" as string | null,
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300 as number | null,
          completion_rate_30d: 0.99 as number | null,
        },
        source_url: `https://example.com/${adId}`,
      });
      if (url.searchParams.get("source_fiat") === "USDC") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
        const whitebird = offer("whitebird", "whitebird-sell-RUB-USDC", "RUB", "USDC");
        whitebird.price = "83.4295";
        whitebird.payment_methods = [];
        whitebird.advertiser = {
          ...whitebird.advertiser,
          id: null,
          nickname: "Whitebird",
          user_type: "service",
          completed_orders_30d: null,
          completion_rate_30d: null,
        };
        whitebird.source_url = "https://whitebird.io/";
        return json({
          search_id: "00000000-0000-4000-8000-000000000105",
          routes_found: 1,
          searched_at: "2026-09-23T10:00:00Z",
          source_fiat: "USDC",
          target_fiat: "RUB",
          source_amount: "100.00",
          assets_searched: ["USDC"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-usdc-whitebird-rub",
            rank: 1,
            asset: "USDC",
            entry_network: "ethereum",
            source_network: "ethereum",
            target_network: null,
            source_fiat: "USDC",
            source_amount: "100.00000000",
            acquired_asset_amount: "100.00000000",
            target_fiat: "RUB",
            target_amount: "8342.95",
            effective_rate: "83.42950000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "crypto_to_fiat",
            payment_methods_verified: false,
            entry_offer: null,
            exit_offer: whitebird,
            warnings: ["Search estimate only."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "USDT") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
        expect(url.searchParams.has("source_payment_method")).toBe(false);
        return json({
          search_id: "00000000-0000-4000-8000-000000000101",
          routes_found: 1,
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "USDT",
          target_fiat: "RUB",
          source_amount: "125.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-usdt-rub",
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
            services: [{ id: "00000000-0000-4000-8000-000000000201", slug: "binance", display_name: "Binance", executions_total: 12400, likes_total: 1800, dislikes_total: 74 }],
            reputation: { executions_average: 12400, likes_average: 1800, dislikes_average: 74 },
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "ETH") {
        expect(url.searchParams.get("source_network")).toBe("base");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({
          search_id: "00000000-0000-4000-8000-000000000102",
          routes_found: 1,
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "ETH",
          target_fiat: "USDT",
          source_amount: "0.03",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-eth-usdt",
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
            services: [{ id: "00000000-0000-4000-8000-000000000201", slug: "binance", display_name: "Binance", executions_total: 12400, likes_total: 1800, dislikes_total: 74 }],
            reputation: { executions_average: 12400, likes_average: 1800, dislikes_average: 74 },
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "BTC") {
        expect(url.searchParams.get("source_network")).toBe("bitcoin");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({
          search_id: "00000000-0000-4000-8000-000000000104",
          routes_found: 1,
          searched_at: "2026-09-19T10:00:00Z",
          source_fiat: "BTC",
          target_fiat: "USDT",
          source_amount: "0.002",
          assets_searched: ["USDC"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-btc-usdc-usdt",
            rank: 1,
            asset: "USDC",
            entry_network: "bitcoin",
            source_network: "bitcoin",
            target_network: "ton",
            source_fiat: "BTC",
            source_amount: "0.002000000000",
            acquired_asset_amount: "126.000000000000",
            target_fiat: "USDT",
            target_amount: "125.800000000000",
            effective_rate: "62900.000000000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: false,
            route_kind: "crypto_to_crypto",
            bridge_currency: "USDC",
            market_path: {
              venue: "binance",
              source_pair: "BTCUSDC",
              target_pair: "USDCUSDT",
              source_rate: "63000.000000000000",
              target_rate: "0.998400000000",
              intermediary_amount: "126.000000000000",
            },
            payment_methods_verified: true,
            entry_offer: null,
            exit_offer: null,
            warnings: ["Network availability is not verified."],
            services: [{ id: "00000000-0000-4000-8000-000000000201", slug: "binance", display_name: "Binance", executions_total: 12400, likes_total: 1800, dislikes_total: 74 }],
            reputation: { executions_average: 12400, likes_average: 1800, dislikes_average: 74 },
          }],
        });
      }
      expect(url.searchParams.get("source_payment_method")).toBe("IDBank");
      expect(url.searchParams.get("target_payment_method")).toBe("Alfa-Bank");
      expect(url.searchParams.get("source_fiat")).toBe("AMD");
      expect(url.searchParams.get("target_fiat")).toBe("RUB");
      expect(url.searchParams.get("allow_cross_venue")).toBe("true");
      const routes = Array.from({ length: 12 }, (_, index) => {
        const best = index === 0;
        const asset = index % 2 === 0 ? "USDT" : "USDC";
        const venue = index % 2 === 0 ? "binance" : "bybit";
        const exitVenue = index === 2 ? "bybit" : venue;
        return {
          route_id: `route-${index + 1}`,
          rank: index + 1,
          asset,
          source_fiat: "AMD",
          source_amount: "100000.00",
          acquired_asset_amount: best ? "253.16455696" : "252.52525252",
          target_fiat: "RUB",
          target_amount: best ? "20350.00" : (20120 - index * 20).toFixed(2),
          effective_rate: best ? "0.20350000" : "0.20100000",
          entry_network: index === 2 ? "ethereum" : undefined,
          same_venue: index !== 2,
          requires_asset_transfer: index === 2,
          transfer_fee_included: true,
          payment_methods_verified: best,
          entry_offer: offer(venue, `entry-${index + 1}`, "AMD", asset),
          exit_offer: offer(exitVenue, `exit-${index + 1}`, "RUB", asset),
          warnings: ["Search estimate only."],
          services: [{ id: venue === "binance" ? "00000000-0000-4000-8000-000000000201" : "00000000-0000-4000-8000-000000000202", slug: venue, display_name: venue === "binance" ? "Binance" : "Bybit", executions_total: best ? 12400 : 6200, likes_total: best ? 1800 : 850, dislikes_total: best ? 74 : 40 }],
          reputation: { executions_average: best ? 12400 : 6200, likes_average: best ? 1800 : 850, dislikes_average: best ? 74 : 40 },
          service_links: index === 1 ? [
            { service_id: "00000000-0000-4000-8000-000000000202", service_slug: "bybit", kind: "entry", tracking_token: "entry-token" },
            { service_id: "00000000-0000-4000-8000-000000000202", service_slug: "bybit", kind: "exit", tracking_token: "exit-token" },
          ] : [],
        };
      });
      return json({
        search_id: "00000000-0000-4000-8000-000000000103",
        routes_found: 24,
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
  await openApp(page);

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
  await expect(page.getByText("24 routes found")).toBeVisible();
  await expect(page.getByText("Showing top 12")).toBeVisible();
  await expect(page.getByTestId("complete-route").first()).toContainText("Used 12.4K times");
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
  await expect(instructions.getByRole("list", { name: "Exchange steps" })).toBeVisible();
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByText("Buy USDT for 100,000 AMD")).toBeVisible();
  await expect(instructions.getByText("Match the advertiser nickname and ad ID before creating the order.")).toHaveCount(2);
  await expect(instructions.getByText("Release the asset only after you have independently confirmed the payment in your bank or payment account.")).toBeVisible();
  const offerLinks = instructions.getByRole("link", { name: /Open Binance P2P and find binance-merchant/ });
  await expect(offerLinks.first()).toHaveAttribute(
    "href",
    "https://example.com/entry-1",
  );
  await expect(offerLinks).toHaveCount(2);
  await instructions.getByRole("button", { name: "Close instructions", exact: true }).click();
  await expect(instructions).toBeHidden();

  await swapDirection.click();
  await expect(amountInput).toHaveValue("20350");
});

test("cross-venue instructions include a numbered transfer step", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();
  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(12);

  await page.getByTestId("complete-route").nth(2).locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2", "3"]);
  await expect(instructions.getByRole("heading", { name: "Transfer USDT to Bybit" })).toBeVisible();
  await expect(instructions.getByText("select the exact Ethereum (ERC-20) network on both venues", { exact: false })).toBeVisible();
  await expect(instructions.getByText("Wait for Bybit to credit the deposit before continuing.")).toBeVisible();
});

test("selected bank currencies override the reversed corridor", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Swap sender and recipient" }).click();
  await page.getByRole("button", { name: "Select sending bank: Sberbank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Ameriabank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();

  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();

  await expect(page.getByText("Estimated RUB")).toBeVisible();
  await expect(page).toHaveURL(/#\/swap\/AMD\/RUB\?amount=100000$/);
});

test("catalog providers can be selected", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Route refresh settings" }).click();
  const whitebird = page.getByRole("button", { name: "Whitebird" });

  await expect(whitebird).toBeEnabled();
  await whitebird.click();
  await expect(whitebird).toHaveAttribute("aria-pressed", "true");
});

test("search venues bounce in the loader and refresh stops spinning after the first route", async ({ page }) => {
  await mockBackend(page);

  let sendFirstRoute: (() => void) | undefined;
  let sendSecondRoute: (() => void) | undefined;
  let finishSearch: (() => void) | undefined;
  let socketConnections = 0;
  await page.routeWebSocket(/\/ws\/p2p\/routes$/, (socket) => {
    socketConnections += 1;
    socket.onMessage((message) => {
      const request = JSON.parse(String(message));
      expect(request.query.sources).toBe("binance,bybit,whitebird");

      const offer = (source: string, adId: string, fiat: string) => ({
        source,
        ad_id: adId,
        fiat,
        asset: "USDT",
        price: "1",
        available_asset: "1000000",
        min_fiat: "1000",
        max_fiat: "10000000",
        payment_methods: ["Bank transfer"],
        pay_time_limit_minutes: 15,
        advertiser: {
          id: `masked-${adId}`,
          nickname: `${source}-merchant`,
          user_type: "merchant",
          is_merchant: true,
          is_verified: true,
          completed_orders_30d: 300,
          completion_rate_30d: 0.99,
        },
        source_url: `https://example.com/${adId}`,
      });
      const firstResponse = {
        search_id: "00000000-0000-4000-8000-000000000106",
        routes_found: 1,
        searched_at: "2026-09-23T10:00:00Z",
        source_fiat: "AMD",
        target_fiat: "RUB",
        source_amount: "100000.00",
        assets_searched: ["USDT"],
        can_exchange_to_target: true,
        routes: [{
          route_id: "route-streamed-first",
          rank: 1,
          asset: "USDT",
          entry_network: "internal",
          source_fiat: "AMD",
          source_amount: "100000.00",
          acquired_asset_amount: "253.16",
          target_fiat: "RUB",
          target_amount: "20350.00",
          effective_rate: "0.2035",
          same_venue: true,
          requires_asset_transfer: false,
          transfer_fee_included: true,
          route_kind: "fiat_to_fiat",
          payment_methods_verified: true,
          entry_offer: offer("bybit", "entry-live", "AMD"),
          exit_offer: offer("bybit", "exit-live", "RUB"),
          warnings: [],
        }],
      };
      const finalResponse = {
        ...firstResponse,
        routes_found: 2,
        routes: [{
          ...firstResponse.routes[0],
          route_id: "route-streamed-second",
          rank: 1,
          target_amount: "20420.00",
          effective_rate: "0.2042",
          entry_offer: offer("whitebird", "entry-live-2", "AMD"),
          exit_offer: offer("whitebird", "exit-live-2", "RUB"),
        }, { ...firstResponse.routes[0], rank: 2 }],
      };

      socket.send(JSON.stringify({ type: "search_started", search_id: firstResponse.search_id, routes_found: 0 }));
      sendFirstRoute = () => socket.send(JSON.stringify({ type: "routes_updated", ...firstResponse }));
      sendSecondRoute = () => socket.send(JSON.stringify({ type: "routes_updated", ...finalResponse }));
      finishSearch = () => socket.send(JSON.stringify({ type: "search_finished", ...finalResponse }));
    });
  });

  await openApp(page);
  await page.getByRole("button", { name: "Route refresh settings" }).click();
  await page.getByRole("button", { name: "Whitebird" }).click();
  await page.getByRole("button", { name: "Route refresh settings" }).click();
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();

  await expect.poll(() => Boolean(sendFirstRoute)).toBe(true);
  await page.waitForTimeout(750);
  expect(socketConnections).toBe(1);
  const panelTop = page.locator("#routes .panelTop");
  const searchingVenues = panelTop.getByTestId("searching-venue");
  await expect(searchingVenues).toHaveCount(3);
  await expect(searchingVenues.nth(0)).toHaveAttribute("title", "Searching Binance");
  await expect(searchingVenues.nth(1)).toHaveAttribute("title", "Searching Bybit");
  await expect(searchingVenues.nth(2)).toHaveAttribute("title", "Searching Whitebird");
  await expect(searchingVenues.nth(0)).toHaveCSS("width", "32px");
  await expect(searchingVenues.nth(0)).toHaveCSS("animation-delay", "0s");
  await expect(searchingVenues.nth(1)).toHaveCSS("animation-delay", "0.13s");

  const refreshButton = page.getByRole("button", { name: "Refresh routes now" });
  await expect(refreshButton.locator("svg")).toHaveClass(/refreshSpin/);
  sendFirstRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  const foundVenues = panelTop.locator(".resultSummary").getByTestId("found-venue");
  await expect(foundVenues).toHaveCount(1);
  await expect(foundVenues.first()).toHaveAttribute("title", "Found on Bybit");
  await expect(searchingVenues).toHaveCount(2);
  await expect(searchingVenues.nth(1)).toHaveAttribute("title", "Searching Whitebird");
  await expect(refreshButton).toBeDisabled();
  await expect(refreshButton.locator("svg")).not.toHaveClass(/refreshSpin/);

  sendSecondRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(2);
  await expect(page.getByTestId("complete-route").first()).toContainText("20420 RUB");
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  await expect(foundVenues).toHaveCount(2);
  await expect(foundVenues.nth(1)).toHaveAttribute("title", "Found on Whitebird");
  await expect(searchingVenues).toHaveCount(1);

  await page.getByTestId("complete-route").nth(1).click();
  await expect(page.getByTestId("complete-route").nth(1)).toHaveClass(/selected/);
  finishSearch?.();
  await expect(refreshButton).toBeEnabled();
  await expect(searchingVenues).toHaveCount(0);
  await expect(page.getByTestId("complete-route").nth(1)).toHaveClass(/selected/);
});

test("cryptocurrency search binds the selected asset to its network", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

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

test("direct Whitebird exchange uses provider wording and local venue icons", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const picker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await picker.getByLabel("Search banks and payment methods").fill("USDC ERC20");
  await picker.getByRole("option", { name: /USD Coin USDC · Ethereum \(ERC-20\)/ }).click();

  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();
  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route.locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDC USD Coin · Ethereum (ERC-20) (Whitebird) → RUB",
  );
  await expect(route.locator(".workflowVenueIcon img")).toHaveAttribute(
    "src",
    "/icons/venues/whitebird.png",
  );

  await route.locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByText("Direct exchange on Whitebird", { exact: true })).toBeVisible();
  await expect(instructions.getByRole("link", { name: "Open Whitebird exchange" })).toHaveAttribute(
    "href",
    "https://whitebird.io/",
  );
  await expect(instructions.getByText("Find by nickname", { exact: true })).toHaveCount(0);
  await expect(instructions.locator(".adHint")).toHaveCount(0);
  await expect(instructions.locator(".counterpartyAvatar .avatarLogo")).toHaveAttribute(
    "src",
    "/icons/venues/whitebird.png",
  );
});

test("Armenian bank picker uses the downloaded local icons", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const picker = page.getByRole("dialog", { name: "Choose where you pay from" });
  const icons = [
    ["Ameriabank", "/icons/assets/ameriabank.png"],
    ["IDBank", "/icons/assets/idbank.png"],
    ["Ardshinbank", "/icons/assets/ardshinbank.png"],
    ["Inecobank", "/icons/assets/inecobank.png"],
    ["Evocabank", "/icons/assets/evocabank.png"],
  ] as const;

  for (const [name, src] of icons) {
    await picker.getByLabel("Search banks and payment methods").fill(name);
    await expect(picker.getByRole("option", { name: new RegExp(name) }).locator("img")).toHaveAttribute("src", src);
  }
});

test("anonymous vote reveals service reputation", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);
  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("IDBank");
  await sourcePicker.getByRole("option", { name: /IDBank/ }).click();
  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(12);

  await page.getByTestId("complete-route").nth(1).locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByText("Was this service useful?")).toBeVisible();
  await expect(instructions.getByLabel("850 likes")).toHaveCount(0);
  await expect(instructions.getByLabel("40 dislikes")).toHaveCount(0);
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "dark";
  });
  await expect(instructions.locator(".serviceReputation article").first()).toHaveCSS(
    "background-color",
    "rgb(34, 34, 34)",
  );
  await expect(instructions.locator(".serviceReputation article strong").first()).toHaveCSS(
    "color",
    "rgb(243, 243, 243)",
  );
  await expect(instructions.getByRole("button", { name: "Like", exact: true })).toHaveCSS(
    "background-color",
    "rgb(43, 43, 43)",
  );
  await instructions.getByRole("button", { name: "Like", exact: true }).click();
  await expect(instructions.getByRole("button", { name: "Like", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(instructions.getByLabel("851 likes")).toBeVisible();
});

test("crypto route keeps distinct source and target networks", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

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

  await page.getByTestId("complete-route").locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1"]);
  await expect(instructions.getByRole("heading", { name: "Convert ETH to USDT" })).toBeVisible();
  await expect(instructions.getByText("Confirm the pair converts ETH into USDT.")).toBeVisible();
});

test("bridged spot instructions split both market trades into separate steps", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("BTC Bitcoin");
  await sourcePicker.getByRole("option", { name: /Bitcoin BTC · Bitcoin/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("USDT TON");
  await targetPicker.getByRole("option", { name: /Tether USDT · TON/ }).click();

  await page.getByLabel("Amount to send").fill("0.002");
  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await page.getByTestId("complete-route").locator(".routeAmount").click();

  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByRole("heading", { name: "Convert BTC to USDC" })).toBeVisible();
  await expect(instructions.getByRole("heading", { name: "Convert USDC to USDT" })).toBeVisible();
  await expect(instructions.getByText("BTCUSDC", { exact: true })).toBeVisible();
  await expect(instructions.getByText("USDCUSDT", { exact: true })).toBeVisible();
});

test("same asset on different networks reports unavailable bridge provider", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

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
