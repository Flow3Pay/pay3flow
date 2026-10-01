use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::p2p::routes::{NormalizedRouteQuery, P2pRoute};
use crate::p2p::P2pOffer;

use super::diversity::seed_diverse_routes;
use super::{compose_fiat_route, positive_number, route_target};

#[derive(Debug, Clone, Copy)]
struct FiatRouteCandidate {
    target_amount: f64,
    entry_index: usize,
    exit_index: usize,
}

impl PartialEq for FiatRouteCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.target_amount.total_cmp(&other.target_amount) == Ordering::Equal
            && self.entry_index == other.entry_index
            && self.exit_index == other.exit_index
    }
}

impl Eq for FiatRouteCandidate {}

impl PartialOrd for FiatRouteCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FiatRouteCandidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.target_amount
            .total_cmp(&other.target_amount)
            .then_with(|| other.entry_index.cmp(&self.entry_index))
            .then_with(|| other.exit_index.cmp(&self.exit_index))
    }
}

pub(in crate::p2p) fn compose_fiat_routes(
    routes: &mut Vec<P2pRoute>,
    query: &NormalizedRouteQuery,
    asset: &str,
    entry_offers: &[P2pOffer],
    exit_offers: &[P2pOffer],
) -> bool {
    let mut entries = entry_offers
        .iter()
        .filter_map(|entry| {
            let entry_price = positive_number(&entry.price)?;
            let acquired_asset = query.source_amount / entry_price;
            positive_number(&entry.available_asset)
                .is_none_or(|available| available >= acquired_asset)
                .then_some((entry, entry_price, acquired_asset))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.1.total_cmp(&right.1));
    let mut exits = exit_offers
        .iter()
        .filter_map(|exit| positive_number(&exit.price).map(|price| (exit, price)))
        .collect::<Vec<_>>();
    exits.sort_by(|left, right| right.1.total_cmp(&left.1));
    if entries.is_empty() || exits.is_empty() {
        return true;
    }

    let candidate_limit = query.limit.saturating_mul(3).max(query.limit);
    routes.reserve(candidate_limit.min(entries.len().saturating_mul(exits.len())));
    let routes_start = routes.len();
    let (mut evaluated, saturated) = seed_diverse_routes(
        routes,
        routes_start,
        candidate_limit,
        query,
        asset,
        &entries,
        &exits,
    );
    if saturated {
        return false;
    }

    let mut frontier = BinaryHeap::with_capacity(entries.len());
    for (entry_index, (_, _, acquired_asset)) in entries.iter().enumerate() {
        frontier.push(FiatRouteCandidate {
            target_amount: acquired_asset * exits[0].1,
            entry_index,
            exit_index: 0,
        });
    }

    while let Some(candidate) = frontier.pop() {
        let (entry, _, acquired_asset) = entries[candidate.entry_index];
        let (exit, exit_price) = exits[candidate.exit_index];
        let next_exit_index = candidate.exit_index + 1;
        if next_exit_index < exits.len() {
            frontier.push(FiatRouteCandidate {
                target_amount: acquired_asset * exits[next_exit_index].1,
                entry_index: candidate.entry_index,
                exit_index: next_exit_index,
            });
        }

        if evaluated.insert((candidate.entry_index, candidate.exit_index)) {
            if let Some(route) = compose_fiat_route(
                query,
                asset,
                entry,
                exit,
                acquired_asset,
                acquired_asset * exit_price,
            ) {
                routes.push(route);
                if routes.len() - routes_start >= candidate_limit {
                    routes[routes_start..].sort_by(|left, right| {
                        route_target(right)
                            .partial_cmp(&route_target(left))
                            .unwrap_or(Ordering::Equal)
                    });
                    return frontier.is_empty();
                }
            }
        }
    }
    routes[routes_start..].sort_by(|left, right| {
        route_target(right)
            .partial_cmp(&route_target(left))
            .unwrap_or(Ordering::Equal)
    });
    true
}
