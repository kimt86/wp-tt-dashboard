// TT operations page. The work pool (per-QC sequence + urgent job front) and the
// vehicle pool are LIVE: the work pool comes from /api/workpool (TOS JOB_QUEUE_SCHEDULE
// + JOB_ORDER_LIST, refreshed ~90s into Postgres) fused with /api/livemap/positions
// (websocket PLC = crane physically cycling, GPS = where the assigned TT actually is).
// Status distribution is live from the dispatch counts. Last Decision + Utilization
// remain visual mocks (future AI-dispatch panels).
import { useEffect, useState } from "react";
import { type Lang } from "./i18n";
import { api, type WorkpoolResponse, type WpMove, type WpQc } from "./api";

const ko = (lang: Lang) => lang === "ko";

// ── shared live sources ──
type Dev = {
  id: string; cls: string; speed?: number; age_s?: number;
  dispatch?: string; dispatch_reason?: string; arrival?: string; topos1?: string;
  plc?: { is_loaded: boolean; age_s: number };
};
type Snap = { connected: boolean; as_of: string | null; dispatch_counts?: Record<string, number>; devices: Dev[] };

function usePositions(ms = 3000) {
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
    const iv = setInterval(poll, ms);
    return () => { alive = false; clearInterval(iv); };
  }, [ms]);
  return { snap, err };
}

function useWorkpool(ms = 15000) {
  const [data, setData] = useState<WorkpoolResponse | null>(null);
  const [err, setErr] = useState(false);
  useEffect(() => {
    let alive = true;
    const poll = () => api.workpool().then((d) => { if (alive) { setData(d); setErr(false); } }).catch(() => { if (alive) setErr(true); });
    poll();
    const iv = setInterval(poll, ms);
    return () => { alive = false; clearInterval(iv); };
  }, [ms]);
  return { data, err };
}

// dispatch-state colors (shared with the live map / vehicle pool)
const DSP_META: Record<string, { ko: string; en: string; color: string }> = {
  idle: { ko: "유휴 (배차 가능)", en: "Idle (available)", color: "#22c55e" },
  soon_idle: { ko: "곧 유휴", en: "Soon idle", color: "#f59e0b" },
  delivering: { ko: "적재 이동", en: "Delivering", color: "#64748b" },
  wait_rtg: { ko: "도착·RTG 대기", en: "Arrived·wait RTG", color: "#ef4444" },
  empty_travel: { ko: "공차 주행 중", en: "Empty traveling", color: "#94a3b8" },
};
const DSP_ORDER = ["idle", "soon_idle", "delivering", "wait_rtg", "empty_travel"];

// ETW countdown (ETW − now), recomputed each positions poll
function etwLabel(etw: string | null, lang: Lang): { text: string; cls: string } | null {
  if (!etw) return null;
  const sec = Math.round((Date.parse(etw) - Date.now()) / 1000);
  const abs = Math.abs(sec);
  const mm = Math.floor(abs / 60), ss = abs % 60;
  const t = mm > 0 ? `${mm}:${String(ss).padStart(2, "0")}` : `${ss}s`;
  if (sec < -30) return { text: ko(lang) ? `지연 ${t}` : `overdue ${t}`, cls: "bad" };
  if (sec < 90) return { text: ko(lang) ? `곧 ${t}` : t, cls: "bad" };
  if (sec < 240) return { text: t, cls: "warn" };
  return { text: t, cls: "ok" };
}

const kindChip = (jt: string | null) => (jt === "DS" ? "dsc" : jt === "LD" ? "lod" : "shf");
const kindLabel = (jt: string | null) => (jt === "DS" ? "DSC" : jt === "LD" ? "LOD" : "SHF");

// ───────────────────────── live vehicle pool ─────────────────────────
type LiveTT = { id: string; cls: string; dispatch?: string; jobtype?: string; topos1?: string; dispatch_reason?: string; swappable?: boolean; dest_remaining_m?: number };

