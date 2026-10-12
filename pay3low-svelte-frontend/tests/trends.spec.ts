import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("pay3flow-locale", "en"));
  await page.route("**/api/**", (route) => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto("/trends");
  await expect(page.getByTestId("trends-workspace")).toBeVisible();
  await expect(page.getByTestId("trends-workspace")).toHaveAttribute("aria-busy", "false");
  await expect(page.locator(".svelte-flow")).toBeVisible();
  await expect(page.locator(".svelte-flow__attribution")).toHaveCount(0);
  await expect(page.locator(".brick")).toHaveCount(60);
  await page.evaluate(() => document.fonts.ready);
  await expect.poll(async () => page.locator(".brickEntry").evaluateAll((nodes) => nodes.every((node) => getComputedStyle(node).animationName === "none"))).toBe(true);
  await page.getByRole("button", { name: "Fit wall", exact: true }).click();
});

test("trends sharing previews the wall and copies a localized page link", async ({ page, browser }) => {
  await page.evaluate(() => Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (value: string) => sessionStorage.setItem("test.trends-share", value) } }));
  await page.getByRole("button", { name: "Switch language", exact: true }).click();
  await page.getByRole("option", { name: "Русский", exact: true }).click();
  const button = page.getByRole("button", { name: "Поделиться трендами", exact: true });
  const transform = await page.locator(".svelte-flow__viewport").getAttribute("style");
  await button.click();
  const dialog = page.getByRole("dialog", { name: "Поделиться трендами", exact: true });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText("По этой ссылке откроется стена трендов обмена.");
  await expect(dialog.getByRole("img", { name: "Стена трендов обмена" })).toHaveAttribute("src", /\/trends\/preview\.png\?lang=ru/);
  await expect.poll(() => dialog.locator(".sharePreview").evaluate((image: HTMLImageElement) => image.naturalWidth)).toBe(1200);
  const link = await dialog.getByLabel("Ссылка на тренды").inputValue();
  expect(new URL(link).pathname).toBe("/trends");
  expect(new URL(link).searchParams.get("lang")).toBe("ru");
  await dialog.getByRole("button", { name: "Скопировать ссылку", exact: true }).click();
  expect(await page.evaluate(() => sessionStorage.getItem("test.trends-share"))).toBe(link);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(button).toBeFocused();
  expect(await page.locator(".svelte-flow__viewport").getAttribute("style")).toBe(transform);
  const fresh = await browser.newPage();
  try {
    await fresh.goto(link);
    await expect(fresh.getByRole("button", { name: "Поделиться трендами", exact: true })).toBeVisible();
  } finally { await fresh.close(); }
});

test("trends links serve localized messenger metadata and a public wall PNG", async ({ request }, testInfo) => {
  for (const [language, title] of [["en", "TRENDS"], ["ru", "Тренды"], ["hy", "Թրենդներ"]]) {
    const response = await request.get(`/trends?lang=${language}`, { headers: { "User-Agent": "TelegramBot (like TwitterBot)" } });
    expect(response.status()).toBe(200);
    const html = await response.text();
    expect(html).toContain(`<title>${title} · Pay3Flow</title>`);
    expect(html).toContain('property="og:image:width" content="1200"');
    expect(html).toContain('property="og:image:height" content="630"');
    expect(html).toContain('name="twitter:card" content="summary_large_image"');
    const images = [...html.matchAll(/property="og:image" content="([^"]+)"/g)];
    expect(images).toHaveLength(1);
    const image = await request.get(images[0][1].replaceAll("&amp;", "&"));
    expect(image.status()).toBe(200);
    expect(image.headers()["content-type"]).toBe("image/png");
    expect(image.headers()["cache-control"]).toContain("public");
    const png = await image.body();
    expect(png.subarray(1, 4).toString()).toBe("PNG");
    expect(png.readUInt32BE(16)).toBe(1200);
    expect(png.readUInt32BE(20)).toBe(630);
    if (language === "ru") await testInfo.attach("trends-preview", { body: png, contentType: "image/png" });
  }
});

