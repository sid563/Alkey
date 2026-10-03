use actix_web::{get, post, web, HttpResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::extractor::AuthedUser;
use crate::errors::ApiError;
use crate::models::Task;
use crate::state::AppState;
use crate::tasks::cache;

const STATUSES: [&str; 3] = ["todo", "in_progress", "done"];
const PRIORITIES: [&str; 3] = ["low", "medium", "high"];

/// The task shape returned by the API and stored in the cache.
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TaskView {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assigned_to: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default = "default_priority")]
    pub priority: String,
}

fn default_status() -> String {
    "todo".into()
}
fn default_priority() -> String {
    "medium".into()
}

/// Create a task. Admin only.
#[post("/tasks")]
pub async fn create_task(
    state: web::Data<AppState>,
    user: AuthedUser,
    body: web::Json<CreateTaskRequest>,
) -> Result<HttpResponse, ApiError> {
    user.require_admin()?;

    if body.title.trim().is_empty() {
        return Err(ApiError::BadRequest("title must not be empty".into()));
    }
    if !STATUSES.contains(&body.status.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "status must be one of {STATUSES:?}"
        )));
    }
    if !PRIORITIES.contains(&body.priority.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "priority must be one of {PRIORITIES:?}"
        )));
    }

    let task: Task = sqlx::query_as(
        r#"
        INSERT INTO tasks (title, description, status, priority, created_by_id)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(body.title.trim())
    .bind(&body.description)
    .bind(&body.status)
    .bind(&body.priority)
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    Ok(HttpResponse::Created().json(json!({
        "id": task.id,
        "title": task.title,
        "description": task.description,
        "status": task.status,
        "priority": task.priority,
        "created_by_id": task.created_by_id,
        "assigned_to_id": task.assigned_to_id,
        "created_at": task.created_at,
        "updated_at": task.updated_at
    })))
}

#[derive(Deserialize)]
pub struct AssignTasksRequest {
    pub task_ids: Vec<Uuid>,
    pub assignee_email: String,
}

/// Assign tasks to a user. Admin only. All-or-nothing: every task id and the
/// assignee must exist or nothing is changed. After commit, the caches of the
/// new assignee AND all previous assignees are invalidated.
#[post("/tasks/assign")]
pub async fn assign_tasks(
    state: web::Data<AppState>,
    user: AuthedUser,
    body: web::Json<AssignTasksRequest>,
) -> Result<HttpResponse, ApiError> {
    user.require_admin()?;

    if body.task_ids.is_empty() {
        return Err(ApiError::BadRequest("task_ids must not be empty".into()));
    }

    let mut tx = state.db.begin().await?;

    let assignee: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&body.assignee_email)
        .fetch_optional(&mut *tx)
        .await?;
    let (assignee_id,) =
        assignee.ok_or_else(|| ApiError::NotFound("assignee user not found".into()))?;

    // Lock the rows and capture previous assignees for cache invalidation.
    let found: Vec<(Uuid, Option<Uuid>)> =
        sqlx::query_as("SELECT id, assigned_to_id FROM tasks WHERE id = ANY($1) FOR UPDATE")
            .bind(&body.task_ids)
            .fetch_all(&mut *tx)
            .await?;

    if found.len() != body.task_ids.len() {
        let found_ids: Vec<Uuid> = found.iter().map(|(id, _)| *id).collect();
        let missing: Vec<String> = body
            .task_ids
            .iter()
            .filter(|id| !found_ids.contains(id))
            .map(|id| id.to_string())
            .collect();
        return Err(ApiError::NotFound(format!(
            "tasks not found: {}",
            missing.join(", ")
        )));
    }

    sqlx::query("UPDATE tasks SET assigned_to_id = $1, updated_at = now() WHERE id = ANY($2)")
        .bind(assignee_id)
        .bind(&body.task_ids)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Invalidate after commit so a concurrent read can't repopulate from
    // pre-commit data and have the invalidation silently undone.
    let mut affected: Vec<Uuid> = found.into_iter().filter_map(|(_, prev)| prev).collect();
    affected.push(assignee_id);
    affected.sort();
    affected.dedup();
    cache::invalidate_users(&state.redis, &affected).await;

    Ok(HttpResponse::Ok().json(json!({
        "assigned": body.task_ids.len(),
        "assignee_email": body.assignee_email
    })))
}

/// List all tasks. Admin only; used by the admin screen to pick tasks to assign.
#[get("/tasks")]
pub async fn list_tasks(
    state: web::Data<AppState>,
    user: AuthedUser,
) -> Result<HttpResponse, ApiError> {
    user.require_admin()?;

    let tasks: Vec<TaskView> = sqlx::query_as(
        r#"
        SELECT t.id, t.title, t.description, t.status, t.priority,
               u.email AS assigned_to
        FROM tasks t
        LEFT JOIN users u ON u.id = t.assigned_to_id
        ORDER BY t.created_at
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(HttpResponse::Ok().json(json!({ "tasks": tasks })))
}

/// Tasks assigned to the logged-in user, cache-aside with per-user keys.
#[get("/tasks/view-my-tasks")]
pub async fn view_my_tasks(
    state: web::Data<AppState>,
    user: AuthedUser,
) -> Result<HttpResponse, ApiError> {
    let (tasks, hit) = match cache::get_tasks(&state.redis, user.id).await {
        Some(tasks) => (tasks, true),
        None => {
            let tasks: Vec<TaskView> = sqlx::query_as(
                r#"
                SELECT t.id, t.title, t.description, t.status, t.priority,
                       u.email AS assigned_to
                FROM tasks t
                LEFT JOIN users u ON u.id = t.assigned_to_id
                WHERE t.assigned_to_id = $1
                ORDER BY t.created_at
                "#,
            )
            .bind(user.id)
            .fetch_all(&state.db)
            .await?;

            cache::set_tasks(
                &state.redis,
                user.id,
                &tasks,
                state.config.cache_ttl_seconds,
            )
            .await;
            (tasks, false)
        }
    };

    Ok(HttpResponse::Ok().json(json!({
        "user": { "email": user.email, "role": user.role },
        "tasks": tasks,
        "summary": { "total_assigned_tasks": tasks.len() },
        "cache": { "hit": hit }
    })))
}
