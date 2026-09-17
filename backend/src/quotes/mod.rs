use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::activitypub::model::AcquirerCandidate;
use crate::activitypub::Service;
use crate::routing::{PaymentRequest, RoutePicker, RouteResolved, RouteSource};

/// Live quote for an exchange pair. Built by asking fmatch which of its
/// solvers (acquirers) can serve the payment task and how expensive they are.
/// fmatch is consumed as an external read-only API: we only push a task into
/// its inbox and read the candidate answer back — nothing inside fmatch is
/// edited.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    /// The request that produced the quote.
    pub request: PaymentRequest,
    /// Where the candidates came from: fmatch (preferred) or local fallback.
    pub source: RouteSource,
    /// Ranked acquirers (rank 1 = cheap-and-good according to the source).
    pub candidates: Vec<AcquirerCandidate>,
    /// The pick of the list: the rank-1 acquirer, when any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best: Option<AcquirerCandidate>,
    /// RFC3339 timestamp of the quote.
    pub quoted_at: String,
}

/// Render a payment request as the structured `content` of the fmatch
/// Proposal(purpose="request"). fmatch reads this to filter its solver pool.
pub fn fmatch_content(req: &PaymentRequest) -> String {
    let mut content = format!(
        "transfer {} {} from {} to {}; currency={}; from={}; to={}",
        fmt_amount(req.amount),
        req.currency,
        req.from,
        req.to,
        req.currency,
        req.from,
        req.to
    );
    if let Some(geo) = &req.to_geo {
        content.push_str(&format!("; to_geo={}", geo.trim()));
    }
    if let Some(method) = &req.method {
        content.push_str(&format!("; method={}", method.trim()));
    }
    content
}

fn fmt_amount(amount: f64) -> String {
    format!("{}", amount)
}

/// Compute a quote for the request: ask fmatch (read-only), fall back to
/// local routing rules when fmatch is unreachable.
pub async fn compute_quote(ap: &Service, picker: &RoutePicker, request: PaymentRequest) -> Quote {
    let resolved = match ap
        .submit_request("candidates", &fmatch_content(&request))
        .await
    {
        Ok((_outcome, body)) => {
            let candidates = body
                .as_ref()
                .map(AcquirerCandidate::from_reply)
                .unwrap_or_default();
            picker.resolve(&request, Some(candidates))
        }
        Err(_) => picker.resolve(&request, None),
    };
    quote_from_result(request, resolved)
}

/// Wrap a resolved route into a quote, pinning the rank-1 acquirer as `best`.
pub fn quote_from_result(request: PaymentRequest, resolved: RouteResolved) -> Quote {
    let candidates = resolved.candidates;
    let best = candidates.iter().min_by_key(|c| c.rank).cloned();
    Quote {
        request,
        source: resolved.source,
        candidates,
        best,
        quoted_at: Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::routing::RouteSource;

    fn cand(name: &str, rank: u64, price: f64) -> AcquirerCandidate {
        AcquirerCandidate {
            name: name.into(),
            short_id: name.to_lowercase(),
            rank,
            price: Some(price),
            quality: Some(1.0),
            raw: json!({}),
        }
    }

    #[test]
    fn content_mentions_amount_currency_and_legs() {
        let req = PaymentRequest::new(100.0, "EUR", "PayPal", "VISA");
        let content = fmatch_content(&req);
        assert!(content.starts_with("transfer 100 EUR from PayPal to VISA"));
        assert!(content.contains("currency=EUR"));
        assert!(content.contains("from=PayPal"));
        assert!(content.contains("to=VISA"));
    }

    #[test]
    fn content_appends_geo_and_method_when_set() {
        let mut req = PaymentRequest::new(250.5, "USD", "PayPal", "Bank");
        req.to_geo = Some("NL".into());
        req.method = Some("SEPA".into());
        let content = fmatch_content(&req);
        assert!(content.starts_with("transfer 250.5 USD"));
        assert!(content.contains("to_geo=NL"));
        assert!(content.contains("method=SEPA"));
    }

    #[test]
    fn best_is_the_rank_one_candidate() {
        let req = PaymentRequest::new(100.0, "EUR", "PayPal", "VISA");
        let resolved = RouteResolved {
            source: RouteSource::Fmatch,
            candidates: vec![cand("B", 2, 0.04), cand("A", 1, 0.02), cand("C", 3, 0.05)],
        };
        let quote = quote_from_result(req, resolved);
        assert_eq!(quote.source, RouteSource::Fmatch);
        assert_eq!(quote.candidates.len(), 3);
        let best = quote.best.expect("rank-1 must be picked");
        assert_eq!(best.name, "A");
        assert_eq!(best.price, Some(0.02));
    }

    #[test]
    fn best_is_none_on_empty_candidates() {
        let req = PaymentRequest::new(100.0, "EUR", "PayPal", "VISA");
        let resolved = RouteResolved {
            source: RouteSource::Fallback,
            candidates: vec![],
        };
        let quote = quote_from_result(req, resolved);
        assert_eq!(quote.source, RouteSource::Fallback);
        assert!(quote.best.is_none());
        assert!(quote.candidates.is_empty());
    }
}
