//! Optional per-user profile: details, avatar config, units, IANA time zone.
//! Ring goals are never set here: they come only from Apple ActivitySummary data.

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
    /// {distance: mi|km, weight: lb|kg, energy: cal|kj, temp: f|c}
    pub units: Option<Value>,
    /// IANA time zone, e.g. "America/New_York". Used by seed-demo for "today";
    /// imported Health local dates are never rewritten from this.
    pub timezone: Option<String>,
}

type Row = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<bool>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub async fn get(db: &SqlitePool, uid: i64) -> anyhow::Result<Profile> {
    let r: Option<Row> = sqlx::query_as(
        "SELECT display_name, birth_date, sex, height_cm, wheelchair, avatar, units, timezone FROM profiles WHERE user_id = ?",
    )
    .bind(uid)
    .fetch_optional(db)
    .await?;
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
        units: json(r.6),
        timezone: r.7,
    })
}

/// Validate and normalise; returns an error code on bad input.
pub fn validate(mut p: Profile) -> Result<Profile, &'static str> {
    let trim = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    p.display_name = trim(p.display_name);
    p.birth_date = trim(p.birth_date);
    p.sex = trim(p.sex);
    p.timezone = trim(p.timezone);
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
    if !range(p.height_cm, 30.0, 272.0) {
        return Err("invalid_profile");
    }
    for v in [&p.avatar, &p.units] {
        if v.as_ref()
            .is_some_and(|v| !v.is_object() || v.to_string().len() > 4000)
        {
            return Err("invalid_profile");
        }
    }
    if let Some(tz) = &p.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() || tz.len() > 64 {
            return Err("invalid_timezone");
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
        "INSERT INTO profiles (user_id, display_name, birth_date, sex, height_cm, wheelchair, avatar, units, timezone)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET display_name = excluded.display_name, birth_date = excluded.birth_date, sex = excluded.sex,
           height_cm = excluded.height_cm, wheelchair = excluded.wheelchair, avatar = excluded.avatar, units = excluded.units,
           timezone = excluded.timezone",
    )
    .bind(uid)
    .bind(&p.display_name)
    .bind(&p.birth_date)
    .bind(&p.sex)
    .bind(p.height_cm)
    .bind(p.wheelchair)
    .bind(s(&p.avatar))
    .bind(s(&p.units))
    .bind(&p.timezone)
    .execute(db)
    .await?;
    Ok(())
}

/// Today's calendar date in the given IANA zone, or local process date if unset/invalid.
pub fn today_in(timezone: Option<&str>) -> chrono::NaiveDate {
    use chrono::{Local, Utc};
    if let Some(tz) = timezone.and_then(|s| s.parse::<chrono_tz::Tz>().ok()) {
        return Utc::now().with_timezone(&tz).date_naive();
    }
    Local::now().date_naive()
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
            height_cm: Some(500.0),
            ..Default::default()
        })
        .is_err());
        assert!(validate(Profile {
            avatar: Some(serde_json::json!([1])),
            ..Default::default()
        })
        .is_err());
        assert!(validate(Profile {
            timezone: Some("America/New_York".into()),
            ..Default::default()
        })
        .is_ok());
        assert_eq!(
            validate(Profile {
                timezone: Some("Not/A_Zone".into()),
                ..Default::default()
            }),
            Err("invalid_timezone")
        );
    }

    #[test]
    fn today_in_known_zone() {
        let d = today_in(Some("UTC"));
        assert_eq!(d, chrono::Utc::now().date_naive());
    }
}
