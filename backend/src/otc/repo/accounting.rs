use crate::otc::error::{Error, Result};
use deadpool_postgres::GenericClient;
use serde_json::Value;
use uuid::Uuid;

const INSERT_ENTRY: &str = "INSERT INTO otc_ledger(id,trade_id,kind,currency,amount,policy,authorized_by) VALUES($1,$2,$3,$4,$5::text::numeric,$6,$7)";

pub(crate) async fn lock_desk_trade(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
) -> Result<String> {
    let row = client
        .query_opt(
            "SELECT state FROM otc_trades WHERE id=$1 AND desk_actor=$2 FOR UPDATE",
            &[id, &actor],
        )
        .await?
        .ok_or(Error::NotFound)?;
    Ok(row.get(0))
}

pub(crate) async fn entry_by_key(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    key: &str,
) -> Result<Option<(Uuid, String, String, Value)>> {
    let row = client.query_opt("SELECT id,kind,amount::text,policy FROM otc_ledger WHERE trade_id=$1 AND policy->>'idempotency_key'=$2",&[trade_id,&key]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2), row.get(3))))
}

/// Returns `(fee_accrued, fee_collected, fee_compensation, fee_refunded)` as decimal text.
pub(crate) async fn fee_totals(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<(String, String, String, String)> {
    let row=client.query_one("SELECT COALESCE(sum(amount) FILTER(WHERE kind='fee_accrued'),0)::text, COALESCE(sum(amount) FILTER(WHERE kind='fee_collected'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_compensation'),0)::text,COALESCE(sum(amount) FILTER(WHERE kind='fee_refunded'),0)::text FROM otc_ledger WHERE trade_id=$1",&[trade_id]).await?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub(crate) async fn valid_payment_receipts(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<i64> {
    Ok(client
        .query_one(
            "SELECT count(*) FROM otc_evidence WHERE trade_id=$1 AND valid AND kind IN ('payment','payout')",
            &[trade_id],
        )
        .await?
        .get(0))
}

pub(crate) async fn insert_entry(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: &Uuid,
    kind: &str,
    currency: &str,
    units: &str,
    policy: &Value,
    actor: &str,
) -> Result<()> {
    client
        .execute(
            INSERT_ENTRY,
            &[id, trade_id, &kind, &currency, &units, &policy, &actor],
        )
        .await?;
    Ok(())
}
