// TT operations page — visual mock (no live data yet). Ports the AI-dispatch
// components from docs/mock-dashboard.html with fake data: QC Sequence & TT Dispatch
// (global pool), TT Status Distribution, TT Utilization (per vehicle), Last Decision.
import { useEffect, useState } from "react";
import { type Lang } from "./i18n";

const ko = (lang: Lang) => lang === "ko";

// bilingual labels so the page follows the KO/EN toggle
const MODE = { discharge: { ko: "양하", en: "Discharge" }, load: { ko: "적하", en: "Loading" } } as const;
type ModeKey = keyof typeof MODE;
const TTS = {
  handover: { ko: "핸드오버 중", en: "Handover" },
  atCrane: { ko: "크레인 도착", en: "At crane" },
  readyIdle: { ko: "유휴 → 출발 대기", en: "Idle → ready" },
  completingPrior: { ko: "이전 작업 완료 중", en: "Completing prior job" },
} as const;
type StatusKey = keyof typeof TTS;

type Task = {
  seq: string;
  kind: "DSC" | "LOD" | "SHF";
  id: string;
  detail: string;
  note?: { ko: string; en: string };
  tt?: string;
  status?: StatusKey;
  movingPct?: number; // when set, status text = "이동 중 (n%)" / "Moving (n%)"
  cls: "now" | "next" | "queued" | "unassigned";
};
type QcCol = {
  id: string; state: "busy" | "idle" | "maint"; mode: ModeKey; vessel: string;
  mph?: number; bay: string; done: number; total: number; tasks: Task[];
};

const QC_COLS: QcCol[] = [
  {
    id: "QC1", state: "busy", mode: "discharge", vessel: "SEMARANG", mph: 28.4,
    bay: "Bay 22, Row 4-7", done: 612, total: 1142,
    tasks: [
      { seq: "NOW", kind: "DSC", id: "K03-1145", detail: "40ft · Bay 22-R5 → K-03 · ETA 14:34:22", tt: "TT-23", status: "handover", cls: "now" },
      { seq: "NEXT", kind: "DSC", id: "K03-1146", detail: "40ft · Bay 22-R6 → K-03", tt: "TT-07", movingPct: 52, cls: "next" },
      { seq: "+2", kind: "DSC", id: "K03-1147", detail: "20ft · Bay 22-R6 → K-03", tt: "TT-31", status: "readyIdle", cls: "queued" },
      { seq: "+3", kind: "DSC", id: "K03-1148", detail: "40ft · Bay 22-R7 → K-03", tt: "TT-14", status: "completingPrior", cls: "queued" },
      { seq: "+4", kind: "DSC", id: "K03-1149", detail: "40ft · Bay 22-R7 → K-03", cls: "unassigned" },
    ],
  },
  {
    id: "QC2", state: "busy", mode: "load", vessel: "COMMITMENT", mph: 31.7,
    bay: "Bay 30, Row 2-5", done: 487, total: 924,
    tasks: [
      { seq: "NOW", kind: "LOD", id: "L05-0822", detail: "20ft · L-05 → Bay 30-R3 · ETA 14:34:01", tt: "TT-12", status: "atCrane", cls: "now" },
      { seq: "NEXT", kind: "LOD", id: "L05-0823", detail: "20ft · L-05 → Bay 30-R3", tt: "TT-41", movingPct: 71, cls: "next" },
      { seq: "+2", kind: "LOD", id: "L05-0824", detail: "40ft · L-05 → Bay 30-R4", tt: "TT-08", status: "readyIdle", cls: "queued" },
      { seq: "+3", kind: "LOD", id: "L05-0825", detail: "40ft · L-05 → Bay 30-R4", tt: "TT-19", status: "completingPrior", cls: "queued" },
      { seq: "+4", kind: "LOD", id: "L05-0826", detail: "20ft · L-05 → Bay 30-R5", cls: "unassigned" },
    ],
  },
  {
    id: "QC3", state: "busy", mode: "load", vessel: "MSC HAMBURG", mph: 29.8,
    bay: "Bay 08, Row 3-6", done: 354, total: 880,
    tasks: [
      { seq: "NOW", kind: "LOD", id: "L02-0451", detail: "40ft · L-02 → Bay 08-R3 · ETA 14:34:10", tt: "TT-17", status: "atCrane", cls: "now" },
      { seq: "NEXT", kind: "LOD", id: "L02-0452", detail: "20ft · L-02 → Bay 08-R4", tt: "TT-33", movingPct: 61, cls: "next" },
      { seq: "+2", kind: "LOD", id: "L02-0453", detail: "40ft · L-02 → Bay 08-R4", tt: "TT-05", status: "readyIdle", cls: "queued" },
      { seq: "+3", kind: "LOD", id: "L02-0454", detail: "20ft · L-02 → Bay 08-R5", tt: "TT-26", status: "completingPrior", cls: "queued" },
      { seq: "+4", kind: "LOD", id: "L02-0455", detail: "40ft · L-02 → Bay 08-R5", cls: "unassigned" },
    ],
  },
  {
    id: "QC4", state: "busy", mode: "discharge", vessel: "CMA PUTRA", mph: 34.1,
    bay: "Bay 14, Row 1-3", done: 731, total: 1056,
    tasks: [
      { seq: "NOW", kind: "DSC", id: "K07-2210", detail: "40ft · Bay 14-R1 → K-07 · ETA 14:33:58", tt: "TT-28", status: "handover", cls: "now" },
      { seq: "NEXT", kind: "DSC", id: "K07-2211", detail: "20ft · Bay 14-R2 → K-07", tt: "TT-44", movingPct: 33, cls: "next" },
      { seq: "+2", kind: "SHF", id: "S02-0184", detail: "40ft · Bay 14-R2 → Y-12", note: { ko: "재배치", en: "reposition" }, tt: "TT-36", status: "readyIdle", cls: "queued" },
      { seq: "+3", kind: "DSC", id: "K07-2212", detail: "40ft · Bay 14-R3 → K-07", tt: "TT-02", status: "completingPrior", cls: "queued" },
      { seq: "+4", kind: "DSC", id: "K07-2213", detail: "40ft · Bay 14-R3 → K-07", cls: "unassigned" },
    ],
  },
];

