//! Custom dashboards: CRUD, validation, seeded default, metric listing and
//! per-user isolation. Synthetic data only.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use healthvibe_core::config::Config;
use healthvibe_server::{router, AppState};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn app() -> (Router, sqlx::SqlitePool) {
    let db = healthvibe_core::db::open_memory().await.unwrap();
    let mut config = Config::from_env();
    config.web_dir = std::env::temp_dir().join("hv-no-web");
    (
        router(AppState {
            config,
            db: db.clone(),
        }),
        db,
    )
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, c);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let v = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, v, cookie)
}

/// Admin set up, plus a second user "sam" who has changed their password.
async fn two_users(app: &Router) -> (String, String) {
    let (s, v, admin) = call(
        app,
        "POST",
        "/api/v1/setup",
        None,
        Some(json!({"username": "Toby", "password": "admin password 1"})),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["user"]["username"], "toby");
    assert_eq!(v["user"]["is_admin"], true);
    let admin = admin.unwrap();
    let (s, _, _) = call(
        app,
        "POST",
        "/api/v1/admin/users",
        Some(&admin),
        Some(json!({"username": "sam", "temporary_password": "temporary pass"})),
    )
    .await;
    assert_eq!(s, 201);
    let (_, _, sam) = call(
        app,
        "POST",
        "/api/v1/login",
        None,
        Some(json!({"username": "sam", "password": "temporary pass"})),
    )
    .await;
    let sam = sam.unwrap();
    let (s, _, _) = call(
        app,
        "POST",
        "/api/v1/me/password",
        Some(&sam),
        Some(json!({"current": "temporary pass", "new": "sam's own password"})),
    )
    .await;
    assert_eq!(s, 200);
    (admin, sam)
}

async fn reading(db: &sqlx::SqlitePool, uid: i64, kind: &str, v: &str, day: &str, n: i64) {
    sqlx::query(
        "INSERT INTO readings (user_id, type, value, unit, start_utc, end_utc, source, channel, hash, local_start, local_end)
         VALUES (?, ?, ?, 'count', ?, ?, '', 'unknown', ?, ?, ?)",
    )
    .bind(uid).bind(kind).bind(v)
    .bind(format!("{day}T08:{n:02}:00Z")).bind(format!("{day}T08:{n:02}:10Z"))
    .bind(format!("{uid}-{kind}-{day}-{n}"))
    .bind(format!("{day} 08:{n:02}:00")).bind(format!("{day} 08:{n:02}:10"))
    .execute(db).await.unwrap();
}

