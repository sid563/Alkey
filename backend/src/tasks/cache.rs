use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use uuid::Uuid;

use crate::tasks::handlers::TaskView;

/// Versioned per-user cache key. Bumping the version instantly invalidates
/// every entry if the cached shape ever changes.
fn key(user_id: Uuid) -> String {
    format!("v1:tasks:{user_id}")
}

/// Read the cached task list. Any Redis error degrades to a cache miss:
/// the cache is an optimization, never a dependency.
pub async fn get_tasks(redis: &ConnectionManager, user_id: Uuid) -> Option<Vec<TaskView>> {
    let mut conn = redis.clone();
    let raw: Option<String> = conn.get(key(user_id)).await.ok().flatten();
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

pub async fn set_tasks(
    redis: &ConnectionManager,
    user_id: Uuid,
    tasks: &[TaskView],
    ttl_seconds: u64,
) {
    let Ok(raw) = serde_json::to_string(tasks) else {
        return;
    };
    let mut conn = redis.clone();
    if let Err(e) = conn
        .set_ex::<_, _, ()>(key(user_id), raw, ttl_seconds)
        .await
    {
        tracing::warn!("failed to populate cache for {user_id}: {e}");
    }
}

/// Invalidate the cache of every affected user (new assignee + previous
/// assignees). Called after the DB transaction commits; failures are logged
/// and absorbed — the TTL is the self-healing backstop.
pub async fn invalidate_users(redis: &ConnectionManager, user_ids: &[Uuid]) {
    if user_ids.is_empty() {
        return;
    }
    let keys: Vec<String> = user_ids.iter().map(|id| key(*id)).collect();
    let mut conn = redis.clone();
    if let Err(e) = conn.del::<_, ()>(keys).await {
        tracing::warn!("cache invalidation failed for {user_ids:?}: {e}");
    }
}
