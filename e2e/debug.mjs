import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

page.on('console', msg => console.log(`[${msg.type()}]`, msg.text()));
page.on('pageerror', err => console.log('[PAGE_ERROR]', err.message));

await page.goto('http://localhost:3000/', { waitUntil: 'networkidle', timeout: 10000 });
await page.waitForTimeout(6000);

const html = await page.content();
console.log('=== HTML (first 2000) ===');
console.log(html.substring(0, 2000));
console.log('=== BODY TEXT ===');
const text = await page.evaluate(() => document.body?.innerText);
console.log(text?.substring(0, 500));

await browser.close();
