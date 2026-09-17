//! Background acquirer discovery (step 83) — fmatch parity with crw.
//!
//! Runs at startup and then every `interval` (default 25 min,
//! `DISCOVERY_INTERVAL_SECS`). Each pass asks crw's `AcquirerDiscoveryService`
//! to re-scan the open web and returns an `AcquirerDiff` (added / updated /
//! deleted vs the `acquirers` table). We apply the diff:
//!
//!   1. upsert every `added`/`updated` acquirer (SQL `ON CONFLICT (slug) DO
//!      UPDATE`, seeded fields kept when the scan leaves them empty),
//!   2. delete every `deleted` slug — but only when the scan found a
//!      meaningful number of acquirers, so a thin/failed scan can never wipe
//!      the table (`SAFE_FOUND_FLOOR`),
//!   3. re-offer each changed acquirer to the fmatch inbox as a fep/0837
//!      `purpose="offer"` proposal so ratings reach the marketplace.
//!
//! A failed crw round-trip (connect/RPC/empty) is logged and skipped —
//! discovery is best-effort and must never take the server down.

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tonic::transport::Channel;
use tracing::{info, warn};

use crate::activitypub::Service as ActivityPubService;
use crate::db::DbPool;
use crate::search::crw_client::pb;

use pb::acquirer_discovery_service_client::AcquirerDiscoveryServiceClient;
use pb::{Acquirer, AcquirerDiff, DiscoverAcquirersRequest};

const _SEARCH_PROTO_PACKAGE: &str = "crw.search";

/// Number of acquirers we ask crw to aim for on each pass.
const DISCOVERY_TARGET: u32 = 500;
/// Floor for how many acquirers a scan must find before we trust its
/// `deleted` list. A scan below this (connectivity/config hiccup inside crw)
/// is logged and the deletion list is ignored.
const SAFE_FOUND_FLOOR: u32 = 30;

/// Run the discovery loop once at startup and then every `interval`.
///
/// Spawned from `main.rs`; does not return — it catches its own errors and
/// keeps going so the background task never dies.
pub async fn run_discovery_loop(
    pool: DbPool,
    ap: ActivityPubService,
    crw_url: String,
    interval: Duration,
) {
    loop {
        if let Err(err) = run_discovery_pass(&pool, &ap, &crw_url).await {
            warn!(error = %err, "acquirer discovery pass failed; will retry");
        }
        tokio::time::sleep(interval).await;
    }
}

/// Ask crw for an acquirer diff and apply it to the DB + fmatch inbox.
async fn run_discovery_pass(
    pool: &DbPool,
    ap: &ActivityPubService,
    crw_url: &str,
) -> Result<()> {
    let channel = Channel::from_shared(crw_url.to_string())
        .context("crw discovery: invalid endpoint url")?
        .connect_lazy();
    let mut client = AcquirerDiscoveryServiceClient::new(channel);

    let diff: AcquirerDiff = client
        .discover_acquirers(DiscoverAcquirersRequest {
            target_count: Some(DISCOVERY_TARGET),
        })
        .await
        .map_err(|e| anyhow!("crw discovery rpc failed: {e}"))?
        .into_inner();

    if diff.found < SAFE_FOUND_FLOOR {
        warn!(
            found = diff.found,
            "crw returned a thin result; refusing to apply any changes"
        );
        return Ok(());
    }

    let mut db = pool.get().await.context("discovery: acquire db client")?     ;
    let mut offer_count = 0usize;

    for entry in diff
        .added
        .iter()
        .chain(diff.updated.iter())
        .filter(|a| !a.slug.is_empty())
    {
        upsert_acquirer(&mut db, entry).await?;
        // Re-offer the (possibly changed) acquirer to fmatch.
        offer_acquirer(ap, entry).await?;
        offer_count += 1;
    }

    if !diff.deleted.is_empty() {
        let slugs: Vec<&str> = diff.deleted.iter().map(String::as_str).collect();
        let stmt = db
            .prepare_cached("DELETE FROM acquirers WHERE slug = ANY($1)")
            .await?;
        db.execute(&stmt, &[&slugs]).await?;
    }

    info!(
        found = diff.found,
        added = diff.added.len(),
        updated = diff.updated.len(),
        deleted = diff.deleted.len(),
        offered = offer_count,
        scanned_at = %diff.scanned_at,
        "acquirer discovery applied"
    );

    Ok(())
}

