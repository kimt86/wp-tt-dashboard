//! Live-map GPS ingest. Connects OUT to the WP-TT GPS websocket — reachable here
//! only through the local SSH tunnel `127.0.0.1:9986` -> azure-wp-poc -> 172.21.30.72:9986
//! (the source is a WSL2 NAT IP unreachable directly). Performs the `wpt_gps` zone
//! handshake (identify -> 2s -> checkin), then keeps the latest fix per device in an
//! in-memory map plus ingest health counters.
//!
//! - `GET /api/livemap/positions` — snapshot the LiveMap polls (active devices).
//! - `GET /api/livemap/health`    — ingest/feed health (connection, freshness, rate).
//!
//! NOTE: this is the ONE outbound network client in the API crate, and it talks ONLY to
//! the local tunnel endpoint — no Oracle/SSH access, cannot reach the production DB.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Json;
use sqlx::PgPool;
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message;

/// Devices fresher than this are "active" and served to the map.
const STALE_AFTER_S: i64 = 120;
/// Fixes older than this are dropped entirely (a device that left the yard).
const LOST_AFTER_S: i64 = 600;
/// Freshness band: a fix newer than this is "fresh".
const FRESH_UNDER_S: i64 = 15;
const SPARK_MIN: usize = 30; // minutes of throughput history

#[derive(Clone, Default)]
struct Pos {
    cls: String, // device-id alpha prefix: TT / RTG / C / TC / ...
    lat: f64,
    lon: f64,
    speed: f64,  // km/h
    engine: i32, // 1 = engine_on contains "ON", else 0
    last_seen_ms: i64,
    // rich fields straight off the gps_update (mostly populated for TT prime movers)
    jobtype: Option<String>,
    vslname: Option<String>,
    container1: Option<String>,
    container2: Option<String>,
    cur_loc: Option<String>,
    topos1: Option<String>,
    arrival: Option<String>, // "ARRIVED" when the TT has reached its handover point
    fuel: Option<f64>,
    accuracy: Option<f64>,
    userid: Option<String>,
    batt: Option<String>,
    nett: Option<String>,
    dtime: Option<String>,
    distance: Option<f64>,
    // TT cycle tracking (carried across fixes). A delivery completes when `container1`
    // changes away from a non-empty value — i.e. nonempty→empty OR nonempty→other (a
    // truck often goes container A→container B with no empty gap, so a loaded→EMPTY-only
    // edge misses ~3/4 of deliveries; observed ~574/hr vs ~90/hr for the empty-only edge).
    // The interval between a truck's consecutive deliveries, capped, is one cycle. The
    // values are exposed via /api/livemap/positions (see the KC websocket-kpi-accuracy doc).
    carry_since_ms: i64, // when the current non-empty container1 began (0 if empty)
    last_drop_ms: i64,   // last counted delivery
    carry_trip_m: f64,   // path length the truck has driven since the current carry began
}

// TT cycle bounds. A loose physical sanity band only — the *real* artifact filter is GPS
// movement (MIN_CARRY_TRIP_M below), not duration. We keep [2,20]m so a single absurd
// interval (clock skew, a >20m idle gap that isn't one cycle) can't poison the median, but
// we do NOT use the lower bound to manufacture a "realistic" number — see the movement
// filter, which is what actually separates a real delivery from a TOS re-assignment.
const MIN_CYCLE_S: i64 = 120;
const MAX_CYCLE_S: i64 = 1200;
// a delivery only counts if the container was actually carried ≥30s (filters a flicker).
const MIN_LOADED_MS: i64 = 30_000;
// the principled artifact filter (per external eval): a *real* laden delivery means the
// truck physically drove the container from pickup to handover. If `container1` changes but
// the truck accumulated < this much path length while carrying it, the truck never moved —
// it is TOS pre-assigning / rewriting the container field while the truck sits, NOT a
// delivery. Validating on movement (not on a duration threshold) avoids circularly using a
// "cycles should be long" prior to inflate the median. ~150m clears GPS jitter and is below
// even the shortest real quay↔block haul.
const MIN_CARRY_TRIP_M: f64 = 150.0;
// a reject whose carried path falls in this near-miss band [100,150)m is tracked separately:
// it is exposed so a reviewer can see how many genuine ultra-short hauls the 150m cut might
// be discarding (the one direction this filter can bias the median upward).
const NEAR_TRIP_M: f64 = 100.0;
// guard: ignore a single inter-fix jump larger than this when accumulating path length
// (GPS teleport / accuracy spike), so jitter can't fake "movement".
const MAX_FIX_STEP_M: f64 = 600.0;
// the median needs a non-trivial sample before it is shown — a 5-sample median is noise.
// Below this the UI shows "collecting n/N" instead of a number (per external eval #4).
const MIN_CYCLE_SAMPLES: usize = 20;
// a working quay crane idle longer than this with no TT present ≈ likely waiting for a
// truck. Set past a normal inter-move gap (~90–120s) so routine gaps don't trip it.
const QCQ_IDLE_S: i64 = 120;

/// Crane PLC state from the `ctab` zone (`plc_data`). Dynamic equipment only
/// (C/M/Z prefixes). Keyed by crane id, which matches the GPS device id.
///
/// We also count *completed moves* from the hook-load signal: each laden→empty
/// transition (the crane set a container down and released) is one move. Counting
/// those over a rolling hour gives a live, per-second-fresh per-QC throughput
/// (move/hr) — a websocket cross-check that refines the coarse TOS K_MPH (whose
/// active_hours is bucketed to whole hours). See `kc/websocket-kpi-accuracy`.
#[derive(Clone, Default)]
struct Plc {
    load_t: Option<f64>, // hook load in metric tons
    lock: Option<bool>,
    land: Option<bool>,
    hpos: Option<f64>, // hoist position (crane-local axis)
    tpos: Option<f64>, // trolley position
    last_seen_ms: i64,
    laden: bool,             // current laden state (hysteresis-debounced)
    moves: VecDeque<i64>,    // pickup (rising-edge) timestamps, pruned to 1h
    last_move_ms: i64,
}

// Hook-load thresholds (tons). Empty hook reads ~0 / slightly negative; a loaded
// spreader reads several tons. Hysteresis (laden ≥2t / empty <0.5t) keeps the state
// from flapping. We count a move on the empty→laden RISING edge (a pickup), and a
// min gap of 40s between counted moves absorbs any mid-cycle flicker (one spreader
// cycle can't complete in <40s) while still counting every real move (cycles are
// ~60–120s apart). Rising-edge + gap is robust to a noisy load signal.
const PLC_LADEN_T: f64 = 2.0;
const PLC_EMPTY_T: f64 = 0.5;
const MIN_MOVE_GAP_MS: i64 = 40_000;
const MOVE_WINDOW_MS: i64 = 3_600_000; // 1 hour → move count == move/hr

