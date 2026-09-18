//! Seed catalog of bank exchange pairs (PLAN 46b): which card of which sender
//! bank can pay which card of which recipient bank. Every scheme (Visa,
//! MasterCard, МИР, UnionPay) is crossed with the real CIS and EU banking
//! catalog, so the router starts with well over 200 live routes. The list is
//! a seed script: the admin endpoint /api/admin/exchange-pairs can toggle,
//! fix or extend any pair on the fly, and the frontend picks the change up on
//! its next GET /api/exchange-pairs.

use crate::db::DbPool;
use crate::pairs::{repo_upsert, NewPair};

/// Bank logos come from the bank's own site favicon (Google's favicon service),
/// so the catalog shows real bank marks without shipping image assets.
const ICON_TMPL: &str = "https://www.google.com/s2/favicons?domain={}&sz=128";

/// A card leg of a pair: the scheme, the issuing bank and its home country
/// and currency (used when generating pairs). `domain` is the bank's website,
/// used to resolve its favicon.
struct Card {
    scheme: &'static str,
    bank: &'static str,
    country: &'static str,
    currency: &'static str,
    domain: &'static str,
}

impl Card {
    fn icon_url(&self) -> String {
        ICON_TMPL.replacen("{}", self.domain, 1)
    }
}

/// Sender side: cards that our payers pay from (CIS retail banks, multi-scheme).
const SENDERS: &[Card] = &[
    Card { scheme: "Visa", bank: "Беларусбанк", country: "BY", currency: "BYN", domain: "belarusbank.by" },
    Card { scheme: "Visa", bank: "Сбербанк", country: "RU", currency: "RUB", domain: "sberbank.ru" },
    Card { scheme: "Visa", bank: "Альфа-Банк", country: "RU", currency: "RUB", domain: "alfabank.by" },
    Card { scheme: "Visa", bank: "ВТБ", country: "RU", currency: "RUB", domain: "vtb.ru" },
    Card { scheme: "Visa", bank: "Т-Банк", country: "RU", currency: "RUB", domain: "tbank.ru" },
    Card { scheme: "Visa", bank: "Газпромбанк", country: "RU", currency: "RUB", domain: "gazprombank.ru" },
    Card { scheme: "Visa", bank: "Росбанк", country: "RU", currency: "RUB", domain: "rosbank.ru" },
    Card { scheme: "Visa", bank: "МТС Банк", country: "RU", currency: "RUB", domain: "mtsbank.ru" },
    Card { scheme: "MasterCard", bank: "Сбербанк", country: "RU", currency: "RUB", domain: "sberbank.ru" },
    Card { scheme: "MasterCard", bank: "Альфа-Банк", country: "RU", currency: "RUB", domain: "alfabank.by" },
    Card { scheme: "MasterCard", bank: "ВТБ", country: "RU", currency: "RUB", domain: "vtb.ru" },
    Card { scheme: "MasterCard", bank: "Т-Банк", country: "RU", currency: "RUB", domain: "tbank.ru" },
    Card { scheme: "MasterCard", bank: "Газпромбанк", country: "RU", currency: "RUB", domain: "gazprombank.ru" },
    Card { scheme: "MasterCard", bank: "Райффайзенбанк", country: "RU", currency: "RUB", domain: "raiffeisen.ru" },
    Card { scheme: "МИР", bank: "Сбербанк", country: "RU", currency: "RUB", domain: "sberbank.ru" },
    Card { scheme: "МИР", bank: "ВТБ", country: "RU", currency: "RUB", domain: "vtb.ru" },
    Card { scheme: "МИР", bank: "Альфа-Банк", country: "RU", currency: "RUB", domain: "alfabank.by" },
    Card { scheme: "МИР", bank: "Т-Банк", country: "RU", currency: "RUB", domain: "tbank.ru" },
    Card { scheme: "МИР", bank: "Газпромбанк", country: "RU", currency: "RUB", domain: "gazprombank.ru" },
    Card { scheme: "UnionPay", bank: "Сбербанк", country: "RU", currency: "RUB", domain: "sberbank.ru" },
    Card { scheme: "UnionPay", bank: "Альфа-Банк", country: "RU", currency: "RUB", domain: "alfabank.by" },
    Card { scheme: "Visa", bank: "Халык Банк", country: "KZ", currency: "KZT", domain: "halykbank.kz" },
    Card { scheme: "MasterCard", bank: "Kaspi.kz", country: "KZ", currency: "KZT", domain: "kaspi.kz" },
    Card { scheme: "Visa", bank: "Ардшинбанк", country: "AM", currency: "AMD", domain: "ardshinbank.am" },
];