#[tokio::test]
async fn crud_and_seeded_default() {
    let (app, _) = app().await;
    let (admin, _) = two_users(&app).await;
    let c = Some(admin.as_str());
    let (s, v, _) = call(&app, "GET", "/api/v1/dashboards", c, None).await;
    assert_eq!(s, 200);
    let list = v["dashboards"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "Summary");
    assert_eq!(list[0]["builtin"], "summary");
    assert_eq!(
        list[0]["widgets"].as_array().unwrap().len(),
        0,
        "new Summary is blank"
    );
    let summary_id = list[0]["id"].as_i64().unwrap();
    let (s, _, _) = call(
        &app,
        "DELETE",
        &format!("/api/v1/dashboards/{summary_id}"),
        c,
        None,
    )
    .await;
    assert_eq!(s, 400, "Summary cannot be deleted");
    let (s, _, _) = call(
        &app,
        "PUT",
        &format!("/api/v1/dashboards/{summary_id}"),
        c,
        Some(json!({"name": "Renamed"})),
    )
    .await;
    assert_eq!(s, 400, "Summary cannot be renamed");

    let w = json!([{"id":"a","kind":"line","metric":"healthvibe.hae.tooth_brushing","days":90,"bucket":"week","size":2}]);
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/dashboards",
        c,
        Some(json!({"name":" Training ","widgets":w})),
    )
    .await;
    assert_eq!(s, 201);
    let id = v["dashboard"]["id"].as_i64().unwrap();
    assert_eq!(v["dashboard"]["name"], "Training");
    assert_eq!(v["dashboard"]["widgets"][0]["bucket"], "week");

    let (s, v, _) = call(
        &app,
        "PUT",
        &format!("/api/v1/dashboards/{id}"),
        c,
        Some(json!({"name":"Run"})),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["dashboard"]["name"], "Run");
    assert_eq!(
        v["dashboard"]["widgets"].as_array().unwrap().len(),
        1,
        "rename keeps widgets"
    );

    for bad in [
        json!({"widgets":[{"id":"x","kind":"pie","metric":"m"}]}),
        json!({"widgets":[{"id":"x","kind":"line"}]}),
        json!({"widgets":[{"id":"x","kind":"bar","metric":"m","size":9}]}),
        json!({"widgets":[{"id":"x","kind":"bar","metric":"m","bucket":"year"}]}),
        json!({"name":""}),
    ] {
        let (s, _, _) = call(
            &app,
            "PUT",
            &format!("/api/v1/dashboards/{id}"),
            c,
            Some(bad),
        )
        .await;
        assert_eq!(s, 400);
    }
    let (s, _, _) = call(&app, "DELETE", &format!("/api/v1/dashboards/{id}"), c, None).await;
    assert_eq!(s, 200);
    let (s, _, _) = call(&app, "GET", &format!("/api/v1/dashboards/{id}"), c, None).await;
    assert_eq!(s, 404);
}

#[tokio::test]
async fn dashboards_and_metrics_are_per_user() {
    let (app, db) = app().await;
    let (admin, sam) = two_users(&app).await;
    let (_, v, _) = call(
        &app,
        "POST",
        "/api/v1/dashboards",
        Some(&admin),
        Some(json!({"name":"Mine"})),
    )
    .await;
    let id = v["dashboard"]["id"].as_i64().unwrap();
    let path = format!("/api/v1/dashboards/{id}");
    assert_eq!(call(&app, "GET", &path, Some(&sam), None).await.0, 404);
    assert_eq!(
        call(
            &app,
            "PUT",
            &path,
            Some(&sam),
            Some(json!({"name":"pwned"}))
        )
        .await
        .0,
        404
    );
    assert_eq!(call(&app, "DELETE", &path, Some(&sam), None).await.0, 404);
    assert_eq!(
        call(&app, "GET", &path, Some(&admin), None).await.1["dashboard"]["name"],
        "Mine"
    );
    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&sam), None).await;
    assert_eq!(
        v["dashboards"].as_array().unwrap().len(),
        1,
        "sam only sees his seeded Summary"
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/dashboards", None, None).await.0,
        401
    );

    reading(
        &db,
        1,
        "HKQuantityTypeIdentifierStepCount",
        "1000",
        "2026-01-05",
        1,
    )
    .await;
    reading(
        &db,
        1,
        "HKQuantityTypeIdentifierStepCount",
        "500",
        "2026-01-05",
        2,
    )
    .await;
    reading(
        &db,
        1,
        "HKQuantityTypeIdentifierStepCount",
        "2000",
        "2026-01-07",
        1,
    )
    .await;
    reading(
        &db,
        1,
        "healthvibe.hae.tooth_brushing",
        "2",
        "2026-01-07",
        1,
    )
    .await;
    let (_, v, _) = call(&app, "GET", "/api/v1/metrics", Some(&admin), None).await;
    let m = v["metrics"].as_array().unwrap();
    assert_eq!(m.len(), 2);
    let steps = m
        .iter()
        .find(|x| x["metric"] == "HKQuantityTypeIdentifierStepCount")
        .unwrap();
    assert_eq!(steps["count"], 3);
    assert_eq!(steps["agg"], "sum");
    assert_eq!(steps["unit"], "count");
    assert!(m.iter().any(|x| x["name"] == "Tooth Brushing"));
    let (_, v, _) = call(&app, "GET", "/api/v1/metrics", Some(&sam), None).await;
    assert_eq!(v["metrics"].as_array().unwrap().len(), 0);

    let q = "/api/v1/series?metric=HKQuantityTypeIdentifierStepCount&days=30";
    let (_, v, _) = call(&app, "GET", q, Some(&admin), None).await;
    assert_eq!(
        v["points"],
        json!([{"date":"2026-01-05","value":1500.0},{"date":"2026-01-07","value":2000.0}])
    );
    let (_, v, _) = call(&app, "GET", &format!("{q}&bucket=week"), Some(&admin), None).await;
    assert_eq!(v["points"], json!([{"date":"2026-01-05","value":3500.0}]));
    let (_, v, _) = call(
        &app,
        "GET",
        &format!("{q}&bucket=week&agg=avg"),
        Some(&admin),
        None,
    )
    .await;
    // avg = mean of daily sample averages: (750 + 2000) / 2
    assert_eq!(v["points"][0]["value"], 1375.0);
    let (_, v, _) = call(&app, "GET", q, Some(&sam), None).await;
    assert_eq!(v["points"], json!([]));
    assert_eq!(
        call(&app, "GET", &format!("{q}&agg=max"), Some(&admin), None)
            .await
            .0,
        400
    );
}

