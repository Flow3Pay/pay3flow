//! Background settlement and expiry for B2B orders.
//!
//! `paid` means the provider delivered to the recipient. The deposit hash is
//! only evidence that funds arrived, never the success signal.

use serde_json::Value;

use super::*;

/// Single-flight key for the order poller. Webhook delivery has its own row
/// locks and does not need this lock.
const WORKER_LOCK: i64 = 0x4232_424C_4F43_4B31;

impl Service {
    pub fn start(self) {
        if !self.is_enabled() {
            return;
        }
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;
                self.tick().await;
            }
        });
    }

    async fn tick(&self) {
        let client = match self.pool.get().await {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(%error, "b2b worker could not get a database connection");
                return;
            }
        };
        let locked: bool = client
            .query_one("SELECT pg_try_advisory_lock($1)", &[&WORKER_LOCK])
            .await
            .map(|row| row.get(0))
            .unwrap_or(false);
        if !locked {
            return;
        }
        if let Err(error) = self.poll_orders().await {
            tracing::warn!(%error, "b2b order poll failed");
        }
        if let Err(error) = self.expire_unaccepted().await {
            tracing::warn!(%error, "b2b quote expiry failed");
        }
        if let Err(error) = self.deliver_webhooks().await {
            tracing::warn!(%error, "b2b webhook delivery failed");
        }
        let _ = client
            .execute("SELECT pg_advisory_unlock($1)", &[&WORKER_LOCK])
            .await;
    }

    async fn poll_orders(&self) -> Result<()> {
        for order in self.accepted_orders().await? {
            let status = self
                .near
                .status_with_memo(&order.deposit_address, order.deposit_memo.as_deref())
                .await;
            let status = match status {
                Ok(status) => status,
                Err(error) => {
                    tracing::warn!(
                        order_id = %order.order_id,
                        %error,
                        "b2b provider status poll failed"
                    );
                    continue;
                }
            };
            let mapped = map_provider_status(&status.status, &status.raw, &order.input_amount);
            if let Some(next) = next_order_status(&order.status, mapped) {
                self.apply_status(&order, next, &status.raw).await?;
            }
        }
        Ok(())
    }

    async fn apply_status(
        &self,
        order: &AcceptedOrder,
        new_status: &str,
        provider_raw: &Value,
    ) -> Result<()> {
        let mut client = self.client().await?;
        let tx = client.transaction().await?;
        let updated = tx
            .execute(
                "UPDATE b2b_orders SET status = $3, updated_at = now() \
                 WHERE id = $1 AND status = $2",
                &[&order.order_id, &order.status, &new_status],
            )
            .await?;
        if updated == 0 {
            // Another worker moved the order first; never emit a duplicate.
            tx.rollback().await?;
            return Ok(());
        }
        self.record_attempts(&tx, order, provider_raw, new_status)
            .await?;
        if matches!(new_status, "paid" | "overpaid") {
            self.record_fee(&tx, order).await?;
        }
        let body = self
            .transfer_body(&tx, order.merchant_id, order.order_id)
            .await?;
        webhook::queue_event(&tx, order.order_id, &format!("order.{new_status}"), &body).await?;
        tx.commit().await?;
        tracing::info!(
            order_id = %order.order_id,
            status = new_status,
            "b2b order status changed"
        );
        Ok(())
    }

    async fn record_fee(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        order: &AcceptedOrder,
    ) -> Result<()> {
        let Some(recipient) = self.fee_recipient() else {
            return Ok(());
        };
        if order.captured_bps <= 0 {
            return Ok(());
        }
        let Some(amount) = bps_amount(&order.input_amount, order.captured_bps) else {
            tracing::warn!(
                order_id = %order.order_id,
                "b2b fee amount could not be computed"
            );
            return Ok(());
        };
        tx.execute(
            r#"
INSERT INTO b2b_fee_ledger
    (merchant_id, order_id, asset, amount, captured_bps, recipient)
SELECT $1, $2, input_asset, $3, $4, $5 FROM b2b_orders WHERE id = $2
"#,
            &[
                &order.merchant_id,
                &order.order_id,
                &amount,
                &order.captured_bps,
                &recipient,
            ],
        )
        .await?;
        Ok(())
    }

    async fn record_attempts(
        &self,
        tx: &tokio_postgres::Transaction<'_>,
        order: &AcceptedOrder,
        raw: &Value,
        status: &str,
    ) -> Result<()> {
        let details = raw.get("swapDetails").cloned().unwrap_or(Value::Null);
        let status = status.to_ascii_uppercase();
        let deposit_amount = details
            .get("depositedAmountFormatted")
            .or_else(|| details.get("amountInFormatted"))
            .and_then(Value::as_str)
            .unwrap_or(&order.input_amount);
        for hash in tx_hashes(&details, "originChainTxHashes") {
            tx.execute(
                r#"
INSERT INTO b2b_attempts
    (order_id, quote_id, kind, chain, tx_hash, amount, status)
VALUES ($1, $2, 'deposit', $3, $4, $5, $6)
ON CONFLICT (chain, tx_hash, kind) DO NOTHING
"#,
                &[
                    &order.order_id,
                    &order.quote_id,
                    &order.input_network,
                    &hash,
                    &deposit_amount,
                    &status,
                ],
            )
            .await?;
        }
        let delivery_amount = details
            .get("amountOutFormatted")
            .and_then(Value::as_str)
            .unwrap_or(&order.output_amount);
        for hash in tx_hashes(&details, "destinationChainTxHashes") {
            tx.execute(
                r#"
INSERT INTO b2b_attempts
    (order_id, quote_id, kind, chain, tx_hash, amount, status)
VALUES ($1, $2, 'delivery', $3, $4, $5, $6)
ON CONFLICT (chain, tx_hash, kind) DO NOTHING
"#,
                &[
                    &order.order_id,
                    &order.quote_id,
                    &order.output_network,
                    &hash,
                    &delivery_amount,
                    &status,
                ],
            )
            .await?;
        }
        if let Some(hash) = details
            .get("refundTxHash")
            .or_else(|| details.get("refundedTxHash"))
            .and_then(Value::as_str)
        {
            let amount = details
                .get("refundedAmountFormatted")
                .and_then(Value::as_str)
                .unwrap_or(&order.input_amount);
            tx.execute(
                r#"
INSERT INTO b2b_attempts
    (order_id, quote_id, kind, chain, tx_hash, amount, status)
VALUES ($1, $2, 'refund', $3, $4, $5, $6)
ON CONFLICT (chain, tx_hash, kind) DO NOTHING
"#,
                &[
                    &order.order_id,
                    &order.quote_id,
                    &order.input_network,
                    &hash,
                    &amount,
                    &status,
                ],
            )
            .await?;
        }
        Ok(())
    }
}

