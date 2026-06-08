// Headless verification of (1) the WS-data health FEED page (plain DOM — renders fully)
// and (2) the clicked-truck detail panel (opened via the ?debug __wpPick hook, since the
// GPU-less server can't render map markers to click).
// usage: node scripts/feedcheck.mjs [baseUrl]
import { chromium } from "playwright";

const base = process.argv[2] || "http://127.0.0.1:8080";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
const errors = [];
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
page.on("pageerror", (e) => errors.push("pageerror: " + e.message));

await page.goto(base + "/?debug=1", { waitUntil: "networkidle" });

// ── FEED page ──
await page.locator(".side-item, .ptab").filter({ hasText: /FEED|데이터/i }).first().click();
await page.waitForSelector(".fh-banner", { timeout: 8000 });
await page.waitForTimeout(2500);
const feed = {
  banner: await page.locator(".fh-banner strong").first().innerText().catch(() => "—"),
  cause: await page.locator(".fh-banner-cause").first().innerText().catch(() => "—"),
  heroBig: await page.locator(".fh-big").allInnerTexts().catch(() => []),
  fresh: await page.locator(".fh-fresh-n").allInnerTexts().catch(() => []),
  nodes: await page.locator(".fh-node-label").allInnerTexts().catch(() => []),
  fleet: (await page.locator(".fh-fleet-row").allInnerTexts().catch(() => [])).slice(0, 5),
};
await page.screenshot({ path: "../docs/feed-health.png", fullPage: true });

// ── truck detail panel (via debug hook) ──
const ttId = await page.evaluate(async () => {
  const r = await fetch("/api/livemap/positions");
  const j = await r.json();
  const tt = j.devices.find((d) => d.id.startsWith("TT") && d.vslname) || j.devices[0];
  return tt ? tt.id : null;
});
await page.locator(".side-item, .ptab").filter({ hasText: /MAP|지도/i }).first().click();
await page.waitForTimeout(4000); // map load + first poll populates liveRef
let panel = { opened: false };
if (ttId) {
  await page.evaluate((id) => window.__wpPick && window.__wpPick(id), ttId);
  await page.waitForSelector(".lvd-root", { timeout: 6000 }).catch(() => {});
  panel = {
    opened: await page.locator(".lvd-root").count() > 0,
    id: await page.locator(".lvd-id").first().innerText().catch(() => "—"),
    rows: await page.locator(".lvd-row").allInnerTexts().catch(() => []),
  };
  await page.locator(".lvd-root").screenshot({ path: "../docs/truck-popup.png" }).catch(() => {});
}

console.log(JSON.stringify({ ttId, feed, panel, errors }, null, 2));
await browser.close();
