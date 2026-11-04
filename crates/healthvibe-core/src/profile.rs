//! Optional per-user profile: details, avatar config, goal overrides, units.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct Profile {
    pub display_name: Option<String>,
    pub birth_date: Option<String>,
    /// female | male | other
    pub sex: Option<String>,
    pub height_cm: Option<f64>,
    pub wheelchair: Option<bool>,
    /// Avatar builder config (opaque JSON object rendered client-side).
    pub avatar: Option<Value>,
    pub move_goal: Option<f64>,
    pub exercise_goal: Option<f64>,
    pub stand_goal: Option<f64>,
    /// {distance: mi|km, weight: lb|kg, energy: cal|kj, temp: f|c}
    pub units: Option<Value>,
}

type Row = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<bool>,
    Option<String>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<String>,
);

pub async fn get(db: &SqlitePool, uid: i64) -> anyhow::Result<Profile> {
    let r: Option<Row> = sqlx::query_as(
        "SELECT display_name, birth_date, sex, height_cm, wheelchair, avatar, move_goal, exercise_goal, stand_goal, units FROM profiles WHERE user_id = ?",
    ).bind(uid).fetch_optional(db).await?;
    let Some(r) = r else {
        return Ok(Profile::default());
    };
    let json = |s: Option<String>| s.and_then(|s| serde_json::from_str(&s).ok());
    Ok(Profile {
        display_name: r.0,
        birth_date: r.1,
        sex: r.2,
        height_cm: r.3,
        wheelchair: r.4,
        avatar: json(r.5),
        move_goal: r.6,
        exercise_goal: r.7,
        stand_goal: r.8,
        units: json(r.9),
    })
}

/// Validate and normalise; returns an error code on bad input.
pub fn validate(mut p: Profile) -> Result<Profile, &'static str> {
    let trim = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    p.display_name = trim(p.display_name);
    p.birth_date = trim(p.birth_date);
    p.sex = trim(p.sex);
    if p.display_name
        .as_ref()
        .is_some_and(|n| n.chars().count() > 60)
    {
        return Err("invalid_profile");
    }
    if let Some(d) = &p.birth_date {
        let ok = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok_and(|x| x.year_ok());
        if !ok {
            return Err("invalid_profile");
        }
    }
    if p.sex
        .as_deref()
        .is_some_and(|s| !["female", "male", "other"].contains(&s))
    {
        return Err("invalid_profile");
    }
    let range = |v: Option<f64>, lo: f64, hi: f64| {
        v.is_none_or(|v| v.is_finite() && (lo..=hi).contains(&v))
    };
    if !range(p.height_cm, 30.0, 272.0)
        || !range(p.move_goal, 10.0, 10_000.0)
        || !range(p.exercise_goal, 1.0, 1440.0)
        || !range(p.stand_goal, 1.0, 24.0)
    {
        return Err("invalid_profile");
    }
    for v in [&p.avatar, &p.units] {
        if v.as_ref()
            .is_some_and(|v| !v.is_object() || v.to_string().len() > 4000)
        {
            return Err("invalid_profile");
        }
    }
    Ok(p)
}

trait YearOk {
    fn year_ok(&self) -> bool;
}
impl YearOk for chrono::NaiveDate {
    fn year_ok(&self) -> bool {
        use chrono::Datelike;
        (1900..=2100).contains(&self.year())
    }
}

pub async fn put(db: &SqlitePool, uid: i64, p: &Profile) -> anyhow::Result<()> {
    let s = |v: &Option<Value>| v.as_ref().map(|v| v.to_string());
    sqlx::query(
        "INSERT INTO profiles (user_id, display_name, birth_date, sex, height_cm, wheelchair, avatar, move_goal, exercise_goal, stand_goal, units)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET display_name = excluded.display_name, birth_date = excluded.birth_date, sex = excluded.sex,
           height_cm = excluded.height_cm, wheelchair = excluded.wheelchair, avatar = excluded.avatar, move_goal = excluded.move_goal,
           exercise_goal = excluded.exercise_goal, stand_goal = excluded.stand_goal, units = excluded.units",
    )
    .bind(uid).bind(&p.display_name).bind(&p.birth_date).bind(&p.sex).bind(p.height_cm).bind(p.wheelchair)
    .bind(s(&p.avatar)).bind(p.move_goal).bind(p.exercise_goal).bind(p.stand_goal).bind(s(&p.units))
    .execute(db).await?;
    Ok(())
}

pub async fn goal_overrides(
    db: &SqlitePool,
    uid: i64,
) -> anyhow::Result<(Option<f64>, Option<f64>, Option<f64>)> {
    Ok(sqlx::query_as(
        "SELECT move_goal, exercise_goal, stand_goal FROM profiles WHERE user_id = ?",
    )
    .bind(uid)
    .fetch_optional(db)
    .await?
    .unwrap_or((None, None, None)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation() {
        assert!(
            validate(Profile {
                display_name: Some("  Toby ".into()),
                ..Default::default()
            })
            .unwrap()
            .display_name
            .as_deref()
                == Some("Toby")
        );
        assert!(validate(Profile {
            birth_date: Some("1890-01-01".into()),
            ..Default::default()
        })
        .is_err());
        assert!(validate(Profile {
            sex: Some("x".into()),
            ..Default::default()
        })
        .is_err());
        assert!(validate(Profile {
            stand_goal: Some(30.0),
            ..Default::default()
        })
        .is_err());
        assert!(validate(Profile {
            avatar: Some(serde_json::json!([1])),
            ..Default::default()
        })
        .is_err());
    }
}
