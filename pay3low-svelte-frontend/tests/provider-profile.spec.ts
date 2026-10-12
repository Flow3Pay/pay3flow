import { expect, test, type Page } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { writeFile } from 'node:fs/promises';

const providers = [
  { id: 'bybit-buy', slug: 'bybit', operation: 'buy', name: 'Bybit Buy', source_url: 'https://www.bybit.com', currencies: ['RUB', 'USDT', 'AMD'], currency_exceptions: [], banks: [], exchange_methods: ['p2p', 'exchanger'], market_types: ['p2p', 'spot'], searchable: true },
  { id: 'whitebird-buy', slug: 'whitebird', operation: 'buy', name: 'Whitebird Buy', source_url: 'https://whitebird.io', currencies: ['RUB', 'USDT'], currency_exceptions: [], banks: [], exchange_methods: ['exchanger'], market_types: ['exchanger'], searchable: true },
];
function statistics(period: string) {
  const days = period === '7d' ? 7 : period === '90d' ? 90 : 30;
  const timeline = Array.from({ length: days }, (_, index) => ({ started_at: new Date(Date.UTC(2026, 9, 10 - days + 1 + index)).toISOString(), searches: 100, top10: 40 + (index * 7 % 50), top1: 10 + (index * 3 % 20) }));
  const sum = (key: 'searches' | 'top10' | 'top1') => timeline.reduce((sum, day) => sum + day[key], 0);
  return { slug: 'bybit', period_days: days, searches: sum('searches'), top10: sum('top10'), top1: sum('top1'), average_rank: 3.7,
    response_samples: days * 100, successful_responses: days * 99, average_response_ms: 420, site_opens: 120,
    first_seen: '2026-08-01T00:00:00Z', updated_at: '2026-10-10T12:00:00Z', days: timeline,
    directions: [{ source_currency: 'USDT', target_currency: 'BTC', searches: 320, top10: 280, best_rank: 1 }, { source_currency: 'RUB', target_currency: 'AMD', searches: 290, top10: 240, best_rank: 2 }],
  };
}
const reviews = { source_url: 'https://www.bestchange.com/bybit-exchanger.html', fetched_at: '2026-10-10T00:00:00Z', reviews: [
  { id: 'positive', author: 'Алексей', text: 'Обмен прошёл быстро, условия совпали с предложением.', rating: 5, created_at: '2026-10-08T00:00:00Z', url: 'https://example.com/review/positive' },
  { id: 'critical', author: 'Мария', text: 'Пришлось подождать ответ поддержки.', rating: 2, created_at: '2026-10-07T00:00:00Z', url: 'https://example.com/review/critical' },
  { id: 'unrated', author: 'Иван', text: 'Комментарий без оценки.', rating: null, created_at: null, url: '' },
] };
async function mock(page: Page) {
  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    const body = url.pathname === '/api/providers' ? providers : url.pathname.endsWith('/statistics') ? statistics(url.searchParams.get('period') ?? '30d') : url.pathname.includes('/reviews/') ? reviews : {};
    await route.fulfill({ contentType: 'application/json', body: JSON.stringify(body) });
  });
}

