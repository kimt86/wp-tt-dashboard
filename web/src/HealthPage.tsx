// AI Dispatch Engine — Health Monitor. Visual mock ported from docs/mock-dashboard.html
// VIEW C (AI Dispatch Health): engine status + 5 stat cards + response-time distribution
// + AI-active-rate trend + recent decision log. Fake data; wire to engine metrics later.
import { type Lang } from "./i18n";

const ko = (l: Lang) => l === "ko";

const STATS = [
  { ko: "가용성 (24H)", en: "Availability (24H)", val: "99.94", unit: "%", note: "SLA ≥ 99.5%", tone: "good" },
  { ko: "배차 응답 p99", en: "Dispatch p99", val: "22", unit: "ms", note: "SLA ≤ 100ms", tone: "good" },
  { ko: "AI 활성 비율", en: "AI Active Rate", val: "99.4", unit: "%", noteKo: "폴백 2건 / 1,842", noteEn: "Fallback 2 / 1,842", tone: "good" },
  { ko: "제약 충족률", en: "Constraint Compliance", val: "100", unit: "%", noteKo: "불가 배차 0건", noteEn: "Invalid dispatch 0", tone: "good" },
  { ko: "매칭 일치율 (vs TOS)", en: "Match Rate (vs TOS)", val: "87.3", unit: "%", noteKo: "불일치 234건 검토", noteEn: "234 mismatches under review", tone: "warn" },
];

// latency histogram buckets (ms) + counts
const HIST = [["0–5", 22], ["5–10", 96], ["10–15", 128], ["15–20", 84], ["20–25", 38], ["25–30", 12], ["30+", 4]] as const;
// AI active-rate trend (24 hourly points, %) with fallback marks
const AIRATE = [99.6, 99.8, 100, 99.9, 100, 99.7, 99.5, 100, 99.9, 98.9, 99.6, 100, 99.8, 99.4, 100, 99.9, 99.3, 100, 99.7, 99.9, 100, 99.6, 99.8, 99.4];
const FALLBACKS = [9, 16];

type Dec = { time: string; resp: string; tt: string; job: string; qc: string; f1: string; f2: string; f3: string; total: string; alt: string; fb?: boolean };
const DECS: Dec[] = [
  { time: "14:32:04", resp: "17ms", tt: "TT-23", job: "DSC-K03-1145", qc: "QC1", f1: "2.1s", f2: "6.4s", f3: "4.4s", total: "12.9s", alt: "TT-07 (14.2s)" },
  { time: "14:32:01", resp: "14ms", tt: "TT-41", job: "LOD-L05-0823", qc: "QC2", f1: "3.4s", f2: "4.1s", f3: "5.2s", total: "12.7s", alt: "TT-08 (13.5s)" },
  { time: "14:31:48", resp: "22ms", tt: "TT-11", job: "DSC-K06-0418", qc: "QC4", f1: "1.8s", f2: "4.0s", f3: "3.9s", total: "9.7s", alt: "TT-28 (11.2s)" },
  { time: "14:31:32", resp: "11ms", tt: "TT-19", job: "SHF-M04-0291", qc: "YC8", f1: "0.5s", f2: "—", f3: "2.8s", total: "3.3s", alt: "TT-37 (4.1s)" },
  { time: "14:31:18", resp: "26ms", tt: "TT-04", job: "LOD-L03-1027", qc: "QC2", f1: "4.2s", f2: "5.8s", f3: "6.1s", total: "16.1s", alt: "TT-19 (16.4s)" },
  { time: "14:31:02", resp: "19ms", tt: "TT-31", job: "DSC-K05-0712", qc: "QC4", f1: "2.7s", f2: "7.2s", f3: "5.5s", total: "15.4s", alt: "TT-14 (16.8s)" },
  { time: "14:30:48", resp: "—", tt: "TT-37", job: "SHF-L02-0184", qc: "YC3", f1: "—", f2: "—", f3: "—", total: "—", alt: "—", fb: true },
  { time: "14:30:34", resp: "16ms", tt: "TT-08", job: "LOD-L05-0822", qc: "QC2", f1: "1.4s", f2: "3.7s", f3: "4.8s", total: "9.9s", alt: "TT-41 (11.5s)" },
  { time: "14:30:21", resp: "14ms", tt: "TT-12", job: "DSC-K03-1144", qc: "QC1", f1: "2.0s", f2: "5.5s", f3: "4.1s", total: "11.6s", alt: "TT-23 (12.9s)" },
  { time: "14:30:08", resp: "20ms", tt: "TT-28", job: "DSC-K06-0417", qc: "QC4", f1: "3.1s", f2: "4.6s", f3: "5.7s", total: "13.4s", alt: "TT-11 (13.9s)" },
];

function Hist() {
  const max = Math.max(...HIST.map((h) => h[1]));
  return (
    <div className="hp-hist">
      {HIST.map(([label, n]) => (
        <div className="hp-hist-col" key={label}>
          <div className="hp-hist-bar"><div className="hp-hist-fill" style={{ height: `${(n / max) * 100}%` }} /></div>
          <div className="hp-hist-lbl mono">{label}</div>
        </div>
      ))}
    </div>
  );
}

