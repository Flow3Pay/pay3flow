//! Pay3Flow OTC surfaced as a live routing venue for the
//! `EVER@everscale <-> USDT@ethereum` (Everscale <-> Ethereum ERC-20) edge.
//!
//! The venue's liquidity is the persistent public-offer book (`otc_public_offers`).
//! Unlike a DEX aggregator, each advertisement fills a single, size-bounded trade,
//! so quotes are amount-aware: an offer is only returned when it can cover the
//! *whole* requested input on its own.  A $10 offer therefore never masquerades as
//! a $100 fill, and the venue is excluded from the amount-blind background
//! coefficient cache (see `PublicRouteProvider::supports_background_coefficient`).
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio_postgres::Row;

use crate::db::DbPool;
use crate::route_engine::{
    Amount, Asset, PublicRouteProvider, PublicRouteQuote, QuoteUnavailable,
};

const PROVIDER: &str = "pay3flow-otc";
const EVER_DECIMALS: u32 = 9;
const USDT_DECIMALS: u32 = 6;
/// Cap the number of distinct covering offers exposed for a single leg.  The book
/// is read up to the search engine's own per-leg ceiling.
const MAX_OFFERS_PER_LEG: usize = 8;
const MAX_BOOK_ROWS: usize = 100;

#[derive(Clone)]
pub struct OtcRouteProvider {
    pool: DbPool,
    source_url: String,
}

struct BookOffer {
    id: String,
    direction: String,
    ever_units: u128,
    usdt_units: u128,
    expires_at: DateTime<Utc>,
}

fn is_usdt(asset: &Asset) -> bool {
    asset.symbol == "USDT" && asset.location.as_deref() == Some("ethereum")
}

fn is_ever(asset: &Asset) -> bool {
    asset.symbol == "EVER" && asset.location.as_deref() == Some("everscale")
}

/// Parse a decimal string into fixed-point units, truncating (flooring) any digits
/// beyond `decimals` instead of rejecting them.  The composed exit leg hands over a
/// provider output with arbitrary precision, so `route_engine::decimal_to_atomic`
/// would error where a floor is the correct, non-overdrawing answer.
pub(crate) fn floor_units(value: &str, decimals: u32) -> Option<u128> {
    let value = value.trim().strip_prefix('+').unwrap_or(value);
    if value.is_empty() {
        return None;
    }
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    if !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let keep = fraction.get(..decimals as usize).unwrap_or(fraction);
    let whole_units: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction_units: u128 = if keep.is_empty() {
        0
    } else {
        let padded = format!("{}{}", keep, "0".repeat(decimals as usize - keep.len()));
        padded.parse().ok()?
    };
    let scale = 10_u128.checked_pow(decimals)?;
    whole_units.checked_mul(scale)?.checked_add(fraction_units)
}

fn offer_from_row(row: &Row) -> Option<BookOffer> {
    let body: Value = row.get("body");
    let units = |key: &str| body.get(key).and_then(Value::as_str).and_then(|raw| raw.parse::<u128>().ok());
    let ever_units = units("ever_units")?;
    let usdt_units = units("usdt_units")?;
    if ever_units == 0 || usdt_units == 0 {
        return None;
    }
    Some(BookOffer {
        id: row.get::<_, uuid::Uuid>("id").to_string(),
        direction: body
            .get("direction")
            .and_then(Value::as_str)
            .unwrap_or("sell")
            .to_string(),
        ever_units,
        usdt_units,
        expires_at: row.get("expires_at"),
    })
}

