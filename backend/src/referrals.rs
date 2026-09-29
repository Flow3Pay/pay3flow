use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_postgres::Transaction;
use uuid::Uuid;

use crate::db::DbPool;
use crate::exchange::{ExchangeOrder, Minor};

pub const COMMISSION_BPS: i32 = 1_000;
const BPS_DENOMINATOR: Minor = 10_000;
const RECENT_COMMISSION_LIMIT: i64 = 50;

#[derive(Debug, Clone, Serialize)]
pub struct ReferralBalance {
    pub currency: String,
    pub available_minor: Minor,
    pub paid_minor: Minor,
    pub total_earned_minor: Minor,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferralCommission {
    pub id: Uuid,
    pub referred_user_id: Uuid,
    pub order_id: Uuid,
    pub service_fee_minor: Minor,
    pub commission_bps: i32,
    pub amount_minor: Minor,
    pub currency: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferralProfile {
    pub referral_code: String,
    pub commission_bps: i32,
    pub direct_referrals: i64,
    pub network_size: i64,
    pub balances: Vec<ReferralBalance>,
    pub recent_commissions: Vec<ReferralCommission>,
}

/// Calculate a direct-referral reward without ever allocating more than the
/// underlying service fee. Fractional minor units are intentionally rounded
/// down because commissions cannot be represented below one minor unit.
pub fn commission_amount(service_fee_minor: Minor) -> Minor {
    let service_fee_minor = service_fee_minor.max(0);
    let commission_bps = Minor::from(COMMISSION_BPS);
    (service_fee_minor / BPS_DENOMINATOR) * commission_bps
        + ((service_fee_minor % BPS_DENOMINATOR) * commission_bps) / BPS_DENOMINATOR
}

/// Credit the direct referrer for a completed exchange within the caller's
/// completion transaction. The unique order constraint makes retries safe.
pub async fn credit_completed_exchange(
    tx: &Transaction<'_>,
    order: &ExchangeOrder,
) -> Result<bool> {
    let amount_minor = commission_amount(order.pay3flow_fee_minor);
    if amount_minor == 0 {
        return Ok(false);
    }

    let referrer_user_id = tx
        .query_opt(
            "SELECT referred_by_user_id FROM users WHERE id = $1",
            &[&order.user_id],
        )
        .await?
        .context("completed exchange user not found")?
        .get::<_, Option<Uuid>>(0);
    let Some(referrer_user_id) = referrer_user_id else {
        return Ok(false);
    };

    let inserted = tx
        .query_opt(
            r#"
INSERT INTO referral_commissions
    (referrer_user_id, referred_user_id, order_id, service_fee_minor,
     commission_bps, amount_minor, currency)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (order_id) DO NOTHING
RETURNING id
"#,
            &[
                &referrer_user_id,
                &order.user_id,
                &order.id,
                &order.pay3flow_fee_minor,
                &COMMISSION_BPS,
                &amount_minor,
                &order.pay3flow_fee_currency,
            ],
        )
        .await?;
    Ok(inserted.is_some())
}

pub async fn profile(pool: &DbPool, user_id: &Uuid) -> Result<ReferralProfile> {
    let client = pool.get().await?;
    let overview = client
        .query_opt(
            r#"
WITH RECURSIVE network(id) AS (
    SELECT id FROM users WHERE referred_by_user_id = $1
    UNION
    SELECT child.id
    FROM users child
    JOIN network parent ON child.referred_by_user_id = parent.id
)
SELECT u.referral_code,
       (SELECT COUNT(*)::BIGINT FROM users WHERE referred_by_user_id = $1),
       (SELECT COUNT(*)::BIGINT FROM network)
FROM users u
WHERE u.id = $1
"#,
            &[user_id],
        )
        .await?
        .context("referral profile user not found")?;

    let balances = client
        .query(
            r#"
SELECT currency,
       COALESCE(SUM(amount_minor) FILTER (WHERE status = 'available'), 0)::BIGINT,
       COALESCE(SUM(amount_minor) FILTER (WHERE status = 'paid'), 0)::BIGINT,
       COALESCE(SUM(amount_minor) FILTER (WHERE status IN ('available', 'paid')), 0)::BIGINT
FROM referral_commissions
WHERE referrer_user_id = $1
GROUP BY currency
ORDER BY currency
"#,
            &[user_id],
        )
        .await?
        .into_iter()
        .map(|row| ReferralBalance {
            currency: row.get(0),
            available_minor: row.get(1),
            paid_minor: row.get(2),
            total_earned_minor: row.get(3),
        })
        .collect();

    let recent_commissions = client
        .query(
            r#"
SELECT id, referred_user_id, order_id, service_fee_minor, commission_bps,
       amount_minor, currency, status, created_at, paid_at
FROM referral_commissions
WHERE referrer_user_id = $1
ORDER BY created_at DESC
LIMIT $2
"#,
            &[user_id, &RECENT_COMMISSION_LIMIT],
        )
        .await?
        .into_iter()
        .map(|row| ReferralCommission {
            id: row.get(0),
            referred_user_id: row.get(1),
            order_id: row.get(2),
            service_fee_minor: row.get(3),
            commission_bps: row.get(4),
            amount_minor: row.get(5),
            currency: row.get(6),
            status: row.get(7),
            created_at: row.get(8),
            paid_at: row.get(9),
        })
        .collect();

    Ok(ReferralProfile {
        referral_code: overview.get(0),
        commission_bps: COMMISSION_BPS,
        direct_referrals: overview.get(1),
        network_size: overview.get(2),
        balances,
        recent_commissions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commission_is_ten_percent_rounded_down_to_minor_units() {
        let cases = [
            (0, 0),
            (1, 0),
            (9, 0),
            (10, 1),
            (99, 9),
            (100, 10),
            (12_345, 1_234),
        ];
        for (fee, expected) in cases {
            assert_eq!(commission_amount(fee), expected, "fee: {fee}");
        }
    }

    #[test]
    fn negative_fee_cannot_create_a_commission() {
        assert_eq!(commission_amount(-100), 0);
    }

    #[test]
    fn commission_math_does_not_overflow_for_large_fees() {
        assert_eq!(commission_amount(Minor::MAX), Minor::MAX / 10);
    }
}
