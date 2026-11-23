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
const RETURN_TO_POOL: &str = "UPDATE otc_temp_wallets SET trade_id=NULL,status='active',updated_at=now() WHERE id=$1 AND status='active'";
const TREASURY: &str = "SELECT id,chain,address,derivation_index,salt,key_ciphertext FROM otc_temp_wallets WHERE role='treasury' AND trade_id IS NULL AND chain=$1 AND status='active' ORDER BY created_at LIMIT 1";
const LOCK_POOL_SLOT: &str = "SELECT id,address,derivation_index,salt,key_ciphertext FROM otc_temp_wallets WHERE trade_id IS NULL AND chain=$1 AND role IN ('usd','usd_in') AND status='active' ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED";
const ATTACH_POOL_SLOT: &str = "UPDATE otc_temp_wallets SET trade_id=$2,role=$3,updated_at=now() WHERE id=$1 AND trade_id IS NULL AND status='active'";
const DELETE_ROLE: &str = "UPDATE otc_temp_wallets SET status='deleted',updated_at=now() WHERE trade_id=$1 AND role=$2 AND status<>'deleted'";
const INSERT_SETTLEMENT: &str = "INSERT INTO otc_temp_settlements(trade_id,wallet_id) VALUES($1,$2) ON CONFLICT (trade_id) DO NOTHING";
const SETTLEMENT_STATE: &str = "SELECT state FROM otc_temp_settlements WHERE trade_id=$1";
const DUE_SETTLEMENTS: &str = "SELECT s.trade_id,s.wallet_id,w.chain,w.role,w.address,w.derivation_index,w.salt,w.key_ciphertext,t.body,s.state,s.reference,s.fee_reference,s.attempts,s.deposit_address,s.deposit_memo,s.quote_id,s.source_units,s.minimum_units,s.reserve_units,s.output_signed_transaction,s.payout_signed_transaction,s.fee_signed_transaction FROM otc_temp_settlements s JOIN otc_temp_wallets w ON w.id=s.wallet_id JOIN otc_p2p_trades t ON t.id=s.trade_id WHERE t.state IN ('delivery_pending','completed') AND (s.state IN ('awaiting_payment','payout_submitted','intents_submitted','intents_payout_submitted','fee_submitted') OR (s.state IN ('payout_claim','intents_payout_claim') AND s.updated_at<now()-interval '2 minutes')) AND s.attempts<10 ORDER BY s.updated_at LIMIT 20";
const CLAIM: &str = "UPDATE otc_temp_settlements SET state='payout_claim',updated_at=now() WHERE trade_id=$1 AND state='awaiting_payment'";
const CLAIM_INTENTS_PAYOUT: &str = "UPDATE otc_temp_settlements SET state='intents_payout_claim',updated_at=now() WHERE trade_id=$1 AND state='intents_submitted'";
const SET_INTENTS_PAYOUT_REFERENCE: &str = "UPDATE otc_temp_settlements SET reference=$2,output_signed_transaction=$3,state='intents_payout_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state='intents_payout_claim'";
const RESET_INTENTS_PAYOUT: &str = "UPDATE otc_temp_settlements SET reference=NULL,output_signed_transaction=NULL,state='intents_submitted',last_error=$2,updated_at=now() WHERE trade_id=$1 AND state='intents_payout_submitted'";
const SET_STATE: &str =
    "UPDATE otc_temp_settlements SET state=$2,last_error=$3,updated_at=now() WHERE trade_id=$1";
