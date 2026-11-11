//! Route coverage for project API keys (§8.1).
//!
//! Every HTTP route the backend serves must fall into one of these classes so a
//! new business endpoint cannot ship unprotected. The table mirrors
//! `server/routing/api.rs` plus the OTC and Hex routers; the unit test fails if
//! an entry is missing a classification.

use super::keys::Scope;

/// How a route is authenticated with respect to merchant API keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    /// Plan allowlist: health, docs, auth, payer checkout, public catalogue,
    /// federation and provider webhooks.
    Keyless,
    /// Merchant business route; requires `Authorization: Bearer sk_…` with
    /// this scope (and the matching environment once enforcement lands).
    Protected(Scope),
    /// `/api/b2b/*` and `/api/v1/*` during the deprecation window: a project
    /// key or the legacy user Bearer JWT is accepted.
    LegacyUserOrKey(Scope),
    /// Staff tooling keeps its own auth and must never accept merchant keys.
    Staff,
    /// Browser-called site routes that stay keyless until the SvelteKit
    /// first-party proxy lands. Remove each entry by the stated date.
    TemporarySite { until: &'static str },
}

#[derive(Debug, Clone, Copy)]
pub struct RouteSpec {
    pub method: &'static str,
    pub path: &'static str,
    pub class: RouteClass,
}

