// KPI history matrix — past KPI values by day / week / month.
// Reads /api/kpis/history (Postgres only, zero Oracle load). Table of buckets (rows,
// newest-first) × KPIs (columns); click a column header to expand a per-KPI trend chart.
import { useEffect, useState } from "react";
import { api, type HistoryResponse } from "./api";
import { LineChart } from "./charts";
import { fmtValue } from "./format";
import { t, type Lang } from "./i18n";

type Gran = "day" | "week" | "month";
const GRANS: Gran[] = ["day", "week", "month"];
const N_BY_GRAN: Record<Gran, number> = { day: 30, week: 12, month: 12 };

// short bucket label per granularity
function bucketLabel(b: HistoryResponse["buckets"][number], gran: Gran): string {
  const md = (iso: string) => iso.slice(5); // MM-DD
  if (gran === "day") return md(b.bucket);
  if (gran === "week") return `${md(b.label_from)}~${md(b.label_to)}`;
  return b.bucket.slice(0, 7); // YYYY-MM
}

export default function HistoryMatrix({ lang }: { lang: Lang }) {
  const s = t(lang);
  const [gran, setGran] = useState<Gran>("day");
  const [data, setData] = useState<HistoryResponse | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [err, setErr] = useState(false);

  useEffect(() => {
    let alive = true;
    setData(null);
    api.kpiHistory(gran, N_BY_GRAN[gran])
      .then((d) => { if (alive) { setData(d); setErr(false); setExpanded((prev) => prev ?? d.kpis[0]?.key ?? null); } })
      .catch(() => { if (alive) setErr(true); });
    return () => { alive = false; };
  }, [gran]);

  const cols = data?.kpis ?? [];
  // hide buckets with no data at all (e.g. days/weeks before extraction began)
  const buckets = (data?.buckets ?? []).filter((b) => cols.some((c) => b.cells[c.key]?.value != null));
  const expCol = cols.find((c) => c.key === expanded);
  // cycle KPIs are stored in seconds but read better as minutes in the matrix/chart.
  const isCycle = (k: string) => k.startsWith("K_CYCLE");
  const fmtCell = (key: string, v: number | null, unit: string): string =>
    v == null ? "—" : isCycle(key) ? `${(v / 60).toFixed(1)}m` : fmtValue(v, unit);
  const colUnit = (key: string, unit: string) => (isCycle(key) ? "min" : unit);
  // chronological series for the expanded KPI (cycle → minutes)
  const chronological = buckets.slice().reverse();
  const series = expCol ? chronological.map((b) => { const v = b.cells[expCol.key]?.value; return v == null ? NaN : isCycle(expCol.key) ? v / 60 : v; }) : [];
  const chartLabels = expCol ? chronological.map((b) => bucketLabel(b, gran)) : [];

  return (
    <>
      <div className="section-title" style={{ marginTop: 22 }}>
        {s.valuesByPeriod}
        <span className="section-sub">{lang === "ko" ? "열(KPI) 클릭 → 추이 차트" : "click a column (KPI) → trend chart"}</span>
      </div>

      <div className="period-bar" style={{ marginBottom: 10 }}>
        <span className="period-label">{s.period}</span>
        <div className="period-group">
          {GRANS.map((g) => (
            <button key={g} className={`period-btn${gran === g ? " active" : ""}`} onClick={() => setGran(g)}>
              {g === "day" ? s.g_day : g === "week" ? s.g_week : s.g_month}
            </button>
          ))}
        </div>
        {data && buckets.length > 0 && (
          <span className="period-range mono">{buckets[buckets.length - 1].label_from} ~ {buckets[0].label_to}</span>
        )}
      </div>

      {!data ? (
        <div className="loading">{err ? s.noData : s.loading}</div>
      ) : buckets.length === 0 ? (
        <div className="loading">{s.noData}</div>
      ) : (
        <>
          {expCol && (
            <div className="hist-chart">
              <div className="hist-chart-h">{lang === "ko" ? expCol.name_ko : expCol.name_en} <span className="unit">{colUnit(expCol.key, expCol.unit)}</span>
                <button className="hist-chart-x" onClick={() => setExpanded(null)} aria-label="close">×</button>
              </div>
              <div className="hist-chart-body">
                <LineChart values={series} labels={chartLabels} axes color="#60a5fa" />
              </div>
            </div>
          )}
          <div className="hist-wrap">
            <table className="hist-table">
              <thead>
                <tr>
                  <th className="hist-bucket">{s.bucket}</th>
                  {cols.map((c) => (
                    <th key={c.key} className={`hist-kpi${expanded === c.key ? " sel" : ""}`} onClick={() => setExpanded(expanded === c.key ? null : c.key)} title={lang === "ko" ? "추이 차트 보기" : "show trend chart"}>
                      <span className="hk-name">{lang === "ko" ? c.name_ko : c.name_en}</span>
                      <span className="hk-unit">{colUnit(c.key, c.unit)}</span>
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {buckets.map((b) => (
                  <tr key={b.bucket}>
                    <td className="hist-bucket">
                      {bucketLabel(b, gran)}
                      {b.is_provisional && <span className="hist-prov">{s.provisional}</span>}
                    </td>
                    {cols.map((c) => (
                      <td key={c.key} className={`hist-val${expanded === c.key ? " sel" : ""}`}>
                        {fmtCell(c.key, b.cells[c.key]?.value ?? null, c.unit)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </>
  );
}