/// Learned position of a yard block/bay code, accumulated from the GPS of TTs observed
/// ARRIVED there. Lets us estimate "how far is an empty TT from its assigned pickup" with
/// no TOS/layout dependency (standalone).
#[derive(Clone, Copy, Default)]
pub struct Centroid {
    lat: f64,
    lon: f64,
    n: u32,
}
impl Centroid {
    fn push(&mut self, lat: f64, lon: f64) {
        self.n = (self.n + 1).min(500); // cap so it stays mildly adaptive
        let k = 1.0 / self.n as f64;
        self.lat += (lat - self.lat) * k;
        self.lon += (lon - self.lon) * k;
    }
}

/// Per-minute throughput ring for the health sparkline.
struct Ring {
    minute: i64,
    buf: [u32; SPARK_MIN],
    idx: usize,
}
impl Ring {
    fn new() -> Self {
        Self { minute: 0, buf: [0; SPARK_MIN], idx: 0 }
    }
    fn advance(&mut self, m: i64) {
        if self.minute == 0 {
            self.minute = m;
            return;
        }
        while self.minute < m {
            self.idx = (self.idx + 1) % SPARK_MIN;
            self.buf[self.idx] = 0;
            self.minute += 1;
        }
    }
    fn bump(&mut self, m: i64) {
        self.advance(m);
        self.buf[self.idx] += 1;
    }
    fn series(&self) -> Vec<u32> {
        (1..=SPARK_MIN).map(|k| self.buf[(self.idx + k) % SPARK_MIN]).collect()
    }
    /// Display rate: the larger of the current (still-filling) and previous minute, so a
    /// busy feed reads a sane number immediately instead of 0 at each minute boundary.
    fn rate(&self) -> u32 {
        let prev = self.buf[(self.idx + SPARK_MIN - 1) % SPARK_MIN];
        self.buf[self.idx].max(prev)
    }
}

/// Shared ingest state.
pub struct LiveMap {
    devices: RwLock<HashMap<String, Pos>>,
    plc: RwLock<HashMap<String, Plc>>, // crane PLC state from the ctab zone
    centroids: RwLock<HashMap<String, Centroid>>, // learned block/bay positions (topos1 → centroid)
    ring: Mutex<Ring>,
    connected: AtomicBool,
    messages: AtomicU64,
    reconnects: AtomicU64,
    last_msg_ms: AtomicU64,
    connected_since_ms: AtomicU64,
    started_ms: AtomicU64,
    last_error: RwLock<Option<String>>,
    plc_connected: AtomicBool,
    plc_messages: AtomicU64,
    // TT cycle: fleet drop timestamps (for throughput λ) + accepted cycle-interval
    // samples (drop_ms, interval_s) for the median. Both pruned to the 1h window.
    tt_drops: Mutex<VecDeque<i64>>,
    tt_cycles: Mutex<VecDeque<(i64, i64)>>,
    // container1 changes rejected by the movement filter (truck didn't move while carrying)
    // — i.e. suspected TOS re-assignment artifacts. Kept for auditability: the artifact:real
    // ratio is exposed so the filter's effect is visible rather than hidden.
    tt_artifacts: Mutex<VecDeque<i64>>,
    // subset of the above whose carried path was in the near-miss band [100,150)m — possibly
    // genuine ultra-short hauls the cut discards. Exposed so the upward-bias is measurable.
    tt_artifacts_near: Mutex<VecDeque<i64>>,
}

impl LiveMap {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: RwLock::new(HashMap::new()),
            plc: RwLock::new(HashMap::new()),
            centroids: RwLock::new(HashMap::new()),
            ring: Mutex::new(Ring::new()),
            tt_drops: Mutex::new(VecDeque::new()),
            tt_cycles: Mutex::new(VecDeque::new()),
            tt_artifacts: Mutex::new(VecDeque::new()),
            tt_artifacts_near: Mutex::new(VecDeque::new()),
            connected: AtomicBool::new(false),
            messages: AtomicU64::new(0),
            reconnects: AtomicU64::new(0),
            last_msg_ms: AtomicU64::new(0),
            connected_since_ms: AtomicU64::new(0),
            started_ms: AtomicU64::new(Utc::now().timestamp_millis() as u64),
            last_error: RwLock::new(None),
            plc_connected: AtomicBool::new(false),
            plc_messages: AtomicU64::new(0),
        })
    }
}

// ───────────────────────── positions endpoint ─────────────────────────

#[derive(Serialize)]
struct DeviceOut {
    id: String,
    cls: String,
    lat: f64,
    lon: f64,
    speed: f64,
    engine: i32,
    age_s: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    jobtype: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vslname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    container2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cur_loc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topos1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arrival: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fuel: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    accuracy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    userid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nett: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dtime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    distance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    plc: Option<PlcOut>,
    // dispatch-state classification (TT only): idle|empty_travel|delivering|soon_idle|wait_rtg
    #[serde(skip_serializing_if = "Option::is_none")]
    dispatch: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dispatch_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nearest_rtg_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dest_remaining_m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    swappable: Option<bool>,
}

/// Crane PLC state served alongside a crane's GPS fix.
#[derive(Serialize)]
struct PlcOut {
    is_loaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    load_t: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lock: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    land: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hpos: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tpos: Option<f64>,
    age_s: i64,
    /// completed moves in the last hour (= live move/hr) counted from the PLC
    mph: u32,
    /// seconds since this crane's last completed move (None if never seen)
    #[serde(skip_serializing_if = "Option::is_none")]
    last_move_age_s: Option<i64>,
}