function ttStatusText(t: Task, lang: Lang): string | null {
  if (t.movingPct != null) return ko(lang) ? `이동 중 (${t.movingPct}%)` : `Moving (${t.movingPct}%)`;
  if (t.status) return TTS[t.status][ko(lang) ? "ko" : "en"];
  return null;
}

const STATUS = [
  { en: "Busy (Loaded)", ko: "Busy (부하 이동)", color: "#22c55e", n: 23 },
  { en: "Empty travel", ko: "공차 이동", color: "#38bdf8", n: 14 },
  { en: "Crane wait", ko: "크레인 대기", color: "#f59e0b", n: 4 },
  { en: "Idle", ko: "Idle", color: "#64748b", n: 9 },
];

type Util = { id: number; v: number; idle?: boolean };
const UTIL: Util[] = Array.from({ length: 50 }, (_, i) => {
  if (i === 7 || i === 28 || i === 41) return { id: i + 1, v: 0, idle: true };
  if (i === 3 || i === 22) return { id: i + 1, v: 41 + (i % 4) };
  const v = Math.round(62 + 33 * Math.abs(Math.sin(i * 1.27 + 0.6)));
  return { id: i + 1, v: Math.min(98, v) };
});
const avgUtil = Math.round(UTIL.filter((u) => !u.idle).reduce((a, u) => a + u.v, 0) / UTIL.filter((u) => !u.idle).length);

