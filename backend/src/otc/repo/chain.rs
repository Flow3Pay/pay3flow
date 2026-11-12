use crate::otc::error::Result;
use deadpool_postgres::GenericClient;
use serde_json::Value;

const EVERSCALE_CHECKPOINT: &str = "SELECT checkpoint FROM otc_observers WHERE chain='everscale'";
const SAVE_EVERSCALE_CHECKPOINT: &str =
    "UPDATE otc_observers SET checkpoint=$1 WHERE chain='everscale'";

/// Checkpoint persisted by the Everscale observer.
pub(crate) async fn everscale_checkpoint(client: &(impl GenericClient + Sync)) -> Result<Value> {
    let row = client.query_one(EVERSCALE_CHECKPOINT, &[]).await?;
    Ok(row.get(0))
}

pub(crate) async fn save_everscale_checkpoint(
    client: &(impl GenericClient + Sync),
    checkpoint: &Value,
) -> Result<()> {
    client
        .execute(SAVE_EVERSCALE_CHECKPOINT, &[checkpoint])
        .await?;
    Ok(())
}
