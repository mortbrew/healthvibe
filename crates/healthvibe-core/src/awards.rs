//! Activity awards computed from stored daily rings, steps and workouts.
//!
//! These mirror the *regular* activity awards in Apple Fitness (ring closes,
//! perfect weeks, streaks, goal multiples, milestones, records, workouts).
//! Apple's own earned-award list is not in the Health export, so everything
//! here is recomputed from data; Apple-only awards are a manual checklist.

use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Default)]
pub struct Day {
    pub date: String,
    pub mv: f64,
    pub mg: f64,
    pub ex: f64,
    pub eg: f64,
    pub st: f64,
    pub sg: f64,
    pub steps: f64,
}

#[derive(Debug, Clone)]
pub struct Workout {
    pub date: String,
    pub kind: String,
    pub minutes: f64,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Earned {
    pub first: String,
    pub latest: String,
    pub count: i64,
    pub dates: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AwardOut {
    pub key: String,
    pub name: String,
    pub group: &'static str,
    pub description: String,
    /// Badge palette: move | exercise | stand | all | steps | workout | record
    pub color: &'static str,
    /// Badge shape: ring | hex | medal
    pub shape: &'static str,
    /// Short text on the badge (e.g. "7", "200%").
    pub mark: String,
    pub detail: Option<String>,
    pub earned: Option<Earned>,
    /// (current, target) for unearned awards.
    pub progress: Option<(f64, f64)>,
}

const MAX_DATES: usize = 50;

fn earned(dates: Vec<String>) -> Option<Earned> {
    let first = dates.first()?.clone();
    let latest = dates.last()?.clone();
    let count = dates.len() as i64;
    let mut tail = dates;
    if tail.len() > MAX_DATES {
        tail.drain(..tail.len() - MAX_DATES);
    }
    tail.reverse();
    Some(Earned {
        first,
        latest,
        count,
        dates: tail,
    })
}

fn nd(d: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()
}

/// Fill gaps so consecutive entries are consecutive calendar days.
fn dense(days: &[Day]) -> Vec<Day> {
    let (Some(a), Some(b)) = (
        days.first().and_then(|d| nd(&d.date)),
        days.last().and_then(|d| nd(&d.date)),
    ) else {
        return vec![];
    };
    let by: HashMap<&str, &Day> = days.iter().map(|d| (d.date.as_str(), d)).collect();
    let mut out = vec![];
    let mut d = a;
    let mut last = days[0].clone();
    while d <= b {
        let k = d.to_string();
        match by.get(k.as_str()) {
            Some(x) => {
                last = (*x).clone();
                out.push(last.clone());
            }
            None => out.push(Day {
                date: k,
                mg: last.mg,
                eg: last.eg,
                sg: last.sg,
                ..Default::default()
            }),
        }
        d += chrono::Duration::days(1);
    }
    out
}

/// Dates on which a streak of `pred` reaches `n` (once per streak), plus current run.
fn streak_hits(days: &[Day], n: usize, pred: impl Fn(&Day) -> bool) -> (Vec<String>, usize) {
    let (mut run, mut hits) = (0usize, vec![]);
    for d in days {
        run = if pred(d) { run + 1 } else { 0 };
        if run == n {
            hits.push(d.date.clone());
        }
    }
    (hits, run)
}

fn longest(days: &[Day], pred: impl Fn(&Day) -> bool) -> (usize, Option<String>) {
    let (mut run, mut best, mut end) = (0, 0, None);
    for d in days {
        run = if pred(d) { run + 1 } else { 0 };
        if run > best && run > 0 {
            best = run;
            end = Some(d.date.clone());
        }
    }
    (best, end)
}

/// Monday–Sunday weeks where every day satisfies `pred`; dated on the Sunday.
fn perfect_weeks(days: &[Day], pred: impl Fn(&Day) -> bool) -> (Vec<String>, usize) {
    let mut weeks: BTreeMap<NaiveDate, (usize, usize, String)> = BTreeMap::new();
    for d in days {
        let Some(x) = nd(&d.date) else { continue };
        let mon = x - chrono::Duration::days(x.weekday().num_days_from_monday() as i64);
        let e = weeks.entry(mon).or_insert((0, 0, String::new()));
        e.0 += 1;
        e.1 += pred(d) as usize;
        e.2 = d.date.clone();
    }
    let cur = weeks
        .values()
        .last()
        .map_or(0, |w| if w.0 == w.1 { w.1 } else { 0 });
    let hits = weeks
        .into_values()
        .filter(|w| w.0 == 7 && w.1 == 7)
        .map(|w| w.2)
        .collect();
    (hits, cur)
}

/// Dates where `val` set a new all-time high after at least `warmup` days of history.
fn records(days: &[Day], warmup: usize, val: impl Fn(&Day) -> f64) -> (Vec<String>, f64) {
    let (mut best, mut hits) = (0.0f64, vec![]);
    for (i, d) in days.iter().enumerate() {
        let v = val(d);
        if v > best {
            if i >= warmup && best > 0.0 {
                hits.push(d.date.clone());
            }
            best = v;
        }
    }
    (hits, best)
}

fn split_words(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && c.is_uppercase() {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

struct B<'a> {
    out: &'a mut Vec<AwardOut>,
}
impl B<'_> {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        key: impl Into<String>,
        name: impl Into<String>,
        group: &'static str,
        desc: impl Into<String>,
        color: &'static str,
        shape: &'static str,
        mark: impl Into<String>,
        dates: Vec<String>,
        progress: Option<(f64, f64)>,
        detail: Option<String>,
    ) {
        let e = earned(dates);
        self.out.push(AwardOut {
            key: key.into(),
            name: name.into(),
            group,
            description: desc.into(),
            color,
            shape,
            mark: mark.into(),
            detail,
            progress: if e.is_some() { None } else { progress },
            earned: e,
        });
    }
}

pub const RINGS: &str = "Close Your Rings";
pub const STREAKS: &str = "Streaks & Perfect Weeks";
pub const WORKOUTS: &str = "Workouts";
pub const RECORDS: &str = "Personal Records";

/// Full award catalog with earned state, computed from data alone.
pub fn compute(days: &[Day], workouts: &[Workout]) -> Vec<AwardOut> {
    let days = dense(days);
    let mut out = vec![];
    let mut b = B { out: &mut out };
    let cm = |d: &Day| d.mg > 0.0 && d.mv >= d.mg;
    let ce = |d: &Day| d.eg > 0.0 && d.ex >= d.eg;
    let cs = |d: &Day| d.sg > 0.0 && d.st >= d.sg;
    let all = |d: &Day| cm(d) && ce(d) && cs(d);
    let when = |p: &dyn Fn(&Day) -> bool| {
        days.iter()
            .filter(|d| p(d))
            .map(|d| d.date.clone())
            .collect::<Vec<_>>()
    };

    // Close your rings + multiples + milestones.
    type Ring<'a> = (&'a str, &'a str, &'static str, &'a dyn Fn(&Day) -> bool);
    let rings: [Ring; 3] = [
        ("move", "Move", "move", &cm),
        ("exercise", "Exercise", "exercise", &ce),
        ("stand", "Stand", "stand", &cs),
    ];
    for (k, n, color, p) in rings {
        let ds = when(p);
        b.add(
            format!("close_{k}"),
            format!("Close Your {n} Ring"),
            RINGS,
            format!("Reach your daily {n} goal."),
            color,
            "ring",
            "",
            ds.clone(),
            Some((0.0, 1.0)),
            None,
        );
        for m in [100usize, 365, 500, 1000] {
            let hit = ds.get(m - 1).cloned().into_iter().collect();
            b.add(
                format!("{k}_goal_{m}"),
                format!("{m} {n} Goals"),
                RINGS,
                format!("Close your {n} ring {m} times."),
                color,
                "hex",
                m.to_string(),
                hit,
                Some((ds.len() as f64, m as f64)),
                None,
            );
        }
    }
    b.add(
        "close_all",
        "Close All Three Rings",
        RINGS,
        "Close Move, Exercise and Stand on the same day.",
        "all",
        "ring",
        "",
        when(&all),
        Some((0.0, 1.0)),
        None,
    );
    let best_mult = days
        .iter()
        .map(|d| if d.mg > 0.0 { d.mv / d.mg } else { 0.0 })
        .fold(0.0, f64::max);
    for m in [2.0, 3.0, 4.0, 5.0] {
        let p = |d: &Day| d.mg > 0.0 && d.mv >= m * d.mg;
        b.add(
            format!("move_{}pct", (m * 100.0) as i64),
            format!("Move Goal {}%", (m * 100.0) as i64),
            RINGS,
            format!("Burn {}× your daily Move goal.", m as i64),
            "move",
            "medal",
            format!("{}%", (m * 100.0) as i64),
            when(&p),
            Some((best_mult * 100.0, m * 100.0)),
            None,
        );
    }

    // Perfect weeks / month and streaks.
    let pw: [Ring; 4] = [
        ("move", "Move", "move", &cm),
        ("exercise", "Exercise", "exercise", &ce),
        ("stand", "Stand", "stand", &cs),
        ("all", "All Activity", "all", &all),
    ];
    for (k, n, color, p) in pw {
        let (hits, cur) = perfect_weeks(&days, p);
        let desc = if k == "all" {
            "Close all three rings every day of a week (Mon–Sun).".to_string()
        } else {
            format!("Close your {n} ring every day of a week (Mon–Sun).")
        };
        b.add(
            format!("perfect_week_{k}"),
            format!("Perfect Week ({n})"),
            STREAKS,
            desc,
            color,
            "hex",
            "7",
            hits,
            Some((cur as f64, 7.0)),
            None,
        );
    }
    let mut months: BTreeMap<String, (u32, usize)> = BTreeMap::new();
    for d in &days {
        let e = months.entry(d.date[..7].to_string()).or_insert((0, 0));
        e.0 += cm(d) as u32;
        e.1 += 1;
    }
    let pm: Vec<String> = months
        .iter()
        .filter(|(m, (c, n))| {
            let first = nd(&format!("{m}-01"));
            let len = first
                .map(|f| (f + chrono::Months::new(1) - f).num_days() as u32)
                .unwrap_or(31);
            *c == len && *n as u32 == len
        })
        .map(|(m, _)| m.clone())
        .collect();
    let cur_m = months.values().last().map_or(0, |v| v.0);
    b.add(
        "perfect_month_move",
        "Perfect Month (Move)",
        STREAKS,
        "Close your Move ring every day of a calendar month.",
        "move",
        "hex",
        "M",
        pm,
        Some((cur_m as f64, 30.0)),
        None,
    );
    let (_, cur_run) = streak_hits(&days, usize::MAX, cm);
    for n in [7usize, 30, 100, 365] {
        let (hits, _) = streak_hits(&days, n, cm);
        b.add(
            format!("move_streak_{n}"),
            format!("{n}-Day Move Streak"),
            STREAKS,
            format!("Close your Move ring {n} days in a row."),
            "move",
            "medal",
            n.to_string(),
            hits,
            Some((cur_run as f64, n as f64)),
            None,
        );
    }
    let (long, end) = longest(&days, cm);
    b.add(
        "longest_move_streak",
        "Longest Move Streak",
        STREAKS,
        "Your longest run of consecutive Move goals.",
        "move",
        "medal",
        long.to_string(),
        if long >= 2 {
            end.into_iter().collect()
        } else {
            vec![]
        },
        Some((long as f64, 2.0)),
        Some(format!("{long} days")),
    );

    // Records.
    type Rec<'a> = (
        &'a str,
        &'a str,
        &'static str,
        &'a str,
        &'a dyn Fn(&Day) -> f64,
    );
    let recs: [Rec; 3] = [
        (
            "record_move",
            "New Move Record",
            "move",
            "CAL",
            &|d: &Day| d.mv,
        ),
        (
            "record_exercise",
            "New Exercise Record",
            "exercise",
            "MIN",
            &|d: &Day| d.ex,
        ),
        (
            "record_steps",
            "New Steps Record",
            "steps",
            "STEPS",
            &|d: &Day| d.steps,
        ),
    ];
    for (k, n, color, unit, f) in recs {
        let (hits, best) = records(&days, 10, f);
        b.add(
            k,
            n,
            RECORDS,
            "Beat your best day ever (after 10 days of history).",
            color,
            "hex",
            "★",
            hits,
            Some((days.len().min(10) as f64, 10.0)),
            (best > 0.0).then(|| format!("{} {unit}", best.round() as i64)),
        );
    }