#[tokio::test]
async fn seeded_summary_clears_but_custom_widgets_and_pins_stay() {
    let (app, db) = app().await;
    let (admin, sam) = two_users(&app).await;
    let seeded = serde_json::to_string(&healthvibe_core::dashboards::default_widgets()).unwrap();
    sqlx::query(
        "INSERT INTO dashboards (user_id, name, position, widgets_json, created_at, updated_at)
         VALUES (1, 'Summary', 0, ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&seeded)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pinned_metrics (user_id, metric, position) VALUES (1, 'HKQuantityTypeIdentifierStepCount', 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let custom =
        json!([{"id":"keep","kind":"activity","days":30,"bucket":"day","size":2}]).to_string();
    sqlx::query(
        "INSERT INTO dashboards (user_id, name, position, widgets_json, created_at, updated_at)
         VALUES (2, 'Summary', 0, ?, '2026-01-01T00:00:00Z', '2026-02-01T00:00:00Z')",
    )
    .bind(&custom)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO pinned_metrics (user_id, metric, position) VALUES (2, 'HKQuantityTypeIdentifierBodyMass', 0)",
    )
    .execute(&db)
    .await
    .unwrap();

    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&admin), None).await;
    let list = v["dashboards"].as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["builtin"], "summary");
    assert_eq!(
        list[0]["widgets"][0]["metric"],
        "HKQuantityTypeIdentifierStepCount"
    );
    assert_eq!(
        list[0]["widgets"].as_array().unwrap().len(),
        1,
        "stored pins become cards once"
    );

    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&sam), None).await;
    assert_eq!(
        v["dashboards"][0]["widgets"][0]["id"], "keep",
        "edited Summary is not wiped"
    );
    assert_eq!(
        v["dashboards"][0]["widgets"][1]["metric"], "HKQuantityTypeIdentifierBodyMass",
        "pins are appended onto an edited Summary"
    );
    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&sam), None).await;
    assert_eq!(
        v["dashboards"][0]["widgets"].as_array().unwrap().len(),
        2,
        "pin import does not repeat"
    );

    let (_, v, _) = call(&app, "GET", "/api/v1/nav", Some(&admin), None).await;
    let kinds: Vec<&str> = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["summary", "fitness", "health", "add"]);

    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/dashboards",
        Some(&admin),
        Some(json!({"name": "Night", "widgets": []})),
    )
    .await;
    assert_eq!(s, 201);
    assert_eq!(v["dashboard"]["widgets"].as_array().unwrap().len(), 0);
    let new_id = v["dashboard"]["id"].as_i64().unwrap();
    let (_, v, _) = call(&app, "GET", "/api/v1/nav", Some(&admin), None).await;
    let items = v["items"].as_array().unwrap();
    assert_eq!(items[items.len() - 2]["id"], new_id);
    assert_eq!(items.last().unwrap()["kind"], "add");
    assert_eq!(
        call(&app, "GET", "/api/v1/nav", Some(&sam), None).await.1["items"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/nav",
        Some(&sam),
        Some(json!({"items":[{"kind":"summary"},{"kind":"health"},{"kind":"fitness"},{"kind":"add"}]})),
    )
    .await;
    assert_eq!(s, 200);
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/nav",
        Some(&sam),
        Some(json!({"items":[{"kind":"fitness"},{"kind":"health"},{"kind":"add"}]})),
    )
    .await;
    assert_eq!(s, 400, "Summary cannot leave the nav");
    let (s, _, _) = call(
        &app,
        "PUT",
        "/api/v1/nav",
        Some(&sam),
        Some(json!({"items":[
            {"kind":"summary"},
            {"kind":"fitness"},
            {"kind":"health"},
            {"kind":"dashboard","id": new_id},
            {"kind":"add"}
        ]})),
    )
    .await;
    assert_eq!(s, 400, "another user's dashboard cannot enter the nav");
    let (_, v, _) = call(&app, "GET", "/api/v1/nav", Some(&admin), None).await;
    assert_eq!(
        v["items"][v["items"].as_array().unwrap().len() - 2]["id"],
        new_id
    );
}

