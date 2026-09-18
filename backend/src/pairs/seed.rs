//! Seed catalog of bank exchange pairs.
//!
//! MVP starts with a single enabled pair: Armenia/AMD -> Russia/RUB. More
//! pairs should be added later through seed data or the admin API, not by
//! expanding the first boot catalog into a huge hardcoded matrix.

use crate::db::DbPool;
use crate::pairs::{repo_upsert, NewPair};

const ICON_TMPL: &str = "https://www.google.com/s2/favicons?domain={}&sz=128";

fn icon_url(domain: &str) -> String {
    ICON_TMPL.replacen("{}", domain, 1)
}

pub fn build_pairs() -> Vec<NewPair> {
    vec![NewPair {
        from_scheme: "Visa".to_string(),
        from_bank: "Ardshinbank".to_string(),
        from_bank_icon_url: Some(icon_url("ardshinbank.am")),
        to_scheme: "MIR".to_string(),
        to_bank: "Sberbank".to_string(),
        to_bank_icon_url: Some(icon_url("sberbank.ru")),
        country: Some("RU".to_string()),
        currencies: Some("AMD,RUB".to_string()),
        status: Some("enabled".to_string()),
        daily_limit_minor: None,
        daily_limit_currency: None,
    }]
}

/// Seed the `exchange_pairs` table idempotently. Admin edits survive because
/// rows are upserted on their route key, never truncated.
pub async fn seed_exchange_pairs(pool: &DbPool) -> anyhow::Result<usize> {
    let pairs = build_pairs();
    for pair in &pairs {
        let _ = repo_upsert(pool, pair).await?;
    }
    Ok(pairs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_starts_with_one_mvp_pair() {
        let pairs = build_pairs();
        assert_eq!(pairs.len(), 1);
    }

    #[test]
    fn mvp_pair_is_armenia_amd_to_russia_rub() {
        let pair = build_pairs().pop().expect("mvp pair exists");

        assert_eq!(pair.from_bank, "Ardshinbank");
        assert_eq!(pair.to_bank, "Sberbank");
        assert_eq!(pair.country.as_deref(), Some("RU"));
        assert_eq!(pair.currencies.as_deref(), Some("AMD,RUB"));
        assert_eq!(pair.status.as_deref(), Some("enabled"));
        assert!(!pair.from_bank_icon_url.as_deref().unwrap_or("").is_empty());
        assert!(!pair.to_bank_icon_url.as_deref().unwrap_or("").is_empty());
    }
}