    // Workouts.
    let mut ws: Vec<&Workout> = workouts.iter().collect();
    ws.sort_by(|a, b| a.date.cmp(&b.date));
    for (n, name) in [
        (1usize, "First Workout".to_string()),
        (10, "10 Workouts".into()),
        (50, "50 Workouts".into()),
        (100, "100 Workouts".into()),
        (250, "250 Workouts".into()),
        (500, "500 Workouts".into()),
        (1000, "1,000 Workouts".into()),
    ] {
        let hit = ws.get(n - 1).map(|w| w.date.clone()).into_iter().collect();
        b.add(
            format!("workouts_{n}"),
            name,
            WORKOUTS,
            format!("Record {n} workout{}.", if n == 1 { "" } else { "s" }),
            "workout",
            if n == 1 { "ring" } else { "hex" },
            n.to_string(),
            hit,
            Some((ws.len() as f64, n as f64)),
            None,
        );
    }
    let mut per_week: BTreeMap<NaiveDate, (usize, String)> = BTreeMap::new();
    for w in &ws {
        if let Some(x) = nd(&w.date) {
            let mon = x - chrono::Duration::days(x.weekday().num_days_from_monday() as i64);
            let e = per_week.entry(mon).or_insert((0, String::new()));
            e.0 += 1;
            if e.0 == 7 {
                e.1 = w.date.clone();
            }
        }
    }
    let cur_w = per_week.values().last().map_or(0, |v| v.0.min(7));
    b.add(
        "seven_workout_week",
        "7-Workout Week",
        WORKOUTS,
        "Record seven workouts in one week (Mon–Sun).",
        "workout",
        "hex",
        "7",
        per_week
            .values()
            .filter(|v| v.0 >= 7)
            .map(|v| v.1.clone())
            .collect(),
        Some((cur_w as f64, 7.0)),
        None,
    );
    let mut by_type: BTreeMap<&str, Vec<&Workout>> = BTreeMap::new();
    for w in &ws {
        by_type.entry(&w.kind).or_default().push(w);
    }
    for (kind, list) in by_type {
        let short = kind.strip_prefix("HKWorkoutActivityType").unwrap_or(kind);
        let label = split_words(short);
        b.add(
            format!("first_{short}"),
            format!("First {label} Workout"),
            WORKOUTS,
            format!("Record your first {label} workout."),
            "workout",
            "ring",
            "1",
            vec![list[0].date.clone()],
            None,
            None,
        );
        let (mut best, mut hits) = (0.0f64, vec![]);
        for (i, w) in list.iter().enumerate() {
            if w.minutes > best {
                if i >= 3 {
                    hits.push(w.date.clone());
                }
                best = w.minutes;
            }
        }
        b.add(
            format!("pr_{short}"),
            format!("Longest {label} Workout"),
            RECORDS,
            format!("Your longest {label} workout yet (after 3 recorded)."),
            "record",
            "hex",
            "PR",
            hits,
            Some((list.len().min(3) as f64, 3.0)),
            Some(format!("{} min", best.round() as i64)),
        );
    }
    out
}

