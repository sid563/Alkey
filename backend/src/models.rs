use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_STAFF: &str = "staff";

/// The columns the auth flow needs; the table also has created_at/updated_at.
#[derive(Debug, FromRow)]
pub struct User {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    pub hashed_password: String,
    pub role: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct PublicUser {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EmailLog {
    pub id: Uuid,
    pub email: String,
    pub subject: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub created_by_id: Uuid,
    pub assigned_to_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