const STALL_EXPIRED_INTENTS: &str = "UPDATE otc_temp_settlements SET state='stalled',last_error='NEAR Intents status remained pending for over two hours; operator review required',updated_at=now() WHERE trade_id=$1 AND state='intents_submitted' AND intents_started_at < now()-interval '2 hours'";
const SET_PAYOUT_REFERENCE: &str = "UPDATE otc_temp_settlements SET reference=$2,state='payout_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('awaiting_payment','payout_claim')";
const SET_FEE_REFERENCE: &str = "UPDATE otc_temp_settlements SET fee_reference=$2,fee_signed_transaction=$3,state='fee_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('payout_submitted','intents_submitted','intents_payout_submitted')";
const SET_TREASURY_PAYMENT: &str = "UPDATE otc_temp_settlements SET reference=$2,reserve_units=$3,payout_signed_transaction=$4,state='payout_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('awaiting_payment','payout_claim')";
const SET_INTENTS_QUOTE: &str = "UPDATE otc_temp_settlements SET reference=$2,deposit_address=$3,deposit_memo=$4,quote_id=$5,source_units=$6,minimum_units=$7,intents_started_at=now(),state='intents_submitted',last_error=NULL,updated_at=now() WHERE trade_id=$1 AND state IN ('awaiting_payment','payout_claim')";
const WALLET_SECRET_FOR_ROLE: &str = "SELECT chain,address,derivation_index,salt,key_ciphertext FROM otc_temp_wallets WHERE trade_id=$1 AND role=$2 AND status='active' LIMIT 1";
/// A row that keeps failing stops being selected at ten attempts, so the tenth
/// failure moves it to an explicit terminal state instead of freezing in a live
/// one that still looks actionable.
const NOTE_ERROR: &str = "UPDATE otc_temp_settlements SET attempts=attempts+1,last_error=$2,state=CASE WHEN attempts+1>=10 THEN 'stalled' ELSE state END,updated_at=now() WHERE trade_id=$1";
/// Gas funding claims one signer per deal wallet role. A stale claim is
/// retried after the same two-minute window used by payouts, so a crashed
/// worker cannot wedge a deal forever.
const CLAIM_GAS: &str = "INSERT INTO otc_temp_gas(trade_id,role,address,state) VALUES($1,$2,$3,'claim') ON CONFLICT(trade_id,role) DO UPDATE SET state='claim',address=EXCLUDED.address,updated_at=now() WHERE otc_temp_gas.state IN ('pending','failed') OR (otc_temp_gas.state='claim' AND otc_temp_gas.updated_at<now()-interval '2 minutes')";
/// The signed envelope is written before broadcast; a retry after an
/// ambiguous RPC response replays exactly these bytes.
const SET_GAS_SIGNED: &str = "UPDATE otc_temp_gas SET state='submitted',reference=$3,signed_transaction=$4,last_error=NULL,updated_at=now() WHERE trade_id=$1 AND role=$2 AND state='claim'";
const NOTE_GAS_ERROR: &str = "UPDATE otc_temp_gas SET attempts=attempts+1,state=CASE WHEN attempts+1>=10 THEN 'failed' ELSE 'pending' END,last_error=$3,updated_at=now() WHERE trade_id=$1 AND role=$2";
const GAS_FUNDING: &str = "SELECT state,reference FROM otc_temp_gas WHERE trade_id=$1 AND role=$2";
/// EVER payouts start only after the USD net is delivered and the payer's
/// verified deposit landed on the deal's `ever` temp wallet.
const DUE_EVER: &str = "SELECT s.trade_id,s.wallet_id,w.chain,w.address,w.derivation_index,w.salt,w.key_ciphertext,t.body,s.ever_state,s.ever_reference,s.ever_attempts,s.ever_signed_message FROM otc_temp_settlements s JOIN otc_p2p_trades t ON t.id=s.trade_id JOIN otc_temp_wallets w ON w.trade_id=s.trade_id AND w.role='ever' WHERE t.state='delivery_pending' AND s.state IN ('settled','fee_retained') AND w.status='active' AND (s.ever_state IS NULL OR s.ever_state='pending' OR (s.ever_state IN ('claim','submitted') AND s.updated_at<now()-interval '2 minutes')) AND s.ever_attempts<60 AND EXISTS(SELECT 1 FROM otc_p2p_attempts a WHERE a.trade_id=s.trade_id AND a.kind='payout' AND a.state='verified' AND a.instructions->'leg'->>'recipient'=w.address) ORDER BY s.updated_at LIMIT 20";
const CLAIM_EVER: &str = "UPDATE otc_temp_settlements SET ever_state='claim',ever_error=NULL,updated_at=now() WHERE trade_id=$1 AND (ever_state IS NULL OR ever_state='pending' OR (ever_state='claim' AND updated_at<now()-interval '2 minutes'))";
const SET_EVER_SIGNED: &str = "UPDATE otc_temp_settlements SET ever_state='submitted',ever_reference=$2,ever_signed_message=$3,ever_error=NULL,updated_at=now() WHERE trade_id=$1 AND ever_state='claim'";
const SET_EVER_STATE: &str = "UPDATE otc_temp_settlements SET ever_state=$2,ever_reference=COALESCE($3,ever_reference),ever_error=$4,updated_at=now() WHERE trade_id=$1";
const NOTE_EVER_ERROR: &str = "UPDATE otc_temp_settlements SET ever_attempts=ever_attempts+1,ever_error=$2,ever_state=CASE WHEN ever_attempts+1>=60 THEN 'failed' WHEN ever_state='submitted' THEN 'submitted' ELSE 'pending' END,updated_at=now() WHERE trade_id=$1";

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
    pub role: String,
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
    /// Net fronted from the platform reserve when this is set.
    pub reserve_units: Option<String>,
    pub output_signed_transaction: Option<Vec<u8>>,
    pub payout_signed_transaction: Option<Vec<u8>>,
    pub fee_signed_transaction: Option<Vec<u8>>,
}