#[derive(Serialize)]
pub struct PositionsOut {
    source: &'static str,
    connected: bool,
    as_of: Option<DateTime<Utc>>,
    count: usize,
    messages: u64,
    /// TT dispatch-state counts (idle/empty_travel/delivering/soon_idle/wait_rtg)
    dispatch_counts: HashMap<&'static str, usize>,
    /// websocket-derived live K_MPH cross-check: total crane moves in the last hour,
    /// the number of cranes that worked in that window, and their average move/hr.
    crane_moves_60m: u32,
    cranes_working: usize,
    crane_mph_live: Option<f64>,
    /// live TT cycle time (s) — replaces the mislabeled TOS span. Little's-law fleet
    /// average + median of per-truck drop intervals, with sample/throughput counts.
    tt_cycle_littles_s: Option<i64>,
    tt_cycle_median_s: Option<i64>,
    /// spread of the cycle samples (25th/75th pctile, s) so a thin/noisy median is visible
    tt_cycle_p25_s: Option<i64>,
    tt_cycle_p75_s: Option<i64>,
    tt_drops_60m: u32,
    tt_cycle_samples: u32,
    /// min samples before the median is shown (UI shows "collecting n/N" below this)
    tt_cycle_min_samples: u32,
    /// container1 changes rejected by the movement filter in the last hour (suspected TOS
    /// re-assignment artifacts). Exposed for audit: artifacts vs real deliveries.
    tt_artifacts_60m: u32,
    /// subset of rejects in the [100,150)m near-miss band — possible ultra-short hauls the
    /// 150m cut discards (measures the one direction this filter can bias the median up).
    tt_artifacts_near_60m: u32,
    /// how full the rolling 1h window is, in minutes (0..=60). <60 ⇒ still settling.
    window_fill_min: u32,
    active_trucks: usize,
    /// live K_UTIL (%) — TRUE utilization: of manned trucks, the fraction with an active job
    /// assignment (allocated→completed, even while stopped). Idle = manned but unassigned.
    tt_util_live: Option<i64>,
    /// secondary (%) — of manned trucks, the fraction physically moving/carrying right now
    /// (the remainder of the assigned ones are queued/waiting within their job).
    tt_engaged_live: Option<i64>,
    /// live K_QC_Q — quay cranes currently starving (idle, no truck) + their avg wait (s)
    qc_starving: usize,
    qc_wait_live_s: Option<i64>,
    devices: Vec<DeviceOut>,
}

const RTG_BAY_M: f64 = 30.0; // RTG within this of a TT ≈ same bay (engaged)
const IDLE_SPEED_KMH: f64 = 3.0;
const SWAP_MIN_M: f64 = 150.0; // an empty TT closer than this to its pickup isn't worth swapping

/// A topos1 like "C46"/"M4"/"Z6" = a quay/dynamic crane (vs a block code like "03U-21").
fn is_crane_code(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 2 && matches!(b[0], b'C' | b'M' | b'Z') && b[1..].iter().all(u8::is_ascii_digit)
}

/// Block/area prefix of a yard code: "07F-06" → "07F", "WHARF_23_B" → "WHARF_23_B".
fn block_prefix(s: &str) -> &str {
    s.split('-').next().unwrap_or(s)
}

/// Approximate ground distance (m) between two lat/lon points (equirectangular).
fn dist_m(a: (f64, f64), b: (f64, f64)) -> f64 {
    let lat = (a.0 + b.0) / 2.0 * std::f64::consts::PI / 180.0;
    let dx = (a.1 - b.1) * 111_320.0 * lat.cos();
    let dy = (a.0 - b.0) * 111_320.0;
    (dx * dx + dy * dy).sqrt()
}

#[derive(Default)]
struct Classed {
    state: &'static str,
    reason: Option<String>,
    nearest_rtg_m: Option<f64>,
    dest_remaining_m: Option<f64>, // empty_travel: distance to its assigned pickup (topos1)
    swappable: Option<bool>,       // empty_travel: still far enough from pickup to re-match
}

/// Classify a TT's dispatch state from its mission fields + RTG proximity + QC PLC, and for
/// empty-travelling TTs assess swap-worthiness (remaining distance to the assigned pickup —
/// crane destinations use live crane GPS, block destinations use learned centroids).
///  - idle: empty + ~stationary (available now)
///  - empty_travel: empty + moving toward a pickup (swap candidate if still far enough)
///  - delivering: loaded + en route
///  - soon_idle: loaded + ARRIVED at final handover AND crane engaged (QC=PLC / block=RTG near)
///  - wait_rtg: loaded + ARRIVED at block but no RTG near yet (arrived ≠ soon-idle)
fn classify_tt(
    p: &Pos,
    rtgs: &[(f64, f64)],
    plc: &HashMap<String, Plc>,
    cranes: &HashMap<String, (f64, f64)>,
    centroids: &HashMap<String, Centroid>,
    now: i64,
) -> Classed {
    let st = |state, reason: Option<String>| Classed { state, reason, ..Default::default() };
    let loaded = p.container1.as_deref().is_some_and(|s| !s.is_empty());
    if !loaded {
        if p.speed < IDLE_SPEED_KMH {
            return st("idle", None);
        }
        // empty + moving = empty_travel. Swap-worthiness = remaining distance to its pickup.
        let topos = p.topos1.as_deref().unwrap_or("");
        if topos.is_empty() {
            return Classed { state: "empty_travel", reason: Some("공차 주행 중 · 회송/대기".into()), swappable: Some(false), ..Default::default() };
        }
        let destpos = if is_crane_code(topos) {
            // live crane GPS, else its learned position (crane may not be broadcasting)
            cranes.get(topos).copied().or_else(|| centroids.get(topos).map(|c| (c.lat, c.lon)))
        } else {
            centroids.get(topos).or_else(|| centroids.get(block_prefix(topos))).map(|c| (c.lat, c.lon))
        };
        let remaining = destpos.map(|dp| dist_m((p.lat, p.lon), dp));
        let rem_r = remaining.map(|r| (r * 10.0).round() / 10.0);
        let swappable = remaining.is_none_or(|r| r >= SWAP_MIN_M);
        let reason = match remaining {
            Some(r) if r < SWAP_MIN_M => format!("공차 주행 중 · 목적지 근접 {r:.0}m (스왑 부적합)"),
            Some(r) => format!("공차 주행 중 · 잔여 {r:.0}m"),
            None => "공차 주행 중 · 목적지 미학습".into(),
        };
        return Classed { state: "empty_travel", reason: Some(reason), dest_remaining_m: rem_r, swappable: Some(swappable), ..Default::default() };
    }
    if p.arrival.as_deref() != Some("ARRIVED") {
        return st("delivering", Some("적재 이동 중".into()));
    }
    let topos = p.topos1.as_deref().unwrap_or("");
    let is_crane = is_crane_code(topos);
    // Which side UNLOADS this job (= frees the TT)? LD unloads at the quay crane; DS/MO/MI at a
    // block. A loaded TT ARRIVED at the *other* side just picked up → still delivering.
    let drop_at_crane = match p.jobtype.as_deref().unwrap_or("") {
        "LD" => true,
        "DS" | "MO" | "MI" => false,
        _ => is_crane,
    };
    if drop_at_crane {
        if !is_crane {
            return st("delivering", Some("적재 이동 (안벽行)".into()));
        }
        let plc_ok = plc.get(topos).is_some_and(|c| (now - c.last_seen_ms) / 1000 <= STALE_AFTER_S);
        let reason = if plc_ok { format!("안벽 {topos} 핸드오버 · PLC 확인") } else { format!("안벽 {topos} 핸드오버") };
        return st("soon_idle", Some(reason));
    }
    if is_crane {
        return st("delivering", Some("적재 이동 (블록行)".into()));
    }
    // block handover — needs an RTG engaged (≈ same bay).
    let nearest = rtgs.iter().map(|r| dist_m((p.lat, p.lon), *r)).fold(f64::INFINITY, f64::min);
    if !nearest.is_finite() {
        return st("wait_rtg", Some("도착 · RTG 미관측".into()));
    }
    let d = (nearest * 10.0).round() / 10.0;
    if d <= RTG_BAY_M {
        Classed { state: "soon_idle", reason: Some(format!("블록 RTG 근접 {d:.0}m")), nearest_rtg_m: Some(d), ..Default::default() }
    } else {
        Classed { state: "wait_rtg", reason: Some(format!("도착 · RTG 대기 (최근접 {d:.0}m)")), nearest_rtg_m: Some(d), ..Default::default() }
    }
}

