import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

page.on('console', msg => console.log(`[${msg.type()}]`, msg.text()));
page.on('pageerror', err => console.log('PAGE_ERR:', err.message));

// 1. Load dashboard
await page.goto('http://localhost:3000/', { waitUntil: 'domcontentloaded', timeout: 15000 });
await page.waitForTimeout(15000);

const text = await page.evaluate(() => document.body.innerText);
console.log('=== TEXT (first 500) ===');
console.log(text?.substring(0, 500));

const hasDashboard = await page.$('.dashboard');
const hasSidebar = await page.$('.sidebar');
const hasProvider = await page.$('.provider-card');
const hasMetric = await page.$('.metric-card');

console.log('\n=== SELECTORS ===');
console.log('.dashboard:', !!hasDashboard);
console.log('.sidebar:', !!hasSidebar);
console.log('.provider-card:', !!hasProvider);
console.log('.metric-card:', !!hasMetric);

// 2. Navigate to providers
if (hasSidebar) {
  await page.click('a[href="/providers"]');
  await page.waitForTimeout(3000);
  const provText = await page.evaluate(() => document.body.innerText);
  console.log('\n=== PROVIDERS PAGE ===');
  console.log(provText?.substring(0, 300));
}

// 3. Navigate to analytics
if (hasSidebar) {
  await page.click('a[href="/analytics"]');
  await page.waitForTimeout(3000);
  const analText = await page.evaluate(() => document.body.innerText);
  console.log('\n=== ANALYTICS PAGE ===');
  console.log(analText?.substring(0, 300));
}

// 4. Navigate to settings
if (hasSidebar) {
  await page.click('a[href="/settings"]');
  await page.waitForTimeout(3000);
  const setText = await page.evaluate(() => document.body.innerText);
  console.log('\n=== SETTINGS PAGE ===');
  console.log(setText?.substring(0, 300));
}

await browser.close();
