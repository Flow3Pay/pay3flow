//! Test OTC data and orders. No settlement or wallet operations are performed.
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use uuid::Uuid;

#[path = "market.rs"]
mod market;

#[derive(Clone, Default)]
pub struct TestOtc(Arc<Mutex<HashMap<Uuid, Session>>>);

#[derive(Default)]
struct Session {
    orders: Vec<Order>,
    touched: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Limit,
    Market,
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Draft {
    pub market_id: String,
    pub side: Side,
    #[serde(rename = "type")]
    pub kind: OrderType,
    pub price: f64,
    pub amount: f64,
    pub send_network: String,
    pub receive_network: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Order {
    id: Uuid,
    #[serde(flatten)]
    draft: Draft,
    created_at: i64,
    status: &'static str,
}

impl TestOtc {
    pub async fn snapshot(&self, session: Uuid, market_id: &str) -> Result<Value, &'static str> {
        let market = market::find(market_id).ok_or("unknown_market")?;
        let now = Utc::now().timestamp_millis();
        let mut sessions = self.0.lock().await;
        sessions.retain(|_, entry| now - entry.touched < 86_400_000);
        if !sessions.contains_key(&session) && sessions.len() >= 1000 {
            return Err("session_capacity");
        }
        let entry = sessions.entry(session).or_default();
        entry.touched = now;
        let orders = entry.orders.clone(); // Own the outbound snapshot before releasing shared state.
        drop(sessions);
        Ok(
            json!({"type": "snapshot", "market": market::details(market), "snapshot": market::snapshot(market, now),
            "networks": [
                {"id": "everscale", "name": "Everscale", "currencies": ["EVER"]},
                {"id": "bitcoin", "name": "Bitcoin", "currencies": ["BTC"]},
                {"id": "ethereum", "name": "Ethereum (ERC-20)", "currencies": ["ETH", "USDT"]},
                {"id": "solana", "name": "Solana", "currencies": ["SOL", "USDT"]},
                {"id": "tron", "name": "TRON (TRC-20)", "currencies": ["USDT"]}
            ], "candles": market::candles(market, now), "orders": orders, "mode": "test"}),
        )
    }

    pub async fn create(
        &self,
        session: Uuid,
        id: Uuid,
        mut draft: Draft,
    ) -> Result<Value, &'static str> {
        let market = market::find(&draft.market_id).ok_or("unknown_market")?;
        market::validate(&draft, market)?;
        let mut sessions = self.0.lock().await;
        let entry = sessions.get_mut(&session).ok_or("subscribe_first")?;
        entry.touched = Utc::now().timestamp_millis();
        if let Some(order) = entry.orders.iter().find(|order| order.id == id) {
            // Market prices are authoritative and may differ from the client's preview.
            let same = order.draft.market_id == draft.market_id
                && order.draft.side == draft.side
                && order.draft.kind == draft.kind
                && order.draft.amount == draft.amount
                && order.draft.send_network == draft.send_network
                && order.draft.receive_network == draft.receive_network
                && (draft.kind == OrderType::Market || order.draft.price == draft.price);
            return if same {
                Ok(json!({"type": "order", "id": id, "order": order}))
            } else {
                Err("id_conflict")
            };
        }
        if entry.orders.len() >= 100 {
            return Err("order_capacity");
        }
        if draft.kind == OrderType::Market {
            draft.price = market::execution_price(market, draft.side, draft.amount)?;
        }
        let status = if draft.kind == OrderType::Market {
            "simulated"
        } else {
            "open"
        };
        entry.orders.insert(
            0,
            Order {
                id,
                draft,
                created_at: entry.touched,
                status,
            },
        );
        Ok(json!({"type": "order", "id": id, "order": entry.orders[0]}))
    }

    pub async fn cancel(
        &self,
        session: Uuid,
        id: Uuid,
        order_id: Uuid,
    ) -> Result<Value, &'static str> {
        let mut sessions = self.0.lock().await;
        let entry = sessions.get_mut(&session).ok_or("subscribe_first")?;
        let order = entry
            .orders
            .iter_mut()
            .find(|order| order.id == order_id)
            .ok_or("order_not_found")?;
        if order.status == "simulated" {
            return Err("order_not_open");
        }
        order.status = "cancelled";
        entry.touched = Utc::now().timestamp_millis();
        Ok(json!({"type": "order", "id": id, "order": order}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn draft() -> Draft {
        Draft {
            market_id: "EVER-USDT".into(),
            side: Side::Buy,
            kind: OrderType::Limit,
            price: 0.01,
            amount: 100.0,
            send_network: "Ethereum (ERC-20)".into(),
            receive_network: "Everscale".into(),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn orders_survive_resubscription_and_are_private_and_idempotent() {
        let otc = TestOtc::default();
        let owner = Uuid::new_v4();
        let other = Uuid::new_v4();
        let id = Uuid::new_v4();
        otc.snapshot(owner, "EVER-USDT").await.unwrap();
        otc.snapshot(other, "EVER-USDT").await.unwrap();
        let first = otc.create(owner, id, draft()).await.unwrap();
        assert_eq!(first, otc.create(owner, id, draft()).await.unwrap());
        assert_eq!(
            otc.snapshot(owner, "EVER-USDT").await.unwrap()["orders"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            otc.cancel(other, Uuid::new_v4(), id).await.unwrap_err(),
            "order_not_found"
        );
        otc.cancel(owner, Uuid::new_v4(), id).await.unwrap();
        assert_eq!(
            otc.snapshot(owner, "BTC-USDT").await.unwrap()["orders"][0]["status"],
            "cancelled"
        );
        let mut changed = draft();
        changed.amount = 200.0;
        assert_eq!(
            otc.create(owner, id, changed).await.unwrap_err(),
            "id_conflict"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rejects_invalid_orders_and_reprices_market_orders() {
        let otc = TestOtc::default();
        let owner = Uuid::new_v4();
        otc.snapshot(owner, "EVER-USDT").await.unwrap();
        for amount in [-1.0, 0.0, f64::NAN, 1e15, 0.0000001] {
            let mut invalid = draft();
            invalid.amount = amount;
            assert!(
                otc.create(owner, Uuid::new_v4(), invalid).await.is_err(),
                "accepted {amount}"
            );
        }
        let mut order = draft();
        order.kind = OrderType::Market;
        order.price = 10.0;
        let result = otc
            .create(owner, Uuid::new_v4(), order.clone())
            .await
            .unwrap();
        assert_eq!(result["order"]["status"], "simulated");
        assert!((result["order"]["price"].as_f64().unwrap() - 0.01001).abs() < 1e-12);
        order.amount = 1e10;
        assert_eq!(
            otc.create(owner, Uuid::new_v4(), order).await.unwrap_err(),
            "liquidity"
        );
    }
}