function Trend() {
  const w = 100, h = 100, lo = 98.5, hi = 100.2;
  const y = (v: number) => h - ((v - lo) / (hi - lo)) * (h - 8) - 4;
  const pts = AIRATE.map((v, i) => `${(i / (AIRATE.length - 1)) * w},${y(v)}`).join(" ");
  return (
    <svg className="hp-trend" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
      <polyline points={pts} fill="none" stroke="#22c55e" strokeWidth="1.4" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
      {FALLBACKS.map((i) => <circle key={i} cx={(i / (AIRATE.length - 1)) * w} cy={y(AIRATE[i])} r="1.6" fill="#fcd34d" vectorEffect="non-scaling-stroke" />)}
    </svg>
  );
}

export default function HealthPage({ lang }: { lang: Lang }) {
  const t = (k: string, e: string) => (ko(lang) ? k : e);
  return (
    <div className="content hp-root">
      {/* engine header */}
      <div className="hp-eng">
        <div>
          <div className="hp-eng-title">{t("AI 배차 엔진 — 헬스 모니터", "AI Dispatch Engine — Health Monitor")}</div>
          <div className="hp-eng-sub">{t("엔진", "Engine")} v0.4.1-poc · Uptime 14d 03:42:18 · {t("마지막 재시작", "Last restart")} 2026-04-30 11:14</div>
        </div>
        <div className="hp-eng-pills">
          <span className="pill good">{t("엔진 정상", "Engine OK")}</span>
          <span className="pill good">{t("SLA 충족", "SLA OK")}</span>
        </div>
      </div>

      {/* 5 stat cards */}
      <div className="grid hp-stats">
        {STATS.map((s) => (
          <div className="stat-card" key={s.en}>
            <div className="label">{ko(lang) ? s.ko : s.en}</div>
            <div className="val">{s.val}<span className="unit">{s.unit}</span></div>
            <div className={`delta ${s.tone}`}>{s.noteKo ? (ko(lang) ? s.noteKo : s.noteEn) : s.note}</div>
          </div>
        ))}
      </div>

      {/* latency dist + AI ratio trend */}
      <div className="grid hp-2col">
        <section className="tcard">
          <div className="tcard-head"><h3>{t("배차 응답 시간 분포", "Dispatch Response Time Dist.")}<span className="h3-sub">{t("최근 1시간", "last 1H")}</span></h3><div className="head-sub"><span className="muted">N = 384</span></div></div>
          <div className="tcard-body">
            <Hist />
            <div className="hp-pcts">
              {[["p50", "9ms", ""], ["p95", "18ms", ""], ["p99", "22ms", "good"], [t("최대", "Max"), "31ms", ""]].map(([k, v, c], i) => (
                <div key={i}><div className="hp-pct-k">{k}</div><div className={`hp-pct-v mono ${c}`}>{v}</div></div>
              ))}
            </div>
          </div>
        </section>
        <section className="tcard">
          <div className="tcard-head"><h3>{t("AI 활성 비율 추이", "AI Active Rate Trend")}<span className="h3-sub">24H</span></h3><div className="head-sub"><span className="muted">{t("폴백 발생 시점 표시", "fallback events marked")}</span></div></div>
          <div className="tcard-body">
            <Trend />
            <div className="hp-pcts">
              <div><div className="hp-pct-k">{t("평균", "Avg")}</div><div className="hp-pct-v mono good">99.4%</div></div>
              <div><div className="hp-pct-k">{t("폴백", "Fallback")}</div><div className="hp-pct-v mono warn">{t("2건", "2")}</div></div>
              <div><div className="hp-pct-k">{t("제약 위반", "Violations")}</div><div className="hp-pct-v mono good">0</div></div>
            </div>
          </div>
        </section>
      </div>

      {/* recent decision log */}
      <section className="tcard">
        <div className="tcard-head"><h3>{t("최근 배차 결정 로그", "Recent Dispatch Decisions")}<span className="h3-sub">{t("10건", "10")}</span></h3><div className="head-sub"><span className="muted">{t("스트림 실시간", "live stream")}</span></div></div>
        <div className="tcard-body" style={{ padding: 0 }}>
          <table className="dec-table">
            <thead>
              <tr>
                <th>{t("시각", "Time")}</th><th>{t("응답", "Resp")}</th><th>{t("차량", "Vehicle")}</th><th>{t("작업", "Job")}</th><th>QC</th>
                <th>F1 {t("차량대기", "Truck")}</th><th>F2 {t("크레인대기", "Crane")}</th><th>F3 {t("공차시간", "Empty")}</th>
                <th>{t("총비용", "Total")}</th><th>{t("차선후보", "Alt")}</th><th>{t("결과", "Result")}</th>
              </tr>
            </thead>
            <tbody>
              {DECS.map((d, i) => (
                <tr key={i}>
                  <td>{d.time}</td><td>{d.resp}</td><td>{d.tt}</td><td>{d.job}</td><td>{d.qc}</td>
                  <td>{d.f1}</td><td>{d.f2}</td><td>{d.f3}</td><td>{d.total}</td><td>{d.alt}</td>
                  <td>{d.fb ? <span className="fb">{t("폴백 (제약)", "Fallback (constraint)")}</span> : <span className="ok">{t("선택 ✓", "Selected ✓")}</span>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
