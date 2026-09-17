pub mod user;

use crate::activitypub;
use crate::activitypub::actor::ActorIdentity;
use crate::config::Config;
use crate::db::DbPool;

pub fn build_activitypub_service(
    cfg: &Config,
    pool: DbPool,
    identity: ActorIdentity,
) -> activitypub::Service {
    activitypub::Service::new(
        pool,
        identity,
        cfg.ap_require_signatures,
        cfg.fmatch_inbox.clone(),
        cfg.fmatch_actor_id.clone(),
        format!("{}/marketplace/resources/acquiring", cfg.ap_origin),
        cfg.ap_origin.clone(),
    )
}
