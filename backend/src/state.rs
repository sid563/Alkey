use redis::aio::ConnectionManager;
use sqlx::PgPool;

use crate::config::Config;
use crate::mailer::Mailer;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub config: Config,
    pub mailer: Mailer,
}