#[tokio::test]
async fn pins_without_a_dashboard_become_summary_cards_once() {
    let (app, db) = app().await;
    let (admin, _) = two_users(&app).await;
    sqlx::query(
        "INSERT INTO pinned_metrics (user_id, metric, position) VALUES (1, 'HKQuantityTypeIdentifierStepCount', 0)",
    )
    .execute(&db)
    .await
    .unwrap();
    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&admin), None).await;
    assert_eq!(v["dashboards"][0]["builtin"], "summary");
    assert_eq!(
        v["dashboards"][0]["widgets"][0]["metric"],
        "HKQuantityTypeIdentifierStepCount"
    );
    let id = v["dashboards"][0]["id"].as_i64().unwrap();
    let (s, _, _) = call(
        &app,
        "PUT",
        &format!("/api/v1/dashboards/{id}"),
        Some(&admin),
        Some(json!({"widgets": []})),
    )
    .await;
    assert_eq!(s, 200);
    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&admin), None).await;
    assert_eq!(
        v["dashboards"][0]["widgets"].as_array().unwrap().len(),
        0,
        "clearing Summary does not restore pins"
    );
    let (s, v, _) = call(
        &app,
        "POST",
        "/api/v1/dashboards",
        Some(&admin),
        Some(json!({"name": "Night"})),
    )
    .await;
    assert_eq!(s, 201);
    assert_eq!(v["dashboard"]["widgets"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn renamed_seed_is_kept_and_a_blank_summary_is_added() {
    let (app, db) = app().await;
    let (admin, _) = two_users(&app).await;
    let seeded = serde_json::to_string(&healthvibe_core::dashboards::default_widgets()).unwrap();
    sqlx::query(
        "INSERT INTO dashboards (user_id, name, position, widgets_json, created_at, updated_at)
         VALUES (1, 'Training', 0, ?, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(&seeded)
    .execute(&db)
    .await
    .unwrap();
    let (_, v, _) = call(&app, "GET", "/api/v1/dashboards", Some(&admin), None).await;
    let list = v["dashboards"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    let summary = list.iter().find(|d| d["builtin"] == "summary").unwrap();
    let training = list.iter().find(|d| d["name"] == "Training").unwrap();
    assert_eq!(summary["widgets"].as_array().unwrap().len(), 0);
    assert_eq!(
        training["widgets"].as_array().unwrap().len(),
        healthvibe_core::dashboards::default_widgets().len(),
        "a renamed copy of the seed is not cleared"
    );
    assert!(training["builtin"].is_null());
}
