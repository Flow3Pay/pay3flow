use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::db::DbPool;
use crate::exchange::model::{
    Minor, NewTokenLedgerOperation, TokenLedgerAccount, TokenLedgerOperation,
};
use crate::exchange::status::{LedgerOperationStatus, LedgerOperationType};

const FAKE_SOLVER_INITIAL_BALANCE_MINOR: Minor = 1_000_000_000_000;

const SELECT_ACCOUNT: &str = r#"
SELECT id, owner_type, owner_id, currency, available_minor, reserved_minor, locked_minor,
       metadata, created_at, updated_at
FROM token_ledger_accounts
"#;

const SELECT_OPERATION: &str = r#"
SELECT id, idempotency_key, account_id, operation_type, amount_minor, currency, status,
       related_order_id, related_settlement_id, previous_operation_id, metadata, created_at
FROM token_ledger_operations
"#;

#[derive(Debug, Clone, Copy, Default)]
pub struct LedgerOperationLinks {
    pub related_order_id: Option<Uuid>,
    pub related_settlement_id: Option<Uuid>,
}

impl LedgerOperationLinks {
    pub fn order_settlement(order_id: Uuid, settlement_id: Uuid) -> Self {
        Self {
            related_order_id: Some(order_id),
            related_settlement_id: Some(settlement_id),
        }
    }
}

pub async fn ensure_solver_account(
    pool: &DbPool,
    solver_id: &Uuid,
    currency: &str,
) -> Result<TokenLedgerAccount> {
    ensure_account(
        pool,
        "solver",
        solver_id,
        currency,
        FAKE_SOLVER_INITIAL_BALANCE_MINOR,
        json!({ "mock": true, "source": "fake_solver_seed" }),
    )
    .await
}

pub async fn ensure_pay3flow_wallet(
    pool: &DbPool,
    order_id: &Uuid,
    currency: &str,
) -> Result<TokenLedgerAccount> {
    ensure_account(
        pool,
        "pay3flow_wallet",
        order_id,
        currency,
        0,
        json!({ "mock": true, "scope": "order_wallet" }),
    )
    .await
}

pub async fn account_by_id(pool: &DbPool, id: &Uuid) -> Result<Option<TokenLedgerAccount>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_ACCOUNT} WHERE id = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[id]).await?;
    Ok(row.map(row_to_account))
}

pub async fn operations_for_order(
    pool: &DbPool,
    order_id: &Uuid,
) -> Result<Vec<TokenLedgerOperation>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!(
            "{SELECT_OPERATION} WHERE related_order_id = $1 ORDER BY created_at"
        ))
        .await?;
    let rows = client.query(&stmt, &[order_id]).await?;
    Ok(rows.into_iter().map(row_to_operation).collect())
}

pub async fn reserve(
    pool: &DbPool,
    account_id: &Uuid,
    amount_minor: Minor,
    currency: &str,
    idempotency_key: &str,
    related_order_id: Option<Uuid>,
    related_settlement_id: Option<Uuid>,
) -> Result<TokenLedgerOperation> {
    apply(
        pool,
        ApplyOperation {
            idempotency_key,
            account_id,
            operation_type: LedgerOperationType::Reserve,
            amount_minor,
            currency,
            related_order_id,
            related_settlement_id,
            previous_operation_id: None,
            metadata: json!({}),
        },
    )
    .await
}

pub async fn lock(
    pool: &DbPool,
    account_id: &Uuid,
    amount_minor: Minor,
    currency: &str,
    idempotency_key: &str,
    previous_operation_id: Uuid,
    links: LedgerOperationLinks,
) -> Result<TokenLedgerOperation> {
    apply(
        pool,
        ApplyOperation {
            idempotency_key,
            account_id,
            operation_type: LedgerOperationType::Lock,
            amount_minor,
            currency,
            related_order_id: links.related_order_id,
            related_settlement_id: links.related_settlement_id,
            previous_operation_id: Some(previous_operation_id),
            metadata: json!({}),
        },
    )
    .await
}

pub async fn release(
    pool: &DbPool,
    account_id: &Uuid,
    amount_minor: Minor,
    currency: &str,
    idempotency_key: &str,
    previous_operation_id: Uuid,
    links: LedgerOperationLinks,
) -> Result<TokenLedgerOperation> {
    apply(
        pool,
        ApplyOperation {
            idempotency_key,
            account_id,
            operation_type: LedgerOperationType::Release,
            amount_minor,
            currency,
            related_order_id: links.related_order_id,
            related_settlement_id: links.related_settlement_id,
            previous_operation_id: Some(previous_operation_id),
            metadata: json!({}),
        },
    )
    .await
}

pub async fn rollback(
    pool: &DbPool,
    account_id: &Uuid,
    amount_minor: Minor,
    currency: &str,
    idempotency_key: &str,
    previous_operation_id: Uuid,
    links: LedgerOperationLinks,
) -> Result<TokenLedgerOperation> {
    apply(
        pool,
        ApplyOperation {
            idempotency_key,
            account_id,
            operation_type: LedgerOperationType::Rollback,
            amount_minor,
            currency,
            related_order_id: links.related_order_id,
            related_settlement_id: links.related_settlement_id,
            previous_operation_id: Some(previous_operation_id),
            metadata: json!({}),
        },
    )
    .await
}

