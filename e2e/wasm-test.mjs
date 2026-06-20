import { chromium } from 'playwright';

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();

page.on('console', msg => console.log(`[${msg.type()}]`, msg.text()));
page.on('pageerror', err => console.log('PAGE_ERR:', err.message));

await page.goto('http://localhost:3000/', { waitUntil: 'domcontentloaded', timeout: 10000 });
await page.waitForTimeout(10000);

const text = await page.evaluate(() => document.body.innerText);
console.log('=== TEXT ===');
console.log(JSON.stringify(text?.substring(0, 300)));

const html = await page.evaluate(() => document.getElementById('app')?.innerHTML?.substring(0, 300));
console.log('APP:', html);

const wasmBindings = await page.evaluate(() => typeof window.wasmBindings !== 'undefined');
console.log('wasmBindings:', wasmBindings);

await browser.close();
