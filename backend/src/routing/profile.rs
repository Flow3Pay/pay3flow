use anyhow::Result;

use crate::db::DbPool;

/// Structured, rules-ready view of an acquirer — parsed from seed data
/// (`geo`/`currencies` split into tokens, `fee` into percent+flat, `limits`
/// into min/max + currency) so the fallback matcher can compare them directly.
#[derive(Debug, Clone)]
pub struct AcquirerProfile {
    pub slug: String,
    pub name: String,
    pub geo: Vec<String>,
    pub currencies: Vec<String>,
    pub fee_percent: f64,
    pub fee_flat: f64,
    pub min_amount: Option<f64>,
    pub max_amount: Option<f64>,
    pub amount_currency: Option<String>,
    pub status: String,
}

/// Load the fallback matcher pool from the database catalog migration.
pub async fn load_pool(pool: &DbPool) -> Result<Vec<AcquirerProfile>> {
    let client = pool.get().await?;
    let rows = client
        .query(
            r#"
SELECT slug, name, geo, currencies, fee_percent, fee_fixed,
       min_amount, max_amount, amount_currency, status
FROM acquirers
WHERE active = TRUE
ORDER BY slug
"#,
            &[],
        )
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| AcquirerProfile {
            slug: row.get("slug"),
            name: row.get("name"),
            geo: split_tokens(row.get("geo")),
            currencies: split_tokens(row.get("currencies")),
            fee_percent: row.get("fee_percent"),
            fee_flat: row.get::<_, i64>("fee_fixed") as f64 / 100.0,
            min_amount: row
                .get::<_, Option<i64>>("min_amount")
                .map(|value| value as f64),
            max_amount: row
                .get::<_, Option<i64>>("max_amount")
                .map(|value| value as f64),
            amount_currency: row.get("amount_currency"),
            status: row.get("status"),
        })
        .collect())
}

fn split_tokens(s: String) -> Vec<String> {
    s.split('|')
        .map(|t| t.trim().to_uppercase())
        .filter(|t| !t.is_empty())
        .collect()
}

#[cfg(test)]
pub(crate) fn test_pool() -> Vec<AcquirerProfile> {
    [
        (
            "stripe", "Stripe", "US|EU", "USD|EUR", 3.1, 0.5, 100_000.0, "USD",
        ),
        (
            "adyen",
            "Adyen",
            "NL|EU|US|GB",
            "EUR|USD|GBP",
            2.9,
            0.3,
            500_000.0,
            "EUR",
        ),
        (
            "checkout",
            "Checkout.com",
            "UK|EU|US",
            "EUR|USD|GBP",
            2.8,
            0.3,
            250_000.0,
            "USD",
        ),
        (
            "mollie", "Mollie", "NL|EU", "EUR", 1.5, 0.25, 50_000.0, "EUR",
        ),
        (
            "payoneer",
            "Payoneer",
            "US|EU|Global",
            "USD|EUR|GBP",
            2.5,
            0.3,
            1_000_000.0,
            "USD",
        ),
        (
            "wise",
            "Wise",
            "UK|EU|US|Global",
            "USD|EUR|GBP",
            2.5,
            0.0,
            1_500_000.0,
            "USD",
        ),
    ]
    .into_iter()
    .map(
        |(slug, name, geo, currencies, fee_percent, fee_flat, max_amount, amount_currency)| {
            AcquirerProfile {
                slug: slug.into(),
                name: name.into(),
                geo: split_tokens(geo.into()),
                currencies: split_tokens(currencies.into()),
                fee_percent,
                fee_flat,
                min_amount: Some(1.0),
                max_amount: Some(max_amount),
                amount_currency: Some(amount_currency.into()),
                status: "active".into(),
            }
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_database_catalog_tokens() {
        assert_eq!(split_tokens("NL|EU".into()), ["NL", "EU"]);
        assert_eq!(split_tokens(" usd | eur ".into()), ["USD", "EUR"]);
    }
}