/// `GET /api/livemap/positions` — active device fixes (age ≤ 120s).
pub async fn positions(State(lm): State<Arc<LiveMap>>, State(pool): State<PgPool>) -> Json<PositionsOut> {
    let now = Utc::now().timestamp_millis();
    // observation window for the live move/hr rate: capped at 1h, but right after a
    // restart we've collected less, so divide the move count by the actual elapsed
    // hours (min 1 min) instead of a full hour — otherwise the rate reads far too low
    // until the 1h ring fills.
    let started = lm.started_ms.load(Ordering::Relaxed) as i64;
    let obs_h = (((now - started) as f64) / 3_600_000.0).clamp(0.1, 1.0);
    let rate = |moves: usize| ((moves as f64 / obs_h).round()) as u32;
    let map = lm.devices.read().await;
    let plc = lm.plc.read().await;
    let centroids = lm.centroids.read().await;
    // fresh RTG positions for the discharge same-bay proximity check
    let rtgs: Vec<(f64, f64)> = map
        .values()
        .filter(|p| p.cls == "RTG" && (now - p.last_seen_ms) / 1000 <= STALE_AFTER_S)
        .map(|p| (p.lat, p.lon))
        .collect();
    // fresh crane (C/M/Z) positions — destination for empty TTs heading to pick up at a quay
    let cranes: HashMap<String, (f64, f64)> = map
        .iter()
        .filter(|(_, p)| matches!(p.cls.as_str(), "C" | "M" | "Z") && (now - p.last_seen_ms) / 1000 <= STALE_AFTER_S)
        .map(|(id, p)| (id.clone(), (p.lat, p.lon)))
        .collect();
    let mut devices: Vec<DeviceOut> = map
        .iter()
        .filter_map(|(id, p)| {
            let age = (now - p.last_seen_ms) / 1000;
            if age > STALE_AFTER_S {
                return None;
            }
            let c = (p.cls == "TT").then(|| classify_tt(p, &rtgs, &plc, &cranes, &centroids, now));
            let dispatch = c.as_ref().map(|c| c.state);
            let dispatch_reason = c.as_ref().and_then(|c| c.reason.clone());
            let nearest_rtg_m = c.as_ref().and_then(|c| c.nearest_rtg_m);
            let dest_remaining_m = c.as_ref().and_then(|c| c.dest_remaining_m);
            let swappable = c.as_ref().and_then(|c| c.swappable);
            // attach crane PLC state (ctab zone) when fresh — id matches the crane id.
            let plc_out = plc.get(id).and_then(|c| {
                let pa = (now - c.last_seen_ms) / 1000;
                (pa <= STALE_AFTER_S).then(|| PlcOut {
                    is_loaded: c.load_t.is_some_and(|t| t >= 1.0),
                    load_t: c.load_t,
                    lock: c.lock,
                    land: c.land,
                    hpos: c.hpos,
                    tpos: c.tpos,
                    age_s: pa.max(0),
                    mph: rate(c.moves.iter().filter(|&&tm| now - tm <= MOVE_WINDOW_MS).count()),
                    last_move_age_s: (c.last_move_ms != 0).then(|| (now - c.last_move_ms) / 1000),
                })
            });
            Some(DeviceOut {
                id: id.clone(),
                cls: p.cls.clone(),
                lat: p.lat,
                lon: p.lon,
                speed: p.speed,
                engine: p.engine,
                age_s: age.max(0),
                jobtype: p.jobtype.clone(),
                vslname: p.vslname.clone(),
                container1: p.container1.clone(),
                container2: p.container2.clone(),
                cur_loc: p.cur_loc.clone(),
                topos1: p.topos1.clone(),
                arrival: p.arrival.clone(),
                fuel: p.fuel,
                accuracy: p.accuracy,
                userid: p.userid.clone(),
                batt: p.batt.clone(),
                nett: p.nett.clone(),
                dtime: p.dtime.clone(),
                distance: p.distance,
                plc: plc_out,
                dispatch,
                dispatch_reason,
                nearest_rtg_m,
                dest_remaining_m,
                swappable,
            })
        })
        .collect();
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    let mut dispatch_counts: HashMap<&'static str, usize> = HashMap::new();
    for d in &devices {
        if let Some(s) = d.dispatch {
            *dispatch_counts.entry(s).or_default() += 1;
        }
    }
    // fleet live K_MPH cross-check: count moves/hr per crane, average over the cranes
    // that actually worked in the window (a crane idle all hour shouldn't drag the mean).
    let mut crane_moves_60m = 0u32;
    let mut cranes_working = 0usize;
    for c in plc.values() {
        let m = c.moves.iter().filter(|&&tm| now - tm <= MOVE_WINDOW_MS).count() as u32;
        if m > 0 {
            crane_moves_60m += m;
            cranes_working += 1;
        }
    }
    let crane_mph_live = (cranes_working > 0)
        .then(|| (crane_moves_60m as f64 / cranes_working as f64 / obs_h * 10.0).round() / 10.0);

    // ── live TT cycle time ── (replaces the mislabeled TOS "cycle"). Two estimates:
    //  • Little's law W = L/λ — robust fleet average: L = trucks in a cycle (non-idle),
    //    λ = fleet delivery (drop) rate. No per-truck edge timing → hard to skew.
    //  • median of per-truck drop-to-drop intervals (capped) — the typical cycle.
    let active_trucks: usize = dispatch_counts.iter().filter(|(k, _)| **k != "idle").map(|(_, v)| *v).sum();
    // L for Little's law = trucks on a laden round-trip arc (en route to pick up, carrying,
    // or at handover). Exclude idle and wait_rtg (parked) so W = L/λ isn't biased high.
    let cycling_trucks: usize = ["empty_travel", "delivering", "soon_idle"]
        .iter()
        .map(|k| dispatch_counts.get(k).copied().unwrap_or(0))
        .sum();
    let drops_60m = {
        let d = lm.tt_drops.lock().await;
        d.iter().filter(|&&t| now - t <= MOVE_WINDOW_MS).count() as u32
    };
    let lambda = drops_60m as f64 / (obs_h * 3600.0); // deliveries / second
    let tt_cycle_littles_s = (cycling_trucks > 0 && lambda > 0.0)
        .then(|| (cycling_trucks as f64 / lambda).round() as i64);
    let mut cyc_samples: Vec<i64> = {
        let c = lm.tt_cycles.lock().await;
        c.iter().filter(|&&(t, _)| now - t <= MOVE_WINDOW_MS).map(|&(_, i)| i).collect()
    };
    let tt_cycle_samples = cyc_samples.len() as u32;
    cyc_samples.sort_unstable();
    let pctile = |v: &[i64], p: f64| v.get(((v.len() as f64 * p) as usize).min(v.len().saturating_sub(1))).copied();
    let have_median = cyc_samples.len() >= MIN_CYCLE_SAMPLES;
    let tt_cycle_median_s = have_median.then(|| cyc_samples[cyc_samples.len() / 2]);
    let tt_cycle_p25_s = have_median.then(|| pctile(&cyc_samples, 0.25)).flatten();
    let tt_cycle_p75_s = have_median.then(|| pctile(&cyc_samples, 0.75)).flatten();
    let tt_cycle_min_samples = MIN_CYCLE_SAMPLES as u32;
    let tt_artifacts_60m = {
        let a = lm.tt_artifacts.lock().await;
        a.iter().filter(|&&t| now - t <= MOVE_WINDOW_MS).count() as u32
    };
    let tt_artifacts_near_60m = {
        let a = lm.tt_artifacts_near.lock().await;
        a.iter().filter(|&&t| now - t <= MOVE_WINDOW_MS).count() as u32
    };
    // how full the rolling 1h window is (min). Until it is full the rates/median are still
    // settling, so the UI labels "window filling" rather than implying a steady-state hour.
    let started = lm.connected_since_ms.load(Ordering::Relaxed) as i64;
    let window_fill_min = if started > 0 {
        (((now - started) / 60_000).clamp(0, MOVE_WINDOW_MS / 60_000)) as u32
    } else { 0 };

    // ── live K_UTIL (TT utilization) ──
    // A truck is utilized from job ALLOCATION to COMPLETION (container handed to the crane) —
    // even while stopped/queued at a crane. Idle = on-duty but with NO assignment.
    // The GPS job fields (topos1/jobtype) are INTERMITTENT — they ride only on assignment
    // events, not on every 3s heartbeat, so a point-in-time snapshot misses most assigned
    // trucks. The authoritative source is the TOS work pool (live_workpool.ytno = the TT
    // assigned to each active/incomplete job), refreshed every ~90s. So:
    //   utilized = TTs assigned in the work pool ;  on-duty = those ∪ manned (GPS engine on).
    let fresh_tt = || map.values().filter(|p| p.cls == "TT" && (now - p.last_seen_ms) / 1000 <= STALE_AFTER_S);
    let total_tt = fresh_tt().count();
    let in_service = fresh_tt().filter(|p| p.engine == 1).count(); // manned (operator aboard)
    let manned_ids: std::collections::HashSet<&str> = map
        .iter()
        .filter(|(_, p)| p.cls == "TT" && p.engine == 1 && (now - p.last_seen_ms) / 1000 <= STALE_AFTER_S)
        .map(|(id, _)| id.as_str())
        .collect();
    // assigned TTs from the work pool (fresh snapshot only; empty on staleness/error → "—")
    let assigned_ids: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT ytno FROM live_workpool
          WHERE ytno IS NOT NULL AND ytno <> '' AND as_of_ts > now() - interval '5 minutes'",
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    let assigned_n = assigned_ids.len();
    // on-duty = manned ∪ assigned (an assigned truck is on duty even if its GPS engine flickers;
    // a manned-but-unassigned truck is on-duty-idle and belongs in the denominator).
    let on_duty: std::collections::HashSet<&str> =
        manned_ids.iter().copied().chain(assigned_ids.iter().map(|s| s.as_str())).collect();
    let tt_util_live = (assigned_n > 0 && !on_duty.is_empty())
        .then(|| (assigned_n as f64 / on_duty.len() as f64 * 100.0).round() as i64);
    // secondary context: of manned trucks, how many are physically moving/carrying right now
    // (the rest of the assigned ones are queued/waiting within their job — still utilized).
    let tt_engaged_live = (in_service > 0).then(|| (active_trucks as f64 / in_service as f64 * 100.0).round() as i64);

    // ── live K_QC_Q (QC waiting for a truck) ── direct starvation: a working quay crane
    // that's been idle past a normal inter-move gap with NO assigned TT arrived at it.
    // Fixes the TOS HAVING-≥10 intra-shift undercount. Conservative (60s gap); quay only.
    let cranes_with_tt: std::collections::HashSet<&str> = map
        .values()
        .filter(|p| p.cls == "TT" && p.arrival.as_deref() == Some("ARRIVED"))
        .filter_map(|p| p.topos1.as_deref())
        .filter(|t| is_crane_code(t))
        .collect();
    let (mut qc_starving, mut qc_wait_sum) = (0usize, 0i64);
    for (id, c) in plc.iter() {
        let working = c.moves.iter().any(|&t| now - t <= MOVE_WINDOW_MS);
        let fresh = (now - c.last_seen_ms) / 1000 <= STALE_AFTER_S;
        if !working || !fresh || c.last_move_ms == 0 {
            continue;
        }
        let idle_s = (now - c.last_move_ms) / 1000;
        if idle_s > QCQ_IDLE_S && !cranes_with_tt.contains(id.as_str()) {
            qc_starving += 1;
            qc_wait_sum += idle_s;
        }
    }
    let qc_wait_live_s = (qc_starving > 0).then(|| qc_wait_sum / qc_starving as i64);

    let last_ms = lm.last_msg_ms.load(Ordering::Relaxed);
    let as_of = (last_ms != 0).then(|| DateTime::from_timestamp_millis(last_ms as i64)).flatten();
    Json(PositionsOut {
        source: "live",
        connected: lm.connected.load(Ordering::Relaxed),
        as_of,
        count: devices.len(),
        messages: lm.messages.load(Ordering::Relaxed),
        dispatch_counts,
        crane_moves_60m,
        cranes_working,
        crane_mph_live,
        tt_cycle_littles_s,
        tt_cycle_median_s,
        tt_cycle_p25_s,
        tt_cycle_p75_s,
        tt_drops_60m: drops_60m,
        tt_cycle_samples,
        tt_cycle_min_samples,
        tt_artifacts_60m,
        tt_artifacts_near_60m,
        window_fill_min,
        active_trucks,
        tt_util_live,
        tt_engaged_live,
        qc_starving,
        qc_wait_live_s,
        devices,
    })
}