test('profile changes periods, restores the period and filters reviews', async ({ page }) => {
  const errors: string[] = []; page.on('pageerror', error => errors.push(error.message));
  await mock(page); await page.goto('/providers/bybit');
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Bybit');
  await expect(page.locator('.venueAvatar img')).toHaveAttribute('src', '/icons/venues/bybit.png');
  await expect(page.locator('.identityMeta')).toContainText('P2P'); await expect(page.locator('.identityMeta')).toContainText('Spot');
  await expect(page.locator('.metric').nth(2).locator('strong')).toHaveText(/3\s?000/);
  await expect(page.locator('.directionPanel, .reviewSection, .aboutGrid')).toHaveCount(0);
  const initial = await page.locator('.line10').getAttribute('d');
  await page.getByRole('button', { name: '7 дней', exact: true }).click();
  await expect(page).toHaveURL(/period=7d/); await expect(page.locator('.metric').nth(2).locator('strong')).toHaveText('700');
  await expect(page.locator('.line10')).not.toHaveAttribute('d', initial!);
  await page.locator('.dayTarget').last().focus(); await expect(page.locator('.tooltip')).toContainText('UTC');
  await page.reload();
  await expect(page.getByRole('button', { name: '7 дней', exact: true })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Отзывы', exact: true }).click(); await page.getByRole('button', { name: 'Критические', exact: true }).click();
  await expect(page.locator('.review')).toHaveCount(1); await expect(page.locator('.review')).toContainText('Мария');
  await expect(page.locator('.metrics, .analyticsGrid, .directionPanel, .aboutGrid')).toHaveCount(0);
  await page.getByRole('button', { name: 'Положительные', exact: true }).click(); await expect(page.locator('.review')).toHaveCount(1); await expect(page.locator('.review')).toContainText('Алексей');
  await page.getByRole('button', { name: 'Направления', exact: true }).click();
  await expect(page.getByRole('link', { name: 'Обменять USDT → BTC' })).toHaveAttribute('href', '/#/swap/USDT/BTC?sources=bybit');
  await expect(page.locator('.metrics, .reviewSection, .aboutGrid')).toHaveCount(0);
  await page.getByRole('button', { name: 'О площадке', exact: true }).click();
  await expect(page.locator('.aboutGrid')).toBeVisible();
  await expect(page.locator('.metrics, .reviewSection, .directionPanel')).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('empty and failed statistics never fabricate numbers and can be retried', async ({ page }) => {
  await mock(page); let fail = true;
  await page.route('**/api/providers/*/statistics?*', route => route.fulfill(fail ? { status: 503, body: '{}' } : { contentType: 'application/json', body: JSON.stringify({ ...statistics('30d'), searches: 0, top10: 0, top1: 0, days: [], directions: [] }) }));
  await page.goto('/providers/whitebird');
  await expect(page.getByRole('heading', { name: 'Статистика временно недоступна' })).toBeVisible();
  await expect(page.locator('.metric.highlight strong')).toHaveText('—%'); fail = false;
  await page.locator('.rankingPanel').getByRole('button', { name: 'Повторить' }).click();
  await expect(page.getByRole('heading', { name: 'Статистика ещё собирается' })).toBeVisible(); await expect(page.locator('.line10')).toHaveCount(0);
  await expect(page.locator('.identityMeta')).toContainText('Обменник'); await expect(page.locator('.identityMeta')).not.toContainText('P2P');
});

test('directory filters platforms and links to their profiles', async ({ page }) => {
  await mock(page); await page.goto('/providers'); await expect(page.locator('.venueCard')).toHaveCount(2);
  await page.getByRole('button', { name: 'Обменник', exact: true }).click(); await expect(page.locator('.venueCard')).toHaveCount(1); await expect(page.locator('.venueCard')).toContainText('Whitebird');
  await page.getByRole('button', { name: 'Все площадки', exact: true }).click(); await page.getByRole('searchbox', { name: 'Найти площадку' }).fill('bybit');
  await page.getByRole('link', { name: 'Открыть профиль: Bybit' }).click(); await expect(page).toHaveURL(/\/providers\/bybit$/);
});

test('profile remains responsive and accessible in both themes', async ({ page }, testInfo) => {
  await mock(page); await page.goto('/providers/bybit'); await expect(page.locator('.line10')).toBeVisible();
  for (const width of [320, 393, 768, 1024, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    expect(await page.evaluate(() => document.body.scrollWidth <= innerWidth)).toBe(true);
    expect(await page.locator('header .inner').evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true);
  }
  for (const theme of ['light', 'dark']) {
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    const accessibility = await new AxeBuilder({ page }).include('main').analyze(); expect(accessibility.violations).toEqual([]);
  }
  await page.evaluate(() => document.documentElement.dataset.theme = 'light');
  await page.screenshot({ path: testInfo.outputPath('provider-profile.png'), fullPage: true });
});


test('one day of history uses bars without inventing previous rankings', async ({ page }) => {
  await mock(page);
  await page.route('**/api/providers/*/statistics?*', route => {
    const summary = statistics('30d');
    summary.days = summary.days.map((day, index) => ({ ...day, searches: index === 29 ? 5 : 0, top10: index === 29 ? 4 : 0, top1: 0 }));
    return route.fulfill({ contentType: 'application/json', body: JSON.stringify({ ...summary, searches: 5, top10: 4, top1: 0 }) });
  });
  await page.goto('/providers/bybit');
  await expect(page.locator('.dayBar')).toHaveCount(2);
  await expect(page.locator('.barValue')).toHaveText(['80%', '0%']);
  await expect(page.locator('.line10, .line1')).toHaveCount(0);
  await expect(page.locator('.singleDayNote')).toContainText('один день');
  await page.locator('.dayTarget').focus();
  await expect(page.locator('.tooltip')).toContainText('Поисков: 5');
});

test('profiles provide server-rendered messenger metadata and distinct platform PNGs', async ({ request }, testInfo) => {
  const response = await request.get('/providers/bybit', { headers: { 'User-Agent': 'TelegramBot (like TwitterBot)' } });
  expect(response.status()).toBe(200);
  const html = await response.text();
  expect(html).toMatch(/property="og:title" content="Bybit[^"\n]+Pay3Flow"/);
  expect(html).toContain('property="og:image:width" content="1200"');
  expect(html).toContain('property="og:image:height" content="630"');
  expect(html).toContain('name="twitter:card" content="summary_large_image"');
  const imageUrl = new URL(html.match(/property="og:image" content="([^"]+)"/)![1].replaceAll('&amp;', '&'));
  expect(imageUrl.protocol).toBe('https:');
  const image = await request.get(imageUrl.pathname + imageUrl.search);
  expect(image.status()).toBe(200);
  expect(image.headers()['content-type']).toBe('image/png');
  const bytes = await image.body();
  expect(bytes.subarray(1, 4).toString()).toBe('PNG');
  expect(bytes.readUInt32BE(16)).toBe(1200);
  expect(bytes.readUInt32BE(20)).toBe(630);
  await writeFile(testInfo.outputPath('provider-messenger-preview.png'), bytes);
  const other = await request.get('/providers/near-intents/preview.png?lang=en');
  expect(other.status()).toBe(200);
  expect(await other.body()).not.toEqual(bytes);
  expect((await request.get('/providers/unknown/preview.png')).status()).toBe(404);
});

test('active tabs have a green underline and the existing share dialog previews the profile', async ({ page }) => {
  await mock(page);
  await page.goto('/providers/bybit?period=7d');
  await expect(page.locator('.metric').nth(2).locator('strong')).toHaveText('700');
  for (const theme of ['light', 'dark']) {
    await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
    for (const name of ['Обзор', 'Направления', 'Отзывы', 'О площадке']) {
      await page.locator('.profileTabs').getByRole('button', { name, exact: true }).click();
      const selected = page.locator('.profileTabs button[aria-pressed="true"]');
      await expect(selected).toHaveText(name);
      await expect(selected).toHaveCSS('border-bottom-color', 'rgb(181, 245, 0)');
      await expect(selected).toHaveCSS('border-bottom-width', '3px');
      for (const button of await page.locator('.profileTabs button[aria-pressed="false"]').all()) {
        await expect(button).toHaveCSS('border-bottom-color', 'rgba(0, 0, 0, 0)');
      }
    }
  }
  const share = page.getByRole('button', { name: 'Поделиться профилем', exact: true });
  await expect(share.locator('img')).toHaveAttribute('src', '/icons/ui/share.png');
  await share.click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.locator('input')).toHaveValue(page.url());
  const preview = dialog.locator('.sharePreview');
  await expect.poll(() => preview.evaluate(image => (image as HTMLImageElement).naturalWidth)).toBe(1200);
  await page.evaluate(() => Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: async (value: string) => { document.documentElement.dataset.copiedProfile = value; } } }));
  await dialog.getByRole('button', { name: 'Скопировать ссылку', exact: true }).click();
  await expect.poll(() => page.evaluate(() => document.documentElement.dataset.copiedProfile)).toBe(page.url());
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(share).toBeFocused();
});