/// Apple Fitness awards healthvibe cannot compute from an export.
pub const APPLE_ONLY: &[(&str, &str, &str)] = &[
    (
        "monthly_challenge",
        "Monthly Challenge",
        "A personal challenge Apple sets each month based on your history.",
    ),
    (
        "limited_edition",
        "Limited Edition Awards",
        "Special awards for completing a workout or goal on a particular day or event.",
    ),
    (
        "special_day",
        "Holiday & Observance Awards",
        "Awards tied to specific days, such as national holidays or awareness days.",
    ),
    (
        "competition_win",
        "Competition Win",
        "Win a 7-day activity competition with a friend.",
    ),
    (
        "competition_done",
        "Competition Completed",
        "Finish a 7-day activity competition with a friend.",
    ),
    (
        "sharing",
        "Activity Sharing Awards",
        "Awards earned through sharing activity with friends.",
    ),
    (
        "service_workouts",
        "Subscription Workout Awards",
        "Milestones for workouts done in Apple's subscription fitness service.",
    ),
];

async fn load(db: &SqlitePool, uid: i64) -> anyhow::Result<(Vec<Day>, Vec<Workout>)> {
    let first: Option<String> = sqlx::query_scalar(
        "SELECT substr(MIN(local_start), 1, 10) FROM readings WHERE user_id = ?",
    )
    .bind(uid)
    .fetch_one(db)
    .await?;
    let Some(end) = crate::stats::latest_day(db, uid).await? else {
        return Ok((vec![], vec![]));
    };
    let start = first.unwrap_or(end.clone());
    let rings = crate::browse::rings_span(db, uid, &start, &end, 20 * 366).await?;
    let span = rings.len() as i64;
    let steps = crate::stats::daily_sum(db, uid, crate::stats::STEPS, &end, span.max(1)).await?;
    let steps: HashMap<String, f64> = steps.into_iter().map(|d| (d.date, d.value)).collect();
    let days = rings
        .into_iter()
        .map(|r| Day {
            steps: steps.get(&r.date).copied().unwrap_or(0.0),
            mv: r.move_kcal,
            mg: r.move_goal,
            ex: r.exercise_min,
            eg: r.exercise_goal,
            st: r.stand_hours,
            sg: r.stand_goal,
            date: r.date,
        })
        .collect();
    let rows: Vec<(String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT substr(local_start, 1, 10), type, value, unit FROM readings WHERE user_id = ? AND type LIKE 'HKWorkoutActivityType%' ORDER BY local_start",
    ).bind(uid).fetch_all(db).await?;
    let workouts = rows
        .into_iter()
        .map(|(date, kind, v, unit)| {
            let v: f64 = v.and_then(|v| v.parse().ok()).unwrap_or(0.0);
            let minutes = match unit.as_deref() {
                Some("s") | Some("sec") => v / 60.0,
                Some("hr") | Some("h") => v * 60.0,
                _ => v,
            };
            Workout {
                date,
                kind,
                minutes,
            }
        })
        .collect();
    Ok((days, workouts))
}

