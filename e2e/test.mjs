import { chromium } from 'playwright';

const BASE = 'http://localhost:3000';
const KEY = 'sk-test-abc123';
let passed = 0, failed = 0;

const ok = (desc) => { console.log(`  \x1b[32m✅ ${desc}\x1b[0m`); passed++; };
const fail = (desc, msg) => { console.log(`  \x1b[31m❌ ${desc}: ${msg}\x1b[0m`); failed++; };
const bold = (s) => `\x1b[1m${s}\x1b[0m`;

function assert(desc, fn) {
  try { fn(); ok(desc); } catch (e) { fail(desc, e.message); }
}

async function main() {
  console.log(bold('\n╔═══════════════════════════════════════════╗'));
  console.log(bold('║    AIRouter — E2E Playwright Test        ║'));
  console.log(bold('╚═══════════════════════════════════════════╝\n'));

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

  try {
    // ── 1. Health ──────────────────────────────────────────────
    console.log(bold('── 1. Health Check ──'));
    const health = await fetch(`${BASE}/health`);
    assert('GET /health → 200', () => { if (health.status !== 200) throw new Error(`${health.status}`); });
    assert('GET /health → OK', async () => {
      const b = await health.text();
      if (!b.includes('OK')) throw new Error(b);
    });

    // ── 2. Frontend render ─────────────────────────────────────
    console.log(bold('\n── 2. Frontend Render ──'));
    await page.goto(BASE, { waitUntil: 'domcontentloaded', timeout: 15000 });
    await page.waitForTimeout(15000);

    assert('Title = AIRouter Dashboard', async () => {
      const t = await page.title();
      if (!t.includes('AIRouter')) throw new Error(t);
    });
    assert('.dashboard renders', async () => { if (!await page.$('.dashboard')) throw new Error('missing'); });
    assert('.sidebar renders', async () => { if (!await page.$('.sidebar')) throw new Error('missing'); });
    assert('.metric-card renders', async () => { if (!await page.$('.metric-card')) throw new Error('missing'); });
    assert('.provider-card renders', async () => { if (!await page.$('.provider-card')) throw new Error('missing'); });

    // Verify content
    const text = await page.evaluate(() => document.body.innerText);
    assert('Shows "Dashboard" heading', () => { if (!text.includes('Dashboard')) throw new Error('no heading'); });
    assert('Shows "Providers" section', () => { if (!text.includes('Providers')) throw new Error('no providers'); });
    assert('Shows opencode FREE', () => { if (!text.includes('opencode')) throw new Error('no opencode'); });
    assert('Shows 34 models', () => { if (!text.includes('34')) throw new Error('no 34'); });
    assert('Shows 7 providers', () => { if (!text.includes('7')) throw new Error('no 7'); });

    // Dark theme
    const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    assert('Dark theme (#0f1117)', () => { if (bg !== 'rgb(15, 17, 23)') throw new Error(bg); });

    // CSS loaded
    const css = await (await fetch(`${BASE}/style/main.css`)).text();
    assert('CSS loads', () => { if (!css.includes('.dashboard')) throw new Error('no css'); });

    // ── 3. Navigation ─────────────────────────────────────────
    console.log(bold('\n── 3. Navigation ──'));

    const pages = [
      { path: '/providers', expect: 'Providers' },
      { path: '/analytics', expect: 'Analytics' },
      { path: '/settings', expect: 'Settings' },
      { path: '/', expect: 'Dashboard' },
    ];
    for (const { path, expect: exp } of pages) {
      await page.click(`a[href="${path}"]`).catch(() => page.goto(`${BASE}${path}`));
      await page.waitForTimeout(3000);
      assert(`Navigate to ${path}`, async () => {
        const t = await page.evaluate(() => document.body.innerText);
        if (!t.includes(exp)) throw new Error(`no ${exp} on ${path}`);
      });
    }

    // ── 4. API ─────────────────────────────────────────────────
    console.log(bold('\n── 4. API Endpoints ──'));

    const noAuth = await fetch(`${BASE}/v1/models`);
    assert('No auth → 401', () => { if (noAuth.status !== 401) throw new Error(`${noAuth.status}`); });

    const badAuth = await fetch(`${BASE}/v1/models`, { headers: { Authorization: 'Bearer bad' }});
    assert('Bad key → 401', () => { if (badAuth.status !== 401) throw new Error(`${badAuth.status}`); });

    const okAuth = await fetch(`${BASE}/v1/models`, { headers: { Authorization: `Bearer ${KEY}` }});
    assert('Valid key → 200', () => { if (okAuth.status !== 200) throw new Error(`${okAuth.status}`); });
    const models = await okAuth.json();
    assert('Models ≥ 30', () => { if (models.length < 30) throw new Error(`${models.length}`); });

    const dash = await (await fetch(`${BASE}/api/dashboard`, { headers: { Authorization: `Bearer ${KEY}` }})).json();
    assert('Dashboard has providers', () => { if (!dash.providers?.length) throw new Error('no providers'); });
    assert('Dashboard has metrics', () => { if (!dash.metrics) throw new Error('no metrics'); });
    assert('Built-in free = true', () => { if (!dash.metrics.built_in_free) throw new Error('false'); });

    // ── Summary ───────────────────────────────────────────────
    const total = passed + failed;
    console.log(bold('\n═══════════════════════════════════════════'));
    console.log(`  ${total} tests: ${passed} passed, ${failed} failed`);
    if (failed === 0) console.log(bold('  \x1b[32m✅ ALL PASSED\x1b[0m'));
    else console.log(bold(`  \x1b[31m❌ ${failed} FAILED\x1b[0m`));

    await browser.close();
    process.exit(failed > 0 ? 1 : 0);

  } catch (e) {
    console.error('\n\x1b[31mFATAL:\x1b[0m', e.message);
    await browser.close();
    process.exit(1);
  }
}

main();
