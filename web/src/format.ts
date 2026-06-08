import type { KpiCard } from "./api";

export function fmtValue(value: number | null, unit: string): string {
  if (value == null) return "—";
  if (unit === "%") return value.toFixed(1);
  if (unit === "km/Job") return value.toFixed(2);
  if (unit === "s") return Math.round(value).toLocaleString();
  if (unit === "move/hr") return value.toFixed(1);
  return String(value);
}

/** True if the as-of value improved relative to baseline, given KPI direction. */
export function isImprovement(c: KpiCard): boolean | null {
  if (c.delta_abs == null || !c.direction) return null;
  return c.direction === "LOWER_BETTER" ? c.delta_abs < 0 : c.delta_abs > 0;
}

/** Delta label: percentage-point for ratio/%-unit KPIs, else relative %. */
export function deltaLabel(c: KpiCard): string | null {
  if (c.delta_abs == null) return null;
  const up = c.delta_abs > 0;
  const arrow = up ? "▲" : "▼";
  if (c.unit === "%") return `${arrow} ${Math.abs(c.delta_abs).toFixed(1)}pp`;
  if (c.delta_pct != null) return `${arrow} ${Math.abs(c.delta_pct).toFixed(1)}%`;
  return `${arrow} ${Math.abs(c.delta_abs).toFixed(2)}`;
}
