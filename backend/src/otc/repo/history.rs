use crate::otc::error::Result;
use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;

const PRINTS: &str = "SELECT 'trade' AS side, created_at AS at,
                        CASE WHEN terms->>'direction'='buy' THEN terms->>'output_units'
                             ELSE terms->>'input_units' END AS ever_units,
                        CASE WHEN terms->>'direction'='buy' THEN terms->>'input_units'
                             ELSE terms->>'output_units' END AS usdt_units
                   FROM otc_trades
                  WHERE state='completed' AND terms->>'direction' IN ('buy','sell')
                    AND created_at>=$1
                 UNION ALL
                 SELECT 'trade', created_at, body->>'ever_units', body->>'usdt_units'
                   FROM otc_p2p_trades
                  WHERE state='completed' AND created_at>=$1
                 UNION ALL
                 SELECT CASE WHEN body->>'direction'='buy' THEN 'bid' ELSE 'ask' END,
                        created_at, body->>'ever_units', body->>'usdt_units'
                   FROM otc_public_offers
                  WHERE created_at>=$1
                  ORDER BY at";

/// Returns `(side, at, ever_units, usdt_units)` per settled print, ordered by time.
pub(crate) async fn prints(
    client: &(impl GenericClient + Sync),
    from: &DateTime<Utc>,
) -> Result<Vec<(String, DateTime<Utc>, Option<String>, Option<String>)>> {
    let rows = client.query(PRINTS, &[from]).await?;
    Ok(rows
        .iter()
        .map(|row| {
            (
                row.get("side"),
                row.get("at"),
                row.get("ever_units"),
                row.get("usdt_units"),
            )
        })
        .collect())
}
