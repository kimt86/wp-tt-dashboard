//! Calendar period -> date range resolution (current + immediately-preceding period).
//! All KPIs are intensive (averages/ratios), so comparing a partial current period
//! against a full previous period is meaningful.

use chrono::{Datelike, Duration, NaiveDate};

#[derive(Clone, Copy)]
pub struct Range {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

pub struct Resolved {
    pub period: String,
    pub cur: Range,
    pub prev: Range,
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
}

fn monday_of(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}

/// Resolve a period name against `today` (server-local current date). Unknown
/// names fall back to "yesterday".
pub fn resolve(period: &str, today: NaiveDate) -> Resolved {
    let day = |d: NaiveDate| Range { from: d, to: d };
    let y = today - Duration::days(1);

    let (cur, prev) = match period {
        "today" => (day(today), day(y)),
        "yesterday" => (day(y), day(today - Duration::days(2))),
        "last7" => {
            let from = today - Duration::days(6);
            (Range { from, to: today }, Range { from: from - Duration::days(7), to: from - Duration::days(1) })
        }
        "last30" => {
            let from = today - Duration::days(29);
            (Range { from, to: today }, Range { from: from - Duration::days(30), to: from - Duration::days(1) })
        }
        "this_week" => {
            let mon = monday_of(today);
            (Range { from: mon, to: today }, Range { from: mon - Duration::days(7), to: mon - Duration::days(1) })
        }
        "last_week" => {
            let mon = monday_of(today);
            let lw_from = mon - Duration::days(7);
            let lw_to = mon - Duration::days(1);
            (Range { from: lw_from, to: lw_to }, Range { from: lw_from - Duration::days(7), to: lw_to - Duration::days(7) })
        }
        "this_month" => {
            let first = first_of_month(today);
            let lm_last = first - Duration::days(1);
            (Range { from: first, to: today }, Range { from: first_of_month(lm_last), to: lm_last })
        }
        "last_month" => {
            let first_this = first_of_month(today);
            let lm_last = first_this - Duration::days(1);
            let lm_first = first_of_month(lm_last);
            let pm_last = lm_first - Duration::days(1);
            (Range { from: lm_first, to: lm_last }, Range { from: first_of_month(pm_last), to: pm_last })
        }
        _ => (day(y), day(today - Duration::days(2))), // default: yesterday
    };

    Resolved {
        period: if matches!(
            period,
            "today" | "yesterday" | "last7" | "last30" | "this_week" | "last_week" | "this_month" | "last_month"
        ) {
            period.to_string()
        } else {
            "yesterday".to_string()
        },
        cur,
        prev,
    }
}

/// Does the range include today (=> the aggregate is provisional)?
pub fn includes_today(r: &Range, today: NaiveDate) -> bool {
    r.from <= today && today <= r.to
}

fn next_month(first_of: NaiveDate) -> NaiveDate {
    if first_of.month() == 12 {
        NaiveDate::from_ymd_opt(first_of.year() + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(first_of.year(), first_of.month() + 1, 1).unwrap()
    }
}

/// Most-recent `n` single-day buckets, newest first.
pub fn day_buckets(today: NaiveDate, n: usize) -> Vec<Range> {
    (0..n as i64)
        .map(|i| {
            let d = today - Duration::days(i);
            Range { from: d, to: d }
        })
        .collect()
}

/// Most-recent `n` ISO-week (Mon–Sun) buckets, newest first. The current week's `to` is
/// clamped to `today`.
pub fn week_buckets(today: NaiveDate, n: usize) -> Vec<Range> {
    let cur_mon = monday_of(today);
    (0..n as i64)
        .map(|i| {
            let mon = cur_mon - Duration::days(7 * i);
            Range { from: mon, to: (mon + Duration::days(6)).min(today) }
        })
        .collect()
}

/// Most-recent `n` calendar-month buckets, newest first. The current month's `to` is
/// clamped to `today`.
pub fn month_buckets(today: NaiveDate, n: usize) -> Vec<Range> {
    let mut out = Vec::with_capacity(n);
    let mut first = first_of_month(today);
    for _ in 0..n {
        let last = next_month(first) - Duration::days(1);
        out.push(Range { from: first, to: last.min(today) });
        first = first_of_month(first - Duration::days(1));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_buckets_newest_first() {
        let t = NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        let b = day_buckets(t, 3);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].from, t);
        assert_eq!(b[0].to, t);
        assert_eq!(b[1].from, NaiveDate::from_ymd_opt(2026, 6, 8).unwrap());
        assert_eq!(b[2].from, NaiveDate::from_ymd_opt(2026, 6, 7).unwrap());
    }

    #[test]
    fn week_buckets_mon_sun_clamped() {
        // 2026-06-09 is a Tuesday → ISO week Mon 06-08 .. Sun 06-14, clamped to today.
        let t = NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        let b = week_buckets(t, 2);
        assert_eq!(b[0].from, NaiveDate::from_ymd_opt(2026, 6, 8).unwrap()); // Monday
        assert_eq!(b[0].to, t); // clamped to today (not Sunday)
        assert_eq!(b[1].from, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()); // prev Monday
        assert_eq!(b[1].to, NaiveDate::from_ymd_opt(2026, 6, 7).unwrap()); // prev Sunday
    }

    #[test]
    fn month_buckets_clamped_and_step() {
        let t = NaiveDate::from_ymd_opt(2026, 6, 9).unwrap();
        let b = month_buckets(t, 3);
        assert_eq!(b[0].from, NaiveDate::from_ymd_opt(2026, 6, 1).unwrap());
        assert_eq!(b[0].to, t); // June clamped to today
        assert_eq!(b[1].from, NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
        assert_eq!(b[1].to, NaiveDate::from_ymd_opt(2026, 5, 31).unwrap());
        assert_eq!(b[2].from, NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
        assert_eq!(b[2].to, NaiveDate::from_ymd_opt(2026, 4, 30).unwrap());
    }

    #[test]
    fn month_buckets_year_boundary() {
        let t = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let b = month_buckets(t, 2);
        assert_eq!(b[1].from, NaiveDate::from_ymd_opt(2025, 12, 1).unwrap());
        assert_eq!(b[1].to, NaiveDate::from_ymd_opt(2025, 12, 31).unwrap());
    }
}
