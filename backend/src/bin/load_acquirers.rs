use tracing_subscriber::EnvFilter;

use pay3flow_backend::acquirer::ACQUIRERS;
use pay3flow_backend::activitypub::actor::ActorIdentity;
use pay3flow_backend::activitypub::model::follow_activity;
use pay3flow_backend::config::Config;
use pay3flow_backend::db;
use pay3flow_backend::service::build_activitypub_service;

/// Loads the first acquirer offers into fmatch:
///   1. Follow handshake (creates marketplace_subscription)
///   2. one Proposal(purpose=offer) per acquirer
/// Re-runs are idempotent (Same-origin activity ids resolve to status:"exists").
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cfg = Config::from_env()?;
    let pool = db::build_pool(&cfg.database_url).await?;
    db::apply_schema(&pool).await?;

    let identity = ActorIdentity::load_or_create(&cfg.ap_key_path, &cfg.ap_origin, &cfg.ap_handle)?;
    let svc = build_activitypub_service(&cfg, pool.clone(), identity.clone());

    let follow = follow_activity(&identity.actor_id, &identity.actor_id, &svc.fmatch_actor_id);
    let outcome = svc
        .delivery
        .deliver(&identity, &svc.fmatch_inbox, &follow)
        .await?;
    tracing::info!(?outcome, activity = "follow", "handshake");

    for seed in ACQUIRERS {
        let proposal = seed.to_proposal(
            &identity.actor_id,
            &svc.marketplace_resource,
            &format!("{}/inbox", cfg.ap_origin),
        );
        let activity = proposal.to_activity();
        let outcome = svc
            .delivery
            .deliver(&identity, &svc.fmatch_inbox, &activity)
            .await?;
        tracing::info!(?outcome, acquirer = seed.name, id = %proposal.id, "offer loaded");
    }

    Ok(())
}
