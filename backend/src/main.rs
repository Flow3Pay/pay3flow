mod config;
mod core;
mod server;
mod service;

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::from_env()?;
    let pool = core::db::build_pool(&cfg.database_url).await?;
    core::db::apply_schema(&pool).await?;

    let state = core::state::AppState::new(pool, core::jwt::Jwt::new(&cfg.jwt_secret));
    let app = server::routing::api::router(state);

    let addr = cfg.http_addr;
    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}