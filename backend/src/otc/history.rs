//! Time-bucketed market history for the OTC chart.
//!
//! Candles and book levels are aggregated on read from the desk's own settled
//! prints and published advertisements; the indicative price series is merged
//! in by the route handler because it lives with the market price service.
use super::{amount, error::Result, Service};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

/// `(range, bucket seconds, bucket count, indicative days)`. The window is
/// always `bucket seconds * bucket count` so the grid tiles it exactly.
/// Ranges shorter than a week pass `0` days: the price provider only answers at
/// day resolution, so intraday detail has to come from the desk's own prints.
const WINDOWS: &[(&str, i64, i64, u32)] = &[
    ("1h", 60, 60, 0),
    ("24h", 900, 96, 0),
    ("1w", 7_200, 84, 8),
    ("1m", 21_600, 120, 32),
    ("6m", 86_400, 183, 186),
    ("1y", 604_800, 52, 372),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Trade,
    Bid,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Print {
    pub side: Side,
    pub at: DateTime<Utc>,
    pub ever_units: u128,
    pub usdt_units: u128,
}

impl Print {
    /// EVER is priced in USDT, so one unit of the pair is `usdt / ever`.
    /// Everscale uses 9 decimals and USDT 6, hence the 10^3 rescale.
    fn price(&self) -> f64 {
        self.usdt_units as f64 * 1_000.0 / self.ever_units as f64
    }
    fn volume(&self) -> f64 {
        self.usdt_units as f64 / 1_000_000.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Window {
    pub range: &'static str,
    pub bucket_seconds: i64,
    pub buckets: i64,
    pub history_days: u32,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl Window {
    pub(crate) fn parse(range: &str, now: DateTime<Utc>) -> Result<Self> {
        let (name, bucket_seconds, buckets, history_days) = *WINDOWS
            .iter()
            .find(|(candidate, ..)| *candidate == range)
            .ok_or_else(|| super::error::invalid("range must be 1h, 24h, 1w, 1m, 6m or 1y"))?;
        Ok(Self {
            range: name,
            bucket_seconds,
            buckets,
            history_days,
            from: now - Duration::seconds(bucket_seconds * buckets),
            to: now,
        })
    }
    fn index(&self, at: DateTime<Utc>) -> Option<usize> {
        let offset = at.timestamp().checked_sub(self.from.timestamp())?;
        let index = offset.div_euclid(self.bucket_seconds);
        (0..self.buckets).contains(&index).then_some(index as usize)
    }
}

#[derive(Debug, Default, Clone)]
struct Bucket {
    open: Option<f64>,
    high: f64,
    low: f64,
    close: Option<f64>,
    volume: f64,
    prints: u32,
    bid: Option<f64>,
    ask: Option<f64>,
}

/// Rows must arrive ordered by time; each bucket then takes its first print as
/// the open and its last as the close, which is what a candle is drawn from.
pub(crate) fn aggregate(rows: &[Print], window: &Window) -> (Vec<Value>, Vec<Value>) {
    let mut buckets = vec![Bucket::default(); window.buckets as usize];
    for row in rows {
        if row.ever_units == 0 {
            continue;
        }
        let Some(index) = window.index(row.at) else {
            continue;
        };
        let bucket = &mut buckets[index];
        let price = row.price();
        match row.side {
            Side::Trade => {
                if bucket.open.is_none() {
                    bucket.open = Some(price);
                }
                bucket.high = bucket.high.max(price);
                bucket.low = if bucket.prints == 0 {
                    price
                } else {
                    bucket.low.min(price)
                };
                bucket.close = Some(price);
                bucket.volume += row.volume();
                bucket.prints += 1;
            }
            Side::Bid => bucket.bid = Some(price),
            Side::Ask => bucket.ask = Some(price),
        }
    }
    let mut candles = Vec::with_capacity(buckets.len());
    let mut levels = Vec::with_capacity(buckets.len());
    let mut carried_bid = None;
    let mut carried_ask = None;
    for (index, bucket) in buckets.iter().enumerate() {
        let t = window.from.timestamp() + index as i64 * window.bucket_seconds;
        candles.push(json!({
            "t": t,
            "open": bucket.open.map(|value| round(value, 8)),
            "high": (bucket.prints > 0).then(|| round(bucket.high, 8)),
            "low": (bucket.prints > 0).then(|| round(bucket.low, 8)),
            "close": bucket.close.map(|value| round(value, 8)),
            "volume": (bucket.prints > 0).then(|| round(bucket.volume, 2)),
            "prints": bucket.prints,
        }));
        carried_bid = bucket.bid.or(carried_bid);
        carried_ask = bucket.ask.or(carried_ask);
        levels.push(json!({
            "t": t,
            "bid": carried_bid.map(|value| round(value, 8)),
            "ask": carried_ask.map(|value| round(value, 8)),
        }));
    }
    (candles, levels)
}

fn round(value: f64, places: u32) -> f64 {
    let factor = 10_f64.powi(places as i32);
    (value * factor).round() / factor
}

impl Service {
    pub(crate) async fn history(&self, window: &Window) -> Result<Vec<Print>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT 'trade' AS side, created_at AS at,
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
                  ORDER BY at",
                &[&window.from],
            )
            .await?;
        let mut prints = Vec::with_capacity(rows.len());
        for row in rows {
            let side = match row.get::<_, String>("side").as_str() {
                "bid" => Side::Bid,
                "ask" => Side::Ask,
                _ => Side::Trade,
            };
            let (Some(ever), Some(usdt)) = (
                row.get::<_, Option<String>>("ever_units"),
                row.get::<_, Option<String>>("usdt_units"),
            ) else {
                continue;
            };
            let (Ok(ever_units), Ok(usdt_units)) = (amount::integer(&ever), amount::integer(&usdt))
            else {
                continue;
            };
            prints.push(Print {
                side,
                at: row.get("at"),
                ever_units,
                usdt_units,
            });
        }
        Ok(prints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(seconds, 0).unwrap()
    }
    fn print(side: Side, seconds: i64, ever_units: u128, usdt_units: u128) -> Print {
        Print {
            side,
            at: at(seconds),
            ever_units,
            usdt_units,
        }
    }

    #[test]
    fn every_supported_range_tiles_its_window_exactly() {
        let now = at(1_761_000_000);
        for (range, bucket_seconds, buckets, history_days) in WINDOWS {
            let window = Window::parse(range, now).unwrap();
            assert_eq!(window.bucket_seconds, *bucket_seconds);
            assert_eq!(window.buckets, *buckets);
            assert_eq!(window.history_days, *history_days);
            assert_eq!(
                (window.to - window.from).num_seconds(),
                bucket_seconds * buckets
            );
        }
        assert!(Window::parse("5m", now).is_err());
    }

    #[test]
    fn trades_become_candles_and_offers_become_the_book() {
        let now = at(1_761_000_000);
        let window = Window::parse("1h", now).unwrap();
        let start = window.from.timestamp();
        // One thousand EVER settling for 13 and then 15 USDT prints 0.013 and
        // 0.015; the two-sided advertisements price the book at 0.012 / 0.015.
        let rows = [
            print(Side::Trade, start + 10, 1_000_000_000_000, 13_000_000),
            print(Side::Trade, start + 40, 1_000_000_000_000, 15_000_000),
            print(Side::Bid, start + 20, 2_000_000_000_000, 24_000_000),
            print(Side::Ask, start + 30, 2_000_000_000_000, 30_000_000),
        ];
        let (candles, levels) = aggregate(&rows, &window);
        assert_eq!(candles.len(), 60);
        let first = &candles[0];
        assert_eq!(first["open"], json!(0.013));
        assert_eq!(first["high"], json!(0.015));
        assert_eq!(first["low"], json!(0.013));
        assert_eq!(first["close"], json!(0.015));
        assert_eq!(first["volume"], json!(28.0));
        assert_eq!(first["prints"], json!(2));
        assert_eq!(levels[0]["bid"], json!(0.012));
        assert_eq!(levels[0]["ask"], json!(0.015));
        // Book levels carry forward until a newer advertisement replaces them.
        assert_eq!(levels[59]["bid"], json!(0.012));
        assert_eq!(levels[59]["ask"], json!(0.015));
        // Buckets with no settlement stay null so the chart draws a gap.
        assert_eq!(candles[30]["close"], Value::Null);
        assert_eq!(candles[30]["volume"], Value::Null);
    }

    #[test]
    fn prints_outside_the_bucket_grid_are_dropped() {
        let now = at(1_761_000_000);
        let window = Window::parse("24h", now).unwrap();
        let rows = [
            print(
                Side::Trade,
                window.from.timestamp() - 1,
                1_000_000_000_000,
                13_000_000,
            ),
            print(
                Side::Trade,
                window.to.timestamp() + 999,
                1_000_000_000_000,
                13_000_000,
            ),
            print(Side::Bid, window.from.timestamp(), 0, 13_000_000),
        ];
        let (candles, levels) = aggregate(&rows, &window);
        assert!(candles.iter().all(|candle| candle["prints"] == json!(0)));
        assert!(levels.iter().all(|level| level["bid"] == Value::Null));
    }
}
