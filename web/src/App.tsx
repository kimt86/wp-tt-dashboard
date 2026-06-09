import { useEffect, useMemo, useState, type ReactElement } from "react";
import { api, type BreakdownResponse, type HealthResponse, type KpiCard, type KpisResponse, type LiveKpi, type LiveResponse, type QcRow, type TrendResponse, type VesselRow, type VesselsResponse } from "./api";
import { LineChart } from "./charts";
import { deltaLabel, fmtValue, isImprovement } from "./format";
import { t, type Lang } from "./i18n";
import TtPage from "./TtPage";
import LiveMapPage from "./LiveMapPage";
import HealthPage from "./HealthPage";
import FeedHealthPage from "./FeedHealthPage";
import HistoryMatrix from "./HistoryMatrix";

function useClock(): string {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);
  return now.toISOString().slice(0, 19).replace("T", " ");
}

interface Data {
  kpis: KpisResponse | null;
  breakdown: BreakdownResponse | null;
  trends: Record<string, TrendResponse>;
}

function useData(period: string): Data {
  const [data, setData] = useState<Data>({ kpis: null, breakdown: null, trends: {} });
  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const [kpis, breakdown] = await Promise.all([api.kpis(period), api.breakdown(period)]);
        // sparkline follows the selected period so it actually changes per period
        const trendList = await Promise.all(
          kpis.kpis.map((k) => api.trend(k.key, { from: kpis.range_from, to: kpis.range_to }))
        );
        if (!alive) return;
        const trends: Record<string, TrendResponse> = {};
        trendList.forEach((tr) => (trends[tr.key] = tr));
        setData({ kpis, breakdown, trends });
      } catch (e) {
        console.error(e);
      }
    };
    load();
    const id = setInterval(load, 30000); // poll
    return () => { alive = false; clearInterval(id); };
  }, [period]);
  return data;
}

const PERIOD_GROUPS: string[][] = [
  ["today", "yesterday"],
  ["this_week", "last_week"],
  ["this_month", "last_month"],
  ["last7", "last30"],
];

function HealthPill({ health, lang }: { health: HealthResponse | null; lang: Lang }) {
  const s = t(lang);
  if (!health) return null;
  const cls = health.overall === "OK" ? "ok" : health.overall === "STALE" ? "warn" : "bad";
  const label = health.overall === "OK" ? s.healthOk : health.overall === "STALE" ? s.healthStale : s.healthDegraded;
  return <span className={`status-pill ${cls}`}><span className="dot" />{label}</span>;
}

function name(c: KpiCard, lang: Lang) { return lang === "ko" ? c.name_ko : c.name_en; }

// Extra sub-values shown inside the distance/ratio cards (no new cards, no backend
// change). Loaded + total travel and loaded-ratio are exact functions of the two
// existing KPIs — empty distance E (km/Job) and empty ratio R (%):
//   total = E·100/R · loaded = total − E · loaded-ratio = 100 − R
type KExtra = { label: string; value: string };
function distanceExtras(items: { key: string; value: number | null }[], key: string, lang: Lang): KExtra[] | undefined {
  if (key !== "K_EMPTY" && key !== "K_EMPTY_R") return undefined;
  const E = items.find((x) => x.key === "K_EMPTY")?.value ?? null;     // empty km/Job
  const R = items.find((x) => x.key === "K_EMPTY_R")?.value ?? null;   // empty ratio %
  if (E == null || R == null || R <= 0) return undefined;
  const ko = lang === "ko";
  if (key === "K_EMPTY") {
    const total = (E * 100) / R;
    const loaded = total - E;
    return [
      { label: ko ? "적재 거리" : "Loaded", value: `${loaded.toFixed(2)} km/Job` },
      { label: ko ? "전체 거리" : "Total", value: `${total.toFixed(2)} km/Job` },
    ];
  }
  return [{ label: ko ? "적재 비율" : "Loaded", value: `${(100 - R).toFixed(1)} %` }];
}
function KpiExtras({ extras }: { extras?: KExtra[] }) {
  if (!extras || extras.length === 0) return null;
  return (
    <div className="kpi-extras">
      {extras.map((e) => (
        <div className="kpi-extra" key={e.label}><span className="kx-l">{e.label}</span><span className="kx-v mono">{e.value}</span></div>
      ))}
    </div>
  );
}