/// One permanent platform treasury for a chain, including its gas reserve.
/// It is stored with salted encryption and is never deleted.
#[derive(Clone)]
pub(crate) struct TreasuryWallet {
    pub id: Uuid,
    pub chain: String,
    pub address: String,
    pub derivation_index: i32,
    pub salt: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
}

/// A previously derived USD address atomically attached to one new deal.
pub(crate) struct PooledWallet {
    pub id: Uuid,
    pub address: String,
    pub derivation_index: i32,
    pub salt: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
}

pub(crate) struct WalletSecret {
    pub chain: String,
    pub address: String,
    pub derivation_index: i32,
    pub salt: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
}

/// One funded `ever` temporary wallet whose USD side already settled.
#[derive(Clone)]
pub(crate) struct EverPayoutRow {
    pub trade_id: Uuid,
    pub wallet_id: Uuid,
    pub chain: String,
    pub address: String,
    pub derivation_index: i32,
    pub salt: Vec<u8>,
    pub key_ciphertext: Vec<u8>,
    pub body: serde_json::Value,
    pub ever_state: Option<String>,
    pub ever_reference: Option<String>,
    pub ever_attempts: i32,
    pub ever_signed_message: Option<Vec<u8>>,
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
            role: row.get(3),
            address: row.get(4),
            derivation_index: row.get(5),
            salt: row.get(6),
            key_ciphertext: row.get(7),
            body: row.get(8),
            state: row.get(9),
            reference: row.get(10),
            fee_reference: row.get(11),
            attempts: row.get(12),
            deposit_address: row.get(13),
            deposit_memo: row.get(14),
            quote_id: row.get(15),
            source_units: row.get(16),
            minimum_units: row.get(17),
            reserve_units: row.get(18),
            output_signed_transaction: row.get(19),
            payout_signed_transaction: row.get(20),
            fee_signed_transaction: row.get(21),
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

/// Stops automatic settlement after a funded Intents swap remains pending for
/// two hours. Funds stay on custodial addresses for operator reconciliation.
pub(crate) async fn stall_expired_intents(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client.execute(STALL_EXPIRED_INTENTS, &[trade_id]).await? == 1)
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
    signed_transaction: &[u8],
) -> Result<()> {
    client
        .execute(
            SET_FEE_REFERENCE,
            &[trade_id, &reference, &signed_transaction],
        )
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

pub(crate) async fn set_intents_payout_reference(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reference: &str,
    signed_transaction: &[u8],
) -> Result<()> {
    let updated = client
        .execute(
            SET_INTENTS_PAYOUT_REFERENCE,
            &[trade_id, &reference, &signed_transaction],
        )
        .await?;
    if updated != 1 {
        return Err(crate::otc::error::Error::Conflict(
            "cross-network payout claim was lost before it was persisted".into(),
        ));
    }
    Ok(())
}

pub(crate) async fn claim_intents_payout(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client.execute(CLAIM_INTENTS_PAYOUT, &[trade_id]).await? == 1)
}

