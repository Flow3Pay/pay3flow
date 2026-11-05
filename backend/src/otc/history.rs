//! Time-bucketed market history for the OTC chart.
//!
//! Candles and book levels are aggregated on read from the desk's own settled
//! prints and published advertisements; the indicative price series is merged
//! in by the route handler because it lives with the market price service.
use super::{amount, error::Result, Service};
use crate::market_prices::ExchangeCandle;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

/// `(range, bucket seconds, bucket count, market interval)`.
///
/// The window is always `bucket seconds * bucket count` so the grid tiles it
/// exactly. `market interval` is the exchange source resolution and must
/// divide the bucket: every source candle then belongs to exactly one bucket
/// and the chart keeps the granularity of the range the reader selected.
const WINDOWS: &[(&str, i64, i64, &str)] = &[
    ("1h", 60, 60, "1m"),
    ("24h", 900, 96, "15m"),
    ("1w", 7_200, 84, "1h"),
    ("1m", 21_600, 120, "1h"),
    ("6m", 86_400, 183, "1d"),
    ("1y", 604_800, 52, "1d"),
];

fn interval_seconds(interval: &str) -> i64 {
    match interval {
        "1m" => 60,
        "5m" => 300,
        "15m" => 900,
        "30m" => 1_800,
        "1h" => 3_600,
        "4h" => 14_400,
        "8h" => 28_800,
        "1d" => 86_400,
        _ => 604_800,
    }
}

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
    pub market_interval: &'static str,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl Window {
    pub(crate) fn parse(range: &str, now: DateTime<Utc>) -> Result<Self> {
        let (name, bucket_seconds, buckets, market_interval) = *WINDOWS
            .iter()
            .find(|(candidate, ..)| *candidate == range)
            .ok_or_else(|| super::error::invalid("range must be 1h, 24h, 1w, 1m, 6m or 1y"))?;
        // Align the end of the window to a bucket boundary. Both the desk
        // prints and the exchange candles are then bucketed on the same grid,
        // and the newest candle is never a partial mixture of two periods.
        let to = now - Duration::seconds(now.timestamp().rem_euclid(bucket_seconds));
        Ok(Self {
            range: name,
            bucket_seconds,
            buckets,
            market_interval,
            from: to - Duration::seconds(bucket_seconds * buckets),
            to,
        })
    }

    /// Enough exchange candles to cover the window plus a small margin for
    /// provider clock skew. Capped by the provider's single-page limit.
    pub(crate) fn market_limit(&self) -> u32 {
        let step = interval_seconds(self.market_interval).max(1);
        ((self.bucket_seconds * self.buckets) / step + 3).clamp(2, 1_000) as u32
    }
    fn index(&self, at: DateTime<Utc>) -> Option<usize> {
        let offset = at.timestamp().checked_sub(self.from.timestamp())?;
        let index = offset.div_euclid(self.bucket_seconds);
        (0..self.buckets).contains(&index).then_some(index as usize)
    }

    /// The provider period, point span and bucket size for the indicative
    /// price overlay. Short ranges use hourly points; longer ranges use the
    /// daily series and let the chart aggregate it to the selected bucket.
    pub(crate) fn spot_layout(&self) -> (&'static str, u32, u32) {
        match self.range {
            "1h" => ("1h", 24, 300),
            "24h" => ("1h", 26, 300),
            "1w" => ("1h", 170, 300),
            "1m" => ("1d", 32, 86_400),
            "6m" => ("1d", 186, 86_400),
            _ => ("1d", 372, 86_400),
        }
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

#[derive(Default, Clone, Copy)]
struct MarketBucket {
    open: Option<f64>,
    high: f64,
    low: f64,
    close: Option<f64>,
    volume: f64,
}

/// Folds exchange candles into the exact window grid. Only buckets that the
/// provider actually covered are returned, so a quiet market draws a gap
/// instead of a fabricated flat candle.
pub(crate) fn aggregate_market(candles: &[ExchangeCandle], window: &Window) -> Vec<Value> {
    let mut buckets = vec![MarketBucket::default(); window.buckets as usize];
    for candle in candles {
        let Some(at) = DateTime::from_timestamp(candle.t, 0) else {
            continue;
        };
        let Some(index) = window.index(at) else {
            continue;
        };
        let bucket = &mut buckets[index];
        if bucket.open.is_none() {
            bucket.open = Some(candle.open);
            bucket.low = candle.low;
        }
        bucket.high = bucket.high.max(candle.high);
        bucket.low = bucket.low.min(candle.low);
        bucket.close = Some(candle.close);
        bucket.volume += candle.volume;
    }
    buckets
        .iter()
        .enumerate()
        .filter_map(|(index, bucket)| {
            let (Some(open), Some(close)) = (bucket.open, bucket.close) else {
                return None;
            };
            if bucket.volume <= 0.0 {
                return None;
            }
            Some(json!({
                "t": window.from.timestamp() + index as i64 * window.bucket_seconds,
                "open": round(open, 8),
                "high": round(bucket.high, 8),
                "low": round(bucket.low, 8),
                "close": round(close, 8),
                "volume": round(bucket.volume, 2),
            }))
        })
        .collect()
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
        for (range, bucket_seconds, buckets, market_interval) in WINDOWS {
            let window = Window::parse(range, now).unwrap();
            assert_eq!(window.bucket_seconds, *bucket_seconds);
            assert_eq!(window.buckets, *buckets);
            assert_eq!(window.market_interval, *market_interval);
            assert_eq!(
                (window.to - window.from).num_seconds(),
                bucket_seconds * buckets
            );
        }
        assert!(Window::parse("5m", now).is_err());
    }

    #[test]
    fn windows_end_on_a_bucket_boundary() {
        let now = at(1_761_000_123);
        for range in ["1h", "24h", "1w", "1m", "6m", "1y"] {
            let window = Window::parse(range, now).unwrap();
            assert_eq!(window.to.timestamp().rem_euclid(window.bucket_seconds), 0);
        }
    }

    #[test]
    fn every_market_interval_divides_its_bucket_and_covers_the_window() {
        let now = at(1_761_000_000);
        for (range, bucket_seconds, buckets, interval) in WINDOWS {
            assert_eq!(
                bucket_seconds.rem_euclid(interval_seconds(interval)),
                0,
                "{range}"
            );
            let window = Window::parse(range, now).unwrap();
            assert!(
                i64::from(window.market_limit()) * interval_seconds(interval)
                    >= bucket_seconds * buckets,
                "{range}"
            );
        }
    }

    #[test]
    fn market_candles_fold_into_the_selected_grid() {
        let now = at(1_761_000_000);
        let window = Window::parse("24h", now).unwrap();
        let start = window.from.timestamp();
        let candles = [
            ExchangeCandle {
                t: start,
                open: 0.1,
                high: 0.12,
                low: 0.09,
                close: 0.11,
                volume: 10.0,
            },
            ExchangeCandle {
                t: start + 900,
                open: 0.11,
                high: 0.13,
                low: 0.1,
                close: 0.12,
                volume: 5.0,
            },
            ExchangeCandle {
                t: start + 3_600,
                open: 0.2,
                high: 0.21,
                low: 0.19,
                close: 0.2,
                volume: 1.0,
            },
        ];
        let bars = aggregate_market(&candles, &window);
        assert_eq!(bars.len(), 3);
        assert_eq!(bars[0]["t"], json!(start));
        assert_eq!(bars[0]["open"], json!(0.1));
        assert_eq!(bars[0]["close"], json!(0.11));
        assert_eq!(bars[2]["t"], json!(start + 3_600));
    }

    #[test]
    fn spot_layout_uses_hourly_points_for_short_ranges_and_daily_after() {
        let now = at(1_761_000_000);
        assert_eq!(
            Window::parse("1h", now).unwrap().spot_layout(),
            ("1h", 24, 300)
        );
        assert_eq!(
            Window::parse("24h", now).unwrap().spot_layout(),
            ("1h", 26, 300)
        );
        assert_eq!(
            Window::parse("1w", now).unwrap().spot_layout(),
            ("1h", 170, 300)
        );
        assert_eq!(
            Window::parse("1m", now).unwrap().spot_layout(),
            ("1d", 32, 86_400)
        );
        assert_eq!(
            Window::parse("6m", now).unwrap().spot_layout(),
            ("1d", 186, 86_400)
        );
        assert_eq!(
            Window::parse("1y", now).unwrap().spot_layout(),
            ("1d", 372, 86_400)
        );
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