/// Canonical coverage table. Paths use Axum's `:param` / `{param}` forms as
/// registered on the router; classification matches by prefix/pattern below.
pub const ROUTES: &[RouteSpec] = &[
    // Allowlist
    RouteSpec {
        method: "GET",
        path: "/health",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/health",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/health/ready",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/metrics",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/openapi.json",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/scalar",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/api/auth/register",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/api/auth/login",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/auth/me",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/api/auth/oauth/{provider}",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/api/anonymous/register",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/pay/:token",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/pay/:token/status",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/pay/:token/quote",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/networks",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/banks",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/providers",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/providers/:slug/statistics",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/api/providers/:provider/webhooks",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/reviews/providers/:slug",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/api/reviews/profile",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/.well-known/webfinger",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/actor",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/actor/:handle",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/marketplace/resources/exchange",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/marketplace/proposals/exchange",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/marketplace/interfaces/exchange",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/marketplace/shapes/exchange-input",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/marketplace/shapes/exchange-output",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/marketplace/preview",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "GET",
        path: "/candidates",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/inbox",
        class: RouteClass::Keyless,
    },
    RouteSpec {
        method: "POST",
        path: "/inbox/:handle",
        class: RouteClass::Keyless,
    },
    // Legacy B2B / v1 (key or user JWT until deprecation ends)
    RouteSpec {
        method: "POST",
        path: "/api/v1/quotes",
        class: RouteClass::LegacyUserOrKey(Scope::QuotesWrite),
    },
    RouteSpec {
        method: "POST",
        path: "/api/v1/transfers",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersWrite),
    },
    RouteSpec {
        method: "GET",
        path: "/api/v1/transfers",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersRead),
    },
    RouteSpec {
        method: "GET",
        path: "/api/v1/transfers/:id",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersRead),
    },
    RouteSpec {
        method: "POST",
        path: "/api/v1/transfers/:id/cancel",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersWrite),
    },
    RouteSpec {
        method: "POST",
        path: "/api/v1/payments",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersWrite),
    },
    RouteSpec {
        method: "POST",
        path: "/api/b2b/webhooks",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersWrite),
    },
    RouteSpec {
        method: "GET",
        path: "/api/b2b/orders",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersRead),
    },
    RouteSpec {
        method: "GET",
        path: "/api/b2b/analytics",
        class: RouteClass::LegacyUserOrKey(Scope::OrdersRead),
    },
    // Staff
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange-pairs",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange-pairs/:id",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "GET",
        path: "/api/admin/exchange/controls",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange/controls",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange/corridors/:id",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange/solvers/:id",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange/orders/:id/manual-review",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/exchange/orders/:id/dispute",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/banks",
        class: RouteClass::Staff,
    },
    RouteSpec {
        method: "POST",
        path: "/api/admin/banks/:name/status",
        class: RouteClass::Staff,
    },
    // Temporary site exceptions — remove when the SvelteKit first-party proxy
    // holds a key for these calls (open question §12.15).
    RouteSpec {
        method: "GET",
        path: "/api/market-prices",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/market-values",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/reference-rates",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange-pairs",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/referrals/me",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/share-links",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/share-links/:id",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/payments",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/payments",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/payments/:id",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/ws",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/ws/rates",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/ws/payments",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/ws/p2p/routes",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/ws/p2p/discovery",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/corridors",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id/quotes",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id/live",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders/:id/discover",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders/:id/auction",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders/:id/confirm",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders/:id/funding/confirm",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id/manual-review",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id/settlement",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id/ledger",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/exchange/orders/:id/proofs",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders/:id/proof",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/exchange/orders/:id/cancel",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/debug/exchange/orders/:id/audit",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/solver/orders/open",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/solver/orders/:id/quotes",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/p2p/search",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/p2p/futures",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/p2p/routes",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/p2p/discovery",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/p2p/route-activity",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/p2p/route-executions",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/p2p/route-executions/prepare",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/p2p/route-executions/:id",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/p2p/route-executions/:id/submissions",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/service-executions/open",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/route-instructions/open",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "PUT",
        path: "/api/services/:id/vote",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "PUT",
        path: "/api/routes/:route_id/vote",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/debug/quote",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/debug/task",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/routing/fallback",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    // OTC — product scope otc:write once enforcement is on; own session today,
    // so temporary until merchant-key OTC lands with the portal.
    RouteSpec {
        method: "GET",
        path: "/ws/otc",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/otc/config",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/hex/config",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/hex/snapshot",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/hex/account",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "GET",
        path: "/api/hex/history",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
    RouteSpec {
        method: "POST",
        path: "/api/hex/quote",
        class: RouteClass::TemporarySite {
            until: "2026-12-11",
        },
    },
];

/// Look up the declared class for an exact method + path template pair.
pub fn class_for(method: &str, path: &str) -> Option<RouteClass> {
    ROUTES
        .iter()
        .find(|route| route.method == method && route.path == path)
        .map(|route| route.class)
}

/// Classify a route, including prefix rules for the OTC and Hex surfaces that
/// still use their own sessions until merchant-key OTC lands.
pub fn classify(method: &str, path: &str) -> Option<RouteClass> {
    if let Some(class) = class_for(method, path) {
        return Some(class);
    }
    if path == "/ws/otc" || path.starts_with("/api/otc/") {
        return Some(RouteClass::TemporarySite {
            until: "2026-12-11",
        });
    }
    if path.starts_with("/api/hex/") {
        return Some(RouteClass::TemporarySite {
            until: "2026-12-11",
        });
    }
    None
}

/// Whether the class may proceed without a project API key.
pub fn allows_keyless(class: RouteClass) -> bool {
    matches!(
        class,
        RouteClass::Keyless | RouteClass::TemporarySite { .. } | RouteClass::Staff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_has_a_class() {
        for route in ROUTES {
            assert!(
                class_for(route.method, route.path).is_some(),
                "{} {} missing from coverage table",
                route.method,
                route.path
            );
        }
    }

    #[test]
    fn no_duplicate_method_path_pairs() {
        let mut seen = std::collections::BTreeSet::new();
        for route in ROUTES {
            assert!(
                seen.insert((route.method, route.path)),
                "duplicate coverage entry: {} {}",
                route.method,
                route.path
            );
        }
    }

    #[test]
    fn v1_and_b2b_stay_on_legacy_or_key() {
        assert_eq!(
            class_for("POST", "/api/v1/quotes"),
            Some(RouteClass::LegacyUserOrKey(Scope::QuotesWrite))
        );
        assert_eq!(
            class_for("GET", "/api/b2b/orders"),
            Some(RouteClass::LegacyUserOrKey(Scope::OrdersRead))
        );
    }

    #[test]
    fn allowlist_stays_keyless() {
        assert_eq!(class_for("GET", "/health"), Some(RouteClass::Keyless));
        assert_eq!(
            class_for("POST", "/api/auth/login"),
            Some(RouteClass::Keyless)
        );
        assert_eq!(class_for("GET", "/pay/:token"), Some(RouteClass::Keyless));
    }

    #[test]
    fn temporary_site_exceptions_are_dated() {
        let Some(RouteClass::TemporarySite { until }) = class_for("GET", "/api/market-prices")
        else {
            panic!("market-prices must be a dated temporary exception until the site proxy");
        };
        assert_eq!(until, "2026-12-11");
    }

    #[test]
    fn otc_and_hex_prefix_rules_cover_unlisted_paths() {
        assert_eq!(
            classify("POST", "/api/otc/rfqs"),
            Some(RouteClass::TemporarySite {
                until: "2026-12-11"
            })
        );
        assert_eq!(
            classify("GET", "/api/hex/account"),
            Some(RouteClass::TemporarySite {
                until: "2026-12-11"
            })
        );
        assert_eq!(classify("GET", "/api/unknown"), None);
    }
}