// A quote deadline cannot prove whether a deposit is still in flight. Keep
// observing issued addresses until delivery/refund, even after local cancellation.
fn next_order_status<'a>(current: &str, mapped: Option<&'a str>) -> Option<&'a str> {
    if mapped == Some("awaiting_deposit") && current != "awaiting_deposit" {
        return None; // Do not reopen a cancelled/failed/underpaid order on a stale pending response.
    }
    mapped.filter(|mapped| *mapped != current)
}

fn tx_hashes(details: &Value, key: &str) -> Vec<String> {
    details
        .get(key)
        .and_then(Value::as_array)
        .map(|hashes| {
            hashes
                .iter()
                .filter_map(|entry| {
                    entry
                        .get("hash")
                        .and_then(Value::as_str)
                        .or_else(|| entry.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Map the provider's status vocabulary onto the B2B state machine.
fn map_provider_status(provider: &str, raw: &Value, input_amount: &str) -> Option<&'static str> {
    match provider.to_ascii_uppercase().as_str() {
        "PENDING_DEPOSIT" => Some("awaiting_deposit"),
        "KNOWN_DEPOSIT_TX" | "PROCESSING" => Some("processing"),
        "SUCCESS" => {
            let deposited = raw
                .get("swapDetails")
                .and_then(|details| {
                    details
                        .get("depositedAmountFormatted")
                        .or_else(|| details.get("amountInFormatted"))
                })
                .and_then(Value::as_str);
            if deposited.is_some_and(|deposited| decimal_greater(deposited, input_amount)) {
                Some("overpaid")
            } else {
                Some("paid")
            }
        }
        "INCOMPLETE_DEPOSIT" => Some("underpaid"),
        "REFUNDED" => Some("refunded"),
        "FAILED" => Some("failed"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn release_regression_pending_deposits_remain_observable_after_quote_expiry() {
        assert_eq!(
            next_order_status("awaiting_deposit", Some("awaiting_deposit")),
            None
        );
        assert_eq!(
            next_order_status("awaiting_deposit", Some("paid")),
            Some("paid")
        );
        assert_eq!(next_order_status("processing", Some("processing")), None);
    }

    #[test]
    fn release_regression_late_delivery_and_refunds_remain_observable() {
        for current in ["cancelled", "expired", "failed", "underpaid"] {
            assert_eq!(next_order_status(current, Some("awaiting_deposit")), None);
            assert_eq!(next_order_status(current, Some("paid")), Some("paid"));
            assert_eq!(
                next_order_status(current, Some("refunded")),
                Some("refunded")
            );
        }
    }

    #[test]
    fn provider_statuses_follow_the_state_machine() {
        let raw = json!({});
        assert_eq!(
            map_provider_status("PENDING_DEPOSIT", &raw, "10"),
            Some("awaiting_deposit")
        );
        assert_eq!(
            map_provider_status("KNOWN_DEPOSIT_TX", &raw, "10"),
            Some("processing")
        );
        assert_eq!(map_provider_status("SUCCESS", &raw, "10"), Some("paid"));
        assert_eq!(
            map_provider_status("INCOMPLETE_DEPOSIT", &raw, "10"),
            Some("underpaid")
        );
        assert_eq!(
            map_provider_status("REFUNDED", &raw, "10"),
            Some("refunded")
        );
        assert_eq!(map_provider_status("FAILED", &raw, "10"), Some("failed"));
        assert_eq!(map_provider_status("UNKNOWN", &raw, "10"), None);
    }

    #[test]
    fn underpaid_never_becomes_paid() {
        let raw = json!({});
        assert_ne!(
            map_provider_status("INCOMPLETE_DEPOSIT", &raw, "10"),
            Some("paid")
        );
    }

    #[test]
    fn overpayment_is_reported_without_losing_delivery() {
        let raw = json!({"swapDetails": {"depositedAmountFormatted": "11"}});
        assert_eq!(map_provider_status("SUCCESS", &raw, "10"), Some("overpaid"));
    }

    #[test]
    fn tx_hash_extraction_accepts_objects_and_strings() {
        let details = json!({"originChainTxHashes": [{"hash": "0xabc"}, "0xdef"]});
        assert_eq!(
            tx_hashes(&details, "originChainTxHashes"),
            ["0xabc", "0xdef"]
        );
    }
}