/// Recompute when the user's data changed since last time, store earned awards,
/// and return the catalog.
pub async fn refresh(db: &SqlitePool, uid: i64) -> anyhow::Result<Vec<AwardOut>> {
    let (days, workouts) = load(db, uid).await?;
    let list = compute(&days, &workouts);
    let stamp: String = sqlx::query_scalar(
        // Raw table count on purpose: any goal write (any channel) invalidates the cache.
        "SELECT (SELECT COUNT(*) || ':' || IFNULL(MAX(id), 0) FROM readings WHERE user_id = ?1) || '/' ||
                (SELECT COUNT(*) FROM activity_goals WHERE user_id = ?1)",
    ).bind(uid).fetch_one(db).await.unwrap_or_default();
    let old: Option<String> =
        sqlx::query_scalar("SELECT stamp FROM awards_state WHERE user_id = ?")
            .bind(uid)
            .fetch_optional(db)
            .await?;
    if old.as_deref() != Some(stamp.as_str()) {
        let mut tx = db.begin().await?;
        sqlx::query("DELETE FROM user_awards WHERE user_id = ?")
            .bind(uid)
            .execute(&mut *tx)
            .await?;
        for a in &list {
            if let Some(e) = &a.earned {
                sqlx::query("INSERT INTO user_awards (user_id, key, first_date, latest_date, count, detail) VALUES (?, ?, ?, ?, ?, ?)")
                    .bind(uid).bind(&a.key).bind(&e.first).bind(&e.latest).bind(e.count).bind(&a.detail)
                    .execute(&mut *tx).await?;
            }
        }
        sqlx::query("INSERT INTO awards_state (user_id, stamp) VALUES (?, ?) ON CONFLICT(user_id) DO UPDATE SET stamp = excluded.stamp")
            .bind(uid).bind(&stamp).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(date: &str, mv: f64) -> Day {
        Day {
            date: date.into(),
            mv,
            mg: 100.0,
            ex: 0.0,
            eg: 30.0,
            st: 0.0,
            sg: 12.0,
            steps: 0.0,
        }
    }
    fn days(start: &str, vals: &[f64]) -> Vec<Day> {
        let s = nd(start).unwrap();
        vals.iter()
            .enumerate()
            .map(|(i, v)| day(&(s + chrono::Duration::days(i as i64)).to_string(), *v))
            .collect()
    }
    fn get<'a>(l: &'a [AwardOut], k: &str) -> &'a AwardOut {
        l.iter().find(|a| a.key == k).unwrap()
    }

    #[test]
    fn perfect_week_needs_monday_to_sunday() {
        // 2026-01-05 is a Monday. Days 1..=7 closed, then a miss, then 6 closed.
        let mut v = vec![150.0; 7];
        v.push(10.0);
        v.extend([150.0; 6]);
        let l = compute(&days("2026-01-05", &v), &[]);
        let pw = get(&l, "perfect_week_move").earned.clone().unwrap();
        assert_eq!((pw.count, pw.first.as_str()), (1, "2026-01-11"));
        assert_eq!(
            get(&l, "move_streak_7").earned.as_ref().unwrap().latest,
            "2026-01-11"
        );
        assert!(get(&l, "move_streak_30").earned.is_none());
        assert_eq!(get(&l, "move_streak_30").progress, Some((6.0, 30.0)));
        assert_eq!(
            get(&l, "longest_move_streak").detail.as_deref(),
            Some("7 days")
        );
        assert_eq!(get(&l, "close_move").earned.as_ref().unwrap().count, 13);
        assert!(get(&l, "perfect_week_exercise").earned.is_none());
    }

    #[test]
    fn starting_midweek_is_not_a_perfect_week() {
        // Wed..Tue: 7 consecutive days but no full Mon–Sun week.
        let l = compute(&days("2026-01-07", &[150.0; 7]), &[]);
        assert!(get(&l, "perfect_week_move").earned.is_none());
        assert!(get(&l, "move_streak_7").earned.is_some());
    }

    #[test]
    fn gaps_break_streaks_and_multiples() {
        let mut d = days("2026-01-05", &[150.0; 4]);
        d.extend(days("2026-01-10", &[520.0; 4])); // gap on the 9th
        let l = compute(&d, &[]);
        assert!(get(&l, "move_streak_7").earned.is_none());
        assert_eq!(get(&l, "move_500pct").earned.as_ref().unwrap().count, 4);
        assert!(get(&l, "move_200pct").earned.is_some());
    }

    #[test]
    fn records_need_history() {
        let mut v = vec![100.0; 12];
        v[3] = 300.0; // too early to count
        v[11] = 400.0;
        let l = compute(&days("2026-01-01", &v), &[]);
        let r = get(&l, "record_move").earned.clone().unwrap();
        assert_eq!((r.count, r.latest.as_str()), (1, "2026-01-12"));
        assert_eq!(get(&l, "record_move").detail.as_deref(), Some("400 CAL"));
    }

    #[test]
    fn workout_awards() {
        let w: Vec<Workout> = (0..8)
            .map(|i| Workout {
                date: format!("2026-01-{:02}", 5 + i.min(6)),
                kind: "HKWorkoutActivityTypeRunning".into(),
                minutes: 20.0 + i as f64,
            })
            .collect();
        let l = compute(&days("2026-01-05", &[0.0; 7]), &w);
        assert_eq!(
            get(&l, "workouts_1").earned.as_ref().unwrap().first,
            "2026-01-05"
        );
        assert_eq!(
            get(&l, "seven_workout_week")
                .earned
                .as_ref()
                .unwrap()
                .latest,
            "2026-01-11"
        );
        assert_eq!(get(&l, "first_Running").name, "First Running Workout");
        assert_eq!(get(&l, "pr_Running").earned.as_ref().unwrap().count, 5);
        assert_eq!(get(&l, "workouts_10").progress, Some((8.0, 10.0)));
    }
}
