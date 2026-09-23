use tracing_subscriber::EnvFilter;

use pay3flow_backend::activitypub::actor::ActorIdentity;
use pay3flow_backend::config::Config;
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

    let acquirer_pool = pay3flow_backend::routing::profile::load_pool(&pool).await?;
    let picker = pay3flow_backend::routing::RoutePicker::new(acquirer_pool);

    let providers =
        pay3flow_backend::payments::providers::ProviderRegistry::from_providers(vec![Box::new(
            pay3flow_backend::payments::providers::stub::StubProvider::new(),
        )]);

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

    let redis_pool = if cfg.redis_url.is_empty() {
        None
    } else {
        Some(
            pay3flow_backend::core::redis::build_pool(&cfg.redis_url).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to build Redis pool; cache will be skipped");
                panic!("dead pool")
            }),
        )
    };

    // Catalog rows are installed by migrations, not by application startup.
    let pairs = pay3flow_backend::pairs::ExchangePairsService::new(
        pool.clone(),
        std::time::Duration::from_secs(cfg.pairs_cache_ttl_secs),
    );

    let banks = pay3flow_backend::banks::BanksService::new(
        pool.clone(),
        std::time::Duration::from_secs(cfg.pairs_cache_ttl_secs),
    );

    let network_catalog = pay3flow_backend::networks::NetworkCatalog::load(&pool).await?;
    let p2p = pay3flow_backend::p2p::P2pSearchService::from_config(&cfg, network_catalog)?;
    let reputation =
        pay3flow_backend::service_reputation::ServiceReputation::new(pool.clone(), &cfg.jwt_secret);

    let state = AppState::new(
        pool,
        Jwt::new(&cfg.jwt_secret),
        ap,
        picker,
        payments,
        pairs,
        banks,
        cfg.admin_token,
        redis_pool,
        p2p,
        reputation,
    );

    tracing::info!(
        actor = %cfg.ap_origin,
        "listening on http://{}",
        cfg.http_addr
    );

    let self_seed = state.ap.clone();
    tokio::spawn(async move {
        let follow = pay3flow_backend::activitypub::model::follow_activity(
            &format!(
                "{}/seed/{}",
                self_seed.identity.actor_id,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ),
            &self_seed.identity.actor_id,
            &self_seed.fmatch_actor_id,
        );
        match self_seed
            .delivery
            .deliver(&self_seed.identity, &self_seed.fmatch_inbox, &follow)
            .await
        {
            Ok(outcome) => {
                tracing::info!(?outcome, "self-seed follow");
                match self_seed.publish_exchange_proposal().await {
                    Ok(outcome) => {
                        tracing::info!(?outcome, "published exchange proposal to fmatch")
                    }
                    Err(error) => tracing::warn!(%error, "exchange proposal publication failed"),
                }
            }
            Err(e) => tracing::warn!(error = %e, "self-seed follow failed"),
        }
    });

    let app = routing::api::router(state);
    let addr = cfg.http_addr;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
