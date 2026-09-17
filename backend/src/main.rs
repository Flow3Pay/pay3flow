use tracing_subscriber::EnvFilter;

use pay3flow_backend::activitypub::actor::ActorIdentity;
use pay3flow_backend::config::Config;
use pay3flow_backend::core::jwt::Jwt;
use pay3flow_backend::core::state::AppState;
use pay3flow_backend::db;
use pay3flow_backend::server::routing;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cfg = Config::from_env()?;
    let pool = db::build_pool(&cfg.database_url).await?;
    db::apply_schema(&pool).await?;

    let identity = ActorIdentity::load_or_create(&cfg.ap_key_path, &cfg.ap_origin, &cfg.ap_handle)?;

    let ap = pay3flow_backend::service::build_activitypub_service(&cfg, pool.clone(), identity);

    let picker = pay3flow_backend::routing::RoutePicker::from_seeds();

    let state = AppState::new(pool, Jwt::new(&cfg.jwt_secret), ap, picker);

    tracing::info!(
        actor = %cfg.ap_origin,
        "listening on http://{}",
        cfg.http_addr
    );

    let self_seed = state.ap.clone();
    tokio::spawn(async move {
        let follow = pay3flow_backend::activitypub::model::follow_activity(
            &self_seed.identity.actor_id,
            &self_seed.identity.actor_id,
            &self_seed.fmatch_actor_id,
        );
        match self_seed
            .delivery
            .deliver(&self_seed.identity, &self_seed.fmatch_inbox, &follow)
            .await
        {
            Ok(outcome) => tracing::info!(?outcome, "self-seed follow"),
            Err(e) => tracing::warn!(error = %e, "self-seed follow failed"),
        }
        let offer = self_seed.self_offer_activity();
        match self_seed
            .delivery
            .deliver(&self_seed.identity, &self_seed.fmatch_inbox, &offer)
            .await
        {
            Ok(outcome) => tracing::info!(?outcome, "self-seed offer"),
            Err(e) => tracing::warn!(error = %e, "self-seed offer failed"),
        }
    });

    let app = routing::api::router(state);
    let addr = cfg.http_addr;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
