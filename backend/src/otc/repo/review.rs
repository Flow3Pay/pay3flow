use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const INSERT_REVIEW: &str = "INSERT INTO otc_evidence(chain,identity,trade_id,kind,evidence,included_at) VALUES($1,$2,$3,'review_payment',$4,$5)";

pub(crate) async fn has_valid_payment(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool, tokio_postgres::Error> {
    Ok(client
        .query_opt(
            "SELECT identity FROM otc_evidence WHERE trade_id=$1 AND kind='payment' AND valid",
            &[trade_id],
        )
        .await?
        .is_some())
}

pub(crate) async fn valid_review_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Option<Value>, tokio_postgres::Error> {
    Ok(client
        .query_opt(
            "SELECT evidence FROM otc_evidence WHERE trade_id=$1 AND kind='review_payment' AND valid",
            &[trade_id],
        )
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn evidence_by_identity(
    client: &(impl GenericClient + Sync),
    chain: &str,
    identity: &str,
) -> Result<Option<(Uuid, String, Value)>, tokio_postgres::Error> {
    Ok(client
        .query_opt(
            "SELECT trade_id,kind,evidence FROM otc_evidence WHERE chain=$1 AND identity=$2",
            &[&chain, &identity],
        )
        .await?
        .map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn replace_review_evidence(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    evidence: &Value,
    included_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE otc_evidence SET valid=TRUE,evidence=$3,included_at=$4 WHERE trade_id=$1 AND kind=$2",
            &[trade_id, &"review_payment", &evidence, &included_at],
        )
        .await?;
    Ok(())
}

pub(crate) async fn insert_review_evidence(
    client: &(impl GenericClient + Sync),
    chain: &str,
    identity: &str,
    trade_id: &Uuid,
    evidence: &Value,
    included_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            INSERT_REVIEW,
            &[&chain, &identity, trade_id, evidence, &included_at],
        )
        .await?;
    Ok(())
}

pub(crate) async fn mark_review_state(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    attempt: &Uuid,
) -> Result<(), tokio_postgres::Error> {
    client
        .execute(
            "UPDATE otc_trades SET state='review',updated_at=now() WHERE id=$1",
            &[trade_id],
        )
        .await?;
    client
        .execute(
            "UPDATE otc_attempts SET state='review',updated_at=now() WHERE id=$1",
            &[attempt],
        )
        .await?;
    Ok(())
}
