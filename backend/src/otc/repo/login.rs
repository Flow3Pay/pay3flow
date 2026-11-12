//! Session, wallet-challenge and relay-link statements for sign-in and revocation.
use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use uuid::Uuid;

const LOCK_WALLET: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1,0))";
const WALLET_OWNERS: &str = "SELECT actor FROM otc_login_wallets WHERE chain=$1 AND address=$2 UNION SELECT actor FROM otc_wallets WHERE chain=$1 AND lower(address)=$2";
const BIND_LOGIN_WALLET: &str =
    "INSERT INTO otc_login_wallets(chain,address,actor) VALUES($1,$2,$3) ON CONFLICT DO NOTHING";
const RECENT_CHALLENGES: &str =
    "SELECT count(*) FROM otc_login_challenges WHERE chain=$1 AND address=$2 AND expires_at>now()";
const INSERT_LOGIN_CHALLENGE: &str =
    "INSERT INTO otc_login_challenges(id,chain,address,message,expires_at) VALUES($1,$2,$3,$4,$5)";
const LOGIN_CHALLENGE: &str = "SELECT chain,address,message FROM otc_login_challenges WHERE id=$1 AND expires_at>now() AND consumed_at IS NULL";
const INSERT_ACTOR_LINK: &str =
    "INSERT INTO otc_actor_links(actor,credential,managed,provisioned) VALUES($1,$2,TRUE,FALSE)";
const CONSUME_LOGIN_CHALLENGE: &str = "UPDATE otc_login_challenges SET consumed_at=now() WHERE id=$1 AND consumed_at IS NULL AND expires_at>now()";
const UNREVOKED_CREDENTIAL: &str =
    "SELECT credential FROM otc_actor_links WHERE actor=$1 AND NOT revoked";
const INSERT_SESSION: &str = "INSERT INTO otc_sessions(id,actor,credential,expires_at,login_chain,login_address) VALUES($1,$2,$3,$4,$5,$6)";
const INSERT_PLAIN_SESSION: &str =
    "INSERT INTO otc_sessions(id,actor,credential,expires_at) VALUES($1,$2,$3,$4)";
const VERIFY_WALLET: &str = "INSERT INTO otc_wallets(actor,chain,address,verified_at) VALUES($1,$2,$3,now()) ON CONFLICT(actor,chain) DO UPDATE SET address=$3,verified_at=now()";
const LOCK_RELAY_LINK: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1,0))";
const ACTOR_LINK: &str =
    "SELECT credential,managed,provisioned FROM otc_actor_links WHERE actor=$1 AND NOT revoked";
const SET_CREDENTIAL: &str =
    "UPDATE otc_actor_links SET credential=$2,updated_at=now() WHERE actor=$1 AND NOT revoked";
const MARK_PROVISIONED: &str =
    "UPDATE otc_actor_links SET provisioned=TRUE,updated_at=now() WHERE actor=$1 AND NOT revoked";
const UPSERT_ACTOR_LINK: &str = "INSERT INTO otc_actor_links(actor,credential) VALUES($1,$2) ON CONFLICT(actor) DO UPDATE SET credential=$2,revoked=FALSE,provisioned=TRUE,updated_at=now()";
const SESSION_INFO: &str = "SELECT expires_at,login_chain,login_address FROM otc_sessions WHERE id=$1 AND NOT revoked AND expires_at>now()";
const VERIFIED_WALLETS: &str = "SELECT chain,address,verified_at FROM otc_wallets WHERE actor=$1 AND verified_at>now()-make_interval(days=>$2) ORDER BY chain";
const INSERT_WALLET_CHALLENGE: &str = "INSERT INTO otc_wallet_challenges(id,session_id,chain,address,message,expires_at) VALUES($1,$2,$3,$4,$5,$6)";
const WALLET_CHALLENGE: &str = "SELECT chain,address,message FROM otc_wallet_challenges WHERE id=$1 AND session_id=$2 AND expires_at>now() AND consumed_at IS NULL";
const CONSUME_WALLET_CHALLENGE: &str = "UPDATE otc_wallet_challenges SET consumed_at=now() WHERE id=$1 AND session_id=$2 AND consumed_at IS NULL AND expires_at>now()";
const LIVE_SESSION: &str =
    "SELECT id FROM otc_sessions WHERE id=$1 AND NOT revoked AND expires_at>now() FOR SHARE";
const REVOKE_SESSION: &str = "UPDATE otc_sessions SET revoked=TRUE WHERE id=$1";

/// Advisory lock keyed on `otc-login:<chain>:<address>`; the caller formats the key.
pub(crate) async fn lock_wallet(client: &(impl GenericClient + Sync), key: &str) -> Result<()> {
    client.query_one(LOCK_WALLET, &[&key]).await?;
    Ok(())
}

pub(crate) async fn wallet_owners(
    client: &(impl GenericClient + Sync),
    chain: &str,
    address: &str,
) -> Result<Vec<String>> {
    Ok(client
        .query(WALLET_OWNERS, &[&chain, &address])
        .await?
        .iter()
        .map(|row| row.get(0))
        .collect())
}

pub(crate) async fn bind_login_wallet(
    client: &(impl GenericClient + Sync),
    chain: &str,
    address: &str,
    actor: &str,
) -> Result<()> {
    client
        .execute(BIND_LOGIN_WALLET, &[&chain, &address, &actor])
        .await?;
    Ok(())
}

pub(crate) async fn recent_challenges(
    client: &(impl GenericClient + Sync),
    chain: &str,
    address: &str,
) -> Result<i64> {
    let row = client
        .query_one(RECENT_CHALLENGES, &[&chain, &address])
        .await?;
    Ok(row.get(0))
}

