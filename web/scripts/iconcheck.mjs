// Verify the equipment icons: (1) enlarge the tab-legend SVG glyphs and screenshot so the
// silhouettes are clearly visible; (2) confirm the map symbol layer + 12 rasters resolve.
import { chromium } from "playwright";
const base = process.argv[2] || "http://127.0.0.1:8080";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 950 } });
const msgs = [];
page.on("console", (m) => msgs.push(`${m.type()}: ${m.text()}`));

await page.goto(base + "/?debug=1", { waitUntil: "networkidle" });
await page.locator(".side-item, .ptab").filter({ hasText: /MAP|지도/i }).first().click();
await page.waitForTimeout(4500);

const info = await page.evaluate(() => {
  const m = window.__wpmap;
  const icons = ["TT-moving", "RTG-idle", "QC-off", "ETC-moving"].filter((n) => m && m.hasImage(n));
  return { layerType: m?.getLayer("veh")?.type, icons, allRegistered: ["TT", "RTG", "QC", "ETC"].flatMap((e) => ["moving", "idle", "off"].map((s) => `${e}-${s}`)).every((n) => m?.hasImage(n)) };
});

// blow up the legend glyphs to 64px on a light card and screenshot each tab
await page.addStyleTag({ content: `
  .map-equip { background:#0b1220; padding:14px 10px; border-radius:10px; gap:14px !important; }
  .meq { flex-direction:column; gap:8px !important; font-size:13px !important; color:#7dd3fc !important; }
  .meq-glyph { width:60px !important; height:60px !important; }
`});
await page.waitForTimeout(300);
await page.locator(".map-equip").screenshot({ path: "../docs/equip-icons.png" });

const warnings = msgs.filter((m) => /missing|could not|error/i.test(m));
console.log(JSON.stringify({ info, warnings }, null, 2));
await browser.close();