function LiveDispatchPool({ lang, snap, err }: { lang: Lang; snap: Snap | null; err: boolean }) {
  const tts = ((snap?.devices ?? []) as LiveTT[]).filter((d) => d.cls === "TT");
  const counts = snap?.dispatch_counts ?? {};
  const soon = tts.filter((d) => d.dispatch === "soon_idle").sort((a, b) => a.id.localeCompare(b.id));
  const idle = tts.filter((d) => d.dispatch === "idle").sort((a, b) => a.id.localeCompare(b.id));
  const empties = tts.filter((d) => d.dispatch === "empty_travel");
  const swap = empties.filter((d) => d.swappable).sort((a, b) => (b.dest_remaining_m ?? 1e9) - (a.dest_remaining_m ?? 1e9));
  const swapExcluded = empties.length - swap.length;
  const ageS = snap?.as_of ? Math.max(0, Math.round((Date.now() - Date.parse(snap.as_of)) / 1000)) : null;

  return (
    <section className="tcard lvp">
      <div className="tcard-head">
        <h3>{ko(lang) ? "실시간 배차 풀 (차량)" : "Live Dispatch Pool (vehicles)"}
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
          <div className="lvp-col">
            <div className="lvp-col-h"><span className="sw" style={{ background: DSP_META.idle.color }} />{ko(lang) ? "현재 유휴" : "Idle now"}<span className="lvp-cn">{idle.length}</span></div>
            <div className="lvp-sub">{ko(lang) ? "즉시 배차 가능" : "dispatchable now"}</div>
            <div className="lvp-chips">
              {idle.length === 0 && <div className="lvp-empty">{ko(lang) ? "없음" : "none"}</div>}
              {idle.slice(0, 48).map((d) => <span className="lvp-chip idle mono" key={d.id}>{d.id}</span>)}
              {idle.length > 48 && <span className="lvp-more">+{idle.length - 48}</span>}
            </div>
          </div>
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
      </div>
    </section>
  );
}

// ───────────────────────── live QC work sequence ─────────────────────────
const QC_CAP = 12;     // columns shown
const MOVE_CAP = 5;    // active move cards per QC

function LiveQcSequence({ lang, wp, snap }: { lang: Lang; wp: WorkpoolResponse | null; snap: Snap | null }) {
  // fuse: live crane PLC (cycling now) + per-TT dispatch state
  const ttState = new Map<string, Dev>();
  const craneFresh = new Map<string, boolean>();
  for (const d of snap?.devices ?? []) {
    if (d.cls === "TT") ttState.set(d.id, d);
    else if (d.plc) craneFresh.set(d.id, (d.plc.age_s ?? 999) <= 120);
  }
  const qcs = (wp?.qcs ?? []).slice(0, QC_CAP);
  const ageS = wp?.as_of ? Math.max(0, Math.round((Date.now() - Date.parse(wp.as_of)) / 1000)) : null;

  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "QC 작업 시퀀스 & 배차 (라이브)" : "QC Work Sequence & Dispatch (live)"}
          <span className="h3-sub">{ko(lang) ? "TOS 작업지시 + PLC/GPS 융합" : "TOS job orders fused with PLC/GPS"}</span></h3>
        <div className="head-sub">
          <span className="pill good">{ko(lang) ? "가동 QC" : "Working QC"} {wp?.qc_count ?? 0}</span>
          <span className="muted">{ko(lang) ? `잔여 ${(wp?.total_remaining ?? 0).toLocaleString()} move` : `${(wp?.total_remaining ?? 0).toLocaleString()} moves left`}</span>
          <span className="muted">{ageS != null ? `⟳ ${ageS}s` : ""}</span>
        </div>
      </div>
      <div className="tcard-body">
        {qcs.length === 0 && <div className="lvp-empty">{ko(lang) ? "가동 중인 QC 없음" : "no working QC"}</div>}
        <div className="qc-panel">
          {qcs.map((q) => <QcCol key={q.qc} q={q} lang={lang} ttState={ttState} working={craneFresh.get(q.qc) ?? false} />)}
        </div>
        {(wp?.qcs.length ?? 0) > QC_CAP && (
          <div className="lvp-note">{ko(lang) ? `+${(wp!.qcs.length - QC_CAP)} QC 더 (작업량 적은 순 생략)` : `+${wp!.qcs.length - QC_CAP} more QC (fewer active moves)`}</div>
        )}
      </div>
    </section>
  );
}