pub(crate) async fn reset_intents_payout(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reason: &str,
) -> Result<()> {
    client
        .execute(RESET_INTENTS_PAYOUT, &[trade_id, &reason])
        .await?;
    Ok(())
}

pub(crate) async fn wallet_secret_for_role(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    role: &str,
) -> Result<Option<WalletSecret>> {
    Ok(client
        .query_opt(WALLET_SECRET_FOR_ROLE, &[trade_id, &role])
        .await?
        .map(|row| WalletSecret {
            chain: row.get("chain"),
            address: row.get("address"),
            derivation_index: row.get("derivation_index"),
            salt: row.get("salt"),
            key_ciphertext: row.get("key_ciphertext"),
        }))
}

/// Marks the net as fronted from the reserve and records the payout hash.
pub(crate) async fn set_treasury_payment(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reference: &str,
    units: &str,
    signed_transaction: &[u8],
) -> Result<()> {
    client
        .execute(
            SET_TREASURY_PAYMENT,
            &[trade_id, &reference, &units, &signed_transaction],
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

/// Atomically takes the right to sign one treasury-to-temp gas transfer for
/// a deal wallet role.
pub(crate) async fn claim_gas(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    role: &str,
    address: &str,
) -> Result<bool> {
    Ok(client
        .execute(CLAIM_GAS, &[trade_id, &role, &address])
        .await?
        == 1)
}

pub(crate) async fn gas_funding(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    role: &str,
) -> Result<Option<(String, Option<String>)>> {
    Ok(client
        .query_opt(GAS_FUNDING, &[trade_id, &role])
        .await?
        .map(|row| (row.get(0), row.get(1))))
}

pub(crate) async fn set_gas_signed(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    role: &str,
    reference: &str,
    signed: &[u8],
) -> Result<()> {
    client
        .execute(SET_GAS_SIGNED, &[trade_id, &role, &reference, &signed])
        .await?;
    Ok(())
}

pub(crate) async fn note_gas_error(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    role: &str,
    error: &str,
) -> Result<()> {
    client
        .execute(NOTE_GAS_ERROR, &[trade_id, &role, &error])
        .await?;
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

/// Returns the same address to the available pool while retaining its dust.
pub(crate) async fn return_to_pool(client: &(impl GenericClient + Sync), id: &Uuid) -> Result<()> {
    let returned = client.execute(RETURN_TO_POOL, &[id]).await?;
    if returned != 1 {
        return Err(crate::otc::error::Error::Conflict(
            "temporary wallet could not be safely returned to the pool".into(),
        ));
    }
    Ok(())
}

/// The active platform reserve for a chain, if it has been provisioned.
pub(crate) async fn treasury(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<Option<TreasuryWallet>> {
    Ok(client
        .query_opt(TREASURY, &[&chain])
        .await?
        .map(|row| TreasuryWallet {
            id: row.get("id"),
            chain: row.get("chain"),
            address: row.get("address"),
            derivation_index: row.get("derivation_index"),
            salt: row.get("salt"),
            key_ciphertext: row.get("key_ciphertext"),
        }))
}

/// Locks one empty USD slot under the caller's deal transaction. Row locking
/// with SKIP LOCKED ensures concurrent accepts can never receive the same
/// deposit address.
pub(crate) async fn lock_pool_slot(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<Option<PooledWallet>> {
    Ok(client
        .query_opt(LOCK_POOL_SLOT, &[&chain])
        .await?
        .map(|row| PooledWallet {
            id: row.get("id"),
            address: row.get("address"),
            derivation_index: row.get("derivation_index"),
            salt: row.get("salt"),
            key_ciphertext: row.get("key_ciphertext"),
        }))
}

/// Attaches a slot locked earlier in the same transaction after its trade row
/// exists (the wallet row has a foreign key to that trade).
pub(crate) async fn attach_pool_slot(
    client: &(impl GenericClient + Sync),
    wallet_id: &Uuid,
    trade_id: &Uuid,
    role: &str,
) -> Result<()> {
    let attached = client
        .execute(ATTACH_POOL_SLOT, &[wallet_id, trade_id, &role])
        .await?;
    if attached != 1 {
        return Err(crate::otc::error::Error::Conflict(
            "temporary wallet slot was claimed concurrently".into(),
        ));
    }
    Ok(())
}

pub(crate) async fn due_ever_payouts(
    client: &(impl GenericClient + Sync),
) -> Result<Vec<EverPayoutRow>> {
    Ok(client
        .query(DUE_EVER, &[])
        .await?
        .iter()
        .map(|row| EverPayoutRow {
            trade_id: row.get(0),
            wallet_id: row.get(1),
            chain: row.get(2),
            address: row.get(3),
            derivation_index: row.get(4),
            salt: row.get(5),
            key_ciphertext: row.get(6),
            body: row.get(7),
            ever_state: row.get(8),
            ever_reference: row.get(9),
            ever_attempts: row.get(10),
            ever_signed_message: row.get(11),
        })
        .collect())
}

/// Claims one payout so competing workers cannot create distinct signed
/// messages for the same temporary wallet.
pub(crate) async fn claim_ever_payout(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
) -> Result<bool> {
    Ok(client.execute(CLAIM_EVER, &[trade_id]).await? == 1)
}

/// Stores the exact signed message and its chain reference before broadcast.
pub(crate) async fn set_ever_signed(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    reference: &str,
    signed_message: &[u8],
) -> Result<bool> {
    Ok(client
        .execute(SET_EVER_SIGNED, &[trade_id, &reference, &signed_message])
        .await?
        == 1)
}

pub(crate) async fn set_ever_state(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    state: &str,
    reference: Option<&str>,
    error: Option<&str>,
) -> Result<()> {
    client
        .execute(SET_EVER_STATE, &[trade_id, &state, &reference, &error])
        .await?;
    Ok(())
}

pub(crate) async fn note_ever_error(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    error: &str,
) -> Result<()> {
    client.execute(NOTE_EVER_ERROR, &[trade_id, &error]).await?;
    Ok(())
}

/// Soft-deletes every wallet row of one role for a deal (the Ever temp after
/// its payout). Rows are kept for forensics, never hard-deleted.
pub(crate) async fn delete_role(
    client: &(impl GenericClient + Sync),
    trade_id: &Uuid,
    role: &str,
) -> Result<()> {
    client.execute(DELETE_ROLE, &[trade_id, &role]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ATTACH_POOL_SLOT, CLAIM_EVER, CLAIM_GAS, COLUMNS, DELETE_ROLE, DUE_EVER, GAS_FUNDING,
        LOCK_POOL_SLOT, NOTE_ERROR, NOTE_GAS_ERROR, RETURN_TO_POOL, SET_EVER_SIGNED,
        SET_GAS_SIGNED, SET_INTENTS_QUOTE, SET_TREASURY_PAYMENT, STALL_EXPIRED_INTENTS, TREASURY,
    };

    #[test]
    fn the_attempt_cap_becomes_a_terminal_stalled_state() {
        assert!(NOTE_ERROR.contains("THEN 'stalled'"));
        assert!(NOTE_ERROR.contains("attempts+1>=10"));
    }

    #[test]
    fn reserve_rows_are_pool_wallets_and_payouts_record_their_amount() {
        assert!(TREASURY.contains("role='treasury'"));
        assert!(TREASURY.contains("trade_id IS NULL"));
        assert!(TREASURY.contains("status='active'"));
        assert!(SET_TREASURY_PAYMENT.contains("reserve_units=$3"));
        assert!(SET_TREASURY_PAYMENT.contains("payout_signed_transaction=$4"));
        assert!(SET_TREASURY_PAYMENT.contains("state='payout_submitted'"));
        assert!(DELETE_ROLE.contains("status='deleted'"));
        assert!(DUE_EVER.contains("w.role='ever'"));
        assert!(DUE_EVER.contains("a.instructions->'leg'->>'recipient'=w.address"));
        assert!(CLAIM_EVER.contains("ever_state='claim'"));
        assert!(SET_EVER_SIGNED.contains("ever_signed_message=$3"));
        assert!(SET_EVER_SIGNED.contains("WHERE trade_id=$1 AND ever_state='claim'"));
    }

    #[test]
    fn usd_pool_slots_are_selected_atomically_and_reassigned_with_the_trade() {
        assert!(LOCK_POOL_SLOT.contains("role IN ('usd','usd_in')"));
        assert!(LOCK_POOL_SLOT.contains("trade_id IS NULL"));
        assert!(LOCK_POOL_SLOT.contains("FOR UPDATE SKIP LOCKED"));
        assert!(ATTACH_POOL_SLOT.contains("trade_id=$2"));
        assert!(ATTACH_POOL_SLOT.contains("trade_id IS NULL"));
        assert!(RETURN_TO_POOL.contains("trade_id=NULL,status='active'"));
    }

    #[test]
    fn gas_funding_persists_the_envelope_and_replays_it() {
        assert!(CLAIM_GAS.contains("ON CONFLICT(trade_id,role)"));
        assert!(CLAIM_GAS.contains("state IN ('pending','failed')"));
        assert!(SET_GAS_SIGNED.contains("signed_transaction=$4"));
        assert!(SET_GAS_SIGNED.contains("state='claim'"));
        assert!(NOTE_GAS_ERROR.contains("attempts+1>=10"));
        assert!(GAS_FUNDING.contains("FROM otc_temp_gas"));
    }

    #[test]
    fn intents_pending_timeout_is_measured_from_quote_creation() {
        assert!(SET_INTENTS_QUOTE.contains("intents_started_at=now()"));
        assert!(STALL_EXPIRED_INTENTS.contains("intents_started_at < now()-interval '2 hours'"));
        assert!(STALL_EXPIRED_INTENTS.contains("state='intents_submitted'"));
        assert!(STALL_EXPIRED_INTENTS.contains("state='stalled'"));
    }

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

// ---------------------------------------------------------------------------
// Treasury gas top-up (automatic USDT->native swap when treasury runs low)
// ---------------------------------------------------------------------------

const INSERT_GAS_TOPUP: &str =
    "INSERT INTO otc_temp_gas_topups(id,chain,treasury_address) VALUES($1,$2,$3)";
const ACTIVE_GAS_TOPUP: &str =
    "SELECT id,state,reference,deposit_address,deposit_memo,quote_id,source_units,target_units,intents_started_at,attempts \
     FROM otc_temp_gas_topups \
     WHERE chain=$1 AND state NOT IN ('completed','failed') \
     ORDER BY created_at DESC LIMIT 1";
const SET_GAS_TOPUP_SUBMITTED: &str =
    "UPDATE otc_temp_gas_topups SET state='submitted',reference=$2,signed_transaction=$3,last_error=NULL,updated_at=now() \
     WHERE id=$1 AND state='claim'";
const SET_GAS_TOPUP_INTENTS: &str = "UPDATE otc_temp_gas_topups \
     SET state='intents_submitted',deposit_address=$2,deposit_memo=$3,quote_id=$4,\
         source_units=$5,target_units=$6,intents_started_at=now(),last_error=NULL,updated_at=now() \
     WHERE id=$1 AND state IN ('claim','submitted')";
const SET_GAS_TOPUP_COMPLETED: &str =
    "UPDATE otc_temp_gas_topups SET state='completed',last_error=NULL,updated_at=now() WHERE id=$1";
const NOTE_GAS_TOPUP_ERROR: &str = "UPDATE otc_temp_gas_topups \
     SET attempts=attempts+1,last_error=$2,\
         state=CASE WHEN attempts+1>=10 THEN 'failed' ELSE state END,updated_at=now() \
     WHERE id=$1";
const STALL_EXPIRED_GAS_TOPUP: &str =
    "UPDATE otc_temp_gas_topups \
     SET state='failed',last_error='NEAR Intents status remained pending for over 2 hours',updated_at=now() \
     WHERE id=$1 AND state='intents_submitted' AND intents_started_at < now()-interval '2 hours'";
const GET_TOPUP_SIGNED: &str = "SELECT signed_transaction FROM otc_temp_gas_topups WHERE id=$1";

#[derive(Clone, Debug)]
pub(crate) struct GasTopupRow {
    pub id: Uuid,
    pub state: String,
    pub reference: Option<String>,
    pub deposit_address: Option<String>,
    pub deposit_memo: Option<String>,
    pub quote_id: Option<String>,
    pub source_units: Option<String>,
    pub target_units: Option<String>,
    pub intents_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub attempts: i32,
}

pub(crate) async fn insert_gas_topup(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    chain: &str,
    treasury_address: &str,
) -> Result<()> {
    client
        .execute(INSERT_GAS_TOPUP, &[id, &chain, &treasury_address])
        .await?;
    Ok(())
}

pub(crate) async fn active_gas_topup(
    client: &(impl GenericClient + Sync),
    chain: &str,
) -> Result<Option<GasTopupRow>> {
    Ok(client
        .query_opt(ACTIVE_GAS_TOPUP, &[&chain])
        .await?
        .map(|row| GasTopupRow {
            id: row.get(0),
            state: row.get(1),
            reference: row.get(2),
            deposit_address: row.get(3),
            deposit_memo: row.get(4),
            quote_id: row.get(5),
            source_units: row.get(6),
            target_units: row.get(7),
            intents_started_at: row.get(8),
            attempts: row.get(9),
        }))
}

pub(crate) async fn set_gas_topup_submitted(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    reference: &str,
    signed: &[u8],
) -> Result<()> {
    client
        .execute(SET_GAS_TOPUP_SUBMITTED, &[id, &reference, &signed])
        .await?;
    Ok(())
}

pub(crate) async fn set_gas_topup_intents(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    deposit_address: &str,
    deposit_memo: Option<&str>,
    quote_id: &str,
    source_units: &str,
    target_units: &str,
) -> Result<()> {
    client
        .execute(
            SET_GAS_TOPUP_INTENTS,
            &[
                id,
                &deposit_address,
                &deposit_memo,
                &quote_id,
                &source_units,
                &target_units,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn set_gas_topup_completed(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<()> {
    client.execute(SET_GAS_TOPUP_COMPLETED, &[id]).await?;
    Ok(())
}

pub(crate) async fn note_gas_topup_error(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    error: &str,
) -> Result<()> {
    client.execute(NOTE_GAS_TOPUP_ERROR, &[id, &error]).await?;
    Ok(())
}

pub(crate) async fn stall_expired_gas_topup(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<bool> {
    Ok(client.execute(STALL_EXPIRED_GAS_TOPUP, &[id]).await? == 1)
}

/// Returns the persisted signed transaction bytes for a topup row, used to
/// re-broadcast after an ambiguous RPC response.
pub(crate) async fn gas_topup_signed(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<Vec<u8>>> {
    Ok(client
        .query_opt(GET_TOPUP_SIGNED, &[id])
        .await?
        .and_then(|row| row.get(0)))
}

#[cfg(test)]
mod topup_tests {
    use super::{
        ACTIVE_GAS_TOPUP, INSERT_GAS_TOPUP, NOTE_GAS_TOPUP_ERROR, SET_GAS_TOPUP_COMPLETED,
        SET_GAS_TOPUP_INTENTS, SET_GAS_TOPUP_SUBMITTED, STALL_EXPIRED_GAS_TOPUP,
    };

    #[test]
    fn gas_topup_state_machine_queries_are_consistent() {
        assert!(INSERT_GAS_TOPUP.contains("otc_temp_gas_topups"));
        assert!(ACTIVE_GAS_TOPUP.contains("state NOT IN ('completed','failed')"));
        assert!(SET_GAS_TOPUP_SUBMITTED.contains("state='submitted'"));
        assert!(SET_GAS_TOPUP_INTENTS.contains("intents_started_at=now()"));
        assert!(SET_GAS_TOPUP_COMPLETED.contains("state='completed'"));
        assert!(NOTE_GAS_TOPUP_ERROR.contains("attempts+1>=10"));
        assert!(STALL_EXPIRED_GAS_TOPUP.contains("2 hours"));
    }
}