pub async fn credit(
    pool: &DbPool,
    account_id: &Uuid,
    amount_minor: Minor,
    currency: &str,
    idempotency_key: &str,
    related_order_id: Option<Uuid>,
    related_settlement_id: Option<Uuid>,
) -> Result<TokenLedgerOperation> {
    apply(
        pool,
        ApplyOperation {
            idempotency_key,
            account_id,
            operation_type: LedgerOperationType::Credit,
            amount_minor,
            currency,
            related_order_id,
            related_settlement_id,
            previous_operation_id: None,
            metadata: json!({ "wallet_ref": account_id }),
        },
    )
    .await
}

async fn ensure_account(
    pool: &DbPool,
    owner_type: &str,
    owner_id: &Uuid,
    currency: &str,
    available_minor: Minor,
    metadata: Value,
) -> Result<TokenLedgerAccount> {
    let client = pool.get().await?;
    let row = client
        .query_one(
            r#"
INSERT INTO token_ledger_accounts
    (owner_type, owner_id, currency, available_minor, metadata)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (owner_type, owner_id, currency) DO UPDATE SET
    metadata = token_ledger_accounts.metadata || EXCLUDED.metadata,
    updated_at = now()
RETURNING id, owner_type, owner_id, currency, available_minor, reserved_minor, locked_minor,
          metadata, created_at, updated_at
"#,
            &[
                &owner_type,
                owner_id,
                &currency,
                &available_minor,
                &metadata,
            ],
        )
        .await?;
    Ok(row_to_account(row))
}

struct ApplyOperation<'a> {
    idempotency_key: &'a str,
    account_id: &'a Uuid,
    operation_type: LedgerOperationType,
    amount_minor: Minor,
    currency: &'a str,
    related_order_id: Option<Uuid>,
    related_settlement_id: Option<Uuid>,
    previous_operation_id: Option<Uuid>,
    metadata: Value,
}

async fn apply(pool: &DbPool, op: ApplyOperation<'_>) -> Result<TokenLedgerOperation> {
    if op.amount_minor <= 0 {
        bail!("ledger amount must be greater than 0");
    }

    if let Some(existing) = operation_by_idempotency_key(pool, op.idempotency_key).await? {
        return Ok(existing);
    }

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let account_row = tx
        .query_one(
            &format!("{SELECT_ACCOUNT} WHERE id = $1 AND currency = $2 FOR UPDATE"),
            &[op.account_id, &op.currency],
        )
        .await
        .context("ledger account not found")?;
    let account = row_to_account(account_row);

    let (available, reserved, locked) =
        apply_balances(&account, op.operation_type, op.amount_minor)?;

    tx.execute(
        r#"
UPDATE token_ledger_accounts
SET available_minor = $2,
    reserved_minor = $3,
    locked_minor = $4,
    updated_at = now()
WHERE id = $1
"#,
        &[op.account_id, &available, &reserved, &locked],
    )
    .await?;

    let operation = insert_operation_tx(
        &tx,
        &NewTokenLedgerOperation {
            idempotency_key: op.idempotency_key.to_string(),
            account_id: *op.account_id,
            operation_type: op.operation_type,
            amount_minor: op.amount_minor,
            currency: op.currency.to_string(),
            related_order_id: op.related_order_id,
            related_settlement_id: op.related_settlement_id,
            previous_operation_id: op.previous_operation_id,
            metadata: op.metadata,
        },
    )
    .await;

    match operation {
        Ok(operation) => {
            tx.commit().await?;
            Ok(operation)
        }
        Err(err) if err.to_string().contains("duplicate key") => {
            tx.rollback().await?;
            operation_by_idempotency_key(pool, op.idempotency_key)
                .await?
                .context("ledger idempotency conflict found no winning operation")
        }
        Err(err) => Err(err),
    }
}

fn apply_balances(
    account: &TokenLedgerAccount,
    operation_type: LedgerOperationType,
    amount: Minor,
) -> Result<(Minor, Minor, Minor)> {
    let mut available = account.available_minor;
    let mut reserved = account.reserved_minor;
    let mut locked = account.locked_minor;

    match operation_type {
        LedgerOperationType::Reserve => {
            if available < amount {
                bail!("ledger available balance is too low");
            }
            available -= amount;
            reserved += amount;
        }
        LedgerOperationType::Lock => {
            if reserved < amount {
                bail!("ledger reserved balance is too low");
            }
            reserved -= amount;
            locked += amount;
        }
        LedgerOperationType::Release => {
            if locked < amount {
                bail!("cannot release ledger funds without a lock");
            }
            locked -= amount;
        }
        LedgerOperationType::Rollback => {
            if locked >= amount {
                locked -= amount;
                available += amount;
            } else if reserved >= amount {
                reserved -= amount;
                available += amount;
            } else {
                bail!("cannot rollback ledger funds without reserve or lock");
            }
        }
        LedgerOperationType::Credit => {
            available += amount;
        }
    }

    Ok((available, reserved, locked))
}