pub(crate) async fn insert_login_challenge(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    chain: &str,
    address: &str,
    message: &str,
    expires: &DateTime<Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT_LOGIN_CHALLENGE,
            &[id, &chain, &address, &message, &expires],
        )
        .await?;
    Ok(())
}

/// Returns `(chain, address, message)`.
pub(crate) async fn login_challenge(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<(String, String, String)>> {
    let row = client.query_opt(LOGIN_CHALLENGE, &[id]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn insert_actor_link(
    client: &(impl GenericClient + Sync),
    actor: &str,
    credential: &str,
) -> Result<()> {
    client
        .execute(INSERT_ACTOR_LINK, &[&actor, &credential])
        .await?;
    Ok(())
}

/// Returns the number of challenges consumed.
pub(crate) async fn consume_login_challenge(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<u64> {
    Ok(client.execute(CONSUME_LOGIN_CHALLENGE, &[id]).await?)
}

pub(crate) async fn unrevoked_credential(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<Option<String>> {
    Ok(client
        .query_opt(UNREVOKED_CREDENTIAL, &[&actor])
        .await?
        .map(|row| row.get(0)))
}

pub(crate) async fn insert_session(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
    credential: &str,
    expires: &DateTime<Utc>,
    login_chain: Option<&String>,
    login_address: Option<&String>,
) -> Result<()> {
    client
        .execute(
            INSERT_SESSION,
            &[
                id,
                &actor,
                &credential,
                &expires,
                &login_chain,
                &login_address,
            ],
        )
        .await?;
    Ok(())
}

pub(crate) async fn insert_plain_session(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    actor: &str,
    credential: &str,
    expires: &DateTime<Utc>,
) -> Result<()> {
    client
        .execute(INSERT_PLAIN_SESSION, &[id, &actor, &credential, &expires])
        .await?;
    Ok(())
}

pub(crate) async fn verify_wallet(
    client: &(impl GenericClient + Sync),
    actor: &str,
    chain: &str,
    address: &str,
) -> Result<()> {
    client
        .execute(VERIFY_WALLET, &[&actor, &chain, &address])
        .await?;
    Ok(())
}

/// Advisory lock keyed on `otc-relay:<actor>`; the caller formats the key.
pub(crate) async fn lock_relay_link(client: &(impl GenericClient + Sync), key: &str) -> Result<()> {
    client.query_one(LOCK_RELAY_LINK, &[&key]).await?;
    Ok(())
}

/// Returns `(credential, managed, provisioned)`.
pub(crate) async fn actor_link(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<Option<(String, bool, bool)>> {
    let row = client.query_opt(ACTOR_LINK, &[&actor]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

pub(crate) async fn set_credential(
    client: &(impl GenericClient + Sync),
    actor: &str,
    credential: &str,
) -> Result<()> {
    client
        .execute(SET_CREDENTIAL, &[&actor, &credential])
        .await?;
    Ok(())
}

/// The relay claim stage commits with its own pooled connection.
pub(crate) async fn mark_provisioned(
    client: &(impl GenericClient + Sync),
    actor: &str,
) -> Result<()> {
    client.execute(MARK_PROVISIONED, &[&actor]).await?;
    Ok(())
}

pub(crate) async fn upsert_actor_link(
    client: &(impl GenericClient + Sync),
    actor: &str,
    credential: &str,
) -> Result<()> {
    client
        .execute(UPSERT_ACTOR_LINK, &[&actor, &credential])
        .await?;
    Ok(())
}

/// Returns `(expires_at, login_chain, login_address)`.
pub(crate) async fn session_info(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
) -> Result<Option<(DateTime<Utc>, Option<String>, Option<String>)>> {
    let row = client.query_opt(SESSION_INFO, &[id]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

/// Returns `(chain, address, verified_at)`.
pub(crate) async fn verified_wallets(
    client: &(impl GenericClient + Sync),
    actor: &str,
    days: i32,
) -> Result<Vec<(String, String, DateTime<Utc>)>> {
    Ok(client
        .query(VERIFIED_WALLETS, &[&actor, &days])
        .await?
        .iter()
        .map(|row| (row.get(0), row.get(1), row.get(2)))
        .collect())
}

pub(crate) async fn insert_wallet_challenge(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    session: &Uuid,
    chain: &str,
    address: &str,
    message: &str,
    expires: &DateTime<Utc>,
) -> Result<()> {
    client
        .execute(
            INSERT_WALLET_CHALLENGE,
            &[id, session, &chain, &address, &message, &expires],
        )
        .await?;
    Ok(())
}

/// Returns `(chain, address, message)`.
pub(crate) async fn wallet_challenge(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    session: &Uuid,
) -> Result<Option<(String, String, String)>> {
    let row = client.query_opt(WALLET_CHALLENGE, &[id, session]).await?;
    Ok(row.map(|row| (row.get(0), row.get(1), row.get(2))))
}

/// Returns the number of challenges consumed.
pub(crate) async fn consume_wallet_challenge(
    client: &(impl GenericClient + Sync),
    id: &Uuid,
    session: &Uuid,
) -> Result<u64> {
    Ok(client
        .execute(CONSUME_WALLET_CHALLENGE, &[id, session])
        .await?)
}

pub(crate) async fn live_session(
    client: &(impl GenericClient + Sync),
    session: &Uuid,
) -> Result<bool> {
    Ok(client.query_opt(LIVE_SESSION, &[session]).await?.is_some())
}

pub(crate) async fn revoke_session(
    client: &(impl GenericClient + Sync),
    session: &Uuid,
) -> Result<()> {
    client.execute(REVOKE_SESSION, &[session]).await?;
    Ok(())
}
