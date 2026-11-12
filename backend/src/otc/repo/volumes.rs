use crate::otc::error::Result;
use deadpool_postgres::GenericClient;

const VOLUMES: &str = "WITH fills AS (
                   SELECT created_at,
                          CASE WHEN terms->>'direction'='buy' THEN terms->>'output_units' ELSE terms->>'input_units' END AS ever_units,
                          CASE WHEN terms->>'direction'='buy' THEN terms->>'input_units' ELSE terms->>'output_units' END AS usdt_units
                     FROM otc_trades
                    WHERE state='completed' AND terms->>'direction' IN ('buy','sell')
                   UNION ALL
                   SELECT created_at, body->>'ever_units', body->>'usdt_units'
                     FROM otc_p2p_trades
                    WHERE state='completed'
                 ), exact AS (
                   SELECT created_at,
                          CASE WHEN ever_units ~ '^[0-9]{1,40}$' THEN ever_units::numeric END AS ever_units,
                          CASE WHEN usdt_units ~ '^[0-9]{1,40}$' THEN usdt_units::numeric END AS usdt_units
                     FROM fills
                 )
                 SELECT
                   count(*) FILTER (WHERE created_at>=now()-interval '24 hours'),
                   count(*) FILTER (WHERE created_at>=now()-interval '7 days'),
                   count(*),
                   COALESCE(sum(ever_units) FILTER (WHERE created_at>=now()-interval '24 hours'),0)::text,
                   COALESCE(sum(ever_units) FILTER (WHERE created_at>=now()-interval '7 days'),0)::text,
                   COALESCE(sum(ever_units),0)::text,
                   COALESCE(sum(usdt_units) FILTER (WHERE created_at>=now()-interval '24 hours'),0)::text,
                   COALESCE(sum(usdt_units) FILTER (WHERE created_at>=now()-interval '7 days'),0)::text,
                   COALESCE(sum(usdt_units),0)::text
                 FROM exact";

/// Completed trade counts and unit sums per window:
/// `(trades 24h, trades 7d, trades all, ever 24h, ever 7d, ever all,
/// usdt 24h, usdt 7d, usdt all)`.
#[allow(clippy::type_complexity)]
pub(crate) async fn volumes(
    client: &(impl GenericClient + Sync),
) -> Result<(
    i64,
    i64,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    let row = client.query_one(VOLUMES, &[]).await?;
    Ok((
        row.get(0),
        row.get(1),
        row.get(2),
        row.get(3),
        row.get(4),
        row.get(5),
        row.get(6),
        row.get(7),
        row.get(8),
    ))
}
