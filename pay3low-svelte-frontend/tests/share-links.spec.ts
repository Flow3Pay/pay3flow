import { expect, test } from '@playwright/test';
import { writeFile } from 'node:fs/promises';

test('short OTC links serve messenger metadata, three-panel PNGs and a no-JavaScript handoff', async ({ request, browser, baseURL }, testInfo) => {
  const target = '/#/otc?market=EVER-USDT&side=sell&type=limit&amount=100.250000&price=0.012340&sendNetwork=everscale&receiveNetwork=tron&lang=ru';
  const preview = { closes: [0.0099, 0.0101, 0.0098, 0.0103, 0.0102, 0.0104], bids: [{ price: 0.00999, amount: 1000 }, { price: 0.00998, amount: 2500 }], asks: [{ price: 0.01001, amount: 1500 }, { price: 0.01002, amount: 3500 }], capturedAt: Date.UTC(2026, 9, 10, 14, 12) };
  const create = await request.post('/share-links', { data: { target, preview } });
  expect(create.ok()).toBe(true);
  const { id } = await create.json();
  expect(id).toMatch(/^[A-Za-z0-9_-]{16}$/);
  const path = `/otc/${id}`;
  const response = await request.get(path, { headers: { 'User-Agent': 'TelegramBot (like TwitterBot)' } });
  expect(response.status()).toBe(200);
  const html = await response.text();
  expect(html).toContain('property="og:image"');
  expect(html).toContain('property="og:image:width" content="1200"');
  expect(html).toContain('name="twitter:card" content="summary_large_image"');
  expect(html).toContain('Bridge · Chart · Orderbook');
  expect(html).toContain('100.250000');
  expect(html).toContain('0.012340');
  const image = await request.get(`${path}/preview.png`);
  expect(image.status()).toBe(200);
  expect(image.headers()['content-type']).toBe('image/png');
  const bytes = await image.body();
  expect(bytes.subarray(1, 4).toString()).toBe('PNG');
  expect(bytes.readUInt32BE(16)).toBe(1200);
  expect(bytes.readUInt32BE(20)).toBe(630);
  await writeFile(testInfo.outputPath('otc-messenger-preview.png'), bytes);
  const second = await request.post('/share-links', { data: { target: target.replace('0.012340', '0.02'), preview } });
  const other = await request.get(`/otc/${(await second.json()).id}/preview.png`);
  expect(await other.body()).not.toEqual(bytes);
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const page = await context.newPage();
    await page.goto(`${baseURL}${path}`);
    await expect(page.getByRole('heading', { level: 1 })).toHaveText('EVER → USDT · Pay3Flow OTC');
    await expect(page.getByRole('img', { name: 'Pay3Flow OTC · Bridge · Chart · Orderbook' })).toBeVisible();
    await expect(page.getByRole('link', { name: 'Открыть обмен →' })).toHaveAttribute('href', target);
  } finally { await context.close(); }
  expect((await request.get('/otc/unknown')).status()).toBe(404);
  expect((await request.get(`/s/${id}`)).status()).toBe(404);
});