// ───────────────────────── health endpoint ─────────────────────────

#[derive(Serialize)]
pub struct HealthOut {
    /// overall state: "green" | "amber" | "red"
    color: &'static str,
    state_word: &'static str,
    cause: String,
    connected: bool,
    /// seconds since the upstream socket connected (null if never / down)
    connected_for_s: Option<i64>,
    /// seconds since the last GPS message (null if none yet)
    last_msg_age_s: Option<i64>,
    last_message_at: Option<DateTime<Utc>>,
    messages_total: u64,
    reconnects: u64,
    last_error: Option<String>,
    uptime_s: i64,
    /// messages in the last completed minute
    rate_per_min: u32,
    /// per-minute counts, oldest→newest (length 30)
    sparkline: Vec<u32>,
    // freshness bands (device counts)
    fresh: usize,
    stale: usize,
    lost: usize,
    total_devices: usize,
    // fleet + quality
    by_class: HashMap<String, usize>,
    engine_on: usize,
    with_job: usize,
    avg_accuracy_m: Option<f64>,
    fresh_under_s: i64,
    stale_after_s: i64,
    // ctab zone (crane PLC)
    plc_connected: bool,
    plc_devices: usize,
    plc_messages: u64,
}

/// `GET /api/livemap/health` — feed health for the WS-data monitoring page.
pub async fn health(State(lm): State<Arc<LiveMap>>) -> Json<HealthOut> {
    let now = Utc::now().timestamp_millis();
    let now_min = now / 60_000;
    let connected = lm.connected.load(Ordering::Relaxed);

    let (sparkline, rate_per_min) = {
        let mut ring = lm.ring.lock().await;
        ring.advance(now_min);
        (ring.series(), ring.rate())
    };

    let last_ms = lm.last_msg_ms.load(Ordering::Relaxed) as i64;
    let last_msg_age_s = (last_ms != 0).then(|| (now - last_ms) / 1000);
    let last_message_at = (last_ms != 0).then(|| DateTime::from_timestamp_millis(last_ms)).flatten();
    let csince = lm.connected_since_ms.load(Ordering::Relaxed) as i64;
    let connected_for_s = (connected && csince != 0).then(|| (now - csince) / 1000);
    let started = lm.started_ms.load(Ordering::Relaxed) as i64;

    let (mut fresh, mut stale, mut lost, mut engine_on, mut with_job) = (0, 0, 0, 0, 0);
    let mut by_class: HashMap<String, usize> = HashMap::new();
    let (mut acc_sum, mut acc_n) = (0.0_f64, 0_u32);
    {
        let map = lm.devices.read().await;
        for p in map.values() {
            let age = (now - p.last_seen_ms) / 1000;
            if age <= FRESH_UNDER_S {
                fresh += 1;
            } else if age <= STALE_AFTER_S {
                stale += 1;
            } else {
                lost += 1;
            }
            if age <= STALE_AFTER_S {
                *by_class.entry(p.cls.clone()).or_default() += 1;
                if p.engine == 1 {
                    engine_on += 1;
                }
                if p.jobtype.as_deref().is_some_and(|s| !s.is_empty()) {
                    with_job += 1;
                }
                if let Some(a) = p.accuracy {
                    acc_sum += a;
                    acc_n += 1;
                }
            }
        }
    }
    let total_devices = fresh + stale + lost;
    let avg_accuracy_m = (acc_n > 0).then(|| (acc_sum / acc_n as f64 * 10.0).round() / 10.0);
    let plc_devices = {
        let p = lm.plc.read().await;
        p.values().filter(|c| (now - c.last_seen_ms) / 1000 <= STALE_AFTER_S).count()
    };

    // overall state: red if not connected or no data >60s; amber if data 20-60s stale
    // or zero active devices; else green.
    let active = fresh + stale;
    let (color, state_word, cause): (&str, &str, String) = if !connected {
        ("red", "장애", "WS 미연결 — SSH 터널/소스 확인".into())
    } else if last_msg_age_s.is_none_or(|a| a > 60) {
        ("red", "장애", "60초 이상 데이터 없음".into())
    } else if active == 0 {
        ("amber", "주의", "활성 장비 없음".into())
    } else if last_msg_age_s.is_some_and(|a| a > 20) {
        ("amber", "주의", format!("최근 수신 {}초 전", last_msg_age_s.unwrap_or(0)))
    } else {
        ("green", "정상", format!("{active}대 추적 중 · {rate_per_min}/분"))
    };

    Json(HealthOut {
        color,
        state_word,
        cause,
        connected,
        connected_for_s,
        last_msg_age_s,
        last_message_at,
        messages_total: lm.messages.load(Ordering::Relaxed),
        reconnects: lm.reconnects.load(Ordering::Relaxed),
        last_error: lm.last_error.read().await.clone(),
        uptime_s: (now - started) / 1000,
        rate_per_min,
        sparkline,
        fresh,
        stale,
        lost,
        total_devices,
        by_class,
        engine_on,
        with_job,
        avg_accuracy_m,
        fresh_under_s: FRESH_UNDER_S,
        stale_after_s: STALE_AFTER_S,
        plc_connected: lm.plc_connected.load(Ordering::Relaxed),
        plc_devices,
        plc_messages: lm.plc_messages.load(Ordering::Relaxed),
    })
}

