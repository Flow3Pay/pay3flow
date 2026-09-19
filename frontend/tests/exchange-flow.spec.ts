import { expect, Page, test } from "@playwright/test";

const order = {
  id: "00000000-0000-4000-8000-000000000001",
  source_country: "AM",
  source_currency: "AMD",
  source_amount_minor: 10_000_000,
  source_method_type: "bank_card",
  target_country: "RU",
  target_currency: "RUB",
  target_method_type: "bank_card",
  funding_status: "not_started",
  status: "created",
  selected_quote_id: null,
  failure_message: null,
  created_at: "2026-09-19T10:00:00Z",
};

async function mockBackend(page: Page) {
  let currentOrder = {
    ...order,
    status: order.status as string,
    funding_status: order.funding_status as string,
    selected_quote_id: null as string | null,
  };
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
    if (url.pathname === "/api/exchange/orders" && method === "GET") return json(currentOrder.status === "created" ? [] : [currentOrder]);
    if (url.pathname === "/api/exchange/orders" && method === "POST") return json(currentOrder);
    if (url.pathname.endsWith("/confirm") && !url.pathname.includes("funding")) {
      currentOrder = { ...currentOrder, status: "locked", funding_status: "shown_to_user", selected_quote_id: "quote-usdt" };
      return json({
        order: currentOrder,
        funding_instruction: {
          id: "funding-1",
          method_type: "bank_card",
          amount_minor: 10_000_000,
          currency: "AMD",
          destination_ref: "solver:demo:route",
          expires_at: "2026-09-19T11:00:00Z",
          raw_payload: { display_text: "Confirm funding" },
        },
        settlement: { id: "settlement-1", solver_id: "solver-1" },
      });
    }
    if (url.pathname.endsWith("/funding/confirm")) {
      currentOrder = { ...currentOrder, status: "proof_pending", funding_status: "solver_acknowledged" };
      return json({ order: currentOrder });
    }
    if (url.pathname.endsWith("/proof")) {
      currentOrder = { ...currentOrder, status: "done" };
      return json({ order: currentOrder });
    }
    return json({ error: `unmocked ${method} ${url.pathname}` }, 500);
  });

  await page.routeWebSocket(/localhost:8080\/api\/exchange\/orders\/.+\/live/, (socket) => {
    const base = {
      order_id: order.id,
      route_id: "route-usdt",
      status: "partial",
      source_amount_minor: 10_000_000,
      source_currency: "AMD",
      entry_asset: "USDT",
      entry_network: "ERC20",
      spread_bps: 8,
      legs: [
        { kind: "entry", from: "AMD", to: "USDT", provider: "am-p2p-mock", status: "found" },
        { kind: "exit", from: "USDT", to: "RUB", provider: "ru-buyer-mock", status: "searching" },
      ],
    };
    socket.send(JSON.stringify({ type: "search_started", order_id: order.id }));
    setTimeout(() => socket.send(JSON.stringify({ type: "entry_leg_found", ...base })), 30);
    setTimeout(() => socket.send(JSON.stringify({
      type: "route_candidate_found",
      ...base,
      status: "complete",
      quote_id: "quote-usdt",
      target_amount_minor: 2_035_000,
      target_currency: "RUB",
      fee_minor: 1200,
      eta_minutes: 12,
      is_current_best: true,
      legs: base.legs.map((leg) => ({ ...leg, status: "found" })),
    })), 120);
    setTimeout(() => socket.send(JSON.stringify({ type: "search_finished", order_id: order.id, best_quote_id: "quote-usdt" })), 180);
  });
}

test("login → live routes → funding consent → done", async ({ page }) => {
  await mockBackend(page);
  await page.goto("/");

  await page.getByRole("button", { name: "Войти", exact: true }).last().click();
  await expect(page.getByTestId("auth-form")).toBeHidden();

  await page.getByTestId("start-search").click();
  await expect(page.getByTestId("partial-route")).toBeVisible();
  await expect(page.getByTestId("complete-route")).toBeVisible();
  await page.getByTestId("complete-route").click();
  await expect(page.getByTestId("selected-route")).toBeVisible();
  await page.getByRole("button", { name: "Подтвердить полную связку" }).click();

  await expect(page.getByTestId("funding-instruction")).toBeVisible();
  await expect(page.getByTestId("confirm-funding")).toBeDisabled();
  await page.getByLabel(/Я подтверждаю инструкцию/).check();
  await page.getByTestId("confirm-funding").click();
  await expect(page.getByTestId("order-done")).toBeVisible();
});