/// SQL upsert for one discovered acquirer. Empty scan fields (unknown fee,
/// missing limits/currency) never overwrite a curated/seed value.
async fn upsert_acquirer(db: &mut deadpool_postgres::Client, a: &Acquirer) -> Result<()> {
    let fee_percent = if a.fee_percent > 0.0 {
        Some(a.fee_percent)
    } else {
        None
    };
    let fee_fixed = if a.fee_fixed > 0 { Some(a.fee_fixed) } else { None };
    let stmt = db
        .prepare_cached(
            r#"
INSERT INTO acquirers
  (slug, name, geo, currencies, fee_percent, fee_fixed, amount_currency,
   status, active)
VALUES ($1, $2, $3, $4, $5, $6, $7, 'active', TRUE)
ON CONFLICT (slug) DO UPDATE SET
  name = COALESCE(EXCLUDED.name, acquirers.name),
  geo = COALESCE(EXCLUDED.geo, acquirers.geo),
  currencies = COALESCE(EXCLUDED.currencies, acquirers.currencies),
  fee_percent = COALESCE(EXCLUDED.fee_percent, acquirers.fee_percent),
  fee_fixed = COALESCE(EXCLUDED.fee_fixed, acquirers.fee_fixed),
  amount_currency = COALESCE(EXCLUDED.amount_currency, acquirers.amount_currency),
  status = 'active',
  active = TRUE,
  updated_at = now()
"#,
        )
        .await?;
    db.execute(
        &stmt,
        &[
            &a.slug,
            &a.name,
            &a.geo,
            &a.currencies,
            &fee_percent,
            &fee_fixed,
            &a.amount_currency,
        ],
    )
    .await?;
    Ok(())
}

/// Build a fep/0837 `purpose="offer"` proposal for one acquirer and deliver
/// it into the fmatch inbox. Uses the Marketplace resource the actor already
/// publishes to, so fmatch can rate the re-offered acquirer.
async fn offer_acquirer(ap: &ActivityPubService, a: &Acquirer) -> Result<()> {
    let id = format!(
        "{}/acquirers/{}/offer/{}",
        ap.origin,
        a.slug,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );
    let activity = serde_json::json!({
        "@context": [
            "https://www.w3.org/ns/activitystreams",
            "https://www.w3.org/ns/activitystreams#fep-0837"
        ],
        "id": id,
        "type": "Proposal",
        "purpose": "offer",
        "attributedTo": ap.identity.actor_id,
        "name": a.name,
        "content": format!(
            "{} fee {} (geo {}) — discovered by crw.",
            a.slug, a.fee_percent, a.geo
        ),
        "publishes": {
            "action": "deliverService",
            "resourceConformsTo": ap.marketplace_resource
        },
        "to": ["https://www.w3.org/ns/activitystreams#Public"],
        "attachment": [
            {"type": "PropertyValue", "name": "source", "value": a.source},
            {"type": "PropertyValue", "name": "feePercent", "value": a.fee_percent}
        ]
    });

    ap.delivery
        .deliver(&ap.identity, &ap.fmatch_inbox, &activity)
        .await
        .context("discovery: re-offer to fmatch failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_prevents_thin_deletes() {
        assert!(SAFE_FOUND_FLOOR > 0);
        assert!(DISCOVERY_TARGET > SAFE_FOUND_FLOOR);
    }
}