// ───────────────────────── ingest loop ─────────────────────────

/// Spawn the background ingest task + a periodic pruner.
pub fn spawn(lm: Arc<LiveMap>) {
    // pruner: drop fixes older than LOST_AFTER_S so the maps can't grow unbounded.
    {
        let lm = lm.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                let cutoff = Utc::now().timestamp_millis() - LOST_AFTER_S * 1000;
                lm.devices.write().await.retain(|_, p| p.last_seen_ms >= cutoff);
                lm.plc.write().await.retain(|_, c| c.last_seen_ms >= cutoff);
            }
        });
    }
    let url = std::env::var("LIVEMAP_WS_URL").unwrap_or_else(|_| "ws://127.0.0.1:9986".into());
    let identify = std::env::var("LIVEMAP_IDENTIFY").unwrap_or_else(|_| "clt_digitaltwin1".into());
    let username = std::env::var("LIVEMAP_USERNAME").unwrap_or_else(|_| "digitaltwin".into());
    let user = std::env::var("LIVEMAP_USER").unwrap_or_else(|_| "clt_digitaltwin1".into());

    // GPS zone (wpt_gps) — primary feed.
    {
        let (lm, url, identify, username, user) =
            (lm.clone(), url.clone(), identify.clone(), username.clone(), user.clone());
        tokio::spawn(async move {
            let mut backoff = 2u64;
            loop {
                match serve_gps(&lm, &url, &identify, &username, &user).await {
                    Ok(()) => backoff = 2,
                    Err(e) => {
                        lm.connected.store(false, Ordering::Relaxed);
                        *lm.last_error.write().await = Some(format!("{e}"));
                        tracing::warn!(error = %e, backoff_s = backoff, "livemap gps ws disconnected");
                    }
                }
                lm.reconnects.fetch_add(1, Ordering::Relaxed);
                tokio::time::sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(30);
            }
        });
    }

    // ctab zone (crane PLC) — secondary feed, identify only (no checkin).
    tokio::spawn(async move {
        let mut backoff = 2u64;
        loop {
            match serve_ctab(&lm, &url, &identify).await {
                Ok(()) => backoff = 2,
                Err(e) => {
                    lm.plc_connected.store(false, Ordering::Relaxed);
                    tracing::warn!(error = %e, backoff_s = backoff, "livemap ctab ws disconnected");
                }
            }
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    });
}

