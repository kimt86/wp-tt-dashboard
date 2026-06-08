// Screenshot the running dashboard after data + charts have rendered.
// usage: node scripts/shot.mjs [url] [outPath]
import { chromium } from "playwright";

const url = process.argv[2] || "http://127.0.0.1:5173";
const out = process.argv[3] || "../docs/dashboard-live.png";

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 1750 } });
await page.goto(url, { waitUntil: "networkidle" });
await page.waitForSelector(".kpi .val", { timeout: 15000 }).catch(() => {});
await page.waitForTimeout(1500); // allow Chart.js to draw
await page.screenshot({ path: out, fullPage: true });
await browser.close();
console.log("wrote", out);
