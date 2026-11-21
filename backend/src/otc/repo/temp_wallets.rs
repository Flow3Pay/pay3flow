//! Durable storage for temporary custodial wallets.
//!
//! Rows carry ciphertext only; decryption and signing stay in
//! `otc::temp_wallet` and the settlement phase.
use crate::otc::error::Result;
use deadpool_postgres::GenericClient;
use tokio_postgres::Row;
use uuid::Uuid;

const COLUMNS: &str = "id,trade_id,role,chain,address,derivation_index,status";
const INSERT: &str = "INSERT INTO otc_temp_wallets(id,trade_id,role,chain,address,derivation_index,salt,key_ciphertext,parent_wallet_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT DO NOTHING";
const FOR_TRADE: &str = "SELECT id,trade_id,role,chain,address,derivation_index,status FROM otc_temp_wallets WHERE trade_id=$1 ORDER BY created_at";
const BY_ADDRESS: &str = "SELECT id,trade_id,role,chain,address,derivation_index,status FROM otc_temp_wallets WHERE chain=$1 AND address=$2 AND status='active' ORDER BY created_at DESC LIMIT 1";
const SET_STATUS: &str = "UPDATE otc_temp_wallets SET status=$2,updated_at=now() WHERE id=$1";
const RECYCLE: &str =
    "UPDATE otc_temp_wallets SET trade_id=NULL,status='recycled',updated_at=now() WHERE id=$1";
const INSERT_SETTLEMENT: &str = "INSERT INTO otc_temp_settlements(trade_id,wallet_id) VALUES($1,$2) ON CONFLICT (trade_id) DO NOTHING";
const SETTLEMENT_STATE: &str = "SELECT state FROM otc_temp_settlements WHERE trade_id=$1";
const DUE_SETTLEMENTS: &str = "SELECT s.trade_id,s.wallet_id,w.chain,w.address,w.derivation_index,w.salt,w.key_ciphertext,t.body,s.state,s.reference,s.fee_reference,s.attempts,s.deposit_address,s.deposit_memo,s.quote_id,s.source_units,s.minimum_units FROM otc_temp_settlements s JOIN otc_temp_wallets w ON w.id=s.wallet_id JOIN otc_p2p_trades t ON t.id=s.trade_id WHERE t.state IN ('delivery_pending','completed') AND (s.state IN ('awaiting_payment','payout_submitted','intents_submitted','fee_submitted') OR (s.state='payout_claim' AND s.updated_at<now()-interval '2 minutes')) AND s.attempts<10 ORDER BY s.updated_at LIMIT 20";
const CLAIM: &str = "UPDATE otc_temp_settlements SET state='payout_claim',updated_at=now() WHERE trade_id=$1 AND state='awaiting_payment'";
const SET_STATE: &str =
    "UPDATE otc_temp_settlements SET state=$2,last_error=$3,updated_at=now() WHERE trade_id=$1";
const SET_PAYOUT_REFERENCE: &str = "UPDATE otc_temp_settlements SET reference=$2,state='payout_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('awaiting_payment','payout_claim')";
const SET_FEE_REFERENCE: &str = "UPDATE otc_temp_settlements SET fee_reference=$2,state='fee_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('payout_submitted','intents_submitted')";
const SET_INTENTS_QUOTE: &str = "UPDATE otc_temp_settlements SET reference=$2,deposit_address=$3,deposit_memo=$4,quote_id=$5,source_units=$6,minimum_units=$7,state='intents_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('awaiting_payment','payout_claim')";
const NOTE_ERROR: &str = "UPDATE otc_temp_settlements SET attempts=attempts+1,last_error=$2,updated_at=now() WHERE trade_id=$1";

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

#[derive(Clone, Debug)]
pub(crate) struct SettlementRow {
    pub trade_id: Uuid,
    pub wallet_id: Uuid,
    pub chain: String,
    pub address: String,
    pub derivation_index: i32,
    pub salt: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
    pub body: serde_json::Value,
    pub state: String,
    pub reference: Option<String>,
    pub fee_reference: Option<String>,
    pub attempts: i32,
    pub deposit_address: Option<String>,
    pub deposit_memo: Option<String>,
    pub quote_id: Option<String>,
    pub source_units: Option<String>,
    pub minimum_units: Option<String>,
}

pub(crate) async fn insert_settlement(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    wallet_id: &Uuid,
) -> Result<()> {
    client
        .execute(INSERT_SETTLEMENT, &[trade_id, wallet_id])
        .await?;
    Ok(())
}

/// Atomically takes the right to sign one payout. A second worker sees `false`
/// and leaves the row to the winner.
pub(crate) async fn claim_awaiting(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client.execute(CLAIM, &[trade_id]).await? == 1)
}

pub(crate) async fn settlement_state(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(SETTLEMENT_STATE, &[trade_id])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn due_settlements(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<SettlementRow>> {
    Ok(client
        .query(DUE_SETTLEMENTS, &[])
        .await?
        .iter()
        .map(|row| SettlementRow {
            trade_id: row.get(0),
            wallet_id: row.get(1),
            chain: row.get(2),
            address: row.get(3),
            derivation_index: row.get(4),
            salt: row.get(5),
            key_ciphertext: row.get(6),
            body: row.get(7),
            state: row.get(8),
            reference: row.get(9),
            fee_reference: row.get(10),
            attempts: row.get(11),
            deposit_address: row.get(12),
            deposit_memo: row.get(13),
            quote_id: row.get(14),
            source_units: row.get(15),
            minimum_units: row.get(16),
        })
        .collect())
}

pub(crate) async fn set_state(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    state: &str,
    error: Option<&str>,
) -> Result<()> {
    client
        .execute(SET_STATE, &[trade_id, &state, &error])
        .await?;
    Ok(())
}

pub(crate) async fn set_payout_reference(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reference: &str,
) -> Result<()> {
    client
        .execute(SET_PAYOUT_REFERENCE, &[trade_id, &reference])
        .await?;
    Ok(())
}

pub(crate) async fn set_fee_reference(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reference: &str,
) -> Result<()> {
    client
        .execute(SET_FEE_REFERENCE, &[trade_id, &reference])
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn set_intents_quote(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reference: &str,
    deposit_address: &str,
    deposit_memo: Option<&str>,
    quote_id: &str,
    source_units: &str,
    minimum_units: &str,
) -> Result<()> {
    client
        .execute(
            SET_INTENTS_QUOTE,
            &[
                trade_id,
                &reference,
                &deposit_address,
                &deposit_memo,
                &quote_id,
                &source_units,
                &minimum_units,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn note_error(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    error: &str,
) -> Result<()> {
    client.execute(NOTE_ERROR, &[trade_id, &error]).await?;
    Ok(())
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