async fn serve_gps(
    lm: &Arc<LiveMap>,
    url: &str,
    identify: &str,
    username: &str,
    user: &str,
) -> anyhow::Result<()> {
    let (ws, _resp) = tokio_tungstenite::connect_async(url).await?;
    let (mut tx, mut rx) = ws.split();
    tracing::info!(%url, "livemap gps ws connected");

    // wpt_gps zone handshake: identify -> wait 2s -> checkin.
    let identify_msg = serde_json::json!({"command":{"identify": identify, "zone":"wpt_gps"}});
    tx.send(Message::Text(identify_msg.to_string())).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let checkin_msg = serde_json::json!({"checkin":{"username": username, "user": user}});
    tx.send(Message::Text(checkin_msg.to_string())).await?;

    lm.connected.store(true, Ordering::Relaxed);
    lm.connected_since_ms.store(Utc::now().timestamp_millis() as u64, Ordering::Relaxed);
    *lm.last_error.write().await = None;

    // The source never pongs our pings (reference disables them); detect a dead socket
    // by a receive timeout instead.
    loop {
        let msg = tokio::time::timeout(Duration::from_secs(60), rx.next()).await?;
        let Some(msg) = msg else { break }; // stream ended
        match msg? {
            Message::Text(t) => ingest_text(lm, &t).await,
            Message::Binary(b) => {
                if let Ok(t) = String::from_utf8(b) {
                    ingest_text(lm, &t).await;
                }
            }
            Message::Ping(p) => {
                let _ = tx.send(Message::Pong(p)).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    lm.connected.store(false, Ordering::Relaxed);
    Ok(())
}

/// ctab zone — crane PLC. Handshake is identify ONLY (no checkin, per the reference).
async fn serve_ctab(lm: &Arc<LiveMap>, url: &str, identify: &str) -> anyhow::Result<()> {
    let (ws, _resp) = tokio_tungstenite::connect_async(url).await?;
    let (mut tx, mut rx) = ws.split();
    tracing::info!(%url, "livemap ctab ws connected");

    let identify_msg = serde_json::json!({"command":{"identify": identify, "zone":"ctab"}});
    tx.send(Message::Text(identify_msg.to_string())).await?;
    lm.plc_connected.store(true, Ordering::Relaxed);

    loop {
        let msg = tokio::time::timeout(Duration::from_secs(60), rx.next()).await?;
        let Some(msg) = msg else { break };
        match msg? {
            Message::Text(t) => ingest_ctab(lm, &t).await,
            Message::Binary(b) => {
                if let Ok(t) = String::from_utf8(b) {
                    ingest_ctab(lm, &t).await;
                }
            }
            Message::Ping(p) => {
                let _ = tx.send(Message::Pong(p)).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
    lm.plc_connected.store(false, Ordering::Relaxed);
    Ok(())
}

/// Parse a ctab `plc_data` frame:
/// `{"data":{"id":"plc_C39_...","zone":"ctab","datas":"{\"plc_data\":{\"crane\":\"C39\",
///   \"load\":0,\"lock\":\"False\",\"land\":\"False\",\"hpos\":\"6.77\",\"tpos\":\"69.35\"}}"}}`.
/// Other ctab kinds (checkin / session_* / rps_*) are ignored.
async fn ingest_ctab(lm: &Arc<LiveMap>, text: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
    let Some(datas) = v.get("data").and_then(|d| d.get("datas")).and_then(|x| x.as_str()) else {
        return;
    };
    let Ok(inner) = serde_json::from_str::<serde_json::Value>(datas) else { return };
    let Some(g) = inner.get("plc_data") else { return };
    let Some(crane) = g.get("crane").and_then(|x| x.as_str()).filter(|s| !s.is_empty()) else {
        return;
    };
    let now = Utc::now().timestamp_millis();
    let load = g.get("load").and_then(num);
    // hysteresis: laden ≥1.0t, empty <0.5t; otherwise keep prior state
    let mut map = lm.plc.write().await;
    let e = map.entry(crane.to_string()).or_default();
    let now_laden = match load {
        Some(t) if t >= PLC_LADEN_T => true,
        Some(t) if t < PLC_EMPTY_T => false,
        _ => e.laden,
    };
    if !e.laden && now_laden {
        // empty→laden = a pickup. Count it as a move unless it's a flicker within one
        // cycle of the last counted move.
        let since_last = if e.last_move_ms == 0 { i64::MAX } else { now - e.last_move_ms };
        if since_last >= MIN_MOVE_GAP_MS {
            e.moves.push_back(now);
            e.last_move_ms = now;
            while e.moves.front().is_some_and(|&f| now - f > MOVE_WINDOW_MS) {
                e.moves.pop_front();
            }
        }
    }
    e.laden = now_laden;
    e.load_t = load;
    e.lock = g.get("lock").and_then(parse_bool);
    e.land = g.get("land").and_then(parse_bool);
    e.hpos = g.get("hpos").and_then(num);
    e.tpos = g.get("tpos").and_then(num);
    e.last_seen_ms = now;
    drop(map);
    lm.plc_messages.fetch_add(1, Ordering::Relaxed);
}

/// "True"/"False" (any case) or 1/0 → bool.
fn parse_bool(v: &serde_json::Value) -> Option<bool> {
    if let Some(b) = v.as_bool() {
        return Some(b);
    }
    if let Some(n) = v.as_f64() {
        return Some(n != 0.0);
    }
    match v.as_str()?.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

/// Parse one frame. GPS frames:
/// `{"data":{"id":"TT1074","zone":"wpt_gps","datas":"<stringified gps_update json>"}}`.
/// `{"disconnect":...}` churn frames are ignored (positions age out by `last_seen`).
async fn ingest_text(lm: &Arc<LiveMap>, text: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return };
    let Some(data) = v.get("data") else { return };
    let Some(id) = data.get("id").and_then(|x| x.as_str()) else { return };
    let Some(datas) = data.get("datas").and_then(|x| x.as_str()) else { return };
    let Ok(inner) = serde_json::from_str::<serde_json::Value>(datas) else { return };
    let Some(g) = inner.get("gps_update") else { return };

    let (Some(lat), Some(lon)) = (g.get("lat").and_then(num), g.get("lon").and_then(num)) else {
        return;
    };
    if lat == 0.0 && lon == 0.0 {
        return; // no fix
    }
    let speed = g
        .get("speed")
        .and_then(|x| x.as_str())
        .map(|s| s.trim_end_matches(|c: char| c.is_alphabetic()).trim())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let engine = match g.get("engine_on").and_then(|x| x.as_str()) {
        Some(s) if s.to_ascii_uppercase().contains("ON") => 1,
        _ => 0,
    };
    let cls: String = id.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let now = Utc::now().timestamp_millis();

    let mut pos = Pos {
        cls,
        lat,
        lon,
        speed,
        engine,
        last_seen_ms: now,
        jobtype: opt_str(g, "jobtype"),
        vslname: opt_str(g, "vslname"),
        container1: opt_str(g, "container1"),
        container2: opt_str(g, "container2"),
        cur_loc: opt_str(g, "cur_loc"),
        topos1: opt_str(g, "topos1"),
        arrival: opt_str(g, "arrival"),
        fuel: g.get("fuel_level").and_then(num),
        accuracy: g.get("accuracy").and_then(num),
        userid: opt_str(g, "userid").map(|s| clean_driver(&s)),
        batt: opt_str(g, "batt"),
        nett: opt_str(g, "nett"),
        dtime: opt_str(g, "dtime"),
        distance: g.get("distance").and_then(num),
        ..Default::default()
    };

    // learn block/bay AND crane positions from TTs observed ARRIVED at a topos — feeds the
    // empty-TT "remaining distance to pickup" used for swap-worthiness (crane fallback when a
    // crane isn't broadcasting GPS).
    if pos.cls == "TT" && pos.arrival.as_deref() == Some("ARRIVED") {
        if let Some(t) = pos.topos1.as_deref() {
            if !t.is_empty() {
                let (full, pre) = (t.to_string(), block_prefix(t).to_string());
                let mut c = lm.centroids.write().await;
                c.entry(full).or_default().push(lat, lon);
                if pre != t {
                    c.entry(pre).or_default().push(lat, lon);
                }
            }
        }
    }
    // TT cycle: carry per-truck tracking across fixes; a delivery = container1 changing
    // away from a non-empty value (→empty OR →another container). Record a fleet delivery
    // (for throughput λ) — always — and, between two of a truck's deliveries, a capped
    // cycle-interval sample (for the median). Fleet-delivery and cycle-sample are separate
    // (the first delivery has no predecessor, so it feeds λ but not the median).
    let mut fleet_drop = false;
    let mut cycle_sample_s: Option<i64> = None;
    let mut artifact = false;
    let mut artifact_near = false;
    {
        let mut devmap = lm.devices.write().await;
        let prev_c1 = devmap.get(id).and_then(|p| p.container1.clone());
        if let Some(prev) = devmap.get(id) {
            pos.carry_since_ms = prev.carry_since_ms;
            pos.last_drop_ms = prev.last_drop_ms;
            // accumulate path length driven since the carry began (jitter-guarded). This is
            // the evidence used to tell a real delivery (truck drove the box) from a TOS
            // re-assignment artifact (container1 rewritten while the truck sits still).
            pos.carry_trip_m = prev.carry_trip_m;
            if pos.cls == "TT" && prev.lat != 0.0 && pos.lat != 0.0 {
                let step = dist_m((prev.lat, prev.lon), (pos.lat, pos.lon));
                if step.is_finite() && step <= MAX_FIX_STEP_M {
                    pos.carry_trip_m += step;
                }
            }
        }
        if pos.cls == "TT" {
            let new_c1 = pos.container1.as_deref().unwrap_or("");
            let old_c1 = prev_c1.as_deref().unwrap_or("");
            if new_c1 != old_c1 {
                if !old_c1.is_empty() {
                    // a delivery requires the box was carried ≥30s AND the truck actually
                    // drove it ≥150m. Carried-but-stationary = TOS re-assignment, not a
                    // delivery: rejected here on the movement signature (not on duration).
                    let held = if pos.carry_since_ms > 0 { now - pos.carry_since_ms } else { i64::MAX };
                    if held >= MIN_LOADED_MS {
                        if pos.carry_trip_m >= MIN_CARRY_TRIP_M {
                            fleet_drop = true;
                            if pos.last_drop_ms != 0 {
                                let iv = now - pos.last_drop_ms;
                                if (MIN_CYCLE_S * 1000..=MAX_CYCLE_S * 1000).contains(&iv) {
                                    cycle_sample_s = Some(iv / 1000);
                                }
                            }
                            pos.last_drop_ms = now;
                        } else {
                            artifact = true; // changed container1 without moving it
                            artifact_near = pos.carry_trip_m >= NEAR_TRIP_M; // possible short haul
                        }
                    }
                }
                // new carry (or empty): reset the trip accumulator for the next box
                pos.carry_since_ms = if new_c1.is_empty() { 0 } else { now };
                pos.carry_trip_m = 0.0;
            }
        }
        devmap.insert(id.to_string(), pos);
    }
    if fleet_drop {
        let mut drops = lm.tt_drops.lock().await;
        drops.push_back(now);
        while drops.front().is_some_and(|&f| now - f > MOVE_WINDOW_MS) { drops.pop_front(); }
    }
    if artifact {
        let mut arts = lm.tt_artifacts.lock().await;
        arts.push_back(now);
        while arts.front().is_some_and(|&f| now - f > MOVE_WINDOW_MS) { arts.pop_front(); }
    }
    if artifact_near {
        let mut near = lm.tt_artifacts_near.lock().await;
        near.push_back(now);
        while near.front().is_some_and(|&f| now - f > MOVE_WINDOW_MS) { near.pop_front(); }
    }
    if let Some(s) = cycle_sample_s {
        let mut cyc = lm.tt_cycles.lock().await;
        cyc.push_back((now, s));
        while cyc.front().is_some_and(|&(t, _)| now - t > MOVE_WINDOW_MS) { cyc.pop_front(); }
    }
    lm.messages.fetch_add(1, Ordering::Relaxed);
    lm.last_msg_ms.store(now as u64, Ordering::Relaxed);
    lm.ring.lock().await.bump(now / 60_000);
}

/// Numbers arrive as JSON strings ("2.9207...") or bare numbers; accept either.
fn num(v: &serde_json::Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

/// Trim a string field, returning None for empty.
fn opt_str(g: &serde_json::Value, key: &str) -> Option<String> {
    g.get(key)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// "P12345<br/>(0136…)" → "P12345 / 0136…" (strip HTML, tidy whitespace).
fn clean_driver(s: &str) -> String {
    let mut out = s.replace("<br/>", " / ").replace("<br>", " / ").replace("<br />", " / ");
    out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    out
}
