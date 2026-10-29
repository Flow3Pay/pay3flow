//! Bounded, resumable discovery. Cursors store traversal state, never P2P answers.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::banks::{Bank, BankFilters};
use crate::core::{redis, state::AppState};
use crate::route_engine::Asset;
use super::routes::{is_fiat_currency, normalize_query, profitability_for_route};
use super::service::PaymentMethodMatch;
use super::{ExchangeMode, P2pRoute, P2pRouteSearchQuery, RouteProfitability, SourceStatus};

const BATCH_SIZE: usize = 16;
const BATCH_SECONDS: u64 = 30;
const CURSOR_TTL: u64 = 900;

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryMode { #[default] Exchange, Arbitrage }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryQuery {
    #[serde(default)] pub mode: DiscoveryMode,
    #[serde(default = "any")] pub source_fiat: String,
    #[serde(default = "any")] pub target_fiat: String,
    pub source_amount: String,
    pub source_network: Option<String>,
    pub target_network: Option<String>,
    /// Stable bank catalog ids. Omission means any method.
    pub source_payment_method: Option<String>,
    pub target_payment_method: Option<String>,
    pub intermediary_assets: Option<String>,
    pub sources: Option<String>,
    #[serde(default)] pub exchange_mode: ExchangeMode,
    pub merchant_only: Option<bool>,
    pub min_orders: Option<u64>,
    pub min_completion_rate: Option<f64>,
    pub allow_cross_venue: Option<bool>,
    pub max_actions: Option<usize>,
    pub max_price_deviation_bps: Option<u32>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}
fn any() -> String { "ANY".into() }