/// Recipient side: banks our payers can receive into (EU/GB/CH/AM).
const RECEIVERS: &[Card] = &[
    Card { scheme: "Visa", bank: "Ameriabank", country: "AM", currency: "AMD", domain: "ameriabank.am" },
    Card { scheme: "MasterCard", bank: "Ameriabank", country: "AM", currency: "AMD", domain: "ameriabank.am" },
    Card { scheme: "Visa", bank: "Deutsche Bank", country: "DE", currency: "EUR", domain: "db.com" },
    Card { scheme: "MasterCard", bank: "Deutsche Bank", country: "DE", currency: "EUR", domain: "db.com" },
    Card { scheme: "Visa", bank: "Commerzbank", country: "DE", currency: "EUR", domain: "commerzbank.de" },
    Card { scheme: "MasterCard", bank: "BNP Paribas", country: "FR", currency: "EUR", domain: "bnpparibas.com" },
    Card { scheme: "Visa", bank: "BNP Paribas", country: "FR", currency: "EUR", domain: "bnpparibas.com" },
    Card { scheme: "MasterCard", bank: "Société Générale", country: "FR", currency: "EUR", domain: "societegenerale.com" },
    Card { scheme: "Visa", bank: "Santander", country: "ES", currency: "EUR", domain: "santander.com" },
    Card { scheme: "MasterCard", bank: "BBVA", country: "ES", currency: "EUR", domain: "bbva.com" },
    Card { scheme: "Visa", bank: "UniCredit", country: "IT", currency: "EUR", domain: "unicreditgroup.eu" },
    Card { scheme: "MasterCard", bank: "Intesa Sanpaolo", country: "IT", currency: "EUR", domain: "intesasanpaolo.com" },
    Card { scheme: "Visa", bank: "ING", country: "NL", currency: "EUR", domain: "ing.com" },
    Card { scheme: "MasterCard", bank: "Rabobank", country: "NL", currency: "EUR", domain: "rabobank.nl" },
    Card { scheme: "Visa", bank: "KBC", country: "BE", currency: "EUR", domain: "kbc.be" },
    Card { scheme: "MasterCard", bank: "Nordea", country: "SE", currency: "EUR", domain: "nordea.com" },
    Card { scheme: "Visa", bank: "Swedbank", country: "SE", currency: "SEK", domain: "swedbank.se" },
    Card { scheme: "MasterCard", bank: "Danske Bank", country: "DK", currency: "DKK", domain: "danskebank.com" },
    Card { scheme: "Visa", bank: "Erste Group", country: "AT", currency: "EUR", domain: "erstegroup.com" },
    Card { scheme: "MasterCard", bank: "Raiffeisen Bank Intl.", country: "AT", currency: "EUR", domain: "rbinternational.com" },
    Card { scheme: "Visa", bank: "UBS", country: "CH", currency: "CHF", domain: "ubs.com" },
    Card { scheme: "MasterCard", bank: "Barclays", country: "GB", currency: "GBP", domain: "barclays.co.uk" },
    Card { scheme: "Visa", bank: "HSBC", country: "GB", currency: "GBP", domain: "hsbc.co.uk" },
    Card { scheme: "MasterCard", bank: "Revolut", country: "GB", currency: "GBP", domain: "revolut.com" },
    Card { scheme: "Visa", bank: "Wise", country: "EU", currency: "EUR", domain: "wise.com" },
    Card { scheme: "MasterCard", bank: "N26", country: "DE", currency: "EUR", domain: "n26.com" },
    Card { scheme: "Visa", bank: "Alpha Bank", country: "GR", currency: "EUR", domain: "alpha.gr" },
];

/// Build the catalog: the cross product of every sender card with every
/// recipient card, `from_currency,to_currency` per pair, `country` = where the
/// money lands. 24 × 27 = 648 routes (well over the ~200 the router needs).
pub fn build_pairs() -> Vec<NewPair> {
    let mut pairs = Vec::with_capacity(SENDERS.len() * RECEIVERS.len());
    for sender in SENDERS {
        for receiver in RECEIVERS {
            pairs.push(NewPair {
                from_scheme: sender.scheme.to_string(),
                from_bank: sender.bank.to_string(),
                from_bank_icon_url: Some(sender.icon_url()),
                to_scheme: receiver.scheme.to_string(),
                to_bank: receiver.bank.to_string(),
                to_bank_icon_url: Some(receiver.icon_url()),
                country: Some(receiver.country.to_string()),
                currencies: Some(format!("{},{}", sender.currency, receiver.currency)),
                status: Some("enabled".to_string()),
                daily_limit_minor: None,
                daily_limit_currency: None,
            });
        }
    }
    pairs
}

/// Seed the `exchange_pairs` table idempotently. Called once at startup in
/// addition to the acquirer seeding; admin edits survive because every row is
/// upserted on its route key, never truncated.
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
    fn catalog_overflows_the_two_hundred_route_requirement() {
        let pairs = build_pairs();
        assert!(pairs.len() > 200, "catalog must hold >200 routes, got {}", pairs.len());
    }

    #[test]
    fn catalog_covers_all_four_schemes_on_the_sender_side() {
        let senders: std::collections::HashSet<&str> =
            SENDERS.iter().map(|c| c.scheme).collect();
        for scheme in ["Visa", "MasterCard", "МИР", "UnionPay"] {
            assert!(senders.contains(scheme), "missing sender scheme {scheme}");
        }
    }

    #[test]
    fn every_pair_has_shared_icons_and_a_sane_currency_leg() {
        for pair in build_pairs() {
            assert!(!pair.from_bank_icon_url.as_deref().unwrap_or("").is_empty());
            assert!(!pair.to_bank_icon_url.as_deref().unwrap_or("").is_empty());
            let currencies = pair.currencies.as_deref().unwrap_or("");
            assert!(currencies.contains(','), "pair must carry from,to currencies: {currencies}");
        }
    }
}