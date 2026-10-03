mod auth;
mod config;
mod errors;
mod health;
mod mailer;
mod models;
mod state;
mod tasks;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let config = Config::from_env();

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to postgres");

    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("failed to run migrations");

    let redis_client = redis::Client::open(config.redis_url.clone()).expect("invalid REDIS_URL");
    let redis = redis::aio::ConnectionManager::new(redis_client)
        .await
        .expect("failed to connect to redis");

    let state = AppState {
        db,
        redis,
        config: config.clone(),
        mailer: mailer::Mailer::from_env(),
    };

    let bind_addr = (config.host.clone(), config.port);
    tracing::info!("listening on http://{}:{}", bind_addr.0, bind_addr.1);

    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin(&state.config.cors_origin)
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(web::Data::new(state.clone()))
            .service(health::health)
            .service(auth::handlers::seed_users)
            .service(auth::handlers::login)
            .service(auth::handlers::latest_email_log)
            .service(auth::handlers::verify_2fa)
            .service(tasks::handlers::create_task)
            .service(tasks::handlers::list_tasks)
            .service(tasks::handlers::assign_tasks)
            .service(tasks::handlers::view_my_tasks)
    })
    .bind(bind_addr)?
    .run()
    .await
}