impl DiscoveryQuery {
    fn normalize(&mut self) -> Result<()> {
        for code in [&mut self.source_fiat, &mut self.target_fiat] {
            *code = code.trim().to_ascii_uppercase();
            if *code == "TON" { *code = "GRAM".into(); }
            if *code != "ANY" { Asset::new(code.as_str(), None)?; }
        }
        let amount: f64 = self.source_amount.parse().context("source_amount must be a positive decimal")?;
        if !amount.is_finite() || amount <= 0.0 || decimal_units(&self.source_amount).is_none() {
            bail!("source_amount must be positive, at most 18 decimal places, and fit the supported amount range");
        }
        self.source_amount = self.source_amount.trim().to_string();
        self.sources = super::normalize_sources(self.sources.take())?;
        self.limit = Some(self.limit.unwrap_or(40).clamp(1, 100));
        if self.max_actions.is_some_and(|n| !(2..=8).contains(&n)) { bail!("max_actions must be between 2 and 8"); }
        if self.min_completion_rate.is_some_and(|n| !n.is_finite() || !(0.0..=1.0).contains(&n)) { bail!("min_completion_rate must be between 0 and 1"); }
        if self.max_price_deviation_bps.is_some_and(|n| n > 5000) { bail!("max_price_deviation_bps must not exceed 5000"); }
        for method in [&mut self.source_payment_method, &mut self.target_payment_method] {
            *method = method.take().map(|v| v.trim().to_string()).filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case("ANY"));
        }
        if self.mode == DiscoveryMode::Arbitrage {
            if self.target_fiat != "ANY" && self.target_fiat != self.source_fiat { bail!("arbitrage must return to the starting currency"); }
            if self.target_payment_method.is_some() && self.target_payment_method != self.source_payment_method { bail!("arbitrage must return to the starting payment method"); }
            if self.target_network.is_some() && self.target_network != self.source_network { bail!("arbitrage must return to the starting network"); }
            self.target_fiat = self.source_fiat.clone();
            self.target_payment_method = self.source_payment_method.clone();
            self.target_network = self.source_network.clone();
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
struct Endpoint { currency: String, network: Option<String> }
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Job { source: Endpoint, target: Endpoint }
#[derive(Deserialize, Serialize)]
struct CursorState { query_hash: String, catalog_hash: String, completed: BTreeSet<usize>, expires_at: i64 }

#[derive(Debug, Clone, Serialize)]
pub struct SettlementEndpoint {
    pub currency: String,
    pub network: Option<String>,
    pub method_id: Option<String>,
    pub method: Option<String>,
    pub venue: Option<String>,
    pub verified: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct Valuation { pub price_usd: f64, pub observed_at: DateTime<Utc>, pub source: String }
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryRoute {
    #[serde(flatten)] pub route: P2pRoute,
    pub source_settlement: SettlementEndpoint,
    pub target_settlement: SettlementEndpoint,
    pub net_profit: Option<String>,
    pub net_profit_usd: Option<f64>,
    pub estimated_profit_before_missing_costs_usd: Option<f64>,
    pub return_percent: Option<f64>,
    pub value_received_per_input: Option<f64>,
    pub source_value_usd: Option<f64>,
    pub target_value_usd: Option<f64>,
    pub source_valuation: Option<Valuation>,
    pub target_valuation: Option<Valuation>,
    pub missing_costs: Vec<String>,
    pub profit_confirmed: bool,
    pub observed_at: DateTime<Utc>,
    pub stale: bool,
}
#[derive(Debug, Clone, Default, Serialize)]
pub struct Coverage {
    pub combinations_total: usize,
    pub combinations_completed: usize,
    pub combinations_failed: usize,
    pub combinations_timed_out: usize,
    pub batch_completed: usize,
    pub combinations_partial: usize,
}
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryResponse {
    pub search_id: Uuid,
    pub mode: DiscoveryMode,
    pub source_amount: String,
    pub amount_basis: &'static str,
    pub reporting_currency: &'static str,
    pub searched_at: DateTime<Utc>,
    pub routes: Vec<DiscoveryRoute>,
    pub routes_found: usize,
    pub routes_exhaustive: bool,
    pub complete: bool,
    pub coverage: Coverage,
    pub provider_statuses: Vec<SourceStatus>,
    pub errors: Vec<String>,
    pub next_cursor: Option<String>,
}

pub fn bank_method_id(bank: &Bank) -> String {
    bank.method_id.clone().unwrap_or_else(|| format!("directory-{}", bank.id))
}
fn method_label(bank: &Bank) -> &str {
    if bank.name == "SBP Fast Payments" { return "SBP"; }
    if !bank.p2p_query.is_empty() { &bank.p2p_query } else { &bank.name }
}
fn role_matches(bank: &Bank, sender: bool) -> bool {
    bank.role == "both" || bank.role == if sender { "sender" } else { "receiver" }
        || (!sender && bank.role == "recipient")
}
fn selected_method<'a>(banks: &'a [Bank], id: &Option<String>, sender: bool) -> Result<Option<&'a Bank>> {
    let Some(id) = id else { return Ok(None); };
    let bank = banks.iter().find(|b| bank_method_id(b) == *id && role_matches(b, sender))
        .context("selected payment method is unavailable for this direction")?;
    Ok(Some(bank))
}

impl super::P2pSearchService {
    async fn discovery_catalog(&self, banks: &[Bank]) -> Vec<Endpoint> {
        let mut endpoints = BTreeSet::new();
        for bank in banks.iter().filter(|b| is_fiat_currency(&b.currency)) {
            endpoints.insert(Endpoint { currency: bank.currency.clone(), network: None });
        }
        for currency in self.networks.assets() {
            for network in self.networks.compatible_networks(&currency) {
                endpoints.insert(Endpoint { currency: currency.clone(), network: Some(network.id) });
            }
        }
        for asset in tokio::time::timeout(Duration::from_secs(8),self.provider_assets()).await.unwrap_or_else(|_|self.cached_provider_assets()) {
            if asset.location.is_some() {
                endpoints.insert(Endpoint { currency: asset.symbol, network: asset.location });
            }
        }
        let mut endpoints = endpoints.into_iter().collect::<Vec<_>>();
        // Broad discovery remains deterministic and advances through every catalog endpoint.
        endpoints.sort_by_key(|e| (match e.currency.as_str() { "RUB" => 0, "USD" => 1, "USDT" => 2, "USDC" => 3, "AMD" => 4, _ => 5 }, e.clone()));
        endpoints
    }
    async fn discovery_fiat_usd(&self, currency: &str) -> Option<Valuation> {
        if currency == "USD" { return Some(Valuation { price_usd: 1.0, observed_at: Utc::now(), source: "USD".into() }); }
        let candidate = self.fiat_quote_cache.read().ok().and_then(|cache| cache.values().filter_map(|c| {
            if c.inserted_at.elapsed() > Duration::from_secs(900) { return None; }
            let q = &c.quote;
            let rate = if q.source_currency == currency && q.target_currency == "USD" { q.target_amount / q.source_amount }
                else if q.source_currency == "USD" && q.target_currency == currency { q.source_amount / q.target_amount } else { return None; };
            (rate.is_finite() && rate > 0.0).then(|| Valuation { price_usd: rate, observed_at: Utc::now() - chrono::Duration::from_std(c.inserted_at.elapsed()).unwrap_or_default(), source: q.provider.clone() })
        }).next());
        if candidate.is_some() { return candidate; }
        let providers = self.fiat_route_providers.iter().filter(|p| p.supports_pair(currency, "USD")).cloned().collect::<Vec<_>>();
        let mut loads = providers.into_iter().map(|p| {
            let service = self.clone();
            let currency = currency.to_string();
            async move {
                let q = tokio::time::timeout(Duration::from_secs(2), service.fiat_quote(p, &currency, "USD", 1000.0)).await.ok()??;
                let rate = q.target_amount / q.source_amount;
                (q.source_currency == currency && q.target_currency == "USD" && rate.is_finite() && rate > 0.0)
                    .then(|| Valuation { price_usd: rate, observed_at: Utc::now(), source: q.provider })
            }
        }).collect::<futures::stream::FuturesUnordered<_>>();
        while let Some(value) = loads.next().await { if value.is_some() { return value; } }
        None
    }
}

fn endpoints_for(catalog: &[Endpoint], currency: &str, network: &Option<String>, method: Option<&Bank>) -> Vec<Endpoint> {
    catalog.iter().filter(|e| (currency == "ANY" || e.currency == currency)
        && !(currency != "ANY" && is_fiat_currency(currency) && network.is_none() && e.network.is_some())
        && network.as_ref().is_none_or(|n| e.network.as_ref() == Some(n))
        && method.is_none_or(|b| e.currency == b.currency && e.network.is_none())).cloned().collect()
}
fn job_count(sources: &[Endpoint], targets: &[Endpoint], mode: DiscoveryMode) -> usize {
    if mode == DiscoveryMode::Arbitrage { sources.len() } else { sources.len().saturating_mul(targets.len()) }
}
fn job_at(index: usize, sources: &[Endpoint], targets: &[Endpoint], mode: DiscoveryMode) -> Job {
    if mode == DiscoveryMode::Arbitrage { return Job { source: sources[index].clone(), target: sources[index].clone() }; }
    // Rotate the target for each source so early batches do not all quote one destination.
    let source_index = index % sources.len();
    let target_index = (index / sources.len() + source_index + 1) % targets.len();
    Job { source: sources[source_index].clone(), target: targets[target_index].clone() }
}
fn concrete_query(query: &DiscoveryQuery, job: &Job, source: Option<&Bank>, target: Option<&Bank>) -> P2pRouteSearchQuery {
    P2pRouteSearchQuery {
        source_fiat: job.source.currency.clone(), target_fiat: job.target.currency.clone(),
        source_amount: query.source_amount.parse().unwrap_or(0.0),
        source_network: job.source.network.clone(), target_network: job.target.network.clone(),
        bridge_fiat: None, assets: None, intermediary_assets: query.intermediary_assets.clone(),
        source_payment_method: source.map(|b| method_label(b).to_string()),
        target_payment_method: target.map(|b| method_label(b).to_string()),
        source_payment_fee_percent: source.and_then(|b| b.bank_fee_percent),
        target_payment_fee_percent: target.and_then(|b| b.bank_fee_percent),
        merchant_only: query.merchant_only, min_orders: query.min_orders, min_completion_rate: query.min_completion_rate,
        allow_cross_venue: Some(query.allow_cross_venue.unwrap_or(query.mode == DiscoveryMode::Arbitrage)),
        max_actions: Some(query.max_actions.unwrap_or(4)), max_price_deviation_bps: query.max_price_deviation_bps,
        limit: Some(100), sources: query.sources.clone(), exchange_mode: query.exchange_mode,
    }
}

fn offer_bank<'a>(banks: &'a [Bank], currency: &str, offer: Option<&super::P2pOffer>, selected: Option<&'a Bank>, sender: bool) -> Option<&'a Bank> {
    let offer = offer?;
    if let Some(bank) = selected {
        return (offer.payment_method_match(method_label(bank)) == PaymentMethodMatch::Exact).then_some(bank);
    }
    banks.iter().filter(|b| b.currency == currency && b.kind != "currency" && b.kind != "wallet" && role_matches(b, sender))
        .find(|b| offer.payment_method_match(method_label(b)) == PaymentMethodMatch::Exact)
}
fn offer_venue(offer: &super::P2pOffer) -> Option<String> {
    if offer.market == super::P2pOfferMarket::DirectExchange {
        return offer.advertiser.id.as_ref().map(|id| format!("{}:{id}",offer.source));
    }
    Some(offer.source.clone())
}
fn settlements(route: &P2pRoute, banks: &[Bank], source: Option<&Bank>, target: Option<&Bank>, mode: DiscoveryMode) -> Option<(SettlementEndpoint, SettlementEndpoint, Option<f64>, Option<f64>)> {
    let mut entry_bank = offer_bank(banks, &route.source_fiat, route.entry_offer.as_ref(), source, true);
    let mut exit_bank = offer_bank(banks, &route.target_fiat, route.exit_offer.as_ref(), target, false);
    if mode == DiscoveryMode::Arbitrage && route.source_network.is_none() {
        let entry = route.entry_offer.as_ref()?;
        let exit = route.exit_offer.as_ref()?;
        // Both endpoints must be the SAME verified method and venue. A favorable rate cannot close a different settlement location.
        if offer_venue(entry).is_none() || offer_venue(entry) != offer_venue(exit) { return None; }
        let common = banks.iter().filter(|b| b.currency == route.source_fiat && b.kind != "currency" && b.kind != "wallet"
            && role_matches(b, true) && role_matches(b, false)
            && source.is_none_or(|s| bank_method_id(s) == bank_method_id(b)))
            .find(|b| entry.payment_method_match(method_label(b)) == PaymentMethodMatch::Exact && exit.payment_method_match(method_label(b)) == PaymentMethodMatch::Exact)?;
        entry_bank = Some(common); exit_bank = Some(common);
    }
    let first = route.cycle_legs.first();
    let last = route.cycle_legs.last();
    let market_cycle = first.is_some_and(|l| l.market_pair.is_some());
    let entry_venue = route.entry_offer.as_ref().and_then(offer_venue).or_else(|| {
        if market_cycle { first.map(|l| l.provider.clone()) } else if route.source_network.is_some() { Some("wallet".into()) } else { route.route_provider.clone() }
    });
    let exit_venue = route.exit_offer.as_ref().and_then(offer_venue).or_else(|| {
        if market_cycle { last.map(|l| l.provider.clone()) } else if route.target_network.is_some() { Some("wallet".into()) } else { route.route_provider.clone() }
    });
    let endpoint = |currency: &String, network: &Option<String>, bank: Option<&Bank>, venue: Option<String>| SettlementEndpoint {
        currency: currency.clone(), network: network.clone(), method_id: bank.map(bank_method_id), method: bank.map(|b| if b.display_name.is_empty() { b.name.clone() } else { b.display_name.clone() }),
        verified: bank.is_some() || (network.is_some() && venue.is_some()), venue,
    };
    let a = endpoint(&route.source_fiat, &route.source_network, entry_bank, entry_venue);
    let b = endpoint(&route.target_fiat, &route.target_network, exit_bank, exit_venue);
    if mode == DiscoveryMode::Arbitrage && (!a.verified || !b.verified || a.currency != b.currency || a.network != b.network || a.method_id != b.method_id || a.venue != b.venue) { return None; }
    Some((a, b, entry_bank.and_then(|b| b.bank_fee_percent), exit_bank.and_then(|b| b.bank_fee_percent)))
}

// Exact fixed-scale arithmetic; values outside the checked range remain unconfirmed.
const DECIMAL_SCALE: i128 = 1_000_000_000_000_000_000;
fn decimal_units(value: &str) -> Option<i128> {
    let value = value.trim();
    let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
    if value.len() > 128 || whole.is_empty() || !whole.bytes().all(|c| c.is_ascii_digit()) || !fraction.bytes().all(|c| c.is_ascii_digit()) { return None; }
    let fraction = fraction.trim_end_matches('0');
    if fraction.len() > 18 { return None; }
    whole.parse::<i128>().ok()?.checked_mul(DECIMAL_SCALE)?.checked_add(if fraction.is_empty() { 0 } else { fraction.parse::<i128>().ok()?.checked_mul(10_i128.checked_pow(18 - fraction.len() as u32)?)? })
}
fn decimal_string(units: i128) -> String {
    let negative = units < 0;
    let units = units.unsigned_abs();
    let fraction = format!("{:018}", units % DECIMAL_SCALE as u128);
    let fraction = fraction.trim_end_matches('0');
    format!("{}{}{}{}", if negative { "-" } else { "" }, units / DECIMAL_SCALE as u128, if fraction.is_empty() { "" } else { "." }, fraction)
}
fn payment_fee(amount: i128, percent: f64) -> Option<i128> {
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) { return None; }
    let bps = (percent * 100.0).ceil() as i128;
    let fee = amount.checked_mul(bps)?.checked_add(9999)?.checked_div(10000)?;
    // Match the concrete fiat scorer: payment fees round upward to a minor unit.
    let minor_unit = DECIMAL_SCALE / 100;
    fee.checked_add(minor_unit - 1)?.checked_div(minor_unit)?.checked_mul(minor_unit)
}

