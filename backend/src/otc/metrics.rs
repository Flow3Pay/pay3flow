use super::{error::Result, Service};
use chrono::{DateTime, Utc};
use std::fmt::Write;

impl Service {
    pub(crate) async fn metrics(&self, actor: &str) -> Result<String> {
        self.desk(actor)?;
        let client = self.pool.get().await?;
        let mut output = String::new();
        let environment = if self.cfg.demo { "demo" } else { "live" };
        let labels = format!("environment=\"{environment}\",desk=\"onboarded\"");
        output.push_str("# TYPE pay3flow_otc_events_total counter\n");
        for kind in [
            "rfq_requested",
            "quoted",
            "booking_requested",
            "accepted",
            "funded",
            "completed",
            "refunded",
            "evidence_invalidated",
            "transfer_failed",
            "blocked_transfer",
            "payment_review",
        ] {
            for direction in ["buy", "sell"] {
                let aggregate = if [
                    "rfq_requested",
                    "quoted",
                    "booking_requested",
                    "accepted",
                    "funded",
                    "completed",
                    "refunded",
                ]
                .contains(&kind)
                {
                    "count(DISTINCT COALESCE(e.trade_id,e.rfq_id))"
                } else {
                    "count(*)"
                };
                let query = format!("SELECT {aggregate} FROM otc_events e LEFT JOIN otc_trades t ON t.id=e.trade_id LEFT JOIN otc_rfq r ON r.id=COALESCE(e.rfq_id,t.rfq_id) WHERE e.kind=$1 AND r.direction=$2");
                let count: i64 = client.query_one(&query, &[&kind, &direction]).await?.get(0);
                let _=writeln!(output,"pay3flow_otc_events_total{{{labels},event=\"{kind}\",direction=\"{direction}\"}} {count}");
            }
        }
        output.push_str("# TYPE pay3flow_otc_trades gauge\n");
        for row in client
            .query("SELECT state,count(*) FROM otc_trades GROUP BY state", &[])
            .await?
        {
            let state: String = row.get(0);
            let count: i64 = row.get(1);
            let _ = writeln!(
                output,
                "pay3flow_otc_trades{{{labels},state=\"{state}\"}} {count}"
            );
        }
        output.push_str("# TYPE pay3flow_otc_observer_last_scan_seconds gauge\n# TYPE pay3flow_otc_observer_healthy gauge\n");
        for row in client
            .query(
                "SELECT chain,last_scan,paused,failures,lag FROM otc_observers",
                &[],
            )
            .await?
        {
            let chain: String = row.get(0);
            let at: Option<DateTime<Utc>> = row.get(1);
            let healthy =
                !row.get::<_, bool>(2) && at.is_some_and(|a| (Utc::now() - a).num_seconds() < 60);
            let _ = writeln!(
                output,
                "pay3flow_otc_observer_last_scan_seconds{{{labels},chain=\"{chain}\"}} {}",
                at.map(|a| a.timestamp()).unwrap_or(0)
            );
            let _ = writeln!(
                output,
                "pay3flow_otc_observer_healthy{{{labels},chain=\"{chain}\"}} {}",
                u8::from(healthy)
            );
            let _ = writeln!(
                output,
                "pay3flow_otc_observer_rpc_failures{{{labels},chain=\"{chain}\"}} {}",
                row.get::<_, i32>(3)
            );
            if let Some(lag) = row.get::<_, Option<i64>>(4) {
                let _ = writeln!(
                    output,
                    "pay3flow_otc_observer_lag{{{labels},chain=\"{chain}\"}} {lag}"
                );
            }
        }
        output.push_str("# TYPE pay3flow_otc_quote_response_seconds histogram\n");
        // Durably linked RFQ cohorts; HTTP retries cannot increase this histogram.
        for bucket in [5, 15, 30, 60, 120, 300] {
            let count:i64=client.query_one("SELECT count(*) FROM otc_rfq r JOIN otc_trades t ON t.rfq_id=r.id JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id' WHERE extract(epoch FROM i.processed_at-r.created_at)<=$1::int",&[&bucket]).await?.get(0);
            let _ = writeln!(
                output,
                "pay3flow_otc_quote_response_seconds_bucket{{{labels},le=\"{bucket}\"}} {count}"
            );
        }
        let row=client.query_one("SELECT count(*),COALESCE(sum(extract(epoch FROM i.processed_at-r.created_at)),0)::double precision FROM otc_rfq r JOIN otc_trades t ON t.rfq_id=r.id JOIN otc_inbox i ON i.actor=t.customer_actor AND i.activity_id=t.terms->'proposal'->>'id'",&[]).await?;
        let count: i64 = row.get(0);
        let sum: f64 = row.get(1);
        let _=writeln!(output,"pay3flow_otc_quote_response_seconds_bucket{{{labels},le=\"+Inf\"}} {count}\npay3flow_otc_quote_response_seconds_count{{{labels}}} {count}\npay3flow_otc_quote_response_seconds_sum{{{labels}}} {sum}");
        for row in client.query("SELECT severity,count(*),min(detected_at) FROM otc_incidents WHERE closed_at IS NULL GROUP BY severity",&[]).await? {
            let severity:String=row.get(0);let count:i64=row.get(1);let at:DateTime<Utc>=row.get(2);let _=writeln!(output,"pay3flow_otc_open_incidents{{{labels},severity=\"{severity}\"}} {count}\npay3flow_otc_oldest_incident_seconds{{{labels},severity=\"{severity}\"}} {}",(Utc::now()-at).num_seconds());
        }
        Ok(output)
    }
}
