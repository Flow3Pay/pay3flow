use tracing_subscriber::EnvFilter;

use pay3flow_backend::activitypub::actor::ActorIdentity;
use pay3flow_backend::config::Config;
use pay3flow_backend::core::crypto::SecretBox;
use pay3flow_backend::core::jwt::Jwt;
use pay3flow_backend::core::state::AppState;
use pay3flow_backend::db;
use pay3flow_backend::payments::rates::Rates;
use pay3flow_backend::payments::service::{PaymentConfig, PaymentService};
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

    // Seed the acquirer table from the curated real-brand list. Failures here
    // are non-fatal: payments still route through the in-memory picker. The
    // discovery worker (below) keeps this table fresh from crw scans.
    for seed in pay3flow_backend::acquirer::ACQUIRERS {
        if let Ok(id) = pay3flow_backend::payments::repo::upsert_acquirer(&pool, seed).await {
            let boxed = SecretBox::new(&cfg.secrets_key);
            let enc = boxed
                .encrypt("stub-secret-placeholder")
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, slug = seed.slug, "encrypt placeholder credential");
                    String::new()
                });
            if !enc.is_empty() {
                let _ = pay3flow_backend::payments::repo::set_credential(
                    &pool,
                    &id,
                    "api_key",
                    &enc,
                )
                .await;
            }
        }
    }

    let providers = pay3flow_backend::payments::providers::ProviderRegistry::from_providers(vec![
        Box::new(pay3flow_backend::payments::providers::stub::StubProvider::new()),
    ]);

    let rates = match cfg.fx_source.to_ascii_lowercase().as_str() {
        "http" => Rates::http(cfg.fx_url.clone(), "EUR".into())
            .with_ttl(std::time::Duration::from_secs(cfg.fx_cache_ttl_secs))
            .with_margin_percent(cfg.fx_margin_percent),
        _ => Rates::mock()
            .with_ttl(std::time::Duration::from_secs(cfg.fx_cache_ttl_secs))
            .with_margin_percent(cfg.fx_margin_percent),
    };

    let payments = PaymentService::new(
        pool.clone(),
        ap.clone(),
        picker.clone(),
        providers,
        rates,
        PaymentConfig {
            service_fee_percent: cfg.service_fee_percent,
        },
    );

    let state = AppState::new(pool, Jwt::new(&cfg.jwt_secret), ap, picker, payments);

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
    });

    // Background acquirer discovery: run once at startup and then every
    // DISCOVERY_INTERVAL (default 25 min). Each run asks crw to scan the
    // internet, receives an acquirer diff, applies it to the acquirers table
    // and re-offers acquirers to fmatch as `purpose="offer"` proposals.
    let discovery_pool = state.pool.clone();
    let discovery_ap = state.ap.clone();
    let discovery_crw_url = cfg.crw_grpc_url.clone();
    let discovery_interval = std::time::Duration::from_secs(
        std::env::var("DISCOVERY_INTERVAL_SECS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(25 * 60),
    );
    tokio::spawn(async move {
        pay3flow_backend::discovery::run_discovery_loop(
            discovery_pool,
            discovery_ap,
            discovery_crw_url,
            discovery_interval,
        )
        .await;
    });

    let app = routing::api::router(state);
    let addr = cfg.http_addr;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
