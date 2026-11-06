//! User-set Activity ring goals (history kept). Export ActivitySummary goals
//! outrank these; see [`crate::stats::goals_for`].

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ManualGoal {
    pub id: i64,
    pub effective_from: String,
    pub move_kcal: Option<f64>,
    pub exercise_min: Option<f64>,
    pub stand_hours: Option<f64>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ManualGoalInput {
    pub effective_from: String,
    #[serde(default)]
    pub move_kcal: Option<f64>,
    #[serde(default)]
    pub exercise_min: Option<f64>,
    #[serde(default)]
    pub stand_hours: Option<f64>,
}

fn date_ok(s: &str) -> bool {
    s.len() == 10 && chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

fn ranges_ok(i: &ManualGoalInput) -> bool {
    let move_ok = i.move_kcal.is_none_or(|v| (0.0..=20_000.0).contains(&v));
    let ex_ok = i.exercise_min.is_none_or(|v| (0.0..=1_440.0).contains(&v));
    let st_ok = i.stand_hours.is_none_or(|v| (0.0..=24.0).contains(&v));
    move_ok && ex_ok && st_ok
}

/// True when at least one ring value is set.
fn any_ring(i: &ManualGoalInput) -> bool {
    i.move_kcal.is_some() || i.exercise_min.is_some() || i.stand_hours.is_some()
}

pub fn validate(i: &ManualGoalInput) -> Result<(), &'static str> {
    if !date_ok(&i.effective_from) {
        return Err("invalid_date");
    }
    if !ranges_ok(i) {
        return Err("invalid_goal");
    }
    if !any_ring(i) {
        return Err("empty_goal");
    }
    Ok(())
}

pub async fn list(db: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<ManualGoal>> {
    Ok(
        sqlx::query_as::<_, (i64, String, Option<f64>, Option<f64>, Option<f64>, String)>(
            "SELECT id, effective_from, move_kcal, exercise_min, stand_hours, created_at
         FROM manual_goals WHERE user_id = ? ORDER BY effective_from DESC, id DESC",
        )
        .bind(user_id)
        .fetch_all(db)
        .await?
        .into_iter()
        .map(
            |(id, effective_from, move_kcal, exercise_min, stand_hours, created_at)| ManualGoal {
                id,
                effective_from,
                move_kcal,
                exercise_min,
                stand_hours,
                created_at,
            },
        )
        .collect(),
    )
}

pub async fn create(
    db: &SqlitePool,
    user_id: i64,
    i: &ManualGoalInput,
) -> anyhow::Result<ManualGoal> {
    validate(i).map_err(anyhow::Error::msg)?;
    let now = Utc::now().to_rfc3339();
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO manual_goals (user_id, effective_from, move_kcal, exercise_min, stand_hours, created_at)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(user_id)
    .bind(&i.effective_from)
    .bind(i.move_kcal)
    .bind(i.exercise_min)
    .bind(i.stand_hours)
    .bind(&now)
    .fetch_one(db)
    .await?;
    Ok(ManualGoal {
        id,
        effective_from: i.effective_from.clone(),
        move_kcal: i.move_kcal,
        exercise_min: i.exercise_min,
        stand_hours: i.stand_hours,
        created_at: now,
    })
}

pub async fn update(
    db: &SqlitePool,
    user_id: i64,
    id: i64,
    i: &ManualGoalInput,
) -> anyhow::Result<Option<ManualGoal>> {
    validate(i).map_err(anyhow::Error::msg)?;
    let n = sqlx::query(
        "UPDATE manual_goals SET effective_from = ?, move_kcal = ?, exercise_min = ?, stand_hours = ?
         WHERE id = ? AND user_id = ?",
    )
    .bind(&i.effective_from)
    .bind(i.move_kcal)
    .bind(i.exercise_min)
    .bind(i.stand_hours)
    .bind(id)
    .bind(user_id)
    .execute(db)
    .await?
    .rows_affected();
    if n == 0 {
        return Ok(None);
    }
    Ok(
        sqlx::query_as::<_, (i64, String, Option<f64>, Option<f64>, Option<f64>, String)>(
            "SELECT id, effective_from, move_kcal, exercise_min, stand_hours, created_at
         FROM manual_goals WHERE id = ? AND user_id = ?",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .map(
            |(id, effective_from, move_kcal, exercise_min, stand_hours, created_at)| ManualGoal {
                id,
                effective_from,
                move_kcal,
                exercise_min,
                stand_hours,
                created_at,
            },
        ),
    )
}

pub async fn delete(db: &SqlitePool, user_id: i64, id: i64) -> anyhow::Result<bool> {
    let n = sqlx::query("DELETE FROM manual_goals WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_id)
        .execute(db)
        .await?
        .rows_affected();
    Ok(n > 0)
}

/// Rows for goal resolution: `(effective_from, move, exercise, stand)`.
pub async fn rows_upto(
    db: &SqlitePool,
    user_id: i64,
    end: &str,
) -> anyhow::Result<Vec<(String, Option<f64>, Option<f64>, Option<f64>)>> {
    Ok(sqlx::query_as(
        "SELECT effective_from, move_kcal, exercise_min, stand_hours FROM manual_goals
         WHERE user_id = ? AND effective_from <= ? ORDER BY effective_from, id",
    )
    .bind(user_id)
    .bind(end)
    .fetch_all(db)
    .await?)
}