fn evaluate(mut route: P2pRoute, observed_at: DateTime<Utc>, stale: bool, banks: &[Bank], query: &DiscoveryQuery, job: &Job, source: Option<&Bank>, target: Option<&Bank>, provider_assets: &[Asset], service: &super::P2pSearchService, prices: &HashMap<String, Valuation>) -> Option<DiscoveryRoute> {
    if route.quote_expires_at.is_some_and(|v| v <= Utc::now()) || route.cycle_legs.iter().any(|l| l.expires_at.is_some_and(|v| v <= Utc::now())) { return None; }
    let (a,b,source_fee,target_fee) = settlements(&route, banks, source, target, query.mode)?;
    let mut concrete = concrete_query(query, job, source, target);
    concrete.source_payment_fee_percent = if a.network.is_some() { Some(0.0) } else { source_fee };
    concrete.target_payment_fee_percent = if b.network.is_some() { Some(0.0) } else { target_fee };
    let normalized = normalize_query(concrete, &service.default_assets, &service.networks, provider_assets).ok()?;
    route.profitability = profitability_for_route(&route, &normalized);
    let mut missing = match &route.profitability {
        Some(RouteProfitability::Unconfirmed { missing_costs, .. }) => missing_costs.iter().map(|c| serde_json::to_value(c).ok()?.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()?,
        _ => Vec::new(),
    };
    if route.entry_offer.iter().chain(route.exit_offer.iter()).any(|o| o.market == super::P2pOfferMarket::DirectExchange) { missing.push("provider_fee".into()); }
    // An advertisement price does not prove a venue's account-specific trading commission.
    if route.entry_offer.iter().chain(route.exit_offer.iter()).any(|o| o.market == super::P2pOfferMarket::P2p) { missing.push("venue_trading_fee".into()); }
    if route.cycle_legs.iter().any(|l| l.market_pair.is_some()) { missing.push("actual_trading_fee_and_slippage".into()); }
    if stale { missing.push("stale_quote".into()); }
    if !a.verified || !b.verified { missing.push("payment_method".into()); }
    if a.network.is_none() && source_fee.is_none() { missing.push("source_payment_fee".into()); }
    if b.network.is_none() && target_fee.is_none() { missing.push("target_payment_fee".into()); }
    if route.requires_asset_transfer && !route.transfer_fee_included { missing.push("network_fee".into()); }
    if !route.cycle_legs.is_empty() && route.cycle_legs.iter().any(|l| l.market_pair.is_none()) { missing.push("wallet_gas_and_deposit_costs".into()); }
    let input = decimal_units(&route.source_amount)?;
    let output = decimal_units(&route.target_amount)?;
    let entry_fee = payment_fee(input, concrete_fee(a.network.is_some(), source_fee))?;
    let exit_fee = payment_fee(output, concrete_fee(b.network.is_some(), target_fee))?;
    let input_with_fees = input.checked_add(entry_fee)?;
    let output_after_fees = output.checked_sub(exit_fee)?;
    let net = output_after_fees.checked_sub(input_with_fees)?;
    missing.sort(); missing.dedup();
    let profit_confirmed = query.mode == DiscoveryMode::Arbitrage && missing.is_empty() && matches!(route.profitability, Some(RouteProfitability::Confirmed { .. }));
    // An explicit token network must not inherit the valuation of a colliding fiat ticker.
    let source_valuation = if a.network.is_some() && is_fiat_currency(&a.currency) { None } else { prices.get(&a.currency).cloned() };
    let target_valuation = if b.network.is_some() && is_fiat_currency(&b.currency) { None } else { prices.get(&b.currency).cloned() };
    let source_value = source_valuation.as_ref().map(|p| (input_with_fees as f64 / DECIMAL_SCALE as f64) * p.price_usd).filter(|v| v.is_finite() && *v > 0.0);
    let target_value = target_valuation.as_ref().map(|p| (output_after_fees as f64 / DECIMAL_SCALE as f64) * p.price_usd).filter(|v| v.is_finite());
    let net_profit_usd = if profit_confirmed { source_valuation.as_ref().map(|p| net as f64 / DECIMAL_SCALE as f64 * p.price_usd).filter(|v| v.is_finite()) } else { None };
    let estimated_profit_before_missing_costs_usd = if query.mode == DiscoveryMode::Arbitrage && !profit_confirmed {
        source_valuation.as_ref().map(|p| net as f64 / DECIMAL_SCALE as f64 * p.price_usd).filter(|v| v.is_finite())
    } else { None };
    let return_percent = if a.currency == b.currency && input_with_fees > 0 { Some(net as f64 / input_with_fees as f64 * 100.0) } else { None };
    // Never leak a confirmed badge from the legacy scorer when discovery found an unpriced cost.
    if !missing.is_empty() { if let Some(RouteProfitability::Confirmed { net_profit_minor, profit_bps }) = route.profitability {
        route.profitability = Some(RouteProfitability::Unconfirmed { gross_profit_minor: net_profit_minor, gross_profit_bps: profit_bps, missing_costs: missing.iter().map(|cost| match cost.as_str() {
            "source_payment_fee" => super::RouteCostKind::SourcePaymentFee,
            "target_payment_fee" => super::RouteCostKind::TargetPaymentFee,
            "stale_quote" => super::RouteCostKind::LiveQuote,
            "provider_fee" | "venue_trading_fee" | "actual_trading_fee_and_slippage" => super::RouteCostKind::ProviderFee,
            _ => super::RouteCostKind::NetworkFee,
        }).collect() });
    } }
    Some(DiscoveryRoute { route, source_settlement: a, target_settlement: b, net_profit: profit_confirmed.then(|| decimal_string(net)), net_profit_usd, estimated_profit_before_missing_costs_usd,
        return_percent, value_received_per_input: source_value.zip(target_value).map(|(a,b)| b / a).filter(|v| v.is_finite()), source_value_usd: source_value, target_value_usd: target_value,
        source_valuation, target_valuation, missing_costs: missing, profit_confirmed, observed_at, stale })
}
fn concrete_fee(crypto: bool, percent: Option<f64>) -> f64 { if crypto { 0.0 } else { percent.unwrap_or(0.0) } }

pub fn sort_results(routes: &mut [DiscoveryRoute], mode: DiscoveryMode) {
    routes.sort_by(|a,b| {
        let category = |r: &DiscoveryRoute| -> u8 {
            if mode == DiscoveryMode::Exchange { return if r.value_received_per_input.is_some() && r.missing_costs.is_empty() {0} else {1}; }
            match r.net_profit_usd { Some(p) if p > 0.0 => 0, Some(_) => 2, None => 1 }
        };
        let score = |r: &DiscoveryRoute| if mode == DiscoveryMode::Arbitrage { r.net_profit_usd.or(r.estimated_profit_before_missing_costs_usd) } else { r.value_received_per_input };
        category(a).cmp(&category(b)).then_with(|| match (score(a),score(b)) { (Some(a),Some(b)) => b.total_cmp(&a), (Some(_),None) => std::cmp::Ordering::Less, (None,Some(_)) => std::cmp::Ordering::Greater, _ => std::cmp::Ordering::Equal })
            .then_with(|| b.return_percent.unwrap_or(f64::NEG_INFINITY).total_cmp(&a.return_percent.unwrap_or(f64::NEG_INFINITY)))
            .then_with(|| a.route.cycle_legs.len().cmp(&b.route.cycle_legs.len())).then_with(|| a.route.route_id.cmp(&b.route.route_id))
    });
    for (index, route) in routes.iter_mut().enumerate() { route.route.rank = index + 1; }
}

fn absorb_snapshot(response: &mut DiscoveryResponse, partitions: &mut HashMap<usize,Vec<DiscoveryRoute>>, index: usize, snapshot: super::P2pRouteSearchResponse, banks: &[Bank], query: &DiscoveryQuery, job: &Job, source: Option<&Bank>, target: Option<&Bank>, provider_assets: &[Asset], service: &super::P2pSearchService, prices: &HashMap<String,Valuation>) {
    for status in snapshot.provider_statuses.iter().chain(snapshot.asset_statuses.iter().flat_map(|s| s.entry_sources.iter().chain(s.exit_sources.iter()))) {
        if !response.provider_statuses.iter().any(|s| s.source==status.source && s.ok==status.ok) { response.provider_statuses.push(status.clone()); }
    }
    partitions.insert(index,snapshot.routes.into_iter().filter_map(|route| evaluate(route,snapshot.searched_at,snapshot.stale,banks,query,job,source,target,provider_assets,service,prices)).collect());
    let mut unique=HashMap::new();
    for route in partitions.values().flatten() { unique.insert(route.route.route_id.clone(),route.clone()); }
    response.routes=unique.into_values().collect();
    sort_results(&mut response.routes,query.mode);
    response.routes_found=response.routes.len();
}

async fn redis_read<T: serde::de::DeserializeOwned>(state: &AppState, key: &str) -> Result<Option<T>> {
    let redis = state.redis.as_ref().context("continuation requires Redis; start a new search")?;
    tokio::time::timeout(Duration::from_secs(1), redis::get_json(redis, key)).await.context("continuation store timed out")?
}

pub async fn run(state: &AppState, mut query: DiscoveryQuery, updates: Option<mpsc::Sender<DiscoveryResponse>>) -> Result<DiscoveryResponse> {
    query.normalize()?;
    let banks = state.banks.list(&BankFilters::default(), u32::MAX, 0).await?.items;
    let source_method = selected_method(&banks, &query.source_payment_method, true)?;
    let target_method = selected_method(&banks, &query.target_payment_method, false)?;
    let catalog = tokio::time::timeout(Duration::from_secs(12), state.p2p.discovery_catalog(&banks)).await.context("provider catalog timed out; retry the search")?;
    let source_endpoints = endpoints_for(&catalog, &query.source_fiat, &query.source_network, source_method);
    let target_endpoints = endpoints_for(&catalog, &query.target_fiat, &query.target_network, target_method);
    if source_endpoints.is_empty() || target_endpoints.is_empty() { bail!("no supported currencies or networks match the selected methods"); }
    let mut identity = query.clone(); identity.cursor = None;
    let query_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&identity)?));
    let catalog_hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&(&catalog, &banks))?));
    let mut cursor = if let Some(token) = &query.cursor {
        Uuid::parse_str(token).context("invalid continuation cursor")?;
        let cursor: CursorState = redis_read(state, &format!("pay3flow:discovery:cursor:{token}")).await?.context("search cursor expired; start a new search")?;
        if cursor.query_hash != query_hash || cursor.catalog_hash != catalog_hash || cursor.expires_at < Utc::now().timestamp() { bail!("search or catalog changed; start a new search"); }
        cursor
    } else { CursorState { query_hash, catalog_hash, completed: BTreeSet::new(), expires_at: Utc::now().timestamp() + CURSOR_TTL as i64 } };
    let total = job_count(&source_endpoints, &target_endpoints, query.mode);
    let indices = (0..total).filter(|i| !cursor.completed.contains(i)).take(BATCH_SIZE).collect::<Vec<_>>();
    let mut response = DiscoveryResponse { search_id: Uuid::new_v4(), mode: query.mode, source_amount: query.source_amount.clone(), amount_basis: "same_numeric_amount_per_starting_currency", reporting_currency: "USD", searched_at: Utc::now(), routes: Vec::new(), routes_found: 0,
        routes_exhaustive: false, complete: false, coverage: Coverage { combinations_total: total, combinations_completed: cursor.completed.len(), ..Coverage::default() }, provider_statuses: Vec::new(), errors: Vec::new(), next_cursor: None };
    if let Some(tx) = &updates { let _ = tx.send(response.clone()).await; }
    let provider_assets = catalog.iter().filter(|e| e.network.is_some()).filter_map(|e| Asset::new(&e.currency, e.network.as_deref()).ok()).collect::<Vec<_>>();
    // Valuation is indicative only, and never uses the payments service's mock FX table.
    let currencies = indices.iter().flat_map(|&i| { let j=job_at(i,&source_endpoints,&target_endpoints,query.mode); [j.source.currency,j.target.currency] }).collect::<BTreeSet<_>>();
    let mut prices = HashMap::new();
    for (asset, price, timestamp) in state.market_prices.discovery_prices().await { prices.insert(asset, Valuation { price_usd: price, observed_at: timestamp, source: "DefiLlama".into() }); }
    let fiats = currencies.into_iter().filter(|c| is_fiat_currency(c)).collect::<Vec<_>>();
    let fiat_jobs = fiats.into_iter().map(|currency| {
        let service = state.p2p.clone();
        async move { let price = service.discovery_fiat_usd(&currency).await; (currency, price) }
    }).collect::<Vec<_>>();
    let fiat_values = stream::iter(fiat_jobs).buffer_unordered(4).collect::<Vec<_>>().await;
    for (currency, price) in fiat_values { if let Some(price) = price { prices.insert(currency, price); } }
    let concrete_jobs = indices.iter().map(|&index| {
        let job=job_at(index,&source_endpoints,&target_endpoints,query.mode);
        let concrete=concrete_query(&query,&job,source_method,target_method);
        (index, job, concrete)
    }).collect::<Vec<_>>();
    let (snapshot_tx, mut snapshots) = mpsc::channel(16);
    let jobs = concrete_jobs.into_iter().map(|(index,job,concrete)| {
        let service = state.p2p.clone();
        let snapshot_tx = snapshot_tx.clone();
        async move {
            let (tx,mut rx) = mpsc::channel(4);
            let search = service.stream_routes(concrete,Uuid::new_v4(),tx);
            tokio::pin!(search);
            let mut receiving = true;
            loop { tokio::select! {
                result = &mut search => return (index,job,result),
                snapshot = rx.recv(), if receiving => match snapshot {
                    Some(snapshot) => { if snapshot_tx.send((index,job.clone(),snapshot)).await.is_err() { return (index,job,Err(anyhow::anyhow!("search cancelled"))); } },
                    None => receiving = false,
                }
            } }
        }
    }).collect::<Vec<_>>();
    drop(snapshot_tx);
    let mut jobs=stream::iter(jobs).buffer_unordered(4);
    let deadline=tokio::time::sleep(Duration::from_secs(BATCH_SECONDS)); tokio::pin!(deadline);
    let mut partitions=HashMap::new();
    let mut partial=HashSet::new();
    let mut snapshots_open=true;
    loop {
        tokio::select! {
            _ = &mut deadline => { response.coverage.combinations_timed_out=indices.len().saturating_sub(response.coverage.batch_completed); break; },
            _ = async { if let Some(tx)=&updates { tx.closed().await } else { std::future::pending::<()>().await } } => bail!("search cancelled"),
            snapshot=snapshots.recv(), if snapshots_open => {
                if let Some((index,job,snapshot))=snapshot {
                    // A sampled combination advances the cursor even if its bounded live search reaches the batch deadline.
                    if cursor.completed.insert(index) { partial.insert(index); }
                    response.coverage.combinations_completed=cursor.completed.len();
                    response.coverage.combinations_partial=partial.len();
                    absorb_snapshot(&mut response,&mut partitions,index,snapshot,&banks,&query,&job,source_method,target_method,&provider_assets,&state.p2p,&prices);
                    if let Some(tx)=&updates { let mut visible=response.clone(); visible.routes.truncate(query.limit.unwrap_or(40)); if tx.send(visible).await.is_err() { bail!("search cancelled"); } }
                } else { snapshots_open=false; }
            },
            item=jobs.next() => {
                let Some((index,job,result))=item else { break; };
                cursor.completed.insert(index); response.coverage.batch_completed+=1; response.coverage.combinations_completed=cursor.completed.len();
                match result {
                    Ok(snapshot) => {
                        partial.remove(&index);
                        absorb_snapshot(&mut response,&mut partitions,index,snapshot,&banks,&query,&job,source_method,target_method,&provider_assets,&state.p2p,&prices);
                    },
                    Err(error) => { response.coverage.combinations_failed+=1; response.errors.push(format!("{} → {}: {error}",job.source.currency,job.target.currency)); }
                }
                response.coverage.combinations_partial=partial.len();
                sort_results(&mut response.routes,query.mode); response.routes_found=response.routes.len();
                if let Some(tx)=&updates { let mut visible=response.clone(); visible.routes.truncate(query.limit.unwrap_or(40)); if tx.send(visible).await.is_err() { bail!("search cancelled"); } }
            }
        }
    }
    response.complete=cursor.completed.len()==total;
    // Individual composers also use bounded beams/top-K, even when every endpoint was visited.
    response.routes_exhaustive=false;
    if !response.complete {
        if let Some(redis)=&state.redis {
            let token=Uuid::new_v4().to_string();
            let ttl=(cursor.expires_at-Utc::now().timestamp()).max(1) as u64;
            match tokio::time::timeout(Duration::from_secs(1), redis::set_json(redis,&format!("pay3flow:discovery:cursor:{token}"),&cursor,ttl)).await {
                Ok(Ok(())) => response.next_cursor=Some(token),
                _ => response.errors.push("Continuation is temporarily unavailable. Start a new search to retry.".into()),
            }
        } else { response.errors.push("Continuation requires Redis. Start a new search to retry.".into()); }
    }
    sort_results(&mut response.routes,query.mode); response.routes.truncate(query.limit.unwrap_or(40));
    tracing::info!(search_id=%response.search_id, mode=?query.mode, completed=response.coverage.batch_completed, total, routes=response.routes_found, "p2p.discovery.completed");
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn exact_decimal_profit_and_fees() {
        let a=decimal_units("1000.000000000000000001").unwrap();
        let b=decimal_units("1000.000000000000000003").unwrap();
        assert_eq!(decimal_string(b-a),"0.000000000000000002");
        assert_eq!(decimal_string(payment_fee(decimal_units("100").unwrap(),0.75).unwrap()),"0.75");
        assert_eq!(decimal_string(payment_fee(decimal_units("100.01").unwrap(),0.75).unwrap()),"0.76");
        assert!(decimal_units("1000000000000000000000").is_none());
        assert!(decimal_units("0.0000000000000000001").is_none());
        assert!(decimal_units("NaN").is_none());
    }
    fn query() -> DiscoveryQuery { serde_json::from_value(serde_json::json!({"mode":"arbitrage","source_amount":"1000"})).unwrap() }
    #[test] fn arbitrage_requires_closure() {
        let mut q=query(); q.source_fiat="RUB".into(); q.target_fiat="USD".into(); assert!(q.normalize().is_err());
        let mut q=query(); q.source_fiat="rub".into(); q.source_payment_method=Some("ru-sbp".into()); q.normalize().unwrap(); assert_eq!(q.target_fiat,"RUB"); assert_eq!(q.source_payment_method,q.target_payment_method);
        let mut q=query(); q.source_amount="NaN".into(); assert!(q.normalize().is_err());
    }
    #[test] fn traversal_covers_every_pair_without_cartesian_allocation() {
        let endpoints=vec![Endpoint{currency:"RUB".into(),network:None},Endpoint{currency:"USD".into(),network:None},Endpoint{currency:"USDT".into(),network:Some("tron".into())}];
        let pairs=(0..9).map(|i|{let j=job_at(i,&endpoints,&endpoints,DiscoveryMode::Exchange);(j.source,j.target)}).collect::<BTreeSet<_>>(); assert_eq!(pairs.len(),9);
        for i in 0..3 {let j=job_at(i,&endpoints,&endpoints,DiscoveryMode::Arbitrage);assert_eq!(j.source,j.target);}
    }
    fn fixture() -> (super::super::P2pSearchService, DiscoveryQuery, Job, Vec<Bank>, P2pRoute) {
        let service=super::super::P2pSearchService::with_sources(Vec::new(),Duration::from_secs(1));
        let banks=vec![Bank {id:Uuid::new_v4(),name:"SBP".into(),method_id:Some("ru-sbp".into()),display_name:"SBP / СБП".into(),role:"both".into(),country:"RU".into(),currency:"RUB".into(),domain:String::new(),icon_url:String::new(),schemes:Vec::new(),kind:"bank".into(),color:"#272727".into(),initials:"SBP".into(),popular:true,bank_fee_percent:Some(0.0),p2p_query:"SBP".into(),currency_group:None,picker_visible:true,status:"enabled".into(),updated_at:Utc::now()}];
        let mut q=query();q.source_fiat="RUB".into();q.normalize().unwrap();
        let endpoint=Endpoint{currency:"RUB".into(),network:None};let job=Job{source:endpoint.clone(),target:endpoint};
        let request=concrete_query(&q,&job,None,None);
        let normalized=normalize_query(request,&service.default_assets,&service.networks,&[]).unwrap();
        let offer=|side| super::super::P2pOffer {market:super::super::P2pOfferMarket::P2p,source:"venue".into(),ad_id:"ad".into(),side,fiat:"RUB".into(),asset:"USDT".into(),network:Some("tron".into()),price:"100".into(),available_asset:"100000".into(),min_fiat:"1".into(),max_fiat:"1000000000".into(),payment_methods:vec!["SBP".into()],pay_time_limit_minutes:None,advertiser:super::super::Advertiser{id:Some("merchant".into()),nickname:"Example".into(),is_merchant:true,is_verified:true,user_type:None,completed_orders_30d:Some(100),completion_rate_30d:Some(1.0),positive_rate:Some(1.0)},advertiser_profile_url:None,source_url:"https://example.com/offer".into(),source_url_is_exact:true};
        let mut route=super::super::routes::compose_fiat_route(&normalized,"USDT",&offer(super::super::P2pSide::BuyCrypto),&offer(super::super::P2pSide::SellCrypto),10.0,1100.0).unwrap();route.route_id="fixture".into();
        (service,q,job,banks,route)
    }
    #[test] fn verifies_same_method_and_venue_and_preserves_unknown_ids() {
        let (_,q,_,banks,mut route)=fixture();
        let (a,b,_,_)=settlements(&route,&banks,None,None,q.mode).unwrap();assert_eq!(a.method_id,b.method_id);assert_eq!(a.venue,b.venue);
        route.exit_offer.as_mut().unwrap().source="other-venue".into();assert!(settlements(&route,&banks,None,None,q.mode).is_none());
        route.exit_offer.as_mut().unwrap().source="venue".into();route.exit_offer.as_mut().unwrap().payment_methods=vec!["123".into()];assert!(settlements(&route,&banks,None,None,q.mode).is_none());
        assert!(settlements(&route,&banks,None,None,DiscoveryMode::Exchange).is_some());
    }
    #[test] fn aggregator_identity_does_not_close_different_exchangers() {
        let (_,q,_,banks,mut route)=fixture();
        for offer in [&mut route.entry_offer,&mut route.exit_offer] {offer.as_mut().unwrap().market=super::super::P2pOfferMarket::DirectExchange;}
        route.exit_offer.as_mut().unwrap().advertiser.id=Some("other-exchanger".into());
        assert!(settlements(&route,&banks,None,None,q.mode).is_none());
    }
    #[test] fn profit_includes_both_endpoint_fees_and_usd_valuation() {
        let (service,q,job,mut banks,route)=fixture();
        let prices=HashMap::from([("RUB".into(),Valuation{price_usd:0.01,observed_at:Utc::now(),source:"fixture".into()})]);
        banks[0].bank_fee_percent=Some(0.75);
        let result=evaluate(route,Utc::now(),false,&banks,&q,&job,None,None,&[],&service,&prices).unwrap();
        assert!(!result.profit_confirmed);assert!(result.net_profit.is_none());assert!((result.estimated_profit_before_missing_costs_usd.unwrap()-0.8425).abs()<1e-12);assert!(result.missing_costs.iter().any(|c|c=="venue_trading_fee"));
    }
    #[test] fn missing_fees_and_stale_quotes_never_produce_confirmed_profit() {
        let (service,q,job,mut banks,route)=fixture();banks[0].bank_fee_percent=None;
        let result=evaluate(route.clone(),Utc::now(),false,&banks,&q,&job,None,None,&[],&service,&HashMap::new()).unwrap();
        assert!(!result.profit_confirmed);assert!(result.net_profit_usd.is_none());assert!(result.missing_costs.iter().any(|c|c=="source_payment_fee"));
        banks[0].bank_fee_percent=Some(0.0);
        let result=evaluate(route,Utc::now(),true,&banks,&q,&job,None,None,&[],&service,&HashMap::new()).unwrap();assert!(!result.profit_confirmed);
    }
    #[test] fn expired_provider_quotes_are_excluded() {
        let (service,q,job,banks,mut route)=fixture();route.quote_expires_at=Some(Utc::now()-chrono::Duration::seconds(1));
        assert!(evaluate(route,Utc::now(),false,&banks,&q,&job,None,None,&[],&service,&HashMap::new()).is_none());
    }

}
