use super::{Draft, Side};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Market {
    id: &'static str,
    base: &'static str,
    quote: &'static str,
    name: &'static str,
    price: f64,
    price_decimals: u32,
    amount_decimals: i32,
    tick_size: f64,
    change: f64,
    volume: f64,
    icon: &'static str,
}
const MARKETS: [Market; 4] = [
    Market {
        id: "EVER-USDT",
        base: "EVER",
        quote: "USDT",
        name: "Everscale",
        price: 0.01,
        price_decimals: 5,
        amount_decimals: 6,
        tick_size: 0.00001,
        change: 2.84,
        volume: 12500.0,
        icon: "/icons/assets/ever.svg",
    },
    Market {
        id: "BTC-USDT",
        base: "BTC",
        quote: "USDT",
        name: "Bitcoin",
        price: 68240.5,
        price_decimals: 2,
        amount_decimals: 6,
        tick_size: 10.0,
        change: 2.84,
        volume: 1248500.0,
        icon: "/icons/assets/btc.png",
    },
    Market {
        id: "ETH-USDT",
        base: "ETH",
        quote: "USDT",
        name: "Ethereum",
        price: 2648.72,
        price_decimals: 2,
        amount_decimals: 5,
        tick_size: 0.5,
        change: 1.62,
        volume: 486320.0,
        icon: "/icons/assets/eth.png",
    },
    Market {
        id: "SOL-USDT",
        base: "SOL",
        quote: "USDT",
        name: "Solana",
        price: 148.36,
        price_decimals: 2,
        amount_decimals: 4,
        tick_size: 0.05,
        change: 4.21,
        volume: 214850.0,
        icon: "/icons/assets/sol.webp",
    },
];
pub fn find(id: &str) -> Option<&'static Market> {
    MARKETS.iter().find(|market| market.id == id)
}
pub fn details(market: &Market) -> Value {
    let mut value = json!(market);
    value["high"] = json!(market.price * 1.014);
    value["low"] = json!(market.price * 0.959);
    value
}
fn round(value: f64, decimals: i32) -> f64 {
    let scale = 10f64.powi(decimals);
    (value * scale).round() / scale
}
fn levels(market: &Market, side: Side) -> Vec<(f64, f64)> {
    (0..9)
        .map(|index| {
            let sign = if side == Side::Buy { -1.0 } else { 1.0 };
            let seed = if side == Side::Buy { 1030 } else { 610 };
            (
                round(
                    market.price + sign * market.tick_size * f64::from(index + 1),
                    market.price_decimals as i32,
                ),
                round(
                    f64::from(3500 + ((index * 2371 + seed) % 15500)) / market.price,
                    market.amount_decimals,
                ),
            )
        })
        .collect()
}
fn book(market: &Market, side: Side) -> Vec<Value> {
    let mut depth = 0.0;
    levels(market, side)
        .into_iter()
        .map(|(price, amount)| {
            depth += price * amount;
            json!({"price": price, "amount": amount, "total": price * amount, "depth": depth})
        })
        .collect()
}
pub fn snapshot(market: &Market, now: i64) -> Value {
    let trades: Vec<Value> = (0..8).map(|index| json!({
        "id": format!("{}-{index}", market.id), "time": now - index * 73000,
        "side": if index % 3 == 0 { "sell" } else { "buy" },
        "price": round(market.price + (index as f64 * 1.7).sin() * market.tick_size * 3.0, market.price_decimals as i32),
        "amount": round((600.0 + index as f64 * 780.0) / market.price, market.amount_decimals)
    })).collect();
    json!({"marketId": market.id, "mode": "test", "bids": book(market, Side::Buy), "asks": book(market, Side::Sell), "trades": trades})
}
pub fn candles(market: &Market, now: i64) -> Value {
    let mut result = serde_json::Map::new();
    for (range, days) in [("1D", 1.0), ("7D", 7.0), ("1M", 30.0), ("1Y", 365.0)] {
        let close_at = |index: f64| {
            let trend = index / 63.0;
            market.price
                * (0.968
                    + trend * 0.032
                    + (index * 0.34 + days * 0.17).sin() * 0.009
                    + (index * 0.91).sin() * 0.0028
                    + if trend > 0.65 {
                        ((trend - 0.65) * 14.0).sin() * 0.012
                    } else {
                        0.0
                    })
        };
        let offset = market.price - close_at(63.0);
        let mut previous = market.price * 0.966;
        let samples: Vec<Value> = (0..64).map(|index| {
            let close = close_at(f64::from(index)); let open = previous; previous = close;
            let volume = 1800.0 + ((f64::from(index) * 2.13).sin() + 1.0) * 1600.0 + if index % 11 == 0 { 3400.0 } else { 0.0 };
            let buy = volume * if close >= open { 0.64 } else { 0.36 };
            json!({"time": now - (days * 86400000.0 * (1.0 - f64::from(index) / 63.0)) as i64,
                "open": open + offset, "close": close + offset,
                "high": open.max(close) + market.price * (0.0015 + f64::from(index % 5) * 0.00035) + offset,
                "low": open.min(close) - market.price * (0.0013 + f64::from(index % 4) * 0.00042) + offset,
                "volume": volume, "buyVolume": buy, "sellVolume": volume - buy})
        }).collect();
        result.insert(range.into(), json!(samples));
    }
    Value::Object(result)
}
pub fn validate(draft: &Draft, market: &Market) -> Result<(), &'static str> {
    if ![draft.price, draft.amount, draft.price * draft.amount]
        .iter()
        .all(|value| value.is_finite() && *value > 0.0 && *value <= 1e12)
    {
        return Err("invalid_amount_or_price");
    }
    let scale = 10f64.powi(market.amount_decimals);
    if draft.amount < 1.0 / scale {
        return Err("precision");
    }
    if (draft.amount * scale - (draft.amount * scale).round()).abs() > 0.001 {
        return Err("precision");
    }
    let supports = |network: &str, asset: &str| {
        matches!(
            (network, asset),
            ("Everscale", "EVER")
                | ("Bitcoin", "BTC")
                | ("Ethereum (ERC-20)", "ETH" | "USDT")
                | ("Solana", "SOL" | "USDT")
                | ("TRON (TRC-20)", "USDT")
        )
    };
    let (send, receive) = if draft.side == Side::Buy {
        (market.quote, market.base)
    } else {
        (market.base, market.quote)
    };
    if !supports(&draft.send_network, send) || !supports(&draft.receive_network, receive) {
        return Err("invalid_network");
    }
    Ok(())
}
pub fn execution_price(market: &Market, side: Side, amount: f64) -> Result<f64, &'static str> {
    let opposite = if side == Side::Buy {
        Side::Sell
    } else {
        Side::Buy
    };
    let mut remaining = amount;
    let mut total = 0.0;
    for (price, available) in levels(market, opposite) {
        let take = remaining.min(available);
        total += take * price;
        remaining -= take;
        if remaining <= amount * 1e-10 {
            return Ok(total / amount);
        }
    }
    Err("liquidity")
}