/// Pure quote builder: turn the book into per-offer quotes for the directed edge,
/// keeping only offers that cover the whole requested input and ordering them so
/// the taker gets the best price.  Split out so the sizing contract is unit-tested
/// without a database.
pub(crate) fn quotes_from_offers(
    offers: &[BookOffer],
    from: &Asset,
    to: &Asset,
    amount: &Amount,
    source_url: &str,
) -> Vec<PublicRouteQuote> {
    let buying_ever = is_usdt(from) && is_ever(to);
    let selling_ever = is_ever(from) && is_usdt(to);
    if !buying_ever && !selling_ever {
        return Vec::new();
    }
    let requested = floor_units(&amount.value, if buying_ever { USDT_DECIMALS } else { EVER_DECIMALS })
        .unwrap_or(0);
    if requested == 0 {
        return Vec::new();
    }

    let mut covering: Vec<&BookOffer> = offers
        .iter()
        .filter(|offer| {
            if offer.expires_at <= Utc::now() {
                return false;
            }
            if buying_ever {
                offer.direction == "sell" && offer.usdt_units >= requested
            } else {
                offer.direction == "buy" && offer.ever_units >= requested
            }
        })
        .collect();

    // Best price for the taker first: when buying EVER prefer the lowest USDT-per-EVER;
    // when selling EVER prefer the highest USDT-per-EVER. Both are exact cross-products.
    covering.sort_by(|a, b| {
        let left = a.usdt_units.checked_mul(b.ever_units).unwrap_or(u128::MAX);
        let right = b.usdt_units.checked_mul(a.ever_units).unwrap_or(u128::MAX);
        if buying_ever {
            left.cmp(&right)
        } else {
            right.cmp(&left)
        }
    });
    covering.truncate(MAX_OFFERS_PER_LEG);

    covering
        .iter()
        .filter_map(|offer| {
            let output_units = if buying_ever {
                requested
                    .checked_mul(offer.ever_units)?
                    .checked_div(offer.usdt_units)?
            } else {
                requested
                    .checked_mul(offer.usdt_units)?
                    .checked_div(offer.ever_units)?
            };
            if output_units == 0 {
                return None;
            }
            let output = Amount::new(
                super::amount::decimal(output_units, if buying_ever { EVER_DECIMALS } else { USDT_DECIMALS }),
                to.clone(),
            )
            .ok()?;
            Some(PublicRouteQuote {
                provider: PROVIDER.to_string(),
                quote_id: Some(offer.id.clone()),
                description: Some(format!(
                    "{} EVER / {} USDT",
                    super::amount::decimal(offer.ever_units, EVER_DECIMALS),
                    super::amount::decimal(offer.usdt_units, USDT_DECIMALS)
                )),
                source_url: Some(format!(
                    "{source_url}&side={}",
                    if buying_ever { "sell" } else { "buy" }
                )),
                from: from.clone(),
                to: to.clone(),
                input: amount.clone(),
                output,
                fees: Vec::new(),
                expires_at: Some(offer.expires_at),
                path: vec![from.clone(), to.clone()],
            })
        })
        .collect()
}

impl OtcRouteProvider {
    pub fn new(pool: DbPool, origin: &str) -> Self {
        let origin = origin.trim_end_matches('/');
        Self {
            pool,
            source_url: format!("{origin}/#/otc?market=EVER-USDT"),
        }
    }

    async fn book(&self) -> Result<Vec<BookOffer>> {
        let client = self
            .pool
            .get()
            .await
            .context("otc route provider: pool acquisition failed")?;
        let rows = client
            .query(
                "SELECT id, body, expires_at FROM otc_public_offers \
                 WHERE state='open' AND expires_at>now() \
                 ORDER BY created_at DESC LIMIT $1",
                &[&(MAX_BOOK_ROWS as i64)],
            )
            .await
            .context("otc route provider: book query failed")?;
        Ok(rows.iter().filter_map(offer_from_row).collect())
    }
}

