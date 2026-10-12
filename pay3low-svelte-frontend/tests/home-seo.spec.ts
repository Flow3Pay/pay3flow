import { expect, test } from "@playwright/test";

const canonical = "https://pay3flow.lefine.pro/";
const heading = "Переводите деньги. Сохраняйте больше.";

test("server HTML contains Russian product content, canonical and brand metadata", async ({ request }) => {
  for (const path of ["/", "/?utm_source=seo-test"]) {
    const response = await request.get(path, { headers: { "Accept-Language": "en-US" } });
    expect(response.status()).toBe(200);
    const html = await response.text();
    expect(html).toMatch(/<html lang="ru"/);
    expect(html).toContain("Переводите деньги.");
    expect(html).toContain("Сохраняйте больше.");
    expect(html.match(/<h1\b/g)).toHaveLength(1);
    expect(html).not.toContain('data-testid="home-overview"');
    expect(html).toMatch(/<title>Pay3Flow — сравнение маршрутов обмена валют и криптовалют<\/title>/);
    expect(html.match(/name="description"/g)).toHaveLength(1);
    expect(html).toContain(`rel="canonical" href="${canonical}"`);
    expect(html).toContain("включите JavaScript в браузере");
    expect(html).not.toMatch(/<div[^>]*class="introOverlay/);
    expect(html).not.toMatch(/<link[^>]+RouteInstructions/);
    const json = html.match(/<script type="application\/ld\+json">(.*?)<\/script>/s);
    expect(json).not.toBeNull();
    expect(JSON.parse(json![1])).toMatchObject({ "@type": "WebSite", name: "Pay3Flow", url: canonical, inLanguage: "ru" });
  }
});

test("homepage product content is usable without JavaScript", async ({ browser, baseURL, isMobile }) => {
  const context = await browser.newContext({
    javaScriptEnabled: false,
    viewport: isMobile ? { width: 393, height: 851 } : { width: 1280, height: 900 },
  });
  try {
    const page = await context.newPage();
    await page.goto(baseURL!);
    await expect(page.getByRole("heading", { level: 1 })).toHaveText(heading);
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await expect(page.locator(".workspace")).toBeVisible();
    await expect(page.locator(".nojsNotice")).toBeVisible();
    await expect(page.locator(".nojsNotice")).toContainText("включите JavaScript");
    await expect(page.getByTestId("home-overview")).toHaveCount(0);
    await page.goto(`${baseURL}/about`);
    await expect(page.getByRole("heading", { level: 1, name: "Что такое Pay3Flow", exact: true })).toBeVisible();
    await expect(page.locator(".header")).toBeVisible();
    await expect(page.locator(".workspace")).toHaveCount(0);
    await expect(page.locator(".flowPreview, .faq")).toHaveCount(0);
    await expect(page.getByRole("link", { name: "Исходный код и сообщения об ошибках" })).toHaveAttribute("href", "https://github.com/Flow3Pay/pay3flow");
    expect(await page.locator("body").evaluate((body) => body.scrollWidth <= window.innerWidth)).toBe(true);
  } finally {
    await context.close();
  }
});

test("content survives API failures and resizing without loading instructions", async ({ page }) => {
  const errors: string[] = [];
  const instructionRequests: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("request", (request) => {
    if (/RouteInstructions\.(?:svelte|[^/]+\.css)/.test(request.url())) instructionRequests.push(request.url());
  });
  await page.route("**/api/**", (route) => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto("/");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(heading);
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.getByRole("button", { name: "Переключить язык" })).toBeVisible();
  for (const width of [1024, 393, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(page.locator(".workspace")).toBeVisible();
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  }
  expect(instructionRequests).toEqual([]);
  expect(errors).toEqual([]);
});

test("saved language restores after hydration and updates page metadata", async ({ page }) => {
  await page.addInitScript(() => {
    if (!localStorage.getItem("pay3flow-locale")) localStorage.setItem("pay3flow-locale", "en");
  });
  await page.route("**/api/**", (route) => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto("/about");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page).toHaveTitle("Pay3Flow — What is Pay3Flow?");
  await expect(page.locator("#about-heading")).toHaveText("What is Pay3Flow?");
  await expect(page.locator('meta[name="description"]')).toHaveAttribute("content", /^Pay3Flow is an experimental/);
  await page.locator(".languageToggle").click();
  const languages = page.getByRole("listbox");
  await expect(languages.getByRole("option").locator("span[lang]")).toHaveText(["English", "Русский", "Հայերեն"]);
  await expect(languages.getByRole("option", { name: "English" })).toHaveAttribute("aria-selected", "true");
  await expect.poll(() => languages.locator("img").evaluateAll(images => images.every(image => (image as HTMLImageElement).complete && (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  await page.keyboard.press("Escape");
  await expect(languages).toHaveCount(0);
  await expect(page.locator(".languageToggle")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(languages.getByRole("option", { name: "English" })).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(languages).toHaveCount(0);
  await expect(page.locator("html")).toHaveAttribute("lang", "ru");
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Что такое Pay3Flow");
  await expect(page).toHaveTitle("Pay3Flow — Что такое Pay3Flow");
  await expect(page.locator("#about-heading")).toHaveText("Что такое Pay3Flow");
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute("href", `${canonical}about`);
  await page.reload();
  await expect.poll(() => page.locator(".appShell").evaluate((element) => getComputedStyle(element, "::before").backgroundImage)).not.toBe("none");
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 6000 });
  await expect(page.locator("html")).toHaveAttribute("lang", "ru");
  await page.locator(".languageToggle").click();
  await page.getByRole("option", { name: "Հայերեն" }).click();
  await expect(page.locator("html")).toHaveAttribute("lang", "hy");
  await expect(page.locator(".languageToggle img")).toHaveAttribute("src", "/icons/flags/am.svg");
  await page.locator(".languageToggle").click();
  await page.locator("#about-heading").click();
  await expect(page.getByRole("listbox")).toHaveCount(0);
  await expect(page).toHaveTitle("Pay3Flow — Ի՞նչ է Pay3Flow-ը");
  await expect(page.locator('meta[name="description"]')).toHaveAttribute("content", /^Pay3Flow/);
});


test("about page moves the overview off swap and preserves the shared header and background", async ({ page, request }) => {
  const response = await request.get("/about");
  expect(response.status()).toBe(200);
  const html = await response.text();
  expect(html.match(/<h1\b/g)).toHaveLength(1);
  expect(html).toContain(`rel="canonical" href="${canonical}about"`);
  expect(html).toContain("OTC (beta)");
  expect(html).toContain("Binance, Bybit, OKX, Bitget");
  expect(html).toContain('data-testid="home-overview"');
  await page.route("**/api/**", route => route.fulfill({ status: 503, contentType: "application/json", body: "{}" }));
  await page.goto("/");
  await expect.poll(() => page.locator(".appShell").evaluate(node => (node as HTMLElement).style.getPropertyValue("--puzzle-pattern"))).not.toBe("");
  await expect(page.locator(".introOverlay")).toHaveCount(0, { timeout: 7000 });
  await expect(page.getByTestId("home-overview")).toHaveCount(0);
  const background = await page.locator(".appShell").evaluate(node => getComputedStyle(node).backgroundImage);
  await page.locator(".menuToggle").click();
  await page.locator(".aboutLink").click();
  await expect(page).toHaveURL(/\/about$/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Что такое Pay3Flow");
  await expect(page.getByTestId("home-overview")).toBeVisible();
  await expect(page.locator(".workspace")).toHaveCount(0);
  await expect(page.locator(".header .productNav a")).toHaveText(["SWAP", "OTC", "Площадки"]);
  await expect.poll(() => page.locator(".appShell").evaluate(node => getComputedStyle(node, "::before").backgroundImage)).not.toBe("none");
  expect(await page.locator(".appShell").evaluate(node => getComputedStyle(node).backgroundImage)).toBe(background);
  for (const width of [1280, 393, 320]) {
    await page.setViewportSize({ width, height: 900 });
    expect(await page.locator("body").evaluate(node => node.scrollWidth <= innerWidth)).toBe(true);
  }
  await page.locator('.productNav a[href="/#/otc"]').click();
  await expect(page.locator(".otcWorkspace")).toBeVisible();
  await expect(page.getByTestId("home-overview")).toHaveCount(0);
  await page.locator('.productNav a').first().click();
  await expect(page.locator(".workspace")).toBeVisible();
  await expect(page.getByTestId("home-overview")).toHaveCount(0);
});

test("robots and sitemap expose canonical public URLs", async ({ request }) => {
  const robots = await request.get("/robots.txt");
  expect(robots.status()).toBe(200);
  expect(await robots.text()).toContain(`Sitemap: ${canonical}sitemap.xml`);
  expect(await robots.text()).toContain("Allow: /");
  const sitemap = await request.get("/sitemap.xml");
  expect(sitemap.status()).toBe(200);
  expect(sitemap.headers()["content-type"]).toMatch(/(?:application|text)\/xml/);
  const xml = await sitemap.text();
  expect(xml).toContain(`<loc>${canonical}</loc>`);
  expect(xml).toContain(`<loc>${canonical}terms</loc>`);
  expect(xml).toContain(`<loc>${canonical}about</loc>`);
  expect(xml).not.toContain("utm_");
});