function SmallCard({ c, trend, lang, extras }: { c: KpiCard; trend?: TrendResponse; lang: Lang; extras?: KExtra[] }) {
  const s = t(lang);
  const imp = isImprovement(c);
  const dl = deltaLabel(c);
  return (
    <div className={`kpi${c.tier === "PRIMARY" ? " primary" : ""}`}>
      <div className="label">{name(c, lang)}</div>
      <div className="vrow">
        <span className="val">{fmtValue(c.value, c.unit)}</span>
        <span className="unit">{c.unit}</span>
      </div>
      <KpiExtras extras={extras} />
      {dl ? (
        <div className={`delta ${imp ? "good" : "bad"}`}>{dl}<span className="vs">{s.vsBaseline}</span></div>
      ) : (
        <div className="delta" style={{ color: "var(--text-mute)" }}>{s.baselinePending}</div>
      )}
      <div className="spark">
        {trend && trend.points.length > 1 && (
          <LineChart values={trend.points.map((p) => p.value)} color={imp === false ? "#f59e0b" : "#60a5fa"} />
        )}
      </div>
    </div>
  );
}

// ---- LIVE tab ----

function useLive() {
  const [live, setLive] = useState<LiveResponse | null>(null);
  const [vessels, setVessels] = useState<VesselsResponse | null>(null);
  const [today, setToday] = useState<KpisResponse | null>(null);
  useEffect(() => {
    let alive = true;
    const load = async () => {
      try {
        const [l, v, td] = await Promise.all([api.live(), api.liveVessels(), api.kpis("today")]);
        if (alive) { setLive(l); setVessels(v); setToday(td); }
      } catch (e) { console.error(e); }
    };
    load();
    const id = setInterval(load, 20000); // fast poll for LIVE
    return () => { alive = false; clearInterval(id); };
  }, []);
  return { live, vessels, today };
}

function LiveCard({ c, lang, extras }: { c: LiveKpi; lang: Lang; extras?: KExtra[] }) {
  const s = t(lang);
  const nm = lang === "ko" ? c.name_ko : c.name_en;
  const imp = c.delta_abs == null || !c.direction ? null : (c.direction === "LOWER_BETTER" ? c.delta_abs < 0 : c.delta_abs > 0);
  const deltaTxt = c.delta_abs == null ? null : (() => {
    const arrow = c.delta_abs > 0 ? "▲" : "▼";
    if (c.unit === "%") return `${arrow} ${Math.abs(c.delta_abs).toFixed(1)}pp`;
    if (c.delta_pct != null) return `${arrow} ${Math.abs(c.delta_pct).toFixed(1)}%`;
    return `${arrow} ${Math.abs(c.delta_abs).toFixed(2)}`;
  })();
  return (
    <div className={`kpi${c.tier === "PRIMARY" ? " primary" : ""}`}>
      <div className="label">{nm}</div>
      <div className="vrow"><span className="val">{fmtValue(c.value, c.unit)}</span><span className="unit">{c.unit}</span></div>
      <KpiExtras extras={extras} />
      {deltaTxt
        ? <div className={`delta ${imp ? "good" : "bad"}`}>{deltaTxt}<span className="vs">{s.vsPrevShift}</span></div>
        : <div className="delta" style={{ color: "var(--text-mute)" }}>{s.noPrevShift}</div>}
      <div className="n" style={{ marginTop: "auto" }}>N {c.sample_n != null ? c.sample_n.toLocaleString() : "—"}</div>
    </div>
  );
}

const vkey = (v: { vessel: string; voyage: string }) => `${v.vessel}/${v.voyage}`;