function QcCol({ q, lang, ttState, working }: { q: WpQc; lang: Lang; ttState: Map<string, Dev>; working: boolean }) {
  const [open, setOpen] = useState(false);
  const tot = q.queues.reduce((a, x) => a + x.total, 0);
  const done = q.queues.reduce((a, x) => a + x.done, 0);
  const pct = tot > 0 ? Math.round((done / tot) * 100) : 0;
  const moves = open ? q.moves : q.moves.slice(0, MOVE_CAP);
  const extra = q.moves.length - MOVE_CAP;
  return (
    <div className="qc-col">
      <div className="qc-head">
        <span className={`id ${working ? "busy" : "idle"}`}><span className="dot" />{q.qc}
          <span className="qc-vessel">{q.vessels.join(" · ") || "—"}</span></span>
        <span className="mph">{ko(lang) ? "잔여" : "rem"} <span className="v">{q.remaining}</span></span>
      </div>
      <div className="qc-progress"><span>{q.active_moves} {ko(lang) ? "작업중" : "active"}{working ? (ko(lang) ? " · PLC 가동" : " · PLC live") : ""}</span><span className="mono">{done.toLocaleString()} / {tot.toLocaleString()}</span></div>
      <div className="qc-progress-bar"><div className="fill" style={{ width: `${pct}%` }} /></div>
      {moves.length === 0 && <div className="lvp-empty" style={{ padding: "10px 0" }}>{ko(lang) ? "활성 작업 없음" : "no active move"}</div>}
      {moves.map((m, i) => {
        const etw = etwLabel(m.etw_ts, lang);
        const tt = m.ytno ? ttState.get(m.ytno) : undefined;
        const dot = tt?.dispatch ? DSP_META[tt.dispatch]?.color : undefined;
        return (
          <div className={`qc-task ${i === 0 ? "now" : "queued"}`} key={`${m.contno}-${i}`}>
            <span className="seq">{i === 0 ? "NOW" : `+${i}`}</span>
            <div className="body">
              <div className="top"><span className={`type-${kindChip(m.jobtype)}`}>{kindLabel(m.jobtype)}</span> {m.contno ?? "—"}{m.twintandem ? ` · ${m.twintandem}` : ""}</div>
              <div className="bot">
                {m.jobtype === "DS" ? `${m.yt_topos ?? m.from_pos ?? "?"} → ${m.armgc ?? "RTG"}` : `${m.armgc ?? m.yt_topos ?? "?"} → ${q.qc}`}
                {etw && <span className={`jetw ${etw.cls}`} style={{ marginLeft: 6 }}>ETW {etw.text}</span>}
              </div>
            </div>
            <div className="assign">
              {m.ytno ? <span className="tt" title={tt?.dispatch_reason}>{dot && <span className="dot" style={{ background: dot, marginRight: 4 }} />}{m.ytno}</span> : <span className="tt-none">{ko(lang) ? "미배차" : "Unassigned"}</span>}
              {tt?.dispatch && <span className="tt-status">{ko(lang) ? DSP_META[tt.dispatch]?.ko : DSP_META[tt.dispatch]?.en}</span>}
            </div>
          </div>
        );
      })}
      {extra > 0 && (
        <button className="qc-more" onClick={() => setOpen((v) => !v)}>
          {open ? (ko(lang) ? "접기 ▲" : "collapse ▲") : (ko(lang) ? `+${extra} 작업 더 보기 ▼` : `+${extra} more ▼`)}
        </button>
      )}
    </div>
  );
}

