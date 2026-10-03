use actix_web::{get, post, web, HttpResponse};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{jwt, twofa};
use crate::errors::ApiError;
use crate::models::{EmailLog, PublicUser, User, ROLE_ADMIN, ROLE_STAFF};
use crate::state::AppState;

/// Seed Admin and James Bond. Idempotent: re-running updates the same rows.
#[post("/seed/users")]
pub async fn seed_users(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let seeds = [
        ("Admin", "admin@example.com", "Admin@123", ROLE_ADMIN),
        (
            "James Bond",
            "jamesbond@example.com",
            "Bond@123",
            ROLE_STAFF,
        ),
    ];

    let mut users = Vec::new();
    for (name, email, password, role) in seeds {
        let hash = twofa::hash_secret(password)
            .map_err(|e| ApiError::Internal(format!("hashing failed: {e}")))?;
        let user: PublicUser = sqlx::query_as(
            r#"
            INSERT INTO users (full_name, email, hashed_password, role)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (email) DO UPDATE
                SET full_name = EXCLUDED.full_name,
                    hashed_password = EXCLUDED.hashed_password,
                    role = EXCLUDED.role,
                    updated_at = now()
            RETURNING id, full_name, email, role
            "#,
        )
        .bind(name)
        .bind(email)
        .bind(&hash)
        .bind(role)
        .fetch_one(&state.db)
        .await?;
        users.push(user);
    }

    Ok(HttpResponse::Ok().json(json!({ "seeded": users })))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Step 1 of login: verify password, create a 2FA challenge, "send" the code
/// via the email_logs table + console. Never returns a JWT.
#[post("/auth/login")]
pub async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginRequest>,
) -> Result<HttpResponse, ApiError> {
    let user: Option<User> = sqlx::query_as(
        "SELECT id, full_name, email, hashed_password, role FROM users WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await?;

    // Same error for unknown email and wrong password: don't leak which emails exist.
    let user = user.ok_or_else(|| ApiError::Unauthorized("invalid credentials".into()))?;
    if !twofa::verify_secret(&body.password, &user.hashed_password) {
        return Err(ApiError::Unauthorized("invalid credentials".into()));
    }

    let code = twofa::generate_code();
    let code_hash = twofa::hash_secret(&code)
        .map_err(|e| ApiError::Internal(format!("hashing failed: {e}")))?;
    let expires_at = Utc::now() + Duration::minutes(state.config.twofa_ttl_minutes);

    let (challenge_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO login_challenges (user_id, code_hash, expires_at) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(user.id)
    .bind(&code_hash)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;

    // The "email": plaintext code goes only here (dev inbox) and to the console.
    let subject = "Your verification code";
    let email_body = format!(
        "Your 2FA verification code is: {code}. It expires in {} minutes.",
        state.config.twofa_ttl_minutes
    );
    sqlx::query("INSERT INTO email_logs (email, subject, body) VALUES ($1, $2, $3)")
        .bind(&user.email)
        .bind(subject)
        .bind(&email_body)
        .execute(&state.db)
        .await?;

    tracing::info!("2FA code for {}: {}", user.email, code);
    // Optional real delivery (fire-and-forget; no-op when SMTP is unconfigured).
    state.mailer.send(&user.email, subject, &email_body);

    Ok(HttpResponse::Ok().json(json!({
        "login_challenge_id": challenge_id,
        "message": "verification code sent to your email",
        "expires_in_minutes": state.config.twofa_ttl_minutes
    })))
}

#[derive(Deserialize)]
pub struct EmailLogQuery {
    pub email: String,
}

/// Dev-only inbox: fetch the most recent "email" sent to an address.
#[get("/dev/email-logs/latest")]
pub async fn latest_email_log(
    state: web::Data<AppState>,
    query: web::Query<EmailLogQuery>,
) -> Result<HttpResponse, ApiError> {
    let log: Option<EmailLog> = sqlx::query_as(
        "SELECT id, email, subject, body, created_at FROM email_logs
         WHERE email = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&query.email)
    .fetch_optional(&state.db)
    .await?;

    let log = log.ok_or_else(|| ApiError::NotFound("no email logs for this address".into()))?;
    Ok(HttpResponse::Ok().json(log))
}

#[derive(Deserialize)]
pub struct Verify2faRequest {
    pub login_challenge_id: Uuid,
    pub code: String,
}

/// Step 2 of login: verify the one-time code and issue a JWT.
/// Runs in a transaction with FOR UPDATE so a code can never be redeemed twice,
/// even by concurrent requests; a wrong code does not consume the challenge.
#[post("/auth/verify-2fa")]
pub async fn verify_2fa(
    state: web::Data<AppState>,
    body: web::Json<Verify2faRequest>,
) -> Result<HttpResponse, ApiError> {
    let mut tx = state.db.begin().await?;

    let challenge: Option<(Uuid, String, chrono::DateTime<Utc>, bool)> = sqlx::query_as(
        "SELECT user_id, code_hash, expires_at, used FROM login_challenges
         WHERE id = $1 FOR UPDATE",
    )
    .bind(body.login_challenge_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (user_id, code_hash, expires_at, used) =
        challenge.ok_or_else(|| ApiError::Unauthorized("invalid challenge".into()))?;

    if used {
        return Err(ApiError::Unauthorized("code already used".into()));
    }
    if expires_at < Utc::now() {
        return Err(ApiError::Unauthorized("code expired".into()));
    }
    if !twofa::verify_secret(&body.code, &code_hash) {
        return Err(ApiError::Unauthorized("incorrect code".into()));
    }

    sqlx::query("UPDATE login_challenges SET used = true WHERE id = $1")
        .bind(body.login_challenge_id)
        .execute(&mut *tx)
        .await?;

    let user: User = sqlx::query_as(
        "SELECT id, full_name, email, hashed_password, role FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    let token = jwt::issue_token(
        user.id,
        &user.email,
        &user.role,
        &state.config.jwt_secret,
        state.config.jwt_ttl_hours,
    )
    .map_err(|e| ApiError::Internal(format!("jwt error: {e}")))?;

    Ok(HttpResponse::Ok().json(json!({
        "access_token": token,
        "token_type": "Bearer",
        "user": { "id": user.id, "email": user.email, "full_name": user.full_name, "role": user.role }
    })))
}
