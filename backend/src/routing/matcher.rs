use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::activitypub::model::AcquirerCandidate;
use crate::routing::profile::{seed_pool, AcquirerProfile};
use crate::routing::request::PaymentRequest;

/// Local rules-based selector used when fmatch is unreachable.
///
/// Candidates come out in the same shape fmatch delivers (`AcquirerCandidate`),
/// so the routing pipeline can swap the source: fmatch when healthy, local
/// rules as fallback. Local rules mirror what fmatch was asked to guarantee:
/// active status, supported currency, geo coverage of the destination and
/// amount within limits; ranking is by cheapest effective fee (worst-case
/// percent, flat part approximated in the payment currency).
#[derive(Debug, Clone, Copy, Default)]
pub struct FallbackMatcher;

impl FallbackMatcher {
    pub fn select(
        &self,
        request: &PaymentRequest,
        pool: &[AcquirerProfile],
    ) -> Vec<AcquirerCandidate> {
        let mut ranked: Vec<(AcquirerCandidate, f64)> = pool
            .iter()
            .filter(|p| eligible(p, request))
            .map(|p| {
                let price = effective_price(p, request);
                (
                    AcquirerCandidate {
                        name: p.name.clone(),
                        short_id: p.slug.clone(),
                        rank: 0,
                        price: Some(price),
                        quality: Some(quality(p, request)),
                        raw: json!({ "source": "fallback" }),
                    },
                    price,
                )
            })
            .collect();
        ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        ranked
            .into_iter()
            .enumerate()
            .map(|(i, (mut c, _))| {
                c.rank = (i + 1) as u64;
                c
            })
            .collect()
    }
}

/// Where the resolved candidate list came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteSource {
    Fmatch,
    Fallback,
}

impl RouteSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteSource::Fmatch => "fmatch",
            RouteSource::Fallback => "fallback",
        }
    }
}

/// Result of the routing step: candidates plus the source that produced them.
#[derive(Debug, Clone, Serialize)]
pub struct RouteResolved {
    pub source: RouteSource,
    pub candidates: Vec<AcquirerCandidate>,
}

/// Entry point for payment routing.
#[derive(Debug, Clone)]
pub struct RoutePicker {
    pub matcher: FallbackMatcher,
    pub pool: Vec<AcquirerProfile>,
}

impl RoutePicker {
    pub fn from_seeds() -> Self {
        Self {
            matcher: FallbackMatcher,
            pool: seed_pool(),
        }
    }

    /// Resolve candidates for a payment request.
    ///
    /// `None` = fmatch unreachable → local fallback rules; `Some(list)` = the
    /// fmatch answer taken as-is (even when empty — fmatch is the source of
    /// truth when reachable).
    pub fn resolve(
        &self,
        request: &PaymentRequest,
        fmatch: Option<Vec<AcquirerCandidate>>,
    ) -> RouteResolved {
        match fmatch {
            Some(candidates) => RouteResolved {
                source: RouteSource::Fmatch,
                candidates,
            },
            None => RouteResolved {
                source: RouteSource::Fallback,
                candidates: self.matcher.select(request, &self.pool),
            },
        }
    }
}

fn eligible(p: &AcquirerProfile, req: &PaymentRequest) -> bool {
    if !p.status.eq_ignore_ascii_case("active") {
        return false;
    }
    if !p
        .currencies
        .iter()
        .any(|c| c.eq_ignore_ascii_case(&req.currency))
    {
        return false;
    }
    if let Some(geo) = &req.to_geo {
        if !geo_covers(&p.geo, geo) {
            return false;
        }
    }
    // Limits are compared only when the acquirer expresses them in the same
    // currency as the request; otherwise we can't convert yet (phase 2) and the
    // limit is not a hard constraint.
    if let Some(limit_currency) = &p.amount_currency {
        if limit_currency.eq_ignore_ascii_case(&req.currency) {
            if let (Some(min), Some(max)) = (p.min_amount, p.max_amount) {
                if req.amount < min || req.amount > max {
                    return false;
                }
            }
        }
    }
    true
}

fn geo_covers(profile_geo: &[String], country: &str) -> bool {
    let country = country.trim().to_uppercase();
    profile_geo.iter().any(|token| {
        let token = token.trim().to_uppercase();
        token == "GLOBAL"
            || token == country
            || (token == "EU" && EU_MEMBERS.contains(&country.as_str()))
    })
}

fn effective_price(p: &AcquirerProfile, req: &PaymentRequest) -> f64 {
    if req.amount <= 0.0 {
        return 0.0;
    }
    (req.amount * p.fee_percent / 100.0 + p.fee_flat) / req.amount
}

