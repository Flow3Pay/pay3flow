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
    if let Some(to_currency) = &req.to_currency {
        content.push_str(&format!("; to_currency={}", to_currency.trim()));
    }
    content
}

fn fmt_amount(amount: f64) -> String {
    format!("{}", amount)
}

/// Compute a quote for the request: ask fmatch (read-only), fall back to
/// local routing rules when fmatch is unreachable. When the request converts
/// to a target currency (`to_currency`), fmatch's answer is re-ranked by the
/// fee each managed solver actually charges on that pair — so `best` is always
/// the cheapest route that can move the money, not just fmatch's relevance
/// winner. fmatch itself stays read-only.
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
    let resolved = match resolved {
        RouteResolved {
            source: RouteSource::Fmatch,
            candidates,
        } => {
            let from = request.currency.as_str();
            let to = request.to_currency.as_deref().unwrap_or(from);
            if from.eq_ignore_ascii_case(to) {
                RouteResolved {
                    source: RouteSource::Fmatch,
                    candidates,
                }
            } else {
                RouteResolved {
                    source: RouteSource::Fmatch,
                    candidates: rank_candidates_by_pair_fee(from, to, candidates),
                }
            }
        }
        other => other,
    };
    quote_from_result(request, resolved)
}

/// Re-rank fmatch candidates by the fee they actually charge on the swap pair
/// `from → to_currency`. fmatch ranks by its own quality/latency/intent model
/// and never sees a percent commission (its price is per-token), so without
/// this step the cheapest route on the pair would lose to a rank-1 candidate
/// that may not even serve the pair. Candidates that do serve the pair come
/// first, cheapest first (fmatch rank as the tiebreak); the rest stay after
/// them in their original order, so `best` (rank-1) is always the cheapest
/// route that can move the money. fmatch itself is untouched.
pub fn rank_candidates_by_pair_fee(
    from: &str,
    to: &str,
    candidates: Vec<AcquirerCandidate>,
) -> Vec<AcquirerCandidate> {
    let pair_fee = |candidate: &AcquirerCandidate| {
        let acquirer = crate::fake_acquirers::fake_acquirer_by_name(&candidate.name)?;
        acquirer
            .commission_for(from, to)
            .and_then(crate::fake_acquirers::commission_pct)
    };

    let mut serving: Vec<(AcquirerCandidate, f64, u64)> = candidates
        .iter()
        .filter_map(|candidate| {
            let fee = pair_fee(candidate)?;
            let mut ranked = candidate.clone();
            ranked.price = Some(fee / 100.0);
            Some((ranked, fee, candidate.rank))
        })
        .collect();
    serving.sort_by(|left, right| {
        left.1
            .partial_cmp(&right.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.2.cmp(&right.2))
    });

    let mut ranked = serving.into_iter().map(|(candidate, ..)| candidate).collect::<Vec<_>>();
    let mut rest = candidates
        .into_iter()
        .filter(|candidate| pair_fee(candidate).is_none())
        .collect::<Vec<_>>();
    rest.sort_by_key(|candidate| candidate.rank);
    ranked.extend(rest);
    for (index, candidate) in ranked.iter_mut().enumerate() {
        candidate.rank = (index + 1) as u64;
    }
    ranked
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

    fn cand(name: &str, rank: u64, price: Option<f64>) -> AcquirerCandidate {
        AcquirerCandidate {
            name: name.into(),
            short_id: name.to_lowercase(),
            rank,
            price: if let Some(p) = price { Some(p) } else { None },
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
            candidates: vec![cand("B", 2, Some(0.04)), cand("A", 1, Some(0.02)), cand("C", 3, Some(0.05))],
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

    #[test]
    fn pair_fee_ranking_prefers_cheapest_serving_solver() {
        // fmatch returns bramba rank-1, but it doesn't serve EUR→USD (4.1%
        // worst commission) while helixpay (rank 3, 1.5%) does.
        let candidates = vec![
            cand("Bramba Global (EU, 4.1)", 1, None),
            cand("Corvus Exchange (US|EU|Global, 2.8)", 2, None),
            cand("Flinger Pay (US|EU|Global, 3.4)", 3, None),
            cand("HelixPay Global (Global, 2.9)", 4, None),
        ];
        let ranked = rank_candidates_by_pair_fee("EUR", "USD", candidates);

        // helixpay (1.5%) > corvus (1.6%) > flinger (1.8%); bramba doesn't
        // serve EUR→USD and is pushed behind every serving candidate.
        assert_eq!(ranked[0].name, "HelixPay Global (Global, 2.9)");
        assert!((ranked[0].price.unwrap() - 0.015).abs() < 1e-9);
        assert_eq!(ranked[0].rank, 1);

        assert_eq!(ranked[1].name, "Corvus Exchange (US|EU|Global, 2.8)");
        assert!((ranked[1].price.unwrap() - 0.016).abs() < 1e-9);
        assert_eq!(ranked[1].rank, 2);

        assert_eq!(ranked[2].name, "Flinger Pay (US|EU|Global, 3.4)");
        assert!((ranked[2].price.unwrap() - 0.018).abs() < 1e-9);
        assert_eq!(ranked[2].rank, 3);

        // bramba still present but can never win `best` (rank-1).
        let bramba = ranked.iter().find(|c| c.name.starts_with("Bramba")).unwrap();
        assert!(bramba.rank > 1);
        assert!(bramba.price.is_none());
    }
}
