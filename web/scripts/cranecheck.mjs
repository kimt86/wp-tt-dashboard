// Verify the crane PLC section in the detail panel + the RTG equipment-tab rename.
import { chromium } from "playwright";
const base = process.argv[2] || "http://127.0.0.1:8080";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 1000 } });
const errors = [];
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
page.on("pageerror", (e) => errors.push("pageerror: " + e.message));

await page.goto(base + "/?debug=1", { waitUntil: "networkidle" });
await page.locator(".side-item, .ptab").filter({ hasText: /MAP|지도/i }).first().click();
await page.waitForTimeout(4000);

// equipment tab labels (expect RTG, not YC)
const equipTabs = await page.locator(".map-equip .meq").allInnerTexts().catch(() => []);

// pick a crane that has PLC
const craneId = await page.evaluate(async () => {
  const r = await fetch("/api/livemap/positions");
  const j = await r.json();
  const c = j.devices.find((d) => d.plc);
  return c ? c.id : null;
});
let panel = { opened: false };
if (craneId) {
  await page.evaluate((id) => window.__wpPick && window.__wpPick(id), craneId);
  await page.waitForSelector(".lvd-root", { timeout: 6000 }).catch(() => {});
  panel = {
    opened: (await page.locator(".lvd-root").count()) > 0,
    id: await page.locator(".lvd-id").first().innerText().catch(() => "—"),
    sections: await page.locator(".lvd-section-h").allInnerTexts().catch(() => []),
    rows: await page.locator(".lvd-row").allInnerTexts().catch(() => []),
  };
  await page.locator(".lvd-root").screenshot({ path: "../docs/crane-popup.png" }).catch(() => {});
}
console.log(JSON.stringify({ equipTabs, craneId, panel, errors }, null, 2));
await browser.close();