function donutGradient() {
  const total = STATUS.reduce((a, s) => a + s.n, 0) || 1;
  let acc = 0;
  const stops = STATUS.filter((s) => s.n > 0).map((s) => {
    const from = (acc / total) * 100;
    acc += s.n;
    const to = (acc / total) * 100;
    return `${s.color} ${from}% ${to}%`;
  });
  return `conic-gradient(${stops.join(", ")})`;
}

function QcSequence({ lang }: { lang: Lang }) {
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "QC 시퀀스 & TT 배차 현황" : "QC Sequence & TT Dispatch (Global Pool)"}
          <span className="h3-sub">{ko(lang) ? "전 항차 통합" : "across all active vessels"}</span></h3>
        <div className="head-sub">
          <span className="pill good">{ko(lang) ? "진행 중" : "In progress"} 4</span>
          <span className="muted">{ko(lang) ? "갱신: 2초 전" : "Updated: 2s ago"}</span>
        </div>
      </div>
      <div className="tcard-body">
        <div className="qc-panel">
          {QC_COLS.map((q) => (
            <div className={`qc-col${q.state === "maint" ? " maint" : ""}`} key={q.id}>
              <div className="qc-head">
                <span className={`id ${q.state}`}><span className="dot" />{q.id} · {MODE[q.mode][ko(lang) ? "ko" : "en"]}
                  <span className="qc-vessel">{q.vessel}</span></span>
                {q.mph != null && <span className="mph">MPH <span className="v">{q.mph}</span></span>}
              </div>
              {q.total > 0 && (
                <>
                  <div className="qc-progress"><span>{q.bay}</span><span className="mono">{q.done.toLocaleString()} / {q.total.toLocaleString()}</span></div>
                  <div className={`qc-progress-bar${q.state === "maint" ? " maint" : ""}`}>
                    <div className="fill" style={{ width: `${Math.round((q.done / q.total) * 100)}%` }} />
                  </div>
                </>
              )}
              {q.state === "maint" && <div className="qc-progress"><span>{q.bay}</span></div>}
              {q.tasks.map((tk, i) => (
                <div className={`qc-task ${tk.cls}`} key={i}>
                  <span className="seq">{tk.seq}</span>
                  <div className="body">
                    <div className="top"><span className={`type-${tk.kind.toLowerCase()}`}>{tk.kind}</span> {tk.id}</div>
                    <div className="bot">{tk.detail}{tk.note ? ` (${tk.note[ko(lang) ? "ko" : "en"]})` : ""}</div>
                  </div>
                  <div className="assign">
                    {tk.tt ? <span className="tt">{tk.tt}</span> : tk.cls === "unassigned" ? <span className="tt-none">{ko(lang) ? "미배차" : "Unassigned"}</span> : null}
                    {ttStatusText(tk, lang) && <span className="tt-status">{ttStatusText(tk, lang)}</span>}
                  </div>
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function StatusDistribution({ lang }: { lang: Lang }) {
  const total = STATUS.reduce((a, s) => a + s.n, 0);
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "TT 상태 분포" : "TT Status Distribution"}</h3>
        <div className="head-sub"><span className="muted">{ko(lang) ? `총 ${total}대` : `Total ${total} units`}</span></div>
      </div>
      <div className="tcard-body">
        <div className="donut-wrap">
          <div className="donut" style={{ background: donutGradient() }}>
            <div className="donut-hole"><span className="dn">{total}</span><span className="dl">{ko(lang) ? "대" : "units"}</span></div>
          </div>
          <div className="legend-list">
            {STATUS.map((s) => (
              <div className="legend-item" key={s.en}>
                <span className="swatch" style={{ background: s.color }} />
                <span className="name">{ko(lang) ? s.ko : s.en}</span>
                <span className="val">{s.n}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function LastDecision({ lang }: { lang: Lang }) {
  const factors = [
    { cls: "cf-f1", l: ko(lang) ? "F1 차량 대기" : "F1 Truck Wait", w: 18, v: "2.1s" },
    { cls: "cf-f2", l: ko(lang) ? "F2 크레인 대기" : "F2 Crane wait", w: 55, v: "6.4s" },
    { cls: "cf-f3", l: ko(lang) ? "F3 공차 시간" : "F3 Empty Time", w: 38, v: "4.4s" },
  ];
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "최근 배차 결정" : "Last Decision"}</h3>
        <div className="head-sub mono">14:32:04 · 17ms</div>
      </div>
      <div className="tcard-body">
        <div className="ai-decision">
          <div className="head">
            <span className="title">TT-23 → DSC-K03-1145</span>
            <span className="pill good">{ko(lang) ? "선택" : "Chosen"}</span>
          </div>
          {factors.map((f) => (
            <div className={`cost-bar ${f.cls}`} key={f.cls}>
              <span className="l">{f.l}</span>
              <div className="bar-bg"><div className="bar-fill" style={{ width: `${f.w}%` }} /></div>
              <span className="v">{f.v}</span>
            </div>
          ))}
          <div className="ai-total">
            <span className="muted">{ko(lang) ? "총 비용 (Hungarian)" : "Total Cost (Hungarian)"}</span>
            <span className="mono total-v">12.9s</span>
          </div>
          <div className="ai-alt">{ko(lang) ? "차선 후보: " : "Alt. candidates: "}TT-07 (14.2s) · TT-31 (15.8s)</div>
        </div>
      </div>
    </section>
  );
}

function Utilization({ lang }: { lang: Lang }) {
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "TT 활용률 (차량별, 직전 1시간)" : "TT Utilization (per vehicle, last 1H)"}</h3>
        <div className="head-sub"><span className="muted">{ko(lang) ? "평균" : "Avg"} {avgUtil}%</span></div>
      </div>
      <div className="tcard-body">
        <div className="util-grid">
          {UTIL.map((u) => {
            const cls = u.idle ? "idle" : u.v < 50 ? "bad" : u.v < 70 ? "warn" : "";
            return (
              <div className={`util-cell ${cls}`} key={u.id} title={`TT-${u.id} · ${u.idle ? "idle" : u.v + "%"}`}>
                <div className="fill" style={{ height: `${u.idle ? 8 : Math.max(8, u.v)}%` }} />
              </div>
            );
          })}
        </div>
        <div className="util-legend">
          <span><i style={{ background: "var(--brand-deep)" }} />{ko(lang) ? "정상" : "Normal"} (≥70%)</span>
          <span><i style={{ background: "var(--warn)" }} />{ko(lang) ? "저조" : "Low"} (50~70%)</span>
          <span><i style={{ background: "var(--bad)" }} />{ko(lang) ? "심각" : "Critical"} (&lt;50%)</span>
          <span><i style={{ background: "var(--text-faint)" }} />{ko(lang) ? "유휴" : "Idle"}</span>
        </div>
      </div>
    </section>
  );
}

// ── candidate VEHICLE pool (dispatch input) ──
const VPOOL = {
  idle: ["TT-09", "TT-21", "TT-38", "TT-50"] as string[],
  soon: [{ tt: "TT-15", eta: 38 }, { tt: "TT-27", eta: 52 }, { tt: "TT-03", eta: 71 }],
  swap: ["TT-30", "TT-42"] as string[],
};
const VPOOL_TOTAL = VPOOL.idle.length + VPOOL.soon.length + VPOOL.swap.length;

// ── candidate JOB pool: QC jobs by ETW urgency; capped to the vehicle-pool size ──
type Job = { qc: string; id: string; kind: "DSC" | "LOD" | "SHF"; size: string; etw: number };
const JOBS_ALL: Job[] = [
  { qc: "QC1", id: "K03-1149", kind: "DSC", size: "40ft", etw: 18 },
  { qc: "QC4", id: "K07-2213", kind: "DSC", size: "40ft", etw: 24 },
  { qc: "QC2", id: "L05-0826", kind: "LOD", size: "20ft", etw: 31 },
  { qc: "QC3", id: "L02-0455", kind: "LOD", size: "40ft", etw: 39 },
  { qc: "QC1", id: "K03-1150", kind: "DSC", size: "20ft", etw: 47 },
  { qc: "QC4", id: "S02-0185", kind: "SHF", size: "40ft", etw: 55 },
  { qc: "QC2", id: "L05-0827", kind: "LOD", size: "40ft", etw: 63 },
  { qc: "QC3", id: "L02-0456", kind: "LOD", size: "20ft", etw: 72 },
  { qc: "QC1", id: "K03-1151", kind: "DSC", size: "40ft", etw: 84 },
  { qc: "QC4", id: "K07-2214", kind: "DSC", size: "40ft", etw: 98 },
  { qc: "QC2", id: "L05-0828", kind: "LOD", size: "20ft", etw: 110 },
  { qc: "QC3", id: "L02-0457", kind: "LOD", size: "40ft", etw: 126 },
];
const JOB_POOL: Job[] = [...JOBS_ALL].sort((a, b) => a.etw - b.etw).slice(0, VPOOL_TOTAL);
const etwCls = (s: number) => (s < 25 ? "bad" : s < 55 ? "warn" : "ok");

function VehiclePool({ lang }: { lang: Lang }) {
  const groups = [
    { color: "#22c55e", ko: "idle 차량", en: "Idle", n: VPOOL.idle.length, chips: VPOOL.idle.map((tt) => ({ tt, sub: "" })) },
    { color: "#f59e0b", ko: "곧 idle 예정", en: "Becoming idle", n: VPOOL.soon.length, chips: VPOOL.soon.map((s) => ({ tt: s.tt, sub: `~${s.eta}s` })) },
    { color: "#38bdf8", ko: "스왑 가능 공차", en: "Swappable empty", n: VPOOL.swap.length, chips: VPOOL.swap.map((tt) => ({ tt, sub: "" })) },
  ];
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "후보 차량 풀" : "Candidate Vehicle Pool"}</h3>
        <div className="head-sub"><span className="muted">{ko(lang) ? "총" : "Total"} {VPOOL_TOTAL}</span></div>
      </div>
      <div className="tcard-body pool-body">
        {groups.map((g) => (
          <div className="pool-grp" key={g.en}>
            <div className="pool-grp-h"><span className="pool-sw" style={{ background: g.color }} />{ko(lang) ? g.ko : g.en}<span className="pool-n">{g.n}</span></div>
            <div className="pool-chips">
              {g.chips.map((c) => (
                <span className="pool-chip" key={c.tt} style={{ borderColor: g.color }}>
                  <span className="mono">{c.tt}</span>{c.sub && <span className="pool-sub">{c.sub}</span>}
                </span>
              ))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function JobPool({ lang }: { lang: Lang }) {
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "후보 작업 풀" : "Candidate Job Pool"}<span className="h3-sub">{ko(lang) ? "ETW 시급순" : "by ETW urgency"}</span></h3>
        <div className="head-sub"><span className="muted">{JOB_POOL.length} / {ko(lang) ? `최대 ${VPOOL_TOTAL}` : `max ${VPOOL_TOTAL}`}</span></div>
      </div>
      <div className="tcard-body">
        <div className="jpool">
          {JOB_POOL.map((j, i) => (
            <div className="jrow" key={j.id}>
              <span className="jrank mono">{i + 1}</span>
              <span className="jqc">{j.qc}</span>
              <span className="jid"><span className={`type-${j.kind.toLowerCase()}`}>{j.kind}</span> <span className="mono">{j.id}</span></span>
              <span className="jsize mono">{j.size}</span>
              <span className={`jetw ${etwCls(j.etw)}`}>ETW {j.etw}s</span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

// ── REAL live dispatch pool (from /api/livemap/positions) ──
type LiveTT = { id: string; cls: string; dispatch?: string; jobtype?: string; topos1?: string; dispatch_reason?: string; nearest_rtg_m?: number; swappable?: boolean; dest_remaining_m?: number };
type Snap = { connected: boolean; as_of: string | null; dispatch_counts?: Record<string, number>; devices: LiveTT[] };
const DSP_META: Record<string, { ko: string; en: string; color: string }> = {
  idle: { ko: "유휴 (배차 가능)", en: "Idle (available)", color: "#22c55e" },
  soon_idle: { ko: "곧 유휴", en: "Soon idle", color: "#f59e0b" },
  delivering: { ko: "적재 이동", en: "Delivering", color: "#64748b" },
  wait_rtg: { ko: "도착·RTG 대기", en: "Arrived·wait RTG", color: "#ef4444" },
  empty_travel: { ko: "공차 주행 중", en: "Empty traveling", color: "#94a3b8" },
};
const DSP_ORDER = ["idle", "soon_idle", "delivering", "wait_rtg", "empty_travel"];

function LiveDispatchPool({ lang }: { lang: Lang }) {
  const [snap, setSnap] = useState<Snap | null>(null);
  const [err, setErr] = useState(false);
  useEffect(() => {
    let alive = true;
    const poll = async () => {
      try {
        const r = await fetch("/api/livemap/positions");
        if (!r.ok) throw new Error();
        const j: Snap = await r.json();
        if (alive) { setSnap(j); setErr(false); }
      } catch { if (alive) setErr(true); }
    };
    poll();
    const iv = setInterval(poll, 2500);
    return () => { alive = false; clearInterval(iv); };
  }, []);
  const tts = (snap?.devices ?? []).filter((d) => d.cls === "TT");
  const counts = snap?.dispatch_counts ?? {};
  const soon = tts.filter((d) => d.dispatch === "soon_idle").sort((a, b) => a.id.localeCompare(b.id));
  const idle = tts.filter((d) => d.dispatch === "idle").sort((a, b) => a.id.localeCompare(b.id));
  const empties = tts.filter((d) => d.dispatch === "empty_travel");
  // swap candidates = empty-traveling toward a pickup with meaningful remaining distance.
  // Near-destination / no-destination (회송) empties are excluded — not worth swapping.
  const swap = empties.filter((d) => d.swappable).sort((a, b) => (b.dest_remaining_m ?? 1e9) - (a.dest_remaining_m ?? 1e9));
  const swapExcluded = empties.length - swap.length;
  const ageS = snap?.as_of ? Math.max(0, Math.round((Date.now() - Date.parse(snap.as_of)) / 1000)) : null;

  return (
    <section className="tcard lvp">
      <div className="tcard-head">
        <h3>{ko(lang) ? "실시간 배차 풀" : "Live Dispatch Pool"}
          <span className="h3-sub">{ko(lang) ? "websocket GPS/PLC · standalone" : "from websocket GPS/PLC"}</span></h3>
        <div className="head-sub">
          <span className={`pill ${snap?.connected ? "good" : "bad"}`}><span className="dot" />{snap?.connected ? "LIVE" : (err ? "OFF" : "…")}</span>
          <span className="muted">{ageS != null ? `⟳ ${ageS}s` : ""}</span>
        </div>
      </div>
      <div className="tcard-body">
        <div className="lvp-stats">
          {DSP_ORDER.map((k) => (
            <div className="lvp-stat" key={k} style={{ borderTopColor: DSP_META[k].color }}>
              <div className="lvp-n">{counts[k] ?? 0}</div>
              <div className="lvp-l">{ko(lang) ? DSP_META[k].ko : DSP_META[k].en}</div>
            </div>
          ))}
        </div>
        <div className="lvp-cols lvp-cols3">
          {/* 1. Idle now — empty + stationary, dispatchable immediately */}
          <div className="lvp-col">
            <div className="lvp-col-h"><span className="sw" style={{ background: DSP_META.idle.color }} />{ko(lang) ? "현재 유휴" : "Idle now"}<span className="lvp-cn">{idle.length}</span></div>
            <div className="lvp-sub">{ko(lang) ? "즉시 배차 가능" : "dispatchable now"}</div>
            <div className="lvp-chips">
              {idle.length === 0 && <div className="lvp-empty">{ko(lang) ? "없음" : "none"}</div>}
              {idle.slice(0, 48).map((d) => <span className="lvp-chip idle mono" key={d.id}>{d.id}</span>)}
              {idle.length > 48 && <span className="lvp-more">+{idle.length - 48}</span>}
            </div>
          </div>
          {/* 2. Soon-idle — finishing the last handover */}
          <div className="lvp-col">
            <div className="lvp-col-h"><span className="sw" style={{ background: DSP_META.soon_idle.color }} />{ko(lang) ? "곧 유휴" : "Soon-idle"}<span className="lvp-cn">{soon.length}</span></div>
            <div className="lvp-sub">{ko(lang) ? "마지막 핸드오버 진행" : "at final handover"}</div>
            <div className="lvp-list">
              {soon.length === 0 && <div className="lvp-empty">{ko(lang) ? "없음" : "none"}</div>}
              {soon.map((d) => (
                <div className="lvp-row" key={d.id}>
                  <span className="lvp-id mono">{d.id}</span>
                  {d.jobtype && <span className={`lvp-job type-${d.jobtype.toLowerCase()}`}>{d.jobtype}</span>}
                  {d.topos1 && <span className="lvp-dest mono">→{d.topos1}</span>}
                  <span className="lvp-why">{d.dispatch_reason}</span>
                </div>
              ))}
            </div>
          </div>
          {/* 3. Swappable empty — empty-traveling toward a job, not yet loaded → re-match candidate */}
          <div className="lvp-col">
            <div className="lvp-col-h"><span className="sw" style={{ background: DSP_META.empty_travel.color }} />{ko(lang) ? "스왑 가능한 공차" : "Swappable empty"}<span className="lvp-cn">{swap.length}</span></div>
            <div className="lvp-sub">{ko(lang) ? `픽업까지 잔여 ≥150m · 근접/회송 ${swapExcluded} 제외` : `≥150m left to pickup · ${swapExcluded} excluded`}</div>
            <div className="lvp-list">
              {swap.length === 0 && <div className="lvp-empty">{ko(lang) ? "없음" : "none"}</div>}
              {swap.map((d) => (
                <div className="lvp-row" key={d.id}>
                  <span className="lvp-id mono">{d.id}</span>
                  {d.jobtype && <span className={`lvp-job type-${d.jobtype.toLowerCase()}`}>{d.jobtype}</span>}
                  {d.topos1 && <span className="lvp-dest mono">→{d.topos1}</span>}
                  <span className="lvp-why">{d.dest_remaining_m != null ? (ko(lang) ? `잔여 ${Math.round(d.dest_remaining_m)}m` : `${Math.round(d.dest_remaining_m)}m left`) : (ko(lang) ? "목적지 학습 중" : "dest learning")}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
        <div className="lvp-note">{ko(lang) ? "곧유휴 = 적재 TT가 마지막 핸드오버 단계 + 크레인 관여(QC PLC / RTG 같은 bay). 스왑 가능한 공차 = 작업을 향해 공차 주행 중이나 아직 미상차 → 더 가까운 작업으로 재매칭(스왑) 대상." : "Soon-idle = loaded TT at its final handover (QC PLC / RTG same bay). Swappable empty = empty-traveling toward a job but not yet loaded → re-match (swap) candidate."}</div>
      </div>
    </section>
  );
}

export default function TtPage({ lang }: { lang: Lang }) {
  return (
    <div className="content tt-page">
      <LiveDispatchPool lang={lang} />
      <QcSequence lang={lang} />
      <div className="grid tt-two">
        <VehiclePool lang={lang} />
        <JobPool lang={lang} />
      </div>
      <div className="grid tt-two">
        <StatusDistribution lang={lang} />
        <LastDecision lang={lang} />
      </div>
      <Utilization lang={lang} />
    </div>
  );
}
