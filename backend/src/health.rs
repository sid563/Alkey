use actix_web::{get, web, HttpResponse};
use serde_json::json;

use crate::errors::ApiError;
use crate::state::AppState;

#[get("/health")]
pub async fn health(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    sqlx::query("SELECT 1").execute(&state.db).await?;

    let mut redis = state.redis.clone();
    redis::cmd("PING").query_async::<String>(&mut redis).await?;

    Ok(HttpResponse::Ok().json(json!({
        "status": "ok",
        "database": "up",
        "redis": "up"
    })))
}
