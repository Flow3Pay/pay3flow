use super::{error::Result, repo, Service};
use chrono::Utc;
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
                let count = repo::metrics::event_count(&client, aggregate, kind, direction).await?;
                let _=writeln!(output,"pay3flow_otc_events_total{{{labels},event=\"{kind}\",direction=\"{direction}\"}} {count}");
            }
        }
        output.push_str("# TYPE pay3flow_otc_trades gauge\n");
        for (state, count) in repo::metrics::trade_states(&client).await? {
            let _ = writeln!(
                output,
                "pay3flow_otc_trades{{{labels},state=\"{state}\"}} {count}"
            );
        }
        output.push_str("# TYPE pay3flow_otc_observer_last_scan_seconds gauge\n# TYPE pay3flow_otc_observer_healthy gauge\n");
        for (chain, at, paused, failures, lag) in repo::metrics::observers(&client).await? {
            let healthy = !paused && at.is_some_and(|a| (Utc::now() - a).num_seconds() < 60);
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
                failures
            );
            if let Some(lag) = lag {
                let _ = writeln!(
                    output,
                    "pay3flow_otc_observer_lag{{{labels},chain=\"{chain}\"}} {lag}"
                );
            }
        }
        output.push_str("# TYPE pay3flow_otc_quote_response_seconds histogram\n");
        // Durably linked RFQ cohorts; HTTP retries cannot increase this histogram.
        for bucket in [5, 15, 30, 60, 120, 300] {
            let count = repo::metrics::quote_response_bucket(&client, bucket).await?;
            let _ = writeln!(
                output,
                "pay3flow_otc_quote_response_seconds_bucket{{{labels},le=\"{bucket}\"}} {count}"
            );
        }
        let (count, sum) = repo::metrics::quote_response_totals(&client).await?;
        let _=writeln!(output,"pay3flow_otc_quote_response_seconds_bucket{{{labels},le=\"+Inf\"}} {count}\npay3flow_otc_quote_response_seconds_count{{{labels}}} {count}\npay3flow_otc_quote_response_seconds_sum{{{labels}}} {sum}");
        for (severity, count, at) in repo::metrics::open_incidents(&client).await? {
            let _=writeln!(output,"pay3flow_otc_open_incidents{{{labels},severity=\"{severity}\"}} {count}\npay3flow_otc_oldest_incident_seconds{{{labels},severity=\"{severity}\"}} {}",(Utc::now()-at).num_seconds());
        }
        Ok(output)
    }
}
