// Typed client for the Rust axum API. Shapes mirror crates/api/src/models.rs.

export interface KpiCard {
  key: string;
  name_en: string;
  name_ko: string;
  unit: string;
  tier: string | null;
  direction: "LOWER_BETTER" | "HIGHER_BETTER" | null;
  value: number | null;
  sample_n: number | null;
  is_provisional: boolean;
  as_of: string;
  baseline: number | null;
  baseline_n_days: number | null;
  delta_abs: number | null;
  delta_pct: number | null;
  p_value: number | null;
  cohens_d: number | null;
  is_significant: boolean | null;
  target: number | null;
  excellent: number | null;
  meets_target: boolean | null;
  meets_excellent: boolean | null;
}
export interface KpisResponse {
  as_of: string;
  period: string;
  range_from: string;
  range_to: string;
  prev_from: string;
  prev_to: string;
  kpis: KpiCard[];
}

export interface TrendPoint { date: string; value: number; sample_n: number | null; }
export interface TrendResponse { key: string; unit: string; target: number | null; baseline: number | null; points: TrendPoint[]; }

// KPI history matrix (by day / week / month)
export interface HistoryCell { value: number | null; sample_n: number | null; }
export interface HistoryColumn { key: string; name_en: string; name_ko: string; unit: string; direction: string | null; }
export interface HistoryBucket { bucket: string; label_from: string; label_to: string; is_provisional: boolean; cells: Record<string, HistoryCell>; }
export interface HistoryResponse { gran: string; kpis: HistoryColumn[]; buckets: HistoryBucket[]; }

export interface QcRow { qc: string; mph: number | null; qc_wait_sec: number | null; status: string | null; }
export interface BreakdownResponse { as_of: string; rows: QcRow[]; }

export interface FreshnessRow { source: string; last_status: string | null; last_success_date: string | null; is_stale: boolean; }
export interface HealthResponse { overall: string; postgres: string; sources: FreshnessRow[]; }

export interface LiveKpi {
  key: string; name_en: string; name_ko: string; unit: string;
  tier: string | null; direction: "LOWER_BETTER" | "HIGHER_BETTER" | null;
  value: number | null; sample_n: number | null;
  prev_value: number | null; delta_abs: number | null; delta_pct: number | null;
  target: number | null; excellent: number | null; meets_target: boolean | null;
}
export interface LiveResponse {
  business_date: string; shift: string; shift_name_ko: string; shift_name_en: string;
  window_start: string; as_of: string; elapsed_min: number; remaining_min: number;
  prev_shift: string; kpis: LiveKpi[];
}
export interface VesselQc {
  qc: string; moves: number | null; load_moves: number | null; discharge_moves: number | null; mph: number | null;
}
export interface VesselRow {
  vessel: string; voyage: string; qcs: string[]; qc_count: number | null;
  moves: number | null; load_moves: number | null; discharge_moves: number | null;
  mph: number | null; first_move: string | null; last_move: string | null;
  planned_moves: number | null; progress_pct: number | null;
  qc_rows: VesselQc[];
}
export interface VesselsResponse { shift: string; as_of: string; vessels: VesselRow[]; }

// Live work pool (per-QC sequence + active move front), from the 90s extractor snapshot.
export interface WpMove {
  qc: string | null; queuename: string; vessel: string; jobtype: string | null;
  yt_status: string | null; ytno: string | null; armgc: string | null;
  etw_ts: string | null; contno: string | null; yt_topos: string | null;
  from_pos: string | null; to_pos: string | null; twintandem: string | null;
}
export interface WpQueue {
  queuename: string; vessel: string; voyage: string | null; disload: string | null;
  seq: number | null; total: number; done: number; remaining: number;
}
export interface WpQc {
  qc: string; vessels: string[]; active_moves: number; remaining: number;
  queues: WpQueue[]; moves: WpMove[];
}
export interface WpCandidate {
  qc: string | null; queuename: string; vessel: string; jobtype: string | null;
  src_block: string | null; rtg: string | null; n: number;
  moves_until: number; active: boolean;
}
export interface WorkpoolResponse {
  as_of: string | null; qc_count: number; active_moves: number; total_remaining: number;
  qcs: WpQc[]; pool: WpMove[];
  candidates: WpCandidate[]; candidate_total: number;
}

async function get<T>(path: string): Promise<T> {
  const r = await fetch(path);
  if (!r.ok) throw new Error(`${path}: ${r.status}`);
  return r.json() as Promise<T>;
}

export const api = {
  kpis: (period: string) => get<KpisResponse>(`/api/kpis?period=${encodeURIComponent(period)}`),
  trend: (key: string, opts?: { days?: number; from?: string; to?: string }) => {
    const qs = opts?.from && opts?.to ? `from=${opts.from}&to=${opts.to}` : `days=${opts?.days ?? 14}`;
    return get<TrendResponse>(`/api/kpis/${key}/trend?${qs}`);
  },
  breakdown: (period: string) => get<BreakdownResponse>(`/api/breakdown/qc?period=${encodeURIComponent(period)}`),
  kpiHistory: (gran: string, n?: number) => get<HistoryResponse>(`/api/kpis/history?gran=${gran}${n ? `&n=${n}` : ""}`),
  health: () => get<HealthResponse>("/api/health"),
  live: () => get<LiveResponse>("/api/live"),
  liveVessels: () => get<VesselsResponse>("/api/live/vessels"),
  workpool: () => get<WorkpoolResponse>("/api/workpool"),
};
