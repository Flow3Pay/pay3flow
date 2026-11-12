//! Public aggregate volume measurement for the EVER/USDT market.
//!
//! Only settled trades count: desk trades in `completed` state and P2P deals
//! whose stablecoin leg was delivered. The figures are aggregates, so the
//! endpoint needs no session and exposes no counterparties.

use super::{amount, error::Result, repo, Service};
use chrono::Utc;
use serde_json::{json, Value};

const EVER_DECIMALS: u32 = 9;
const STABLECOIN_DECIMALS: u32 = 6;

/// Guard the JSONB text before casting: a malformed row must not break the sum.
fn units(value: Option<String>) -> u128 {
    value
        .as_deref()
        .and_then(|raw| raw.parse::<u128>().ok())
        .unwrap_or(0)
}

fn window(trades: i64, ever: Option<String>, stablecoin: Option<String>) -> Value {
    let ever = units(ever);
    let stablecoin = units(stablecoin);
    json!({
        "trades": trades,
        "ever": amount::decimal(ever, EVER_DECIMALS),
        "ever_units": ever.to_string(),
        "usdt": amount::decimal(stablecoin, STABLECOIN_DECIMALS),
        "usdt_units": stablecoin.to_string(),
    })
}

impl Service {
    /// Completed volumes over the last 24 hours, 7 days and since launch.
    pub(crate) async fn volumes(&self) -> Result<Value> {
        let client = self.pool.get().await?;
        let row = repo::volumes::volumes(&client).await?;
        Ok(json!({
            "market": "EVER-USDT",
            "updated_at": Utc::now(),
            "windows": {
                "24h": window(row.0, row.3, row.6),
                "7d": window(row.1, row.4, row.7),
                "all": window(row.2, row.5, row.8),
            },
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_units_never_break_the_sum() {
        assert_eq!(units(Some("11889000".into())), 11_889_000);
        assert_eq!(units(Some("1e3".into())), 0);
        assert_eq!(units(Some("-5".into())), 0);
        assert_eq!(units(None), 0);
    }

    #[test]
    fn windows_render_exact_decimals() {
        let value = window(2, Some("11000000000".into()), Some("11889000".into()));
        assert_eq!(value["trades"], 2);
        assert_eq!(value["ever"], "11");
        assert_eq!(value["usdt"], "11.889");
    }
}