// ───────────────────────── live urgent job front ─────────────────────────
function LiveJobPool({ lang, wp, snap }: { lang: Lang; wp: WorkpoolResponse | null; snap: Snap | null }) {
  const ttState = new Map<string, Dev>();
  for (const d of snap?.devices ?? []) if (d.cls === "TT") ttState.set(d.id, d);
  const pool = (wp?.pool ?? []).slice(0, 24);
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "임박 작업 프론트" : "Imminent Work Front"}<span className="h3-sub">{ko(lang) ? "ETW 시급순" : "by ETW urgency"}</span></h3>
        <div className="head-sub"><span className="muted">{ko(lang) ? `상위 ${pool.length}` : `top ${pool.length}`}</span></div>
      </div>
      <div className="tcard-body">
        <div className="jpool">
          {pool.length === 0 && <div className="lvp-empty">{ko(lang) ? "데이터 없음" : "no data"}</div>}
          {pool.map((m: WpMove, i) => {
            const etw = etwLabel(m.etw_ts, lang);
            const tt = m.ytno ? ttState.get(m.ytno) : undefined;
            const dot = tt?.dispatch ? DSP_META[tt.dispatch]?.color : undefined;
            return (
              <div className="jrow" key={`${m.contno}-${i}`}>
                <span className="jrank mono">{i + 1}</span>
                <span className="jqc">{m.qc}</span>
                <span className="jid"><span className={`type-${kindChip(m.jobtype)}`}>{kindLabel(m.jobtype)}</span> <span className="mono">{m.contno ?? "—"}</span></span>
                <span className="jsize mono" title={tt?.dispatch_reason}>{dot && <span className="dot" style={{ background: dot, marginRight: 4 }} />}{m.ytno ?? "—"}</span>
                {etw && <span className={`jetw ${etw.cls}`}>ETW {etw.text}</span>}
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

// ───────────────────────── status distribution (live) ─────────────────────────
function StatusDistribution({ lang, snap }: { lang: Lang; snap: Snap | null }) {
  const counts = snap?.dispatch_counts ?? {};
  const STATUS = DSP_ORDER.map((k) => ({ key: k, color: DSP_META[k].color, ko: DSP_META[k].ko, en: DSP_META[k].en, n: counts[k] ?? 0 }));
  const total = STATUS.reduce((a, s) => a + s.n, 0);
  let acc = 0;
  const stops = STATUS.filter((s) => s.n > 0).map((s) => {
    const from = (acc / (total || 1)) * 100; acc += s.n; const to = (acc / (total || 1)) * 100;
    return `${s.color} ${from}% ${to}%`;
  });
  const grad = total > 0 ? `conic-gradient(${stops.join(", ")})` : "conic-gradient(#222d44 0% 100%)";
  return (
    <section className="tcard">
      <div className="tcard-head">
        <h3>{ko(lang) ? "TT 상태 분포 (라이브)" : "TT Status Distribution (live)"}</h3>
        <div className="head-sub"><span className="muted">{ko(lang) ? `총 ${total}대` : `Total ${total} units`}</span></div>
      </div>
      <div className="tcard-body">
        <div className="donut-wrap">
          <div className="donut" style={{ background: grad }}>
            <div className="donut-hole"><span className="dn">{total}</span><span className="dl">{ko(lang) ? "대" : "units"}</span></div>
          </div>
          <div className="legend-list">
            {STATUS.map((s) => (
              <div className="legend-item" key={s.key}>
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

export default function TtPage({ lang }: { lang: Lang }) {
  const { snap, err } = usePositions();
  const { data: wp } = useWorkpool();
  return (
    <div className="content tt-page">
      <LiveDispatchPool lang={lang} snap={snap} err={err} />
      <LiveQcSequence lang={lang} wp={wp} snap={snap} />
      <div className="grid tt-two">
        <LiveJobPool lang={lang} wp={wp} snap={snap} />
        <StatusDistribution lang={lang} snap={snap} />
      </div>
    </div>
  );
}