#[async_trait]
impl PublicRouteProvider for OtcRouteProvider {
    fn name(&self) -> &str {
        PROVIDER
    }

    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        (is_usdt(from) && is_ever(to)) || (is_ever(from) && is_usdt(to))
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        [
            Asset::new("EVER", Some("everscale")),
            Asset::new("USDT", Some("ethereum")),
        ]
        .into_iter()
        .filter_map(Result::ok)
        .collect()
    }

    async fn normalize_input(&self, amount: Amount) -> Result<Amount> {
        let decimals = if is_ever(&amount.asset) {
            EVER_DECIMALS
        } else {
            USDT_DECIMALS
        };
        let floored = floor_units(&amount.value, decimals).unwrap_or(0);
        Ok(Amount::new(
            super::amount::decimal(floored, decimals),
            amount.asset,
        )?)
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        let quotes = self.quotes(from, to, amount).await?;
        quotes.into_iter().next().ok_or_else(|| {
            QuoteUnavailable {
                provider: PROVIDER,
                reason: "no public OTC offer covers this amount in this direction".into(),
            }
            .into()
        })
    }

    async fn quotes(&self, from: Asset, to: Asset, amount: Amount) -> Result<Vec<PublicRouteQuote>> {
        if !self.supports_pair(&from, &to) {
            return Ok(Vec::new());
        }
        let offers = self.book().await?;
        Ok(quotes_from_offers(
            &offers,
            &from,
            &to,
            &amount,
            &self.source_url,
        ))
    }

    fn supports_background_coefficient(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ever() -> Asset {
        Asset::new("EVER", Some("everscale")).unwrap()
    }
    fn usdt() -> Asset {
        Asset::new("USDT", Some("ethereum")).unwrap()
    }
    fn offer(id: &str, direction: &str, ever: u128, usdt: u128) -> BookOffer {
        BookOffer {
            id: id.to_string(),
            direction: direction.to_string(),
            ever_units: ever * 10_u128.pow(EVER_DECIMALS),
            usdt_units: usdt * 10_u128.pow(USDT_DECIMALS),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        }
    }

    #[test]
    fn floor_units_truncates_extra_precision_without_erroring() {
        assert_eq!(floor_units("100", 6), Some(100_000_000));
        assert_eq!(floor_units("12.5", 6), Some(12_500_000));
        // A composed exit-leg input can carry more decimals than the token supports.
        assert_eq!(floor_units("0.30000000000000004", 9), Some(300_000_000));
        assert_eq!(floor_units("1.5", 0).unwrap(), 1);
        assert_eq!(floor_units("abc", 6), None);
        assert_eq!(floor_units("", 6), None);
    }

    #[test]
    fn buys_ever_only_from_sell_offers_that_cover_the_full_usdt() {
        // A $10 offer must never be reported as a $100 fill.
        let book = vec![
            offer("small", "sell", 1_000, 10),
            offer("big", "sell", 100_000, 100),
            offer("wrongside", "buy", 100_000, 100),
        ];
        let amount = Amount::new("100", usdt()).unwrap();
        let quotes = quotes_from_offers(&book, &usdt(), &ever(), &amount, "https://x/#/otc");
        assert_eq!(quotes.len(), 1, "only the 100-USDT offer covers 100 USDT");
        let quote = &quotes[0];
        assert_eq!(quote.quote_id.as_deref(), Some("big"));
        assert_eq!(quote.input, amount, "the requested input is echoed verbatim");
        assert_eq!(quote.from, usdt());
        assert_eq!(quote.to, ever());
        // 100 USDT * (100_000 EVER / 100 USDT) = 100_000 EVER.
        assert_eq!(quote.output.value, "100000");
        assert_eq!(quote.output.asset, ever());
    }

    #[test]
    fn sells_ever_only_from_buy_offers_and_prefers_the_highest_usdt() {
        let book = vec![
            offer("cheap", "buy", 500, 200), // 0.4 USDT/EVER
            offer("rich", "buy", 500, 250),  // 0.5 USDT/EVER
            offer("tiny", "buy", 1, 250),    // cannot cover 50 EVER
        ];
        let amount = Amount::new("500", ever()).unwrap();
        let quotes = quotes_from_offers(&book, &ever(), &usdt(), &amount, "https://x/#/otc");
        assert_eq!(quotes.len(), 2);
        assert_eq!(quotes[0].quote_id.as_deref(), Some("rich"), "best price first");
        assert_eq!(quotes[0].output.value, "250");
        assert_eq!(quotes[1].quote_id.as_deref(), Some("cheap"));
        assert_eq!(quotes[1].output.value, "200");
    }

    #[test]
    fn buying_ever_sorts_by_cheapest_ever_first() {
        let book = vec![
            offer("pricey", "sell", 100, 50), // 0.5 USDT/EVER -> 100 EVER
            offer("value", "sell", 200, 50),  // 0.25 USDT/EVER -> 200 EVER
        ];
        let amount = Amount::new("50", usdt()).unwrap();
        let quotes = quotes_from_offers(&book, &usdt(), &ever(), &amount, "https://x/#/otc");
        assert_eq!(quotes[0].quote_id.as_deref(), Some("value"));
        assert_eq!(quotes[0].output.value, "200");
    }

    #[test]
    fn unsupported_direction_yields_no_quotes() {
        let book = vec![offer("a", "sell", 100, 100)];
        let amount = Amount::new("50", ever()).unwrap();
        // Selling EVER but the book only has sell offers -> nothing covers.
        assert!(quotes_from_offers(&book, &ever(), &usdt(), &amount, "https://x").is_empty());
        // A pair the venue never serves.
        let usdc = Asset::new("USDC", Some("ethereum")).unwrap();
        assert!(quotes_from_offers(&book, &usdc, &ever(), &amount, "https://x").is_empty());
    }

    #[test]
    fn quote_is_bounded_by_the_smallest_covering_offer_size() {
        let book = vec![offer("a", "sell", 1_000_000, 5)];
        // Requesting more USDT than the whole 5-USDT book holds must yield no quote.
        let amount = Amount::new("6", usdt()).unwrap();
        assert!(quotes_from_offers(&book, &usdt(), &ever(), &amount, "https://x").is_empty());
    }
}
