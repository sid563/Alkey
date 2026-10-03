//! Full assignment-flow integration test.
//!
//! Requires the whole stack running locally:
//!   docker compose up -d && cargo run
//! Then: cargo test -- --ignored
//!
//! The test resets task/challenge/email state (and the Redis cache) first so
//! it is deterministic across repeated runs.

use serde_json::{json, Value};

const API: &str = "http://127.0.0.1:8080";

async fn reset_state() {
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/taskdb".into());
    let pool = sqlx::postgres::PgPool::connect(&db_url)
        .await
        .expect("connect to postgres (is docker compose up?)");
    sqlx::query("TRUNCATE tasks, login_challenges, email_logs")
        .execute(&pool)
        .await
        .unwrap();

    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let client = redis::Client::open(redis_url).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    redis::cmd("FLUSHDB")
        .query_async::<()>(&mut conn)
        .await
        .unwrap();
}

/// Pull the 6-digit code out of the email body: the first run of exactly
/// six consecutive digits (a naive "first 6 digits" grabs the '2' in "2FA").
fn extract_code(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        if bytes[start].is_ascii_digit() {
            let end = bytes[start..]
                .iter()
                .position(|b| !b.is_ascii_digit())
                .map_or(bytes.len(), |p| start + p);
            if end - start == 6 {
                return body[start..end].to_string();
            }
            start = end;
        } else {
            start += 1;
        }
    }
    panic!("no 6-digit code in email body: {body}");
}

async fn login(client: &reqwest::Client, email: &str, password: &str) -> String {
    let res: Value = client
        .post(format!("{API}/auth/login"))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let challenge_id = res["login_challenge_id"].as_str().unwrap().to_string();

    let log: Value = client
        .get(format!("{API}/dev/email-logs/latest?email={email}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = extract_code(log["body"].as_str().unwrap());

    let verify: Value = client
        .post(format!("{API}/auth/verify-2fa"))
        .json(&json!({ "login_challenge_id": challenge_id, "code": code }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    verify["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
#[ignore = "requires running API + Postgres + Redis"]
async fn full_assignment_flow() {
    reset_state().await;
    let client = reqwest::Client::new();

    // Seed users.
    let res = client
        .post(format!("{API}/seed/users"))
        .send()
        .await
        .expect("is the API running on :8080?");
    assert!(res.status().is_success());

    // Login must return a challenge, never a token.
    let login_res: Value = client
        .post(format!("{API}/auth/login"))
        .json(&json!({ "email": "admin@example.com", "password": "Admin@123" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(login_res["login_challenge_id"].is_string());
    assert!(login_res.get("access_token").is_none());

    // Wrong code is rejected (and does not consume the challenge).
    let challenge_id = login_res["login_challenge_id"].as_str().unwrap();
    let bad = client
        .post(format!("{API}/auth/verify-2fa"))
        .json(&json!({ "login_challenge_id": challenge_id, "code": "000000" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);

    // Correct code works; reuse is rejected.
    let log: Value = client
        .get(format!(
            "{API}/dev/email-logs/latest?email=admin@example.com"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = extract_code(log["body"].as_str().unwrap());
    let ok = client
        .post(format!("{API}/auth/verify-2fa"))
        .json(&json!({ "login_challenge_id": challenge_id, "code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    let admin_token = ok.json::<Value>().await.unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string();
    let reuse = client
        .post(format!("{API}/auth/verify-2fa"))
        .json(&json!({ "login_challenge_id": challenge_id, "code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(reuse.status(), 401, "codes must be one-time use");

    // Create exactly 5 tasks as admin.
    let mut task_ids = Vec::new();
    for (i, p) in ["high", "medium", "low", "high", "medium"]
        .iter()
        .enumerate()
    {
        let task: Value = client
            .post(format!("{API}/tasks"))
            .bearer_auth(&admin_token)
            .json(&json!({
                "title": format!("Task-{}-{p}", i + 1),
                "description": format!("desc {}", i + 1),
                "status": "todo",
                "priority": p
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        task_ids.push(task["id"].as_str().unwrap().to_string());
    }
    assert_eq!(task_ids.len(), 5);

    // Assign exactly 3 to James Bond.
    let assign = client
        .post(format!("{API}/tasks/assign"))
        .bearer_auth(&admin_token)
        .json(&json!({ "task_ids": task_ids[..3], "assignee_email": "jamesbond@example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(assign.status(), 200);

    // James logs in; creating a task must be 403.
    let james_token = login(&client, "jamesbond@example.com", "Bond@123").await;
    let forbidden = client
        .post(format!("{API}/tasks"))
        .bearer_auth(&james_token)
        .json(&json!({ "title": "Nope", "description": "", "status": "todo", "priority": "low" }))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 403);

    // First view: 3 tasks, cache miss. Second: cache hit.
    let view1: Value = client
        .get(format!("{API}/tasks/view-my-tasks"))
        .bearer_auth(&james_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(view1["summary"]["total_assigned_tasks"], 3);
    assert_eq!(view1["cache"]["hit"], false);
    assert_eq!(view1["user"]["email"], "jamesbond@example.com");
    assert_eq!(view1["user"]["role"], "staff");
    for task in view1["tasks"].as_array().unwrap() {
        assert_eq!(task["assigned_to"], "jamesbond@example.com");
    }

    let view2: Value = client
        .get(format!("{API}/tasks/view-my-tasks"))
        .bearer_auth(&james_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(view2["cache"]["hit"], true);

    // Reassigning one task away must invalidate James's cache.
    let reassign = client
        .post(format!("{API}/tasks/assign"))
        .bearer_auth(&admin_token)
        .json(&json!({ "task_ids": [task_ids[0]], "assignee_email": "admin@example.com" }))
        .send()
        .await
        .unwrap();
    assert_eq!(reassign.status(), 200);

    let view3: Value = client
        .get(format!("{API}/tasks/view-my-tasks"))
        .bearer_auth(&james_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        view3["cache"]["hit"], false,
        "assignment must invalidate cache"
    );
    assert_eq!(view3["summary"]["total_assigned_tasks"], 2);

    // Unauthenticated access is rejected.
    let unauthed = client
        .get(format!("{API}/tasks/view-my-tasks"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthed.status(), 401);
}
