use std::collections::HashSet;
use std::sync::Arc;

use crate::p2p::service::P2pOffer;
use crate::route_engine::{Amount, Asset, PublicRouteProvider, PublicRouteQuote};

#[derive(Clone)]
pub(in crate::p2p) struct RouteProviderCapability {
    pub(in crate::p2p) provider: Arc<dyn PublicRouteProvider>,
    pub(in crate::p2p) assets: HashSet<Asset>,
}

impl RouteProviderCapability {
    pub(in crate::p2p) fn supports(&self, from: &Asset, to: &Asset) -> bool {
        self.assets.contains(from) && self.assets.contains(to)
    }
}

pub(in crate::p2p) struct FiatProviderQuoteJob {
    pub(in crate::p2p) provider: Arc<dyn PublicRouteProvider>,
    pub(in crate::p2p) from: Asset,
    pub(in crate::p2p) to: Asset,
    pub(in crate::p2p) amount: Amount,
    pub(in crate::p2p) entry_offer: P2pOffer,
    pub(in crate::p2p) exit_offers: Arc<[P2pOffer]>,
}

pub(in crate::p2p) fn provider_quote_key(
    provider: &str,
    from: &Asset,
    to: &Asset,
    amount: &Amount,
) -> String {
    format!("route|{provider}|{from}|{to}|{}", amount.value)
}

pub(in crate::p2p) fn fiat_quote_key(
    provider: &str,
    source: &str,
    target: &str,
    amount: f64,
) -> String {
    format!("fiat|{provider}|{source}|{target}|{amount:.2}")
}

pub(in crate::p2p) fn provider_priority(provider: &str) -> u8 {
    match provider {
        "symbiosis" => 0,
        "bestchange" => 1,
        "cow-swap" => 2,
        "near-intents" => 3,
        _ => 4,
    }
}

pub(in crate::p2p) fn network_priority(network: Option<&str>) -> u8 {
    match network {
        Some("ethereum") => 0,
        Some("tron") => 1,
        Some("bnb-smart-chain") => 2,
        Some("polygon-pos") => 3,
        Some("arbitrum-one") => 4,
        Some("optimism") => 5,
        Some("avalanche-c") => 6,
        Some("solana") => 7,
        _ => 8,
    }
}

pub(in crate::p2p) async fn quote_provider(
    provider: Arc<dyn PublicRouteProvider>,
    from: Asset,
    to: Asset,
    amount: Amount,
    quote_semaphore: Arc<tokio::sync::Semaphore>,
) -> Option<(String, PublicRouteQuote)> {
    let provider_name = provider.name().to_string();
    let from_label = from.to_string();
    let to_label = to.to_string();
    let amount_label = amount.value.clone();
    // The permit intentionally covers the whole external call, including its
    // timeout, so a slow provider cannot keep consuming request slots.
    let _permit = match quote_semaphore.acquire_owned().await {
        Ok(permit) => permit,
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                "provider quote semaphore closed"
            );
            return None;
        }
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        provider.quote(from, to, amount),
    )
    .await;
    match result {
        Ok(Ok(quote)) => Some((provider_name, quote)),
        Ok(Err(error)) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quote failed"
            );
            None
        }
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quote timed out"
            );
            None
        }
    }
}

pub(in crate::p2p) async fn quote_provider_many(
    provider: Arc<dyn PublicRouteProvider>,
    from: Asset,
    to: Asset,
    amount: Amount,
    quote_semaphore: Arc<tokio::sync::Semaphore>,
) -> Option<(String, Vec<PublicRouteQuote>)> {
    let provider_name = provider.name().to_string();
    let from_label = from.to_string();
    let to_label = to.to_string();
    let amount_label = amount.value.clone();
    let _permit = match quote_semaphore.acquire_owned().await {
        Ok(permit) => permit,
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                "provider quote semaphore closed"
            );
            return None;
        }
    };
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        provider.quotes(from, to, amount),
    )
    .await;
    match result {
        Ok(Ok(quotes)) if !quotes.is_empty() => Some((provider_name, quotes)),
        Ok(Ok(_)) => None,
        Ok(Err(error)) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quotes failed"
            );
            None
        }
        Err(error) => {
            tracing::warn!(
                %error,
                provider = %provider_name,
                from = %from_label,
                to = %to_label,
                amount = %amount_label,
                "public route quotes timed out"
            );
            None
        }
    }
}
