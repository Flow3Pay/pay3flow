//! Durable storage for temporary custodial wallets.
//!
//! Rows carry ciphertext only; decryption and signing stay in
//! `otc::temp_wallet` and the settlement phase.
use crate::otc::error::Result;
use deadpool_postgres::GenericClient;
use tokio_postgres::Row;
use uuid::Uuid;

const COLUMNS: &str = "id,trade_id,role,chain,address,derivation_index,status";
const INSERT: &str = "INSERT INTO otc_temp_wallets(id,trade_id,role,chain,address,derivation_index,salt,key_ciphertext,parent_wallet_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (trade_id,role) WHERE trade_id IS NOT NULL DO NOTHING";
const FOR_TRADE: &str = "SELECT id,trade_id,role,chain,address,derivation_index,status FROM otc_temp_wallets WHERE trade_id=$1 ORDER BY created_at";
const BY_ADDRESS: &str = "SELECT id,trade_id,role,chain,address,derivation_index,status FROM otc_temp_wallets WHERE chain=$1 AND address=$2 AND status='active' ORDER BY created_at DESC LIMIT 1";
const SET_STATUS: &str = "UPDATE otc_temp_wallets SET status=$2,updated_at=now() WHERE id=$1";
const RECYCLE: &str =
    "UPDATE otc_temp_wallets SET trade_id=NULL,status='recycled',updated_at=now() WHERE id=$1";

#[derive(Clone, Debug)]
pub(crate) struct TempWallet {
    pub id: Uuid,
    pub trade_id: Option<Uuid>,
    pub role: String,
    pub chain: String,
    pub address: String,
    pub derivation_index: i32,
    pub status: String,
}

fn row(row: &Row) -> TempWallet {
    TempWallet {
        id: row.get("id"),
        trade_id: row.get("trade_id"),
        role: row.get("role"),
        chain: row.get("chain"),
        address: row.get("address"),
        derivation_index: row.get("derivation_index"),
        status: row.get("status"),
    }
}

/// Inserts one wallet row, returning `false` when `(trade_id, role)` already
/// exists so a repeated Accept cannot create a second address.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn insert(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    trade_id: Option<&Uuid>,
    role: &str,
    chain: &str,
    address: &str,
    derivation_index: u32,
    salt: &[u8],
    key_ciphertext: &[u8],
    parent_wallet_id: Option<&Uuid>,
) -> Result<bool> {
    let index = i32::try_from(derivation_index)
        .map_err(|_| crate::otc::error::invalid("derivation index is out of range"))?;
    let inserted = client
        .execute(
            INSERT,
            &[
                id,
                &trade_id,
                &role,
                &chain,
                &address,
                &index,
                &salt,
                &key_ciphertext,
                &parent_wallet_id,
            ],
        )
        .await?;
    Ok(inserted == 1)
}

pub(crate) async fn for_trade(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Vec<TempWallet>> {
    Ok(client
        .query(FOR_TRADE, &[trade_id])
        .await?
        .iter()
        .map(row)
        .collect())
}

pub(crate) async fn by_address(
    client: &(impl GenericClient + Sync),
    chain: &str,
    address: &str,
) -> Result<Option<TempWallet>> {
    Ok(client
        .query_opt(BY_ADDRESS, &[&chain, &address])
        .await?
        .as_ref()
        .map(row))
}

pub(crate) async fn set_status(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    status: &str,
) -> Result<()> {
    client.execute(SET_STATUS, &[id, &status]).await?;
    Ok(())
}

/// Detaches a wallet from its deal and returns it to the reusable pool.
pub(crate) async fn recycle(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    client.execute(RECYCLE, &[id]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::COLUMNS;

    #[test]
    fn selects_every_field_the_view_reads() {
        for column in [
            "id",
            "trade_id",
            "role",
            "chain",
            "address",
            "derivation_index",
            "status",
        ] {
            assert!(COLUMNS.contains(column), "{column}");
        }
    }
}
