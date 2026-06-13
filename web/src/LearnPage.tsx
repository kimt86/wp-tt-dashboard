// 학습 센터 — 축적되는 학습데이터와 모델 성능·개선 추이.
// v1 모델 = 블록 작업지점 좌표(②): TT가 topos 타깃에 ARRIVED한 GPS를 누적해 좌표를 학습.
// API /api/learn/topos: 학습된 점(point) + 요약 + 시간순 품질(metric_series).
import { useEffect, useMemo, useState } from "react";
import { type Lang } from "./i18n";
import { api, type LearnTopos, type LearnToposPoint } from "./api";
import { LineChart } from "./charts";

const ko = (lang: Lang) => lang === "ko";
const fmtN = (n: number) => n.toLocaleString();
const mPrec = (m: number | null | undefined) => (m == null ? "—" : `${m.toFixed(1)}m`);
const stamp = (iso: string | null | undefined) =>
  iso ? new Date(iso).toLocaleString([], { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit", hour12: false }) : "—";

function Tile({ label, value, unit, accent }: { label: string; value: string; unit?: string; accent?: string }) {
  return (
    <div className="cyc-tile" style={accent ? { borderTopColor: accent } : undefined}>
      <div className="cyc-tile-l">{label}</div>
      <div className="cyc-tile-v">
        {value}
        {unit && <span className="cyc-tile-u">{unit}</span>}
      </div>
    </div>
  );
}

function PointRow({ p, lang }: { p: LearnToposPoint; lang: Lang }) {
  return (
    <div className={`learn-row${p.n >= 30 ? " conf" : ""}`}>
      <span className="mono">{p.topos}</span>
      <span style={{ color: p.is_crane ? "#f59e0b" : "#0ea5e9" }}>{p.is_crane ? (ko(lang) ? "크레인" : "crane") : (ko(lang) ? "블록" : "block")}</span>
      <span className="mono">{p.n}</span>
      <span className="mono">{p.obs.toLocaleString()}</span>
      <span className="mono">{mPrec(p.spread_m)}</span>
      <span className="mono" style={{ fontSize: 11 }}>{p.lat.toFixed(5)}, {p.lon.toFixed(5)}</span>
      <span className="mono" style={{ fontSize: 11, color: "var(--text-mute)" }}>{stamp(p.updated_at)}</span>
    </div>
  );
}

export default function LearnPage({ lang }: { lang: Lang }) {
  const [d, setD] = useState<LearnTopos | null>(null);
  const [err, setErr] = useState(false);
  const [onlyBlock, setOnlyBlock] = useState(true);
  const [q, setQ] = useState("");

  useEffect(() => {
    let alive = true;
    const load = () => api.learnTopos().then((r) => { if (alive) { setD(r); setErr(false); } }).catch(() => alive && setErr(true));
    load();
    const id = setInterval(load, 30000);
    return () => { alive = false; clearInterval(id); };
  }, []);

  const ms = d?.metric_series ?? [];
  const covVals = ms.map((p) => p.confident_topos);
  const spreadVals = ms.map((p) => p.median_spread_m ?? 0).filter((v) => v > 0);
  const obsVals = ms.map((p) => Number(p.total_obs));

  const points = useMemo(() => {
    let pts = d?.points ?? [];
    if (onlyBlock) pts = pts.filter((p) => !p.is_crane);
    if (q) pts = pts.filter((p) => p.topos.toLowerCase().includes(q.toLowerCase()));
    return pts;
  }, [d, onlyBlock, q]);

  return (
    <div className="content cyc-page">
      <div className="cyc-head">
        <div className="cyc-title">
          <h2>{ko(lang) ? "학습 센터" : "Learning Center"}</h2>
          <span className="cyc-title-sub">
            {ko(lang) ? "② 블록 작업지점 좌표 — GPS 도착 관측을 누적해 좌표를 학습 (모델이 쌓일수록 정밀해짐)" : "Block work-point coordinates — learned from GPS arrivals (sharpens as data accumulates)"}
          </span>
        </div>
      </div>

      <div className="cyc-tiles">
        <Tile label={ko(lang) ? "누적 관측" : "Observations"} value={d ? fmtN(d.total_obs) : "—"} accent="#60a5fa" />
        <Tile label={ko(lang) ? "학습된 작업지점" : "Learned points"} value={d ? fmtN(d.distinct_topos) : "—"} accent="#0ea5e9" />
        <Tile label={ko(lang) ? "확신 (n≥30)" : "Confident (n≥30)"} value={d ? fmtN(d.confident_topos) : "—"} accent="#34d399" />
        <Tile label={ko(lang) ? "블록 작업지점" : "Block points"} value={d ? fmtN(d.block_points) : "—"} accent="#a78bfa" />
        <Tile label={ko(lang) ? "중앙 정밀도" : "Median precision"} value={d ? mPrec(d.median_spread_m) : "—"} accent="#f59e0b" />
      </div>

      <div className="learn-charts">
        <div className="cyc-tp">
          <div className="cyc-sec-h">
            {ko(lang) ? "모델 개선 — 확신 지점 수 (↑ 커버리지)" : "Model improving — confident points (↑)"}
            {err && <span className="cyc-err">{ko(lang) ? " · 연결 오류" : " · offline"}</span>}
          </div>
          <div className="cyc-tp-box">{covVals.length > 1 ? <LineChart values={covVals} color="#34d399" axes /> : <div className="cyc-empty">{ko(lang) ? "품질 스냅샷 수집 중 (시간당 1회)" : "collecting quality snapshots (hourly)"}</div>}</div>
        </div>
        <div className="cyc-tp">
          <div className="cyc-sec-h">{ko(lang) ? "모델 개선 — 중앙 정밀도 (↓ 산포 m)" : "Model improving — median precision (↓ m)"}</div>
          <div className="cyc-tp-box">{spreadVals.length > 1 ? <LineChart values={spreadVals} color="#f59e0b" axes /> : <div className="cyc-empty">{ko(lang) ? "수집 중" : "collecting"}</div>}</div>
        </div>
      </div>

      <div className="cyc-tp">
        <div className="cyc-sec-h">{ko(lang) ? "누적 학습데이터 — 관측 수 추이" : "Accumulating training data — observations over time"}</div>
        <div className="cyc-tp-box">{obsVals.length > 1 ? <LineChart values={obsVals} color="#60a5fa" axes /> : <div className="cyc-empty">{ko(lang) ? "수집 중" : "collecting"}</div>}</div>
      </div>

      <div className="cyc-board" style={{ marginTop: 14 }}>
        <div className="cyc-board-head">
          <span>{ko(lang) ? "학습된 작업지점 (관측 많은 순)" : "Learned work-points (by observations)"}</span>
          <span style={{ display: "flex", gap: 10, alignItems: "center" }}>
            <label style={{ fontSize: 11, color: "var(--text-dim)", cursor: "pointer" }}>
              <input type="checkbox" checked={onlyBlock} onChange={(e) => setOnlyBlock(e.target.checked)} /> {ko(lang) ? "블록만" : "blocks only"}
            </label>
            <input className="cyc-search mono" placeholder={ko(lang) ? "topos 검색" : "find topos"} value={q} onChange={(e) => setQ(e.target.value)} />
          </span>
        </div>
        <div className="learn-cols">
          <span>topos</span><span>{ko(lang) ? "종류" : "kind"}</span><span>n</span><span>{ko(lang) ? "누적" : "obs"}</span><span>{ko(lang) ? "정밀도" : "prec"}</span><span>{ko(lang) ? "좌표" : "coord"}</span><span>{ko(lang) ? "갱신" : "updated"}</span>
        </div>
        <div className="learn-list">
          {points.length === 0 && <div className="cyc-empty">{ko(lang) ? "아직 학습된 지점 없음 (적재 중)" : "none yet (accumulating)"}</div>}
          {points.slice(0, 300).map((p) => <PointRow key={p.topos} p={p} lang={lang} />)}
        </div>
      </div>
    </div>
  );
}
