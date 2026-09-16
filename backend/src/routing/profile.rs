use crate::acquirer::AcquirerSeed;

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

impl From<&AcquirerSeed> for AcquirerProfile {
    fn from(seed: &AcquirerSeed) -> Self {
        let (min_amount, max_amount, amount_currency) = parse_limits(seed.limits);
        Self {
            slug: seed.slug.to_string(),
            name: seed.name.to_string(),
            geo: split_tokens(seed.geo),
            currencies: split_tokens(seed.currencies),
            // For a range ("0.99%..2.5%") we take the worst case (plan: margin).
            fee_percent: parse_percent(seed.fee).unwrap_or(0.0),
            fee_flat: parse_flat(seed.fee).unwrap_or(0.0),
            min_amount,
            max_amount,
            amount_currency,
            status: "active".to_string(),
        }
    }
}

/// The manually curated fallback pool (seeds from `acquirer.rs`).
pub fn seed_pool() -> Vec<AcquirerProfile> {
    crate::acquirer::ACQUIRERS
        .iter()
        .map(AcquirerProfile::from)
        .collect()
}

fn split_tokens(s: &str) -> Vec<String> {
    s.split('|')
        .map(|t| t.trim().to_uppercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn parse_percent(fee: &str) -> Option<f64> {
    // For a range ("0.99%..2.5%") take the tail (worst case, margin per plan).
    let s = fee.split_once("..").map(|(_, tail)| tail).unwrap_or(fee);
    let pct_pos = s.find('%')?;
    let before = &s[..pct_pos];
    let token = before
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .rfind(|t| !t.is_empty())?;
    token.parse::<f64>().ok()
}

fn parse_flat(fee: &str) -> Option<f64> {
    let plus = fee.find('+')?;
    let after = &fee[plus + 1..];
    let flat_pos = after.find("flat")?;
    after[..flat_pos].trim().parse::<f64>().ok()
}

fn parse_limits(s: &str) -> (Option<f64>, Option<f64>, Option<String>) {
    let mut min = None;
    let mut max = None;
    let mut currency = None;
    for part in s.split(',') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("min ") {
            min = rest
                .split_whitespace()
                .next()
                .and_then(|t| t.parse::<f64>().ok());
        } else if let Some(rest) = part.strip_prefix("max ") {
            let mut tokens = rest.split_whitespace();
            if let Some(num) = tokens.next() {
                max = num.parse::<f64>().ok();
                currency = tokens.next().map(|c| c.to_uppercase());
            }
        }
    }
    (min, max, currency)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_percent_and_flat() {
        assert_eq!(parse_percent("3.1% + 0.50 flat (card)").unwrap(), 3.1);
        assert_eq!(parse_flat("3.1% + 0.50 flat (card)").unwrap(), 0.50);
        assert_eq!(parse_percent("0.99%..2.5% (fx+transfer)").unwrap(), 2.5);
        assert_eq!(parse_flat("0.99%..2.5% (fx+transfer)"), None);
        assert_eq!(parse_flat("2.5% + 0.30 flat").unwrap(), 0.30);
    }

    #[test]
    fn parses_limits() {
        let (min, max, cur) = parse_limits("min 1, max 100000 USD");
        assert_eq!(min, Some(1.0));
        assert_eq!(max, Some(100000.0));
        assert_eq!(cur.as_deref(), Some("USD"));
    }

    #[test]
    fn seed_pool_covers_all_known_acquirers() {
        let pool = seed_pool();
        assert_eq!(pool.len(), crate::acquirer::ACQUIRERS.len());
        let mollie = pool.iter().find(|p| p.slug == "mollie").unwrap();
        assert!(mollie.currencies.contains(&"EUR".to_string()));
        assert!(mollie.geo.contains(&"NL".to_string()));
        assert_eq!(mollie.fee_percent, 1.5);
        assert_eq!(mollie.max_amount, Some(50000.0));
    }
}