async fn operation_by_idempotency_key(
    pool: &DbPool,
    idempotency_key: &str,
) -> Result<Option<TokenLedgerOperation>> {
    let client = pool.get().await?;
    let stmt = client
        .prepare_cached(&format!("{SELECT_OPERATION} WHERE idempotency_key = $1"))
        .await?;
    let row = client.query_opt(&stmt, &[&idempotency_key]).await?;
    Ok(row.map(row_to_replayed_operation))
}

async fn insert_operation_tx(
    tx: &tokio_postgres::Transaction<'_>,
    op: &NewTokenLedgerOperation,
) -> Result<TokenLedgerOperation> {
    let row = tx
        .query_one(
            r#"
INSERT INTO token_ledger_operations
    (idempotency_key, account_id, operation_type, amount_minor, currency, status,
     related_order_id, related_settlement_id, previous_operation_id, metadata)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
RETURNING id, idempotency_key, account_id, operation_type, amount_minor, currency, status,
          related_order_id, related_settlement_id, previous_operation_id, metadata, created_at
"#,
            &[
                &op.idempotency_key,
                &op.account_id,
                &op.operation_type.as_str(),
                &op.amount_minor,
                &op.currency,
                &LedgerOperationStatus::Applied.as_str(),
                &op.related_order_id,
                &op.related_settlement_id,
                &op.previous_operation_id,
                &op.metadata,
            ],
        )
        .await?;
    Ok(row_to_operation(row))
}

fn row_to_account(row: tokio_postgres::Row) -> TokenLedgerAccount {
    TokenLedgerAccount {
        id: row.get(0),
        owner_type: row.get(1),
        owner_id: row.get(2),
        currency: row.get(3),
        available_minor: row.get(4),
        reserved_minor: row.get(5),
        locked_minor: row.get(6),
        metadata: row.get(7),
        created_at: row.get(8),
        updated_at: row.get(9),
    }
}

fn row_to_operation(row: tokio_postgres::Row) -> TokenLedgerOperation {
    TokenLedgerOperation {
        id: row.get(0),
        idempotency_key: row.get(1),
        account_id: row.get(2),
        operation_type: LedgerOperationType::parse(&row.get::<_, String>(3))
            .unwrap_or(LedgerOperationType::Reserve),
        amount_minor: row.get(4),
        currency: row.get(5),
        status: LedgerOperationStatus::parse(&row.get::<_, String>(6))
            .unwrap_or(LedgerOperationStatus::Applied),
        related_order_id: row.try_get(7).ok().flatten(),
        related_settlement_id: row.try_get(8).ok().flatten(),
        previous_operation_id: row.try_get(9).ok().flatten(),
        metadata: row.get(10),
        created_at: row.get(11),
    }
}

fn row_to_replayed_operation(row: tokio_postgres::Row) -> TokenLedgerOperation {
    TokenLedgerOperation {
        status: LedgerOperationStatus::Replayed,
        ..row_to_operation(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn account(available: Minor, reserved: Minor, locked: Minor) -> TokenLedgerAccount {
        TokenLedgerAccount {
            id: Uuid::new_v4(),
            owner_type: "solver".into(),
            owner_id: Uuid::new_v4(),
            currency: "RUB".into(),
            available_minor: available,
            reserved_minor: reserved,
            locked_minor: locked,
            metadata: json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn release_requires_locked_balance() {
        let err = apply_balances(&account(100, 50, 0), LedgerOperationType::Release, 50)
            .unwrap_err()
            .to_string();

        assert!(err.contains("without a lock"));
    }

    #[test]
    fn balance_cannot_be_over_reserved() {
        let err = apply_balances(&account(49, 0, 0), LedgerOperationType::Reserve, 50)
            .unwrap_err()
            .to_string();

        assert!(err.contains("too low"));
    }

    #[test]
    fn double_release_and_double_rollback_are_rejected_without_idempotency_replay() {
        let (available, reserved, locked) =
            apply_balances(&account(100, 0, 50), LedgerOperationType::Release, 50).unwrap();
        assert_eq!((available, reserved, locked), (100, 0, 0));

        let released = account(available, reserved, locked);
        assert!(apply_balances(&released, LedgerOperationType::Release, 50).is_err());
        assert!(apply_balances(&released, LedgerOperationType::Rollback, 50).is_err());
    }

    #[test]
    fn rollback_returns_reserved_or_locked_funds_once() {
        let (available, reserved, locked) =
            apply_balances(&account(100, 50, 0), LedgerOperationType::Rollback, 50).unwrap();
        assert_eq!((available, reserved, locked), (150, 0, 0));

        let (available, reserved, locked) =
            apply_balances(&account(100, 0, 50), LedgerOperationType::Rollback, 50).unwrap();
        assert_eq!((available, reserved, locked), (150, 0, 0));
    }
}
