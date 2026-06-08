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
