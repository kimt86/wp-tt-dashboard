// Verify equipment-shape markers: registered icons, symbol layer, no missing-image
// warnings, feature icon resolution, and the tab shape-legend (DOM screenshot).
import { chromium } from "playwright";
const base = process.argv[2] || "http://127.0.0.1:8080";
const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1500, height: 950 } });
const msgs = [];
page.on("console", (m) => msgs.push(`${m.type()}: ${m.text()}`));
page.on("pageerror", (e) => msgs.push("pageerror: " + e.message));

await page.goto(base + "/?debug=1", { waitUntil: "networkidle" });
await page.locator(".side-item, .ptab").filter({ hasText: /MAP|지도/i }).first().click();
await page.waitForTimeout(4500);

const info = await page.evaluate(() => {
  const m = window.__wpmap;
  if (!m) return { error: "no map" };
  const names = ["TT-moving", "TT-idle", "TT-off", "RTG-moving", "QC-moving", "ETC-off"];
  const haveIcons = names.filter((n) => m.hasImage(n));
  const vlayer = m.getLayer("veh");
  const feats = m.querySourceFeatures("vehicles");
  const byEq = {};
  for (const f of feats) { const e = f.properties.eq; byEq[e] = (byEq[e] || 0) + 1; }
  return {
    layerType: vlayer ? vlayer.type : null,
    iconImageExpr: vlayer ? JSON.stringify(m.getLayoutProperty("veh", "icon-image")) : null,
    registeredIcons: haveIcons,
    featureCount: feats.length,
    byEq,
  };
});

// shape legend on the equipment tabs
const glyphCount = await page.locator(".map-equip .meq .meq-glyph").count();
await page.locator(".map-top").screenshot({ path: "../docs/equip-legend.png" }).catch(() => {});

const warnings = msgs.filter((m) => /missing|could not|error/i.test(m));
console.log(JSON.stringify({ info, glyphCount, warnings }, null, 2));
await browser.close();