fn quality(p: &AcquirerProfile, req: &PaymentRequest) -> f64 {
    if let (Some(max), Some(cur)) = (&p.max_amount, &p.amount_currency) {
        if cur.eq_ignore_ascii_case(&req.currency) && *max > 0.0 {
            return ((*max - req.amount) / *max).clamp(0.0, 1.0);
        }
    }
    0.9
}

const EU_MEMBERS: &[&str] = &[
    "AT", "BE", "BG", "HR", "CY", "CZ", "DK", "EE", "FI", "FR", "DE", "GR", "HU", "IE", "IT", "LV",
    "LT", "LU", "MT", "NL", "PL", "PT", "RO", "SK", "SI", "ES", "SE",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::profile::seed_pool;

    fn with_geo(mut req: PaymentRequest, geo: &str) -> PaymentRequest {
        req.to_geo = Some(geo.to_string());
        req
    }

    #[test]
    fn cheapest_eu_candidate_wins() {
        let matcher = FallbackMatcher;
        let req = with_geo(PaymentRequest::new(100.0, "EUR", "PayPal", "VISA"), "NL");
        let cands = matcher.select(&req, &seed_pool());
        assert_eq!(cands[0].name, "Mollie");
        assert_eq!(cands[0].rank, 1);
        assert!(cands[0].price.unwrap() < cands[1].price.unwrap());
    }

    #[test]
    fn unsupported_currency_excluded() {
        let matcher = FallbackMatcher;
        let req = with_geo(PaymentRequest::new(100.0, "GBP", "Bank", "IBAN"), "NL");
        let cands = matcher.select(&req, &seed_pool());
        assert!(cands.iter().all(|c| c.name != "Mollie"));
        assert!(cands.iter().any(|c| c.name == "Adyen"));
    }

    #[test]
    fn geo_not_covered_excluded() {
        let matcher = FallbackMatcher;
        // Belarus is not in the EU set: only "Global"/exact matches survive.
        let req = with_geo(
            PaymentRequest::new(100.0, "EUR", "PayPal", "VISA BelorusBank"),
            "BY",
        );
        let cands = matcher.select(&req, &seed_pool());
        assert!(!cands.is_empty());
        for c in &cands {
            assert!(["Wise", "Payoneer"].contains(&c.name.as_str()));
        }
        assert_eq!(cands[0].name, "Wise");
    }

    #[test]
    fn amount_out_of_limits_excluded() {
        let matcher = FallbackMatcher;
        let req = with_geo(
            PaymentRequest::new(150_000.0, "USD", "PayPal", "Bank"),
            "US",
        );
        let cands = matcher.select(&req, &seed_pool());
        assert!(cands.iter().all(|c| c.name != "Stripe"));
        assert!(cands.iter().any(|c| c.name == "Checkout.com"));
    }

    #[test]
    fn inactive_acquirer_excluded() {
        let mut pool = seed_pool();
        for profile in &mut pool {
            if profile.name == "Adyen" {
                profile.status = "disabled".to_string();
            }
        }
        let matcher = FallbackMatcher;
        let req = with_geo(PaymentRequest::new(100.0, "EUR", "PayPal", "VISA"), "NL");
        let cands = matcher.select(&req, &pool);
        assert!(cands.iter().all(|c| c.name != "Adyen"));
    }

    #[test]
    fn no_eligible_leaves_empty_list() {
        let matcher = FallbackMatcher;
        let req = PaymentRequest::new(100.0, "XYZ", "PayPal", "Bank");
        assert!(matcher.select(&req, &seed_pool()).is_empty());
    }

    #[test]
    fn picker_prefers_fmatch_when_available() {
        let picker = RoutePicker::from_seeds();
        let req = with_geo(PaymentRequest::new(100.0, "EUR", "PayPal", "VISA"), "NL");
        let with_fmatch = vec![AcquirerCandidate {
            name: "FakeAcquirer".into(),
            short_id: "f1".into(),
            rank: 1,
            price: Some(0.01),
            quality: Some(0.99),
            raw: json!({}),
        }];
        let res = picker.resolve(&req, Some(with_fmatch));
        assert_eq!(res.source, RouteSource::Fmatch);
        assert_eq!(res.candidates.len(), 1);
        assert_eq!(res.candidates[0].name, "FakeAcquirer");
    }

    #[test]
    fn picker_falls_back_when_fmatch_unreachable() {
        let picker = RoutePicker::from_seeds();
        let req = with_geo(PaymentRequest::new(100.0, "EUR", "PayPal", "VISA"), "NL");
        let res = picker.resolve(&req, None);
        assert_eq!(res.source, RouteSource::Fallback);
        assert!(!res.candidates.is_empty());
        assert!(res.candidates.iter().all(|c| c.raw["source"] == "fallback"));
    }
}
