use std::cmp::Ordering;
use std::collections::HashSet;

use crate::p2p::routes::{NormalizedRouteQuery, P2pRoute};
use crate::p2p::P2pOffer;

use super::{compose_fiat_route, route_target};

pub(super) fn seed_diverse_routes(
    routes: &mut Vec<P2pRoute>,
    routes_start: usize,
    candidate_limit: usize,
    query: &NormalizedRouteQuery,
    asset: &str,
    entries: &[(&P2pOffer, f64, f64)],
    exits: &[(&P2pOffer, f64)],
) -> (HashSet<(usize, usize)>, bool) {
    let mut evaluated = HashSet::new();
    let mut source_names = Vec::new();
    let mut seen_sources = HashSet::new();
    for index in 0..entries.len().max(exits.len()) {
        if let Some((entry, _, _)) = entries.get(index) {
            if seen_sources.insert(entry.source.as_str()) {
                source_names.push(entry.source.as_str());
            }
        }
        if let Some((exit, _)) = exits.get(index) {
            if seen_sources.insert(exit.source.as_str()) {
                source_names.push(exit.source.as_str());
            }
        }
    }

    for source in source_names {
        let mut best = None;
        for (entry_index, (entry, _, acquired_asset)) in entries.iter().enumerate() {
            if entry.source != source {
                continue;
            }
            for (exit_index, (exit, exit_price)) in exits.iter().enumerate() {
                let target_amount = acquired_asset * exit_price;
                if let Some(route) =
                    compose_fiat_route(query, asset, entry, exit, *acquired_asset, target_amount)
                {
                    if best
                        .as_ref()
                        .is_none_or(|(current, _, _, _)| target_amount > *current)
                    {
                        best = Some((target_amount, entry_index, exit_index, route));
                    }
                    break;
                }
            }
        }
        for (exit_index, (exit, exit_price)) in exits.iter().enumerate() {
            if exit.source != source {
                continue;
            }
            for (entry_index, (entry, _, acquired_asset)) in entries.iter().enumerate() {
                let target_amount = acquired_asset * exit_price;
                if let Some(route) =
                    compose_fiat_route(query, asset, entry, exit, *acquired_asset, target_amount)
                {
                    if best
                        .as_ref()
                        .is_none_or(|(current, _, _, _)| target_amount > *current)
                    {
                        best = Some((target_amount, entry_index, exit_index, route));
                    }
                    break;
                }
            }
        }
        if let Some((_, entry_index, exit_index, route)) = best {
            if evaluated.insert((entry_index, exit_index)) {
                routes.push(route);
            }
        }
        if routes.len() - routes_start >= candidate_limit {
            routes[routes_start..].sort_by(|left, right| {
                route_target(right)
                    .partial_cmp(&route_target(left))
                    .unwrap_or(Ordering::Equal)
            });
            return (evaluated, true);
        }
    }
    (evaluated, false)
}
