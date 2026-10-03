#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub jwt_ttl_hours: i64,
    pub twofa_ttl_minutes: i64,
    pub cache_ttl_seconds: u64,
    pub cors_origin: String,
}

impl Config {
    pub fn from_env() -> Self {
        fn var(key: &str) -> String {
            std::env::var(key).unwrap_or_else(|_| panic!("missing env var {key}"))
        }
        fn var_or(key: &str, default: &str) -> String {
            std::env::var(key).unwrap_or_else(|_| default.to_string())
        }

        Self {
            host: var_or("HOST", "127.0.0.1"),
            port: var_or("PORT", "8080")
                .parse()
                .expect("PORT must be a number"),
            database_url: var("DATABASE_URL"),
            redis_url: var_or("REDIS_URL", "redis://127.0.0.1:6379"),
            jwt_secret: var("JWT_SECRET"),
            jwt_ttl_hours: var_or("JWT_TTL_HOURS", "24")
                .parse()
                .expect("JWT_TTL_HOURS"),
            twofa_ttl_minutes: var_or("TWOFA_TTL_MINUTES", "5")
                .parse()
                .expect("TWOFA_TTL_MINUTES"),
            cache_ttl_seconds: var_or("CACHE_TTL_SECONDS", "60")
                .parse()
                .expect("CACHE_TTL_SECONDS"),
            cors_origin: var_or("CORS_ORIGIN", "http://localhost:5173"),
        }
    }
}
