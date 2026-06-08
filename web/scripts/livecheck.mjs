// Functional smoke for the LIVE MAP live feed (map canvas itself can't render
// headlessly — no GPU/swiftshader). Loads the page, navigates to MAP, waits for the
// poll + render loop, then reads the live indicator, device count, and state chips.
// usage: node scripts/livecheck.mjs [baseUrl]
import { chromium } from "playwright";

const base = process.argv[2] || "http://127.0.0.1:8080";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 950 } });
const errors = [];
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text()); });
page.on("pageerror", (e) => errors.push("pageerror: " + e.message));

await page.goto(base, { waitUntil: "networkidle" });

// go to the MAP page (sidebar/tab item labelled MAP/지도)
await page.getByText(/^MAP$/i).first().click().catch(async () => {
  await page.locator(".side-item, .ptab").filter({ hasText: /MAP|지도/i }).first().click();
});

// wait for the map "load" → render loop to populate the live count (non "0 / 0")
await page.waitForTimeout(7000);

const liveText = await page.locator(".map-live").first().innerText().catch(() => "(none)");
const countText = await page.locator(".map-count").first().innerText().catch(() => "(none)");
const clockText = await page.locator(".map-clock").first().innerText().catch(() => "(none)");
const chips = await page.locator(".map-chips .mchip .cn").allInnerTexts().catch(() => []);

console.log(JSON.stringify({ liveText, countText, clockText, chips, errors }, null, 2));
await browser.close();
