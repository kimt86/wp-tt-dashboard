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

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::Json;
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
    fuel: Option<f64>,
    accuracy: Option<f64>,
    userid: Option<String>,
    batt: Option<String>,
    nett: Option<String>,
    dtime: Option<String>,
    distance: Option<f64>,
}

/// Crane PLC state from the `ctab` zone (`plc_data`). Dynamic equipment only
/// (C/M/Z prefixes). Keyed by crane id, which matches the GPS device id.
#[derive(Clone, Default)]
struct Plc {
    load_t: Option<f64>, // hook load in metric tons
    lock: Option<bool>,
    land: Option<bool>,
    hpos: Option<f64>, // hoist position (crane-local axis)
    tpos: Option<f64>, // trolley position
    last_seen_ms: i64,
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
}

impl LiveMap {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            devices: RwLock::new(HashMap::new()),
            plc: RwLock::new(HashMap::new()),
            ring: Mutex::new(Ring::new()),
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
}

#[derive(Serialize)]
pub struct PositionsOut {
    source: &'static str,
    connected: bool,
    as_of: Option<DateTime<Utc>>,
    count: usize,
    messages: u64,
    devices: Vec<DeviceOut>,
}

/// `GET /api/livemap/positions` — active device fixes (age ≤ 120s).
pub async fn positions(State(lm): State<Arc<LiveMap>>) -> Json<PositionsOut> {
    let now = Utc::now().timestamp_millis();
    let map = lm.devices.read().await;
    let plc = lm.plc.read().await;
    let mut devices: Vec<DeviceOut> = map
        .iter()
        .filter_map(|(id, p)| {
            let age = (now - p.last_seen_ms) / 1000;
            if age > STALE_AFTER_S {
                return None;
            }
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
                fuel: p.fuel,
                accuracy: p.accuracy,
                userid: p.userid.clone(),
                batt: p.batt.clone(),
                nett: p.nett.clone(),
                dtime: p.dtime.clone(),
                distance: p.distance,
                plc: plc_out,
            })
        })
        .collect();
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    let last_ms = lm.last_msg_ms.load(Ordering::Relaxed);
    let as_of = (last_ms != 0).then(|| DateTime::from_timestamp_millis(last_ms as i64)).flatten();
    Json(PositionsOut {
        source: "live",
        connected: lm.connected.load(Ordering::Relaxed),
        as_of,
        count: devices.len(),
        messages: lm.messages.load(Ordering::Relaxed),
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
    let plc = Plc {
        load_t: g.get("load").and_then(num),
        lock: g.get("lock").and_then(parse_bool),
        land: g.get("land").and_then(parse_bool),
        hpos: g.get("hpos").and_then(num),
        tpos: g.get("tpos").and_then(num),
        last_seen_ms: Utc::now().timestamp_millis(),
    };
    lm.plc.write().await.insert(crane.to_string(), plc);
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

    let pos = Pos {
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
        fuel: g.get("fuel_level").and_then(num),
        accuracy: g.get("accuracy").and_then(num),
        userid: opt_str(g, "userid").map(|s| clean_driver(&s)),
        batt: opt_str(g, "batt"),
        nett: opt_str(g, "nett"),
        dtime: opt_str(g, "dtime"),
        distance: g.get("distance").and_then(num),
    };

    lm.devices.write().await.insert(id.to_string(), pos);
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
