use tracing_subscriber::EnvFilter;

use pay3flow_backend::activitypub::actor::ActorIdentity;
use pay3flow_backend::config::Config;
use pay3flow_backend::core::jwt::Jwt;
use pay3flow_backend::core::state::AppState;
use pay3flow_backend::db;
use pay3flow_backend::p2p::{IdPayRouteProvider, PublicFiatRouteProvider};
use pay3flow_backend::payments::rates::Rates;
use pay3flow_backend::payments::service::{PaymentConfig, PaymentService};
use pay3flow_backend::route_engine::{
    Asset, CowRouteProvider, LiveEdgeQuoteSource, NearIntentsProvider, PublicRouteProvider,
    RouteEngine, RouteGraphConfig, RouteQuoteService, RouteRefreshConfig, SymbiosisRouteProvider,
};
use pay3flow_backend::server::routing;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::load()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&cfg.log_filter))
        .init();

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

    let near_intents = {
        let provider =
            NearIntentsProvider::new(cfg.near_intents_url.clone(), cfg.near_intents_jwt.clone())?;
        let provider = match (
            cfg.near_intents_quote_recipient.clone(),
            cfg.near_intents_quote_refund_to.clone(),
        ) {
            (Some(recipient), Some(refund_to)) => {
                provider.with_quote_addresses(recipient, refund_to)?
            }
            _ => provider,
        };
        provider.with_quote_network_addresses(
            &cfg.near_intents_quote_recipients,
            &cfg.near_intents_quote_refunds,
        )?
    };
    let mut public_route_providers: Vec<Arc<dyn PublicRouteProvider>> =
        vec![Arc::new(near_intents.clone())];
    if let Some(cow) = CowRouteProvider::from_config(
        &cfg.cow_api_urls,
        &cfg.cow_tokens,
        cfg.cow_quote_address.as_deref(),
    )? {
        public_route_providers.push(Arc::new(cow));
    }
    public_route_providers.push(Arc::new(SymbiosisRouteProvider::new(
        cfg.symbiosis_url.clone(),
        cfg.symbiosis_partner_id.clone(),
        cfg.symbiosis_quote_address.clone(),
        cfg.symbiosis_slippage_bps,
    )?));
    let public_fiat_route_providers: Vec<Arc<dyn PublicFiatRouteProvider>> =
        vec![Arc::new(IdPayRouteProvider::new()?)];
    let network_catalog = pay3flow_backend::networks::NetworkCatalog::load(&pool).await?;
    let p2p = pay3flow_backend::p2p::P2pSearchService::from_database(
        &cfg,
        network_catalog,
        &pool,
        public_route_providers,
        public_fiat_route_providers,
    )
    .await?;
    let route_engine = RouteEngine::new(RouteGraphConfig {
        max_depth: cfg.route_max_depth,
    })?;
    let route_refresh = RouteRefreshConfig {
        source_fiats: cfg.route_source_fiats.clone(),
        p2p_assets: cfg.route_p2p_assets.clone(),
        intent_assets: cfg
            .route_intent_assets
            .iter()
            .map(|asset| Asset::parse(asset))
            .collect::<anyhow::Result<Vec<_>>>()?,
        withdrawal_edges: pay3flow_backend::route_engine::parse_route_edges(
            &cfg.route_withdrawals,
        )?,
    };
    let route_quotes = Arc::new(RouteQuoteService::new(
        route_engine.registry.clone(),
        Arc::new(LiveEdgeQuoteSource::new(p2p.clone(), near_intents.clone())),
    ));
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
        route_engine,
        near_intents,
        route_quotes,
        reputation,
    );

    tracing::info!(
        actor = %cfg.ap_origin,
        "listening on http://{}",
        cfg.http_addr
    );

    let self_seed = state.ap.clone();
    let route_engine = state.route_engine.clone();
    let route_p2p = state.p2p.clone();
    let route_near = state.near_intents.clone();
    let route_ap = state.ap.clone();
    let refresh_secs = cfg.near_intents_refresh_secs.max(30);
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
        loop {
            match route_near.load_supported_tokens().await {
                Ok(tokens) => tracing::info!(count = tokens.len(), "refreshed NEAR Intents tokens"),
                Err(error) => tracing::warn!(%error, "NEAR Intents token refresh failed"),
            }
            match route_engine
                .refresh(&route_p2p, &route_near, &route_refresh)
                .await
            {
                Ok(reconciliation) => {
                    let results = route_ap.publish_route_reconciliation(&reconciliation).await;
                    let failures = results.iter().filter(|result| result.is_err()).count();
                    tracing::info!(
                        upserted = reconciliation.upsert.len(),
                        disabled = reconciliation.disable.len(),
                        failures,
                        "refreshed Pay3Flow route capabilities"
                    );
                }
                Err(error) => tracing::warn!(%error, "Pay3Flow route refresh failed"),
            }
            tokio::time::sleep(std::time::Duration::from_secs(refresh_secs)).await;
        }
    });

    let app = routing::api::router(state);
    let addr = cfg.http_addr;
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
