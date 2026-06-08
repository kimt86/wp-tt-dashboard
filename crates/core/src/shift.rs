//! Operating-shift model (research-confirmed): Night 00:00–08:00, Day 08:00–16:00,
//! Evening 16:00–24:00 (each 7h50m work + ~10m handover, the handover counting to
//! the ending shift). All three shifts belong to the calendar day they start on, so
//! a shift window is always a sub-range of one business day — this preserves the
//! index-safe single-day date predicate in the extractor SQL.

use chrono::{Duration, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};

/// Westports terminal timezone (MYT, UTC+8, no DST). The operational data
/// (MCH_OPERATION / JOB_ORDER_HISTORY wall-clock columns) is in this zone, so all
/// shift detection and time-window math MUST use it — NOT the server's local clock.
pub fn terminal_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).unwrap()
}

/// "Now" in the terminal's timezone.
pub fn terminal_now() -> chrono::DateTime<FixedOffset> {
    Utc::now().with_timezone(&terminal_offset())
}

/// Interpret a terminal-local wall-clock instant as an absolute UTC instant
/// (for storing in TIMESTAMPTZ columns).
pub fn terminal_to_utc(naive: NaiveDateTime) -> chrono::DateTime<Utc> {
    terminal_offset()
        .from_local_datetime(&naive)
        .single()
        .unwrap_or_else(|| Utc.from_utc_datetime(&naive).into())
        .with_timezone(&Utc)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shift {
    Night,
    Day,
    Evening,
}

impl Shift {
    /// One-letter code stored in the DB (`kpi_shift.shift`).
    pub fn label(&self) -> &'static str {
        match self {
            Shift::Night => "N",
            Shift::Day => "D",
            Shift::Evening => "E",
        }
    }

    pub fn from_label(s: &str) -> Option<Shift> {
        match s {
            "N" => Some(Shift::Night),
            "D" => Some(Shift::Day),
            "E" => Some(Shift::Evening),
            _ => None,
        }
    }

    pub fn name_en(&self) -> &'static str {
        match self {
            Shift::Night => "Night",
            Shift::Day => "Day",
            Shift::Evening => "Evening",
        }
    }

    pub fn name_ko(&self) -> &'static str {
        match self {
            Shift::Night => "야간",
            Shift::Day => "주간",
            Shift::Evening => "저녁",
        }
    }

    /// Shift-start hour (0 / 8 / 16).
    fn start_hour(&self) -> u32 {
        match self {
            Shift::Night => 0,
            Shift::Day => 8,
            Shift::Evening => 16,
        }
    }
}

/// The shift containing `now`, and the business date it belongs to (= the calendar
/// date, since every shift starts on its own day).
pub fn current(now: NaiveDateTime) -> (NaiveDate, Shift) {
    let shift = match now.hour() {
        0..=7 => Shift::Night,
        8..=15 => Shift::Day,
        _ => Shift::Evening,
    };
    (now.date(), shift)
}

/// `[start, nominal_end)` for a shift on a business date. Night→[00:00,08:00),
/// Day→[08:00,16:00), Evening→[16:00, next-day 00:00).
pub fn window(date: NaiveDate, shift: Shift) -> (NaiveDateTime, NaiveDateTime) {
    let start = date.and_hms_opt(shift.start_hour(), 0, 0).unwrap();
    let nominal_end = match shift {
        Shift::Night => date.and_hms_opt(8, 0, 0).unwrap(),
        Shift::Day => date.and_hms_opt(16, 0, 0).unwrap(),
        Shift::Evening => (date + Duration::days(1)).and_hms_opt(0, 0, 0).unwrap(),
    };
    (start, nominal_end)
}

/// The shift immediately preceding `(date, shift)`. Night's predecessor is the prior
/// calendar day's Evening.
pub fn previous(date: NaiveDate, shift: Shift) -> (NaiveDate, Shift) {
    match shift {
        Shift::Night => (date - Duration::days(1), Shift::Evening),
        Shift::Day => (date, Shift::Night),
        Shift::Evening => (date, Shift::Day),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    #[test]
    fn current_shift_by_hour() {
        assert_eq!(current(dt(2026, 6, 5, 3, 0)).1, Shift::Night);
        assert_eq!(current(dt(2026, 6, 5, 7, 59)).1, Shift::Night); // handover counts to Night
        assert_eq!(current(dt(2026, 6, 5, 8, 0)).1, Shift::Day);
        assert_eq!(current(dt(2026, 6, 5, 15, 59)).1, Shift::Day);
        assert_eq!(current(dt(2026, 6, 5, 16, 0)).1, Shift::Evening);
        assert_eq!(current(dt(2026, 6, 5, 23, 59)).1, Shift::Evening);
        // business date = calendar date for all
        assert_eq!(current(dt(2026, 6, 5, 23, 59)).0, NaiveDate::from_ymd_opt(2026, 6, 5).unwrap());
    }

    #[test]
    fn windows() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        assert_eq!(window(d, Shift::Night), (dt(2026, 6, 5, 0, 0), dt(2026, 6, 5, 8, 0)));
        assert_eq!(window(d, Shift::Day), (dt(2026, 6, 5, 8, 0), dt(2026, 6, 5, 16, 0)));
        assert_eq!(window(d, Shift::Evening), (dt(2026, 6, 5, 16, 0), dt(2026, 6, 6, 0, 0)));
    }

    #[test]
    fn previous_shift() {
        let d = NaiveDate::from_ymd_opt(2026, 6, 5).unwrap();
        assert_eq!(previous(d, Shift::Evening), (d, Shift::Day));
        assert_eq!(previous(d, Shift::Day), (d, Shift::Night));
        assert_eq!(previous(d, Shift::Night), (NaiveDate::from_ymd_opt(2026, 6, 4).unwrap(), Shift::Evening));
    }

    #[test]
    fn label_roundtrip() {
        for s in [Shift::Night, Shift::Day, Shift::Evening] {
            assert_eq!(Shift::from_label(s.label()), Some(s));
        }
    }
}