function VesselPanel({ vessels, lang, onSelect }: { vessels: VesselRow[]; lang: Lang; onSelect: (v: VesselRow) => void }) {
  const s = t(lang);
  const hhmm = (ts: string | null) => (ts && ts.length >= 12 ? `${ts.slice(8, 10)}:${ts.slice(10, 12)}` : "—");
  return (
    <div className="grid vessel-grid">
      {vessels.map((v) => (
        <div className="vessel-card clickable" key={vkey(v)} onClick={() => onSelect(v)} title={s.qcThroughput}>
          <div className="vtop"><span className="vname">{v.vessel}</span><span className="vvoy mono">{v.voyage}</span><span className="spacer" /><span className="vmore">QC ›</span></div>
          <div className="vqcs">{v.qcs.map((q) => <span className="qc-chip" key={q}>{q}</span>)}</div>
          <div className="vprog-row">
            <span className="mono">{v.moves?.toLocaleString() ?? "—"} {s.moves}{v.planned_moves ? ` / ${v.planned_moves.toLocaleString()}` : ""}</span>
            <span className="mono" style={{ color: "var(--brand)" }}>{v.progress_pct != null ? `${v.progress_pct}%` : ""}</span>
          </div>
          {v.progress_pct != null && <div className="vbar"><div className="vbar-fill" style={{ width: `${Math.min(100, v.progress_pct)}%` }} /></div>}
          <div className="vmeta mono">
            <span>{s.mph} {v.mph?.toFixed(1) ?? "—"}</span>
            <span>{s.ldDs} {v.load_moves ?? 0}/{v.discharge_moves ?? 0}</span>
            <span>{hhmm(v.first_move)}→{hhmm(v.last_move)}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

// Today-cumulative KPI card (full-day-so-far, vs previous period). Mirrors LiveCard.
function TodayCard({ c, lang, extras }: { c: KpiCard; lang: Lang; extras?: KExtra[] }) {
  const s = t(lang);
  const imp = isImprovement(c);
  const dl = deltaLabel(c);
  return (
    <div className={`kpi${c.tier === "PRIMARY" ? " primary" : ""}`}>
      <div className="label">{name(c, lang)}</div>
      <div className="vrow"><span className="val">{fmtValue(c.value, c.unit)}</span><span className="unit">{c.unit}</span></div>
      <KpiExtras extras={extras} />
      {dl
        ? <div className={`delta ${imp ? "good" : "bad"}`}>{dl}<span className="vs">{s.vsBaseline}</span></div>
        : <div className="delta" style={{ color: "var(--text-mute)" }}>{s.baselinePending}</div>}
      <div className="n" style={{ marginTop: "auto" }}>N {c.sample_n != null ? c.sample_n.toLocaleString() : "—"}</div>
    </div>
  );
}

// Modern popup: per-QC throughput for one vessel, opened by clicking its card.
function VesselQcModal({ vessel, lang, onClose }: { vessel: VesselRow; lang: Lang; onClose: () => void }) {
  const s = t(lang);
  const hhmm = (ts: string | null) => (ts && ts.length >= 12 ? `${ts.slice(8, 10)}:${ts.slice(10, 12)}` : "—");
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const qcs = vessel.qc_rows ?? [];
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <div><span className="vname">{vessel.vessel}</span> <span className="vvoy mono">{vessel.voyage}</span></div>
          <button className="modal-x" onClick={onClose} aria-label="close">×</button>
        </div>
        <div className="modal-sub mono">
          {(vessel.moves ?? 0).toLocaleString()} {s.moves}
          {vessel.planned_moves ? ` / ${vessel.planned_moves.toLocaleString()}` : ""}
          {vessel.progress_pct != null ? `  ·  ${vessel.progress_pct}%` : ""}
          {"  ·  "}{s.mph} {vessel.mph?.toFixed(1) ?? "—"}
          {"  ·  "}{s.ldDs} {vessel.load_moves ?? 0}/{vessel.discharge_moves ?? 0}
          {"  ·  "}{hhmm(vessel.first_move)}→{hhmm(vessel.last_move)}
        </div>
        <div className="modal-qc-title">{s.qcThroughput}<span className="section-sub">{qcs.length}</span></div>
        {qcs.length === 0 ? (
          <div className="loading">{s.noData}</div>
        ) : (
          <div className="qc-vcards">
            {qcs.map((q) => (
              <div className="qc-mini" key={q.qc}>
                <div className="qc-mini-id mono">{q.qc}</div>
                <div className="qc-mini-mph">{q.mph?.toFixed(1) ?? "—"}<span className="qc-mini-unit"> {s.mph}</span></div>
                <div className="qc-mini-mv mono">{(q.moves ?? 0).toLocaleString()} {s.moves} · {q.load_moves ?? 0}/{q.discharge_moves ?? 0}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function LiveTab({ lang }: { lang: Lang }) {
  const s = t(lang);
  const { live, vessels, today } = useLive();
  const [selKey, setSelKey] = useState<string | null>(null);
  if (!live) return <div className="loading">{s.loading}</div>;
  const shiftName = (s as Record<string, string>)["shift_" + live.shift];
  const vrows = vessels?.vessels ?? [];
  const sel = vrows.find((v) => vkey(v) === selKey) ?? null; // re-resolve each poll so the modal stays live
  return (
    <>
      {/* 현재 쉬프트 정보 */}
      <div className="shift-header">
        <span className="shift-badge">{shiftName} <span className="mono">({live.shift})</span></span>
        <span className="mono shift-win">{live.window_start} → {live.as_of}</span>
        <span className="shift-elapsed">{s.elapsed} {live.elapsed_min}{s.min} · {s.remaining} {live.remaining_min}{s.min}</span>
        <span className="spacer" />
        <span style={{ color: "var(--text-dim)", fontSize: 11 }} className="mono">{live.business_date}</span>
      </div>

      {/* 현재 쉬프트 KPI 7개 */}
      <div className="section-title">{s.shiftKpis}<span className="section-sub">{s.vsPrevShift}</span></div>
      <div className="grid kpi-strip">
        {live.kpis.map((c) => <LiveCard key={c.key} c={c} lang={lang} extras={distanceExtras(live.kpis, c.key, lang)} />)}
      </div>

      {/* 오늘 누적 KPI 7개 */}
      <div className="section-title" style={{ marginTop: 18 }}>{s.todayKpis}<span className="section-sub">{s.vsBaseline}</span></div>
      <div className="grid kpi-strip">
        {(today?.kpis ?? []).map((c) => <TodayCard key={c.key} c={c} lang={lang} extras={distanceExtras(today?.kpis ?? [], c.key, lang)} />)}
      </div>

      {/* 현재 작업 중인 선박 (카드 클릭 → QC별 처리량 팝업) */}
      <div className="section-title" style={{ marginTop: 18 }}>{s.activeVessels}<span className="section-sub">{vrows.length}</span></div>
      <VesselPanel vessels={vrows} lang={lang} onSelect={(v) => setSelKey(vkey(v))} />

      {sel && <VesselQcModal vessel={sel} lang={lang} onClose={() => setSelKey(null)} />}
    </>
  );
}

// Period QC throughput as a modern card grid (replaces the sparse table): each crane
// is a card with its moves/hr (bar = relative to the period's fastest crane) and wait.
function QcGrid({ rows, lang }: { rows: QcRow[]; lang: Lang }) {
  const s = t(lang);
  if (rows.length === 0) return <div className="loading">{s.noData}</div>;
  const maxMph = Math.max(1, ...rows.map((r) => r.mph ?? 0));
  return (
    <div className="qc-grid">
      {rows.map((r) => {
        const pct = r.mph != null ? Math.round((r.mph / maxMph) * 100) : 0;
        return (
          <div className="qcg-card" key={r.qc}>
            <div className="qcg-top"><span className="qcg-id mono">{r.qc}</span></div>
            <div className="qcg-mph">{r.mph != null ? r.mph.toFixed(1) : "—"}<span className="qcg-unit"> {s.mph}</span></div>
            <div className="qcg-bar"><div className="qcg-bar-fill" style={{ width: `${pct}%` }} /></div>
            <div className="qcg-wait mono">{s.qcWait} {r.qc_wait_sec != null ? Math.round(r.qc_wait_sec) : "—"}</div>
          </div>
        );
      })}
    </div>
  );
}

// ---- HISTORY section (period browser; lives at the bottom of the page) ----

function HistoryTab({ lang }: { lang: Lang }) {
  const [period, setPeriod] = useState<string>("last7");
  const data = useData(period);
  const s = t(lang);
  const kpis = data.kpis?.kpis ?? [];
  return (
      <>
        <div className="period-bar">
          <span className="period-label">{s.period}</span>
          {PERIOD_GROUPS.map((g, i) => (
            <div className="period-group" key={i}>
              {g.map((p) => (
                <button key={p} className={`period-btn${period === p ? " active" : ""}`} onClick={() => setPeriod(p)}>
                  {(s as Record<string, string>)["p_" + p]}
                </button>
              ))}
            </div>
          ))}
          {data.kpis && (
            <span className="period-range mono">
              {data.kpis.range_from} ~ {data.kpis.range_to}
              <span style={{ color: "var(--text-faint)" }}> ({s.vs} {data.kpis.prev_from}~{data.kpis.prev_to})</span>
            </span>
          )}
        </div>
        {kpis.length === 0 ? (
          <div className="loading">{s.loading}</div>
        ) : (
          <>
            <div className="section-title">{s.headline}</div>
            <div className="grid kpi-strip">
              {kpis.map((c) => <SmallCard key={c.key} c={c} trend={data.trends[c.key]} lang={lang} extras={distanceExtras(kpis, c.key, lang)} />)}
            </div>

            <div className="section-title" style={{ marginTop: 18 }}>{s.qcBreakdown}<span className="section-sub">{data.breakdown?.as_of}</span></div>
            <QcGrid rows={data.breakdown?.rows ?? []} lang={lang} />
          </>
        )}
      </>
  );
}

// ---- App shell ----

function useHealth(): HealthResponse | null {
  const [h, setH] = useState<HealthResponse | null>(null);
  useEffect(() => {
    let alive = true;
    const load = () => api.health().then((x) => alive && setH(x)).catch(() => {});
    load();
    const id = setInterval(load, 30000);
    return () => { alive = false; clearInterval(id); };
  }, []);
  return h;
}

function KpiPage({ lang }: { lang: Lang }) {
  const s = t(lang);
  return (
    <div className="content">
      <LiveTab lang={lang} />
      <div className="area-divider"><span>{s.areaHistory}</span></div>
      <HistoryTab lang={lang} />
      <HistoryMatrix lang={lang} />
    </div>
  );
}

const IconKpi = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <line x1="6" y1="20" x2="6" y2="13" /><line x1="12" y1="20" x2="12" y2="7" /><line x1="18" y1="20" x2="18" y2="11" />
  </svg>
);
const IconTt = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <rect x="1" y="6" width="13" height="10" rx="1.5" /><path d="M14 9h3.5L21 12.5V16h-7z" /><circle cx="5.5" cy="18.5" r="1.6" /><circle cx="17.5" cy="18.5" r="1.6" />
  </svg>
);
const IconMap = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 21s7-6.3 7-11a7 7 0 1 0-14 0c0 4.7 7 11 7 11z" /><circle cx="12" cy="10" r="2.5" />
  </svg>
);
const IconHealth = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 12h4l2 5 4-12 2 7h6" />
  </svg>
);
const IconFeed = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M4 11a9 9 0 0 1 9 9" /><path d="M4 4a16 16 0 0 1 16 16" /><circle cx="5" cy="19" r="1.5" fill="currentColor" stroke="none" />
  </svg>
);

type PageKey = "kpi" | "tt" | "map" | "health" | "feed";
const PAGES: { key: PageKey; label: string; Icon: () => ReactElement; ko: string; en: string }[] = [
  { key: "kpi", label: "KPI", Icon: IconKpi, ko: "KPI 운영 지표", en: "KPI Metrics" },
  { key: "tt", label: "TT", Icon: IconTt, ko: "TT 배차 현황", en: "TT Dispatch" },
  { key: "map", label: "MAP", Icon: IconMap, ko: "라이브 맵", en: "Live Map" },
  { key: "health", label: "HEALTH", Icon: IconHealth, ko: "AI 배차 헬스", en: "Dispatch Health" },
  { key: "feed", label: "FEED", Icon: IconFeed, ko: "WS 데이터 헬스", en: "WS Data Health" },
];

export default function App() {
  const [lang, setLang] = useState<Lang>("en");
  const [page, setPage] = useState<PageKey>("kpi");
  const clock = useClock();
  const health = useHealth();
  useMemo(() => { document.documentElement.setAttribute("data-lang", lang); }, [lang]);
  const pageName = (p: typeof PAGES[number]) => (lang === "ko" ? p.ko : p.en);

  return (
    <>
      <div className="header">
        <img className="brand-logo" src="/clt-logo-w.png" alt="CLT" />
        <span className="brand-div" />
        <span className="logo">TT <span className="accent">AiOps</span><span className="logo-sub">Platform</span></span>
        <span className="site-chip" title={lang === "ko" ? "제품이 적용된 사이트" : "deployment site"}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
            <path d="M12 21s7-5.5 7-11a7 7 0 1 0-14 0c0 5.5 7 11 7 11Z" /><circle cx="12" cy="10" r="2.5" />
          </svg>
          <span className="site-k">SITE</span>Westports Malaysia
        </span>
        <span className="spacer" />
        <HealthPill health={health} lang={lang} />
        <span className="clock">{clock}</span>
        <div className="lang-toggle">
          <button className={`lang-btn${lang === "ko" ? " active" : ""}`} onClick={() => setLang("ko")}>KO</button>
          <button className={`lang-btn${lang === "en" ? " active" : ""}`} onClick={() => setLang("en")}>EN</button>
        </div>
      </div>
      <div className="app-body">
        <nav className="sidebar">
          {PAGES.map((p) => (
            <button key={p.key} className={`side-item${page === p.key ? " active" : ""}`} onClick={() => setPage(p.key)} title={pageName(p)}>
              <p.Icon />
              <span className="label">{p.label}</span>
            </button>
          ))}
          <span className="side-spacer" />
        </nav>
        <div className="main-col">
          <div className="tabbar">
            {PAGES.map((p) => (
              <button key={p.key} className={`ptab${page === p.key ? " active" : ""}`} onClick={() => setPage(p.key)}>
                <p.Icon /><span>{pageName(p)}</span>
              </button>
            ))}
          </div>
          {page === "kpi" ? <KpiPage lang={lang} /> : page === "tt" ? <TtPage lang={lang} /> : page === "map" ? <LiveMapPage lang={lang} /> : page === "health" ? <HealthPage lang={lang} /> : <FeedHealthPage lang={lang} />}
        </div>
      </div>
    </>
  );
}
