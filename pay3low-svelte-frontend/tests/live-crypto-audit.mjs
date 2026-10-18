// Explicit read-only browser audit: node tests/live-crypto-audit.mjs BASE_URL OUTPUT
import { chromium } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

const base = process.argv[2] ?? 'http://localhost:3000';
const output = process.argv[3] ?? '/tmp/pay3flow-browser-audit.json';
const gram = process.env.PAY3FLOW_AUDIT_LEGACY_TON === '1' ? 'TON' : 'GRAM';
const browser = await chromium.launch({ executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE });
const context = await browser.newContext({ viewport: { width: 1440, height: 1000 } });
await context.addInitScript(() => {
  localStorage.setItem('pay3flow-locale', 'en');
  localStorage.setItem('pay3flow.exchange.refresh', '0');
});
const page = await context.newPage();
const snapshots = [], sent = [], errors = [];
function receive(payload) {
  try {
    const value = typeof payload === 'string' ? JSON.parse(payload) : payload;
    if (value.type === 'search_finished' || value.routes && !value.type) snapshots.push(value);
  } catch {}
}
page.on('websocket', socket => {
  socket.on('framesent', frame => { try { sent.push(JSON.parse(String(frame.payload))); } catch {} });
  socket.on('framereceived', frame => receive(String(frame.payload)));
});
page.on('response', async response => {
  if (new URL(response.url()).pathname === '/api/p2p/routes') {
    try { receive(await response.json()); } catch {}
  }
});
page.on('pageerror', error => errors.push(error.message));
const evidence = { observed_at: new Date().toISOString(), base_url: base, scope: 'Actual asset and network picker clicks; read-only route searches', results: [] };
async function save() {
  await mkdir(dirname(output), { recursive: true });
  await writeFile(output, JSON.stringify({ ...evidence, page_errors: errors }, null, 2) + '\n');
}
async function choose(side, asset, network) {
  await page.getByRole('button', { name: new RegExp(`^Select ${side} (?:bank|payment method|asset):`) }).click();
  const dialog = page.getByRole('dialog', { name: side === 'sending' ? 'Choose where you pay from' : 'Choose where the recipient gets paid' });
  await dialog.getByRole('textbox', { name: 'Currencies and digital assets' }).fill(asset);
  await dialog.getByRole('option', { name: new RegExp(`^${asset}\\b`) }).first().click();
  await dialog.getByRole('textbox', { name: 'Blockchains' }).fill(network);
  await dialog.getByRole('option', { name: new RegExp(`^${network.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`) }).first().click();
}
const cases = [
  ['ETH', 'Ethereum', 'BTC', 'Bitcoin', '0.05'],
  ['ETH', 'Ethereum', 'AVAX', 'Avalanche', '0.05'],
  ['ETH', 'Ethereum', 'DOT', 'Polkadot', '0.05'],
  ['ETH', 'Ethereum', gram, 'TON', '0.05'],
  ['ETH', 'Ethereum', gram, 'TON', '0.5'],
  ['BTC', 'Bitcoin', 'AVAX', 'Avalanche', '0.01'],
  ['AVAX', 'Avalanche', 'BTC', 'Bitcoin', '100'],
  ['DOT', 'Polkadot', 'ETH', 'Ethereum', '100'],
  [gram, 'TON', 'BTC', 'Bitcoin', '1000'],
  [gram, 'TON', 'USDC', 'Ethereum', '1000'],
];
try {
  await page.goto(base, { waitUntil: 'domcontentloaded' });
  await page.locator('.workspace').waitFor({ timeout: 45000 });
  await page.locator('.introOverlay').waitFor({ state: 'hidden', timeout: 15000 });
  for (const [source, sourceNetwork, target, targetNetwork, amount] of cases) {
    const result = { source, source_network_label: sourceNetwork, target, target_network_label: targetNetwork, amount };
    const start = snapshots.length, sentStart = sent.length;
    try {
      await page.getByLabel('Amount to send', { exact: true }).fill('0');
      await choose('sending', source, sourceNetwork);
      await choose('recipient', target, targetNetwork);
      await page.getByLabel('Amount to send', { exact: true }).fill(amount);
      const deadline = Date.now() + 75000;
      let snapshot;
      while (Date.now() < deadline) {
        snapshot = snapshots.slice(start).find(value => value.source_fiat === source && value.target_fiat === target && Number(value.source_amount) === Number(amount));
        if (snapshot) break;
        await new Promise(resolve => setTimeout(resolve, 250));
      }
      result.sent_queries = sent.slice(sentStart).map(value => value.query).filter(Boolean);
      result.controls = await page.locator('.methodTrigger, .networkButton').evaluateAll(nodes => nodes.map(node => ({ label: node.getAttribute('aria-label'), text: node.textContent?.trim() })));
      if (!snapshot) result.error = 'No matching completed search within 75 seconds';
      else {
        result.routes_found = snapshot.routes_found;
        result.routes_exhaustive = snapshot.routes_exhaustive;
        result.provider_statuses = snapshot.provider_statuses;
        result.routes = snapshot.routes.map(route => Object.fromEntries(['route_kind', 'route_provider', 'market_path', 'target_amount', 'route_path', 'warnings'].map(key => [key, route[key]])));
      }
      if (snapshot) await page.waitForFunction(expected => document.querySelectorAll('[data-testid="complete-route"]').length === expected, Math.min(snapshot.routes.length, 40), { timeout: 5000 });
      result.visible_cards = await page.locator('[data-testid="complete-route"]').count();
    } catch (error) { result.error = error.message; }
    evidence.results.push(result);
    await save();
    console.log(`${source} -> ${target} (${amount}): ${result.routes_found ?? result.error}`);
    await page.screenshot({ path: `${dirname(output)}/${source.toLowerCase()}-${target.toLowerCase()}-${amount}-browser.png` });
  }
} finally { await save(); await browser.close(); }
