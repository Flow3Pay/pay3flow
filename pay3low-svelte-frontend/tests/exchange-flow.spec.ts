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

async function mockBackend(page: Page, options: { includeNewProviders?: boolean } = {}) {
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
        { id: "near", name: "NEAR", currencies: ["BTC", "USDT"] },
      ]);
    }
    if (url.pathname === "/api/providers") {
      const providers: Array<Record<string, unknown>> = [
        { slug: "binance", name: "Binance", side: "sell", source_url: "https://p2p.binance.com", currencies: ["AMD", "RUB"], banks: [], searchable: true },
        { slug: "bybit", name: "Bybit", side: "sell", source_url: "https://www.bybit.com/fiat/trade/otc", currencies: ["AMD", "RUB"], banks: [], searchable: true },
        { slug: "cifra-broker", name: "Cifra Markets Sell", side: "sell", source_url: "https://cifra.by/", currencies: ["BYN", "RUB", "USD"], banks: [], searchable: true },
        { slug: "whitebird", name: "Whitebird Sell", side: "sell", source_url: "https://whitebird.io", currencies: ["BYN", "USD", "EUR", "RUB"], banks: [], searchable: false },
        { slug: "bestchange", name: "BestChange Sell", side: "sell", source_url: "https://bestchange.app/?lang=en", currencies: ["BYN", "EUR", "RUB", "USD"], banks: [], searchable: false },
        { slug: "dzengi", name: "Dzengi Sell", side: "sell", source_url: "https://dzengi.com/ru/kalkulyator-kriptovalyut", currencies: ["BYN", "EUR", "RUB", "USD"], banks: [], searchable: false },
        { slug: "exnode", name: "Exnode Sell", side: "sell", source_url: "https://exnode.ru/exchange", currencies: ["BYN", "EUR", "RUB", "USD"], banks: [], searchable: false },
        { slug: "cow-swap", name: "CoW Protocol Live Sell", side: "sell", source_url: "https://swap.cow.fi", currencies: ["USDC", "USDT"], banks: [], searchable: true, search_mode: "selectable" },
        { slug: "near-intents", name: "NEAR 1Click Sell", side: "sell", source_url: "https://1click.chaindefuser.com", currencies: ["BTC", "USDT"], banks: [], searchable: true, search_mode: "selectable" },
        { slug: "id-pay", name: "ID Pay Live Sell", side: "sell", source_url: "https://id-pay.ru/", currencies: ["AMD", "RUB"], banks: [], searchable: true, search_mode: "selectable" },
      ];
      if (options.includeNewProviders) {
        providers.push(
          { slug: "bitcoin-center", name: "Bitcoin Center Buy", side: "buy", source_url: "https://www.bitcoincenter.am", currencies: ["AMD"], banks: ["Bank Transfer"], searchable: true, search_mode: "selectable" },
          { slug: "bncex", name: "bncex Buy", side: "buy", source_url: "https://www.bncex.com/en", currencies: ["AMD"], banks: [], searchable: true, search_mode: "selectable" },
          { slug: "skylabs", name: "SkyLabs Buy", side: "buy", source_url: "https://skylabs.world", currencies: ["AMD"], banks: [], searchable: true, search_mode: "selectable" },
          { slug: "symbiosis", name: "Symbiosis Buy", side: "buy", source_url: "https://api.symbiosis.finance", currencies: [], banks: [], searchable: true, search_mode: "selectable" },
        );
      }
      return json(providers);
    }
    if (url.pathname === "/api/service-executions/open" && method === "POST") {
      return json({
        execution_id: "00000000-0000-4000-8000-000000000301",
        newly_recorded: true,
        redirect_url: "about:blank",
        service: { id: "00000000-0000-4000-8000-000000000202", slug: "bybit", display_name: "Bybit", executions_total: 6201, likes_total: 850, dislikes_total: 40 },
      });
    }
    if (url.pathname === "/api/routes/route-2/vote" && method === "PUT") {
      return json({ likes_total: 1, dislikes_total: 0, viewer_vote: "like" });
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
      if (url.searchParams.get("source_fiat") === "RUB" && url.searchParams.get("target_fiat") === "RUB") {
        const sbpOffer = (source: string, adId: string, fiat: string) => ({
          ...offer(source, adId, fiat, "USDT"),
          payment_methods: ["СБП"],
        });
        expect(url.searchParams.get("source_payment_method")).toBe("Sberbank");
        expect(url.searchParams.get("target_payment_method")).toBe("Alfa-Bank");
        return json({
          search_id: "00000000-0000-4000-8000-000000000107",
          routes_found: 1,
          searched_at: "2026-09-25T10:00:00Z",
          source_fiat: "RUB",
          target_fiat: "RUB",
          source_amount: "10000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-rub-sber-alfa-sbp",
            rank: 1,
            asset: "USDT",
            entry_network: null,
            source_network: null,
            target_network: null,
            source_fiat: "RUB",
            source_amount: "10000.00",
            acquired_asset_amount: "100.00000000",
            target_fiat: "RUB",
            target_amount: "9900.00",
            effective_rate: "0.99000000",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_fiat",
            payment_methods_verified: true,
            entry_offer: sbpOffer("binance", "entry-rub-sbp", "RUB"),
            exit_offer: sbpOffer("binance", "exit-rub-sbp", "RUB"),
            warnings: ["Search estimate only."],
          }],
        });
      }
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
      if (url.searchParams.get("source_fiat") === "USDT" && url.searchParams.get("target_fiat") === "USDC") {
        expect(url.searchParams.get("source_network")).toBe("ethereum");
        expect(url.searchParams.get("target_network")).toBe("ethereum");
        const directRoute = (provider: string, amount: string, rank: number) => ({
          route_id: `route-usdt-usdc-${provider}`,
          rank,
          asset: "USDC",
          entry_network: "ethereum",
          source_network: "ethereum",
          target_network: "ethereum",
          source_fiat: "USDT",
          source_amount: "100",
          acquired_asset_amount: amount,
          target_fiat: "USDC",
          target_amount: amount,
          effective_rate: (Number(amount) / 100).toFixed(12),
          same_venue: false,
          requires_asset_transfer: true,
          transfer_fee_included: true,
          route_kind: "crypto_to_crypto",
          route_provider: provider,
          route_path: ["USDT@ethereum", "USDC@ethereum"],
          route_fees: [{ asset: "USDT@ethereum", amount: provider === "cow-swap" ? "2.5" : "0.25" }],
          quote_expires_at: "2030-03-17T17:46:40Z",
          payment_methods_verified: true,
          entry_offer: null,
          exit_offer: null,
          warnings: [`Live dry quote from ${provider}.`],
        });
        return json({
          search_id: "00000000-0000-4000-8000-000000000107",
          routes_found: 2,
          searched_at: "2026-09-26T10:00:00Z",
          source_fiat: "USDT",
          target_fiat: "USDC",
          source_amount: "100",
          assets_searched: [],
          can_exchange_to_target: true,
          routes: [directRoute("near-intents", "99.6", 1), directRoute("cow-swap", "97.3", 2)],
        });
      }
      if (url.searchParams.get("source_fiat") === "USDT" && url.searchParams.get("target_fiat") === "USDT") {
        expect(url.searchParams.get("source_network")).toBe("tron");
        expect(url.searchParams.get("target_network")).toBe("ton");
        return json({ error: "No live bridge provider is configured for USDT: TRON (TRC-20) → TON" }, 400);
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
      if (url.searchParams.get("source_fiat") === "AMD" && url.searchParams.get("target_fiat") === "BTC") {
        expect(url.searchParams.get("source_amount")).toBe("10000");
        expect(url.searchParams.get("target_network")).toBe("near");
        expect(url.searchParams.get("source_payment_method")).toBe("Ameriabank");
        return json({
          search_id: "00000000-0000-4000-8000-000000000106",
          routes_found: 1,
          searched_at: "2026-09-25T10:00:00Z",
          source_fiat: "AMD",
          target_fiat: "BTC",
          source_amount: "10000.00",
          assets_searched: ["USDT"],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-amd-usdt-btc-near",
            rank: 1,
            asset: "USDT",
            entry_network: "optimism",
            source_network: null,
            target_network: "near",
            source_fiat: "AMD",
            source_amount: "10000.00",
            acquired_asset_amount: "2.77777778",
            target_fiat: "BTC",
            target_amount: "0.00032478",
            effective_rate: "0.000000032478",
            same_venue: false,
            requires_asset_transfer: true,
            transfer_fee_included: true,
            route_kind: "fiat_to_crypto",
            route_provider: "near-intents",
            route_path: ["AMD", "USDT@optimism", "BTC@near"],
            payment_methods_verified: true,
            entry_offer: offer("bybit", "entry-amd-usdt", "AMD", "USDT"),
            exit_offer: null,
            warnings: ["Live dry quote from near-intents; execution and wallet compatibility are not verified."],
          }],
        });
      }
      if (url.searchParams.get("source_fiat") === "AMD" && url.searchParams.get("target_fiat") === "RUB" && url.searchParams.get("source_amount") === "42269") {
        const idPay = offer("id-pay", "indicative-amd-rub", "AMD", "RUB");
        idPay.price = "4.2269";
        idPay.payment_methods = ["IDBank", "Alfa-Bank"];
        idPay.advertiser = {
          ...idPay.advertiser,
          id: null,
          nickname: "ID Pay",
          user_type: "service",
          completed_orders_30d: null,
          completion_rate_30d: null,
        };
        idPay.source_url = "https://id-pay.ru/";
        return json({
          search_id: "00000000-0000-4000-8000-000000000108",
          routes_found: 1,
          searched_at: "2026-09-26T11:00:00Z",
          source_fiat: "AMD",
          target_fiat: "RUB",
          source_amount: "42269.00",
          assets_searched: [],
          can_exchange_to_target: true,
          routes: [{
            route_id: "route-amd-rub-id-pay",
            rank: 1,
            asset: "RUB",
            source_fiat: "AMD",
            source_amount: "42269.00",
            acquired_asset_amount: "10000.00",
            target_fiat: "RUB",
            target_amount: "10000.00",
            effective_rate: "0.23657900",
            same_venue: true,
            requires_asset_transfer: false,
            transfer_fee_included: true,
            route_kind: "fiat_to_fiat",
            payment_methods_verified: false,
            entry_offer: idPay,
            exit_offer: null,
            warnings: ["Indicative direct-transfer quote."],
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
  const refreshSettings = page.getByRole("dialog", { name: "Refresh settings" });
  if ((page.viewportSize()?.width ?? 0) > 640) await expect(refreshSettings).toHaveCSS("width", "310px");
  await refreshSettings.getByRole("button", { name: "5m" }).click();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("pay3flow.exchange.refresh-seconds"))).toBe("300");

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
  const bestRoute = page.getByTestId("complete-route").first();
  const alternativeRoute = page.getByTestId("complete-route").nth(1);
  await alternativeRoute.locator(".routeRank").click();
  await expect(alternativeRoute).toHaveClass(/selected/);
  await expect(bestRoute).not.toHaveClass(/selected/);
  await expect(alternativeRoute.getByRole("button", { name: /Select route 2:/ })).toHaveAttribute("aria-pressed", "true");
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
  await expect(instructions.getByText("Bank fees: IDBank: 0.75% bank fee")).toBeVisible();
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

test("RUB to RUB bank routes explain SBP payment", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("Sberbank");
  await sourcePicker.getByRole("option", { name: /Sberbank/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("Alfa");
  await targetPicker.getByRole("option", { name: /Alfa-Bank/ }).click();

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();
  const route = page.getByTestId("complete-route").first();
  await expect(route).toBeVisible();
  await route.locator(".routeAmount").click();

  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByText("Buy USDT for 10,000 RUB")).toBeVisible();
  await expect(instructions.getByText("For RUB, use СБП from Sberbank using the exact recipient details shown in the order.")).toBeVisible();
  await expect(instructions.getByText("For RUB payout to Alfa-Bank, confirm the СБП transfer has arrived before releasing the crypto.")).toBeVisible();
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

test("catalog and direct quote providers are separately selectable", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Choose exchanges" }).click();
  await expect(page.getByRole("dialog", { name: "Exchange settings" })).toBeVisible();
  const whitebird = page.getByRole("button", { name: "Whitebird" });
  const cifra = page.getByRole("button", { name: "Cifra Markets" });
  const bestchange = page.getByRole("button", { name: "BestChange" });
  const dzengi = page.getByRole("button", { name: "Dzengi" });
  const exnode = page.getByRole("button", { name: "Exnode" });
  const cow = page.getByRole("button", { name: "CoW Protocol Live" });
  const near = page.getByRole("button", { name: "NEAR 1Click" });
  const idPay = page.getByRole("button", { name: "ID Pay Live" });

  await expect(cifra).toBeEnabled();
  await expect(cifra).toHaveAttribute("aria-pressed", "true");
  await expect(cifra.locator("img")).toHaveAttribute("src", "/icons/venues/cifra-broker.png");
  await expect(whitebird).toBeEnabled();
  await whitebird.click();
  await expect(whitebird).toHaveAttribute("aria-pressed", "true");
  for (const [button, icon] of [[bestchange, "/icons/venues/bestchange.svg"], [dzengi, "/icons/venues/dzengi.svg"], [exnode, "/icons/venues/exnode.svg"]] as const) {
    await expect(button).toBeEnabled();
    await expect(button).toHaveAttribute("aria-pressed", "false");
    await expect(button.locator("img")).toHaveAttribute("src", icon);
    await button.click();
    await expect(button).toHaveAttribute("aria-pressed", "true");
  }
  await expect(cow).toBeEnabled();
  await expect(cow).toHaveAttribute("aria-pressed", "true");
  await expect(cow.locator("img")).toHaveAttribute("src", "/icons/venues/cow-swap-favicon.svg");
  await expect(near).toBeEnabled();
  await expect(near).toHaveAttribute("aria-pressed", "true");
  await expect(near.locator("img")).toHaveAttribute("src", "/icons/assets/near.webp");
  await expect(idPay).toBeEnabled();
  await expect(idPay).toHaveAttribute("aria-pressed", "true");
  await expect(idPay.locator("img")).toHaveAttribute("src", "/icons/venues/id-pay.svg");
});

test("saved provider choices adopt new providers and retain later deselections", async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem("pay3flow.exchange.p2p-sources", "cow-swap");
  });
  await mockBackend(page, { includeNewProviders: true });
  await openApp(page);

  await page.getByRole("button", { name: "Choose exchanges" }).click();
  const picker = page.getByRole("dialog", { name: "Exchange settings" });
  for (const name of ["Bitcoin Center", "bncex", "SkyLabs", "Symbiosis"]) {
    await expect(picker.getByRole("button", { name })).toHaveAttribute("aria-pressed", "true");
  }
  const symbiosis = picker.getByRole("button", { name: "Symbiosis" });
  await symbiosis.click();
  await expect(symbiosis).toHaveAttribute("aria-pressed", "false");

  await page.reload();
  await page.getByRole("button", { name: "Choose exchanges" }).click();
  await expect(page.getByRole("dialog", { name: "Exchange settings" }).getByRole("button", { name: "Symbiosis" }))
    .toHaveAttribute("aria-pressed", "false");
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
      expect(request.query.sources).toBe("binance,bybit,cifra-broker,cow-swap,id-pay,near-intents,whitebird");

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
        asset_statuses: [{
          asset: "USDT",
          entry_offers: 3,
          exit_offers: 2,
          routes_built: 1,
          can_exchange_to_target: true,
          entry_sources: [
            { source: "bybit", ok: true, latency_ms: 10, offers_found: 1, error: null },
            { source: "skylabs", ok: true, latency_ms: 15, offers_found: 2, error: null },
            { source: "binance", ok: true, latency_ms: 20, offers_found: 0, error: null },
          ],
          exit_sources: [{ source: "okx", ok: true, latency_ms: 12, offers_found: 1, error: null }],
        }],
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
  const exchangesButton = page.getByRole("button", { name: "Choose exchanges" });
  await exchangesButton.click();
  await page.getByRole("button", { name: "Whitebird" }).click();
  if ((page.viewportSize()?.width ?? 0) <= 640) {
    await page.locator(".settingsBackdrop").dispatchEvent("mousedown");
  } else {
    await exchangesButton.click();
  }
  await expect(page.getByRole("dialog", { name: "Exchange settings" })).toHaveCount(0);
  await page.getByLabel("Amount to send").fill("100000");
  await page.getByTestId("start-search").click();

  await expect.poll(() => Boolean(sendFirstRoute)).toBe(true);
  await page.waitForTimeout(750);
  expect(socketConnections).toBe(1);
  const panelTop = page.locator("#routes .panelTop");
  const searchingVenues = panelTop.getByTestId("searching-venue");
  await expect(searchingVenues).toHaveCount(5);
  await expect(searchingVenues.nth(0)).toHaveAttribute("title", "Searching Binance");
  await expect(searchingVenues.nth(1)).toHaveAttribute("title", "Searching Bybit");
  await expect(searchingVenues.nth(2)).toHaveAttribute("title", "Searching Cifra Markets");
  await expect(searchingVenues.nth(3)).toHaveAttribute("title", "Searching CoW Protocol Live");
  await expect(searchingVenues.nth(4)).toHaveAttribute("title", "Searching ID Pay Live");
  await expect(panelTop.getByTestId("searching-venues-overflow")).toHaveText("...");
  await expect(searchingVenues.nth(0)).toHaveCSS("width", "32px");
  await expect(searchingVenues.nth(0)).toHaveCSS("animation-delay", "0s");
  await expect(searchingVenues.nth(1)).toHaveCSS("animation-delay", "0.13s");

  const refreshButton = page.getByRole("button", { name: "Refresh routes now" });
  await expect(refreshButton.locator("svg")).toHaveClass(/refreshSpin/);
  sendFirstRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(1);
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  const foundVenues = panelTop.locator(".resultSummary").getByTestId("found-venue");
  await expect(foundVenues).toHaveCount(3);
  await expect(foundVenues.first()).toHaveAttribute("title", "Found on Bybit");
  await expect(foundVenues.nth(1)).toHaveAttribute("title", "Found on SkyLabs");
  await expect(foundVenues.nth(2)).toHaveAttribute("title", "Found on Okx");
  await expect(searchingVenues).toHaveCount(5);
  await expect(searchingVenues.nth(4)).toHaveAttribute("title", "Searching Whitebird");
  await expect(panelTop.getByTestId("searching-venues-overflow")).toHaveCount(0);
  await expect(refreshButton).toBeDisabled();
  await expect(refreshButton.locator("svg")).not.toHaveClass(/refreshSpin/);

  sendSecondRoute?.();
  await expect(page.getByTestId("complete-route")).toHaveCount(2);
  await expect(page.getByTestId("complete-route").first()).toContainText("20420 RUB");
  await expect(page.getByTestId("complete-route").first()).toHaveClass(/selected/);
  await expect(foundVenues).toHaveCount(2);
  await expect(foundVenues.nth(1)).toHaveAttribute("title", "Found on Whitebird");
  await expect(searchingVenues).toHaveCount(4);

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

test("direct provider quotes keep their API names and independent prices", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select sending bank: Ameriabank" }).click();
  const sourcePicker = page.getByRole("dialog", { name: "Choose where you pay from" });
  await sourcePicker.getByLabel("Search banks and payment methods").fill("USDT ERC20");
  await sourcePicker.getByRole("option", { name: /Tether USDT · Ethereum \(ERC-20\)/ }).click();

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("USDC ERC20");
  await targetPicker.getByRole("option", { name: /USD Coin USDC · Ethereum \(ERC-20\)/ }).click();

  await page.getByLabel("Amount to send").fill("100");
  await page.getByTestId("start-search").click();

  const cards = page.getByTestId("complete-route");
  await expect(cards).toHaveCount(2);
  await expect(cards.nth(0)).toContainText("99.6 USDC");
  await expect(cards.nth(0)).not.toContainText("Quote by");
  await expect(cards.nth(1)).toContainText("97.3 USDC");
  await expect(cards.nth(1)).not.toContainText("Quote by");
  await expect(cards.nth(0).locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDT Tether · ethereum → USDC USD Coin · ethereum (NEAR 1Click)",
  );
  await expect(cards.nth(1).locator(".workflow")).toHaveAttribute(
    "aria-label",
    "USDT Tether · ethereum → USDC USD Coin · ethereum (CoW Protocol Live)",
  );
});

test("ID Pay provides a direct AMD to RUB route with its API name", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByLabel("Amount to send").fill("42269");
  await page.getByTestId("start-search").click();

  const route = page.getByTestId("complete-route");
  await expect(route).toHaveCount(1);
  await expect(route).toContainText("10,000 RUB");
  await expect(route).not.toContainText("Quote by");
  await expect(route.locator(".workflow")).toHaveAttribute(
    "aria-label",
    "AMD → RUB (ID Pay Live)",
  );

  await route.locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expect(instructions.getByRole("heading", { name: "Transfer AMD to RUB via ID Pay Live" })).toBeVisible();
  await expect(instructions.getByRole("link", { name: "Open ID Pay Live exchange" })).toHaveAttribute("href", "https://id-pay.ru/");
});

test("small AMD to BTC@near routes keep crypto precision", async ({ page }) => {
  await mockBackend(page);
  await openApp(page);

  await page.getByRole("button", { name: "Select recipient bank: Sberbank" }).click();
  const targetPicker = page.getByRole("dialog", { name: "Choose where the recipient gets paid" });
  await targetPicker.getByLabel("Search banks and payment methods").fill("BTC NEAR");
  await targetPicker.getByRole("option", { name: /Bitcoin BTC · NEAR/ }).click();

  await page.getByLabel("Amount to send").fill("10000");
  await page.getByTestId("start-search").click();

  const route = page.getByTestId("complete-route").first();
  await expect(route).toBeVisible();
  await expect(route.locator(".routeAmount")).toHaveText("0.00032478 BTC");
  await expect(page.locator("#exchange-output")).toHaveText("0.00032478");
  await expect(route.locator(".workflow")).toHaveAttribute(
    "aria-label",
    "AMD → USDT Tether · optimism (Bybit) → BTC Bitcoin · near (NEAR 1Click)",
  );
  await route.locator(".routeAmount").click();
  const instructions = page.getByRole("dialog", { name: "How to complete this exchange" });
  await expectNumberedTimeline(instructions, ["1", "2"]);
  await expect(instructions.getByText("Buy USDT for 10,000 AMD")).toBeVisible();
  await expect(instructions.getByRole("heading", { name: "Swap USDT for BTC via NEAR 1Click" })).toBeVisible();
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

  const routeCard = page.getByTestId("complete-route").nth(1);
  await expect(routeCard.getByRole("button", { name: "Like this route", exact: true })).toBeVisible();
  await expect(routeCard.getByRole("button", { name: "Dislike this route", exact: true })).toBeVisible();
  await expect(routeCard.getByLabel("850 likes")).toHaveCount(0);
  await expect(routeCard.getByLabel("40 dislikes")).toHaveCount(0);
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "dark";
  });
  await expect(routeCard).toHaveCSS(
    "background-color",
    "rgb(32, 32, 32)",
  );
  await expect(routeCard.locator(".routeFeedback")).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await routeCard.getByRole("button", { name: "Like this route", exact: true }).click();
  await expect(routeCard.getByRole("button", { name: "Like this route", exact: true })).toHaveAttribute("aria-pressed", "true");
  await expect(routeCard.getByRole("button", { name: "Like this route", exact: true })).toHaveCSS("color", "rgb(197, 255, 34)");
  await expect(routeCard.getByLabel("1 likes")).toBeVisible();
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
  await page.reload();
  await expect(page.getByRole("button", { name: "Select sending asset: Bitcoin" })).toContainText("Bitcoin");
  await expect(page.locator(".moneyPanelSource .networkCopy strong")).toHaveText("Bitcoin");
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