test("wall filters, periods, ranking and details work on desktop and touch", async ({ page }, testInfo) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await expect(page.getByRole("link", { name: "TRENDS", exact: true })).toHaveAttribute("aria-current", "page");
  const bricks = page.locator(".brick");
  await expect(bricks).toHaveCount(60);
  await expect(page.locator('.brick[data-parts="2"]')).toHaveCount(12);
  await expect(page.locator('.brick[data-parts="2"] .workflowVenue')).toHaveCount(0);
  await expect(page.locator('.brick[data-parts="3"]')).toHaveCount(48);
  await expect(page.locator('.brick .workflowVenue')).toHaveCount(48);
  const first = await bricks.first().getAttribute("aria-label");
  if (testInfo.project.name === "chromium") await bricks.first().hover();
  else await bricks.first().focus();
  const details = page.getByRole("region", { name: "Combination details" });
  await expect(details).toBeVisible();
  await expect(details.getByRole("link", { name: "Find a route" })).toHaveAttribute("href", /#\/swap\//);
  await page.keyboard.press("Escape");
  await expect(details).toHaveCount(0);
  await expect(bricks.first()).toBeFocused();
  await page.keyboard.press("Tab");
  await bricks.first().focus();
  await expect(details).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(details).toHaveCount(0);
  await page.getByRole("button", { name: "In results", exact: true }).click();
  await expect(bricks.first()).not.toHaveAttribute("aria-label", first!);
  const week = await bricks.first().getAttribute("aria-label");
  await page.getByRole("button", { name: "24H", exact: true }).click();
  await expect(bricks.first()).not.toHaveAttribute("aria-label", week!);
  const filter = page.getByRole("textbox", { name: "Find an asset, network or platform" });
  await filter.fill("Bitcoin");
  await expect(bricks).toHaveCount(5);
  await filter.fill("no-such-asset");
  await expect(page.getByRole("heading", { name: "No matching bricks" })).toBeVisible();
  await filter.fill("");
  await page.getByRole("button", { name: "My activity", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Your wall starts with a search." })).toBeVisible();
  for (const width of [320, 393, 768, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    expect(await page.locator("header .inner").evaluate((node) => node.scrollWidth <= node.clientWidth)).toBe(true);
  }
  expect(errors).toEqual([]);
});

test("browser activity survives reload, stays separate from examples and updates across tabs", async ({ page }) => {
  const event = { at: Date.now(), blocks: [{ asset: "BTC", network: "bitcoin", provider: "bestchange", appearances: 3, source: "RUB", target: "BTC" }] };
  await page.evaluate((event) => {
    localStorage.setItem("pay3flow.trends.activity.v1", JSON.stringify([event]));
    window.dispatchEvent(new StorageEvent("storage", { key: "pay3flow.trends.activity.v1" }));
  }, event);
  await page.getByRole("button", { name: "My activity", exact: true }).click();
  await expect(page.locator(".brick")).toHaveCount(1);
  await expect(page.locator(".brick")).toHaveAttribute("aria-label", /Searches: 1/);
  await page.getByRole("button", { name: "In results", exact: true }).click();
  await expect(page.locator(".brick")).toHaveAttribute("aria-label", /In results: 3/);
  await page.reload();
  await page.getByRole("button", { name: "My activity", exact: true }).click();
  await expect(page.locator(".brick")).toHaveCount(1);
  await page.getByRole("button", { name: "Example", exact: true }).click();
  await expect(page.locator(".brick")).toHaveCount(60);
});

test("the wall supports keyboard access and has no serious accessibility violations", async ({ page }) => {
  await page.locator(".brick").first().focus();
  await expect(page.getByRole("region", { name: "Combination details" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("region", { name: "Combination details" })).toHaveCount(0);
  const results = await new AxeBuilder({ page }).include("main").analyze();
  expect(results.violations.filter((violation) => violation.impact === "serious" || violation.impact === "critical")).toEqual([]);
});

test("network-only bricks and platform bricks retain their respective details and filters", async ({ page }) => {
  await page.evaluate(() => {
    localStorage.setItem("pay3flow.trends.activity.v1", JSON.stringify([{ at: Date.now(), blocks: [
      { asset: "BTC", network: "bitcoin", provider: "", appearances: 2, source: "BTC", target: "USDT" },
      { asset: "USDT", network: "tron", provider: "bestchange", appearances: 2, source: "BTC", target: "USDT" },
    ] }]));
    window.dispatchEvent(new StorageEvent("storage", { key: "pay3flow.trends.activity.v1" }));
  });
  await page.getByRole("button", { name: "My activity", exact: true }).click();
  await expect(page.locator(".brick")).toHaveCount(2);
  const brick = page.locator('.brick[data-parts="2"]').first();
  await brick.focus();
  const details = page.getByRole("region", { name: "Combination details" });
  await expect(details).toBeVisible();
  const href = await details.getByRole("link", { name: "Find a route" }).getAttribute("href");
  expect(new URLSearchParams(href!.split("?")[1]).has("sources")).toBe(false);
  await page.keyboard.press("Escape");
  const venueBrick = page.locator('.brick[data-parts="3"]');
  await expect(venueBrick.locator(".workflowVenue")).toHaveText("BestChange");
  await venueBrick.focus();
  await expect(details.locator(".detailHeader")).toContainText("BestChange");
  const venueHref = await details.getByRole("link", { name: "Find a route" }).getAttribute("href");
  expect(new URLSearchParams(venueHref!.split("?")[1]).get("sources")).toBe("bestchange");
  await page.keyboard.press("Escape");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("bricks fit their contents and details leave the expanded brick visible", async ({ page }, testInfo) => {
  const bare = page.locator('.brick[data-parts="2"]').filter({ has: page.locator('.workflowAsset strong', { hasText: /^USDT$/ }) }).first();
  const venue = page.locator('.brick[data-parts="3"]').filter({ has: page.locator('.workflowAsset strong', { hasText: /^USDT$/ }) }).first();
  await expect.poll(async () => (await bare.boundingBox())!.width).toBeLessThan((await venue.boundingBox())!.width);
  for (const width of [320, 393, 768, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await expect.poll(async () => page.locator('.brick').evaluateAll((nodes) => {
      const boxes = nodes.map((node) => node.getBoundingClientRect());
      return boxes.every((box, index) => boxes.slice(index + 1).every((other) => Math.abs(box.top - other.top) > 2 || box.right <= other.left + 1 || other.right <= box.left + 1));
    })).toBe(true);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.setViewportSize({ width: testInfo.project.name === "chromium" ? 1280 : 393, height: 900 });
  await page.getByRole("button", { name: "Fit wall", exact: true }).click();
  const before = await bare.boundingBox();
  if (testInfo.project.name === "chromium") await bare.hover();
  else await bare.focus();
  const details = page.getByRole("region", { name: "Combination details" });
  await expect(details).toBeVisible();
  await expect.poll(async () => {
    const card = await details.boundingBox();
    const brick = await bare.boundingBox();
    return Boolean(card && brick && (card.y + card.height <= brick.y - 4 || card.y >= brick.y + brick.height + 4));
  }).toBe(true);
  // Expansion keeps the block attached to its course rather than translating upward.
  await expect(bare).toHaveCSS("transform", "matrix(1.045, 0, 0, 1.045, 0, 0)");
  expect((await bare.boundingBox())!.width).toBeGreaterThan(before!.width);
  await page.keyboard.press("Escape");
  await expect(details).toHaveCount(0);
});

test("clicking a brick opens the same route and filters as Find a route", async ({ page, isMobile }) => {
  const brick = page.locator('.brick[data-parts="3"]').first();
  await brick.focus();
  const route = page.getByRole("region", { name: "Combination details" }).getByRole("link", { name: "Find a route" });
  const href = await route.getAttribute("href");
  await expect(brick).toHaveAttribute("href", href!);
  if (isMobile) {
    const box = (await brick.boundingBox())!;
    await page.touchscreen.tap(box.x + box.width / 2, box.y + box.height / 2);
  } else await brick.click();
  await expect(page).toHaveURL(new RegExp(href!.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "$"));
});

test("canvas fills the viewport, pans and zooms while its toolbar stays fixed", async ({ page }) => {
  const canvas = page.locator(".svelte-flow");
  const viewport = page.locator(".svelte-flow__viewport");
  const toolbar = page.locator(".trendsToolbar");
  const original = await viewport.getAttribute("style");
  const toolbarBox = await toolbar.boundingBox();
  const canvasBox = (await canvas.boundingBox())!;
  expect(canvasBox.y + canvasBox.height).toBeCloseTo((await page.evaluate(() => innerHeight)), 0);
  await expect(page.locator(".intro, .wallFooter, .wallHeader, .summary, .leaders, .methodNote")).toHaveCount(0);
  expect(await page.getByTestId("trend-brick-background").locator("pattern rect").count()).toBeGreaterThan(3);
  const x = canvasBox.x + canvasBox.width - 40;
  const y = canvasBox.y + canvasBox.height - 180;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x - 100, y - 80, { steps: 12 });
  await page.mouse.up();
  await expect(viewport).not.toHaveAttribute("style", original!);
  expect(await toolbar.boundingBox()).toEqual(toolbarBox);
  expect(await page.evaluate(() => scrollY)).toBe(0);
  const panned = await viewport.getAttribute("style");
  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await expect(viewport).not.toHaveAttribute("style", panned!);
  const zoomed = await viewport.getAttribute("style");
  await page.getByRole("button", { name: "Zoom out", exact: true }).click();
  await expect(viewport).not.toHaveAttribute("style", zoomed!);
  await page.getByRole("button", { name: "Fit wall", exact: true }).click();
  expect(await toolbar.boundingBox()).toEqual(toolbarBox);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.locator("header .themeToggle").click();
  const colors = await page.getByRole("button", { name: "Zoom in", exact: true }).evaluate((node) => {
    const button = getComputedStyle(node);
    const panel = getComputedStyle(document.querySelector(".mode")!);
    return { background: button.backgroundColor, color: button.color, panel: panel.backgroundColor };
  });
  expect(colors.background).toBe(colors.panel);
  expect(colors.color).not.toBe(colors.background);
});

test("details after canvas movement do not scroll the document or cover the brick", async ({ page }) => {
  const brick = page.locator(".brick").first();
  await brick.evaluate((node) => (node as HTMLElement).focus({ preventScroll: true }));
  const details = page.getByRole("region", { name: "Combination details" });
  await expect(details).toBeVisible();
  await expect.poll(async () => {
    const card = (await details.boundingBox())!;
    const anchor = (await brick.boundingBox())!;
    return card.y >= anchor.y + anchor.height + 4 || card.y + card.height <= anchor.y - 4;
  }).toBe(true);
  expect(await page.evaluate(() => scrollY)).toBe(0);
  const canvas = (await page.locator(".svelte-flow").boundingBox())!;
  await page.mouse.move(canvas.width - 5, canvas.y + 180);
  await page.mouse.wheel(0, -100);
  await expect(details).toHaveCount(0);
  expect(await page.evaluate(() => scrollY)).toBe(0);
});

test("empty slots follow natural brick widths and never sit behind occupied blocks", async ({ page }) => {
  const background = page.getByTestId("trend-brick-background");
  const patternWidths = await background.locator("pattern rect").evaluateAll((nodes) => [...new Set(nodes.map((node) => node.getAttribute("width")))]);
  expect(patternWidths.length).toBeGreaterThan(2);
  for (const width of [393, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await page.getByRole("button", { name: "Fit wall", exact: true }).click();
    await expect.poll(async () => page.locator(".emptySlot").evaluateAll((nodes) => {
      const bricks = [...document.querySelectorAll(".brick")].map((node) => node.getBoundingClientRect());
      return nodes.every((node) => {
        const slot = node.getBoundingClientRect();
        return bricks.every((brick) => slot.right <= brick.left || slot.left >= brick.right || slot.bottom <= brick.top || slot.top >= brick.bottom);
      });
    })).toBe(true);
  }
  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await expect.poll(async () => page.locator(".emptySlot").evaluateAll((nodes) => {
    const bricks = [...document.querySelectorAll(".brick")].map((node) => node.getBoundingClientRect());
    return nodes.every((node) => {
      const slot = node.getBoundingClientRect();
      return bricks.every((brick) => slot.right <= brick.left || slot.left >= brick.right || slot.bottom <= brick.top || slot.top >= brick.bottom);
    });
  })).toBe(true);
});


test("navigation translates Trends while keeping SWAP and OTC labels", async ({ page }) => {
  await page.getByRole("button", { name: "Switch language", exact: true }).click();
  await page.getByRole("option", { name: "Русский", exact: true }).click();
  await expect(page.locator('nav.productNav a[href="/trends"]')).toHaveText("Тренды");
  await expect(page.locator('nav.productNav a').nth(0)).toHaveText("SWAP");
  await expect(page.locator('nav.productNav a').nth(1)).toHaveText("OTC");
  const brick = page.locator(".brick").first();
  await brick.evaluate((node) => (node as HTMLElement).focus({ preventScroll: true }));
  const details = page.locator(".trendDetails");
  await expect(details).toBeVisible();
  await expect(details.getByRole("button")).toHaveCount(0);
  expect((await details.boundingBox())!.width).toBeLessThanOrEqual(281);
});


test("bricks appear in courses from the bottom and respect reduced motion", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.reload();
  await expect(page.locator(".brickEntry")).toHaveCount(60);
  const delays = await page.locator(".brickEntry").evaluateAll((nodes) => nodes.map((node) => parseFloat((node as HTMLElement).style.animationDelay)));
  expect(delays[0]).toBeGreaterThan(delays.at(-1)!);
  await expect.poll(async () => page.locator(".brickEntry").evaluateAll((nodes) => nodes.every((node) => getComputedStyle(node).animationName === "none"))).toBe(true);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.reload();
  await expect(page.locator(".brickEntry")).toHaveCount(60);
  expect(await page.locator(".brickEntry").evaluateAll((nodes) => nodes.every((node) => getComputedStyle(node).animationName === "none" && getComputedStyle(node).opacity === "1"))).toBe(true);
});
