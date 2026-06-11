// Render-check one KC page: node kc-render-check.mjs <file.html>
// Asserts: exactly one global sidebar, no legacy back-link, hero present, no JS errors,
// no second fixed left nav. Prints JSON.
import { chromium } from 'playwright-core';
const page = process.argv[2];
const exe = process.env.HOME + '/.cache/ms-playwright/chromium_headless_shell-1223/chrome-headless-shell-linux64/chrome-headless-shell';
const browser = await chromium.launch({ executablePath: exe });
const ctx = await browser.newContext({ viewport: { width: 1480, height: 950 } });
const pg = await ctx.newPage();
const errs = [];
pg.on('pageerror', e => errs.push('pageerror: ' + e.message));
pg.on('console', m => { if (m.type() === 'error') errs.push('console: ' + m.text()); });
await pg.goto('http://localhost:8080/kc/' + page, { waitUntil: 'networkidle' });
const info = await pg.evaluate(() => {
  const sidebars = document.querySelectorAll('aside.kc-sidebar');
  const back = [...document.querySelectorAll('a')].filter(a => a.textContent.includes('지식센터') && a.textContent.includes('←'));
  // any OTHER fixed-position element pinned to the left edge (double-sidebar detector)
  const fixedLeft = [...document.querySelectorAll('body *')].filter(el => {
    if (el.closest('aside.kc-sidebar') || el.classList.contains('kc-topbar')) return false;
    const cs = getComputedStyle(el);
    if (cs.position !== 'fixed' && cs.position !== 'sticky') return false;
    const r = el.getBoundingClientRect();
    return r.left < 300 && r.width > 80 && r.height > 300;
  }).map(el => el.tagName + '.' + el.className.toString().slice(0, 40));
  const txt = (document.body.innerText || '').replace(/\s+/g, '');
  return {
    sidebars: sidebars.length,
    sidebarVisible: sidebars.length ? getComputedStyle(sidebars[0]).transform === 'none' : false,
    legacyBackLinks: back.length,
    hasH1: !!document.querySelector('h1'),
    hasKicker: !!document.querySelector('.kicker, .hero .kicker'),
    otherFixedLeftNav: fixedLeft,
    visibleChars: txt.length,
    slideRemnants: document.querySelectorAll('.slide-number, .toolbar, [class*="present-mode"]').length,
  };
});
console.log(JSON.stringify({ page, ...info, jsErrors: errs }, null, 0));
await browser.close();
