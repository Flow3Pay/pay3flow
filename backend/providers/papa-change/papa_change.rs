use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::p2p::{
    Advertiser, P2pOffer, P2pOfferMarket, P2pSearchQuery, P2pSide, P2pSource,
};
use crate::provider_adapter::PapaChangeAdapterConfig;
use crate::providers::ProviderAdapterRecord;

#[derive(Debug, Deserialize)]
struct CatalogResponse {
    #[serde(default)]
    exchanges: HashMap<String, ApiExchange>,
    #[serde(rename = "availableDirections", default)]
    available_directions: Vec<ApiDirection>,
}

#[derive(Debug, Deserialize)]
struct RatesResponse {
    #[serde(default)]
    rates: HashMap<String, ApiRate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiExchange {
    id: u64,
    name: String,
    #[serde(default)]
    name_en: String,
    currency: String,
    #[serde(rename = "type")]
    kind: String,
    best_change_currency_name: Option<String>,
    balance: Option<f64>,
    exchange_comission_percent: Option<f64>,
    exchange_comission_percent_income: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiDirection {
    id: u64,
    #[serde(default)]
    is_enabled: bool,
    base_exchange_id: u64,
    target_exchange_id: u64,
    commission_percent: f64,
    base_exchange_min_incoming: Option<f64>,
    base_exchange_max_incoming: Option<f64>,
    target_exchange_min_outgoing: Option<f64>,
    target_exchange_max_outgoing: Option<f64>,
    #[serde(default)]
    base_volume_rate_ranges: Vec<VolumeRateRange>,
    #[serde(default)]
    target_volume_rate_ranges: Vec<VolumeRateRange>,
    #[serde(rename = "DiscountRule", default)]
    discount_rules: Vec<DiscountRule>,
    discount_rule_to_export_xmlid: Option<u64>,
    first_group_rate: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VolumeRateRange {
    min: Option<f64>,
    max: Option<f64>,
    rate: f64,
    #[serde(default)]
    is_main: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscountRule {
    id: u64,
    rule_type: String,
    min_amount_usdt: Option<f64>,
    max_amount_usdt: Option<f64>,
    discount_percent: Option<f64>,
    percent: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiRate {
    prefered_exchange_rate: Option<f64>,
}

#[derive(Debug)]
struct Snapshot {
    exchanges: HashMap<String, ApiExchange>,
    directions: Vec<ApiDirection>,
    rates: HashMap<String, ApiRate>,
}

impl Snapshot {
    fn exchange(&self, id: u64) -> Option<&ApiExchange> {
        self.exchanges.get(&id.to_string())
    }

    fn rate(&self, id: u64) -> Option<f64> {
        positive(
            self.rates
                .get(&id.to_string())?
                .prefered_exchange_rate?,
        )
    }
}

#[derive(Debug)]
struct CachedSnapshot {
    loaded_at: Instant,
    value: Arc<Snapshot>,
}

pub(crate) struct PapaChangeSource {
    client: Client,
    slug: String,
    config: PapaChangeAdapterConfig,
    cache: Mutex<Option<CachedSnapshot>>,
}

impl PapaChangeSource {
    pub(crate) fn from_record(client: Client, record: &ProviderAdapterRecord) -> Option<Self> {
        let config = record.config.as_ref()?.papa_change.clone()?;
        Some(Self::new(client, record.slug.clone(), config))
    }

    fn new(client: Client, slug: String, config: PapaChangeAdapterConfig) -> Self {
        Self {
            client,
            slug,
            config,
            cache: Mutex::new(None),
        }
    }

    async fn api_json<T: DeserializeOwned>(&self, url: &str, resource: &str) -> Result<T> {
        self.client
            .get(url)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .with_context(|| format!("request Papa Change {resource}"))?
            .error_for_status()
            .with_context(|| format!("request Papa Change {resource}"))?
            .json()
            .await
            .with_context(|| format!("decode Papa Change {resource}"))
    }

    async fn snapshot(&self) -> Result<Arc<Snapshot>> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref() {
            if cached.loaded_at.elapsed() < Duration::from_millis(self.config.cache_ttl_ms) {
                return Ok(cached.value.clone());
            }
        }

        let (catalog, rates) = tokio::try_join!(
            self.api_json::<CatalogResponse>(&self.config.directions_endpoint, "directions"),
            self.api_json::<RatesResponse>(&self.config.rates_endpoint, "rates"),
        )?;
        if catalog.exchanges.is_empty() || catalog.available_directions.is_empty() {
            bail!("Papa Change returned an empty direction catalog");
        }
        if rates.rates.is_empty() {
            bail!("Papa Change returned an empty rate table");
        }

        let value = Arc::new(Snapshot {
            exchanges: catalog.exchanges,
            directions: catalog.available_directions,
            rates: rates.rates,
        });
        *cache = Some(CachedSnapshot {
            loaded_at: Instant::now(),
            value: value.clone(),
        });
        Ok(value)
    }

    fn offers(&self, query: &P2pSearchQuery, snapshot: &Snapshot) -> Vec<P2pOffer> {
        snapshot
            .directions
            .iter()
            .filter_map(|direction| self.offer(query, snapshot, direction))
            .take(query.fetch_limit().min(self.config.max_results))
            .collect()
    }

    fn offer(
        &self,
        query: &P2pSearchQuery,
        snapshot: &Snapshot,
        direction: &ApiDirection,
    ) -> Option<P2pOffer> {
        if !direction.is_enabled {
            return None;
        }
        let base = snapshot.exchange(direction.base_exchange_id)?;
        let target = snapshot.exchange(direction.target_exchange_id)?;
        let (fiat, crypto) = match query.side {
            P2pSide::BuyCrypto if is_fiat(base) && is_crypto(target) => (base, target),
            P2pSide::SellCrypto if is_crypto(base) && is_fiat(target) => (target, base),
            _ => return None,
        };
        if !fiat.currency.eq_ignore_ascii_case(&query.fiat)
            || crypto_asset(crypto)? != query.asset
        {
            return None;
        }

        let base_rate = snapshot.rate(base.id)?;
        let target_rate = snapshot.rate(target.id)?;
        let price = effective_price(
            query.side,
            query.amount,
            direction,
            base,
            target,
            base_rate,
            target_rate,
        )?;
        let (min_fiat, max_fiat, available_asset) =
            limits(query.side, direction, target, price)?;
        let source_url = direction_url(&self.config.public_endpoint, base, target, query.amount);
        let mut payment_methods = vec![fiat.name.trim().to_string()];
        let english_name = fiat.name_en.trim();
        if !english_name.is_empty()
            && !payment_methods
                .iter()
                .any(|name| name.eq_ignore_ascii_case(english_name))
        {
            payment_methods.push(english_name.to_string());
        }

        Some(P2pOffer {
            market: P2pOfferMarket::DirectExchange,
            source: self.slug.clone(),
            ad_id: direction.id.to_string(),
            side: query.side,
            fiat: query.fiat.clone(),
            asset: query.asset.clone(),
            network: crypto_network(crypto).map(str::to_string),
            price: number(price),
            available_asset: number(available_asset),
            min_fiat: number(min_fiat),
            max_fiat: number(max_fiat),
            payment_methods,
            pay_time_limit_minutes: None,
            advertiser: Advertiser {
                id: None,
                nickname: "Papa Change".into(),
                user_type: Some("service".into()),
                is_merchant: true,
                is_verified: true,
                completed_orders_30d: None,
                completion_rate_30d: None,
                positive_rate: None,
            },
            advertiser_profile_url: None,
            source_url,
            source_url_is_exact: false,
        })
    }
}

#[async_trait]
impl P2pSource for PapaChangeSource {
    fn name(&self) -> &str {
        &self.slug
    }

    fn timeout(&self, default: Duration) -> Duration {
        Duration::from_millis(self.config.timeout_ms).min(default)
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let snapshot = self.snapshot().await?;
        Ok(self.offers(query, &snapshot))
    }
}

#[allow(clippy::too_many_arguments)]
fn effective_price(
    side: P2pSide,
    fiat_amount: Option<f64>,
    direction: &ApiDirection,
    base: &ApiExchange,
    target: &ApiExchange,
    default_base_rate: f64,
    default_target_rate: f64,
) -> Option<f64> {
    let source_amount = match side {
        P2pSide::BuyCrypto => fiat_amount
            .and_then(positive)
            .or_else(|| direction.base_exchange_min_incoming.and_then(positive))
            .unwrap_or(1.0),
        P2pSide::SellCrypto => {
            let baseline = default_base_rate
                / default_target_rate
                / (1.0 + direction.commission_percent / 100.0);
            fiat_amount
                .and_then(positive)
                .map(|amount| amount / baseline)
                .or_else(|| direction.base_exchange_min_incoming.and_then(positive))
                .unwrap_or(1.0)
        }
    };
    let base_rate = volume_rate(
        &direction.base_volume_rate_ranges,
        source_amount,
        default_base_rate,
    )?;
    let preliminary_target = source_amount * base_rate / default_target_rate;
    let target_rate = volume_rate(
        &direction.target_volume_rate_ranges,
        preliminary_target,
        default_target_rate,
    )?;
    let commission = commission_percent(direction, base, target, source_amount * base_rate)?;
    let mut target_amount = source_amount * base_rate / target_rate / (1.0 + commission / 100.0);
    let has_export_discount = direction.discount_rule_to_export_xmlid.is_some_and(|id| {
        direction.discount_rules.iter().any(|rule| rule.id == id)
    });
    if !has_export_discount {
        if let Some(first_group_rate) = direction.first_group_rate.and_then(positive) {
            target_amount = target_amount.min(source_amount * first_group_rate);
        }
    }
    let raw_price = match side {
        P2pSide::BuyCrypto => source_amount / target_amount,
        P2pSide::SellCrypto => target_amount / source_amount,
    };
    positive(raw_price)
}

fn volume_rate(ranges: &[VolumeRateRange], amount: f64, fallback: f64) -> Option<f64> {
    let matching = ranges
        .iter()
        .filter(|range| {
            let minimum = range.min.unwrap_or(0.0);
            let maximum = range.max.unwrap_or(f64::INFINITY);
            amount > 0.0 && amount >= minimum && amount <= maximum
        })
        .max_by(|left, right| {
            left.min
                .unwrap_or(0.0)
                .total_cmp(&right.min.unwrap_or(0.0))
        })
        .or_else(|| ranges.iter().find(|range| range.is_main));
    positive(matching.map_or(fallback, |range| range.rate))
}

fn commission_percent(
    direction: &ApiDirection,
    base: &ApiExchange,
    target: &ApiExchange,
    amount_usdt: f64,
) -> Option<f64> {
    let fallback = direction.commission_percent;
    let matching = direction
        .discount_rules
        .iter()
        .filter(|rule| {
            let minimum = rule.min_amount_usdt.unwrap_or(0.0);
            let maximum = rule.max_amount_usdt.unwrap_or(f64::INFINITY);
            amount_usdt >= minimum && amount_usdt <= maximum
        })
        .max_by(|left, right| {
            left.min_amount_usdt
                .unwrap_or(0.0)
                .total_cmp(&right.min_amount_usdt.unwrap_or(0.0))
        });
    let value = match matching {
        Some(rule) if rule.rule_type == "DISCOUNT" => {
            fallback * (1.0 - rule.discount_percent.unwrap_or(0.0) / 100.0)
        }
        Some(rule) if rule.rule_type == "PERCENT" => {
            rule.percent.unwrap_or(fallback)
                + target.exchange_comission_percent.unwrap_or(0.0)
                + base.exchange_comission_percent_income.unwrap_or(0.0)
        }
        _ => fallback,
    };
    (value.is_finite() && value > -100.0).then_some(value)
}

fn limits(
    side: P2pSide,
    direction: &ApiDirection,
    target: &ApiExchange,
    price: f64,
) -> Option<(f64, f64, f64)> {
    let base_min = direction.base_exchange_min_incoming.and_then(positive);
    let base_max = direction.base_exchange_max_incoming.and_then(positive);
    let target_min = direction.target_exchange_min_outgoing.and_then(positive);
    let target_max = direction.target_exchange_max_outgoing.and_then(positive);
    let target_balance = target.balance.and_then(positive);

    let (minimum, maximum, available_asset) = match side {
        P2pSide::BuyCrypto => {
            let minimum = max_present([base_min, target_min.map(|value| value * price)])?;
            let maximum = min_present([
                base_max,
                target_max.map(|value| value * price),
                target_balance.map(|value| value * price),
            ])?;
            let available_asset = min_present([target_max, target_balance, Some(maximum / price)])?;
            (minimum, maximum, available_asset)
        }
        P2pSide::SellCrypto => {
            let minimum = max_present([target_min, base_min.map(|value| value * price)])?;
            let maximum = min_present([target_max, base_max.map(|value| value * price), target_balance])?;
            (minimum, maximum, maximum / price)
        }
    };
    (minimum <= maximum).then_some((minimum, maximum, available_asset))
}

fn max_present(values: [Option<f64>; 2]) -> Option<f64> {
    values.into_iter().flatten().reduce(f64::max)
}

fn min_present(values: [Option<f64>; 3]) -> Option<f64> {
    values.into_iter().flatten().reduce(f64::min)
}

fn is_crypto(exchange: &ApiExchange) -> bool {
    exchange.kind.eq_ignore_ascii_case("crypto")
}

fn is_fiat(exchange: &ApiExchange) -> bool {
    !is_crypto(exchange)
}

fn crypto_asset(exchange: &ApiExchange) -> Option<&'static str> {
    let code = exchange
        .best_change_currency_name
        .as_deref()
        .unwrap_or(&exchange.currency)
        .to_ascii_uppercase();
    [
        "USDT", "USDC", "BTC", "BCH", "ETH", "BNB", "SOL", "TRX", "TON", "DOGE",
        "LTC", "AVAX", "XMR", "DASH", "SHIB", "ARB", "OP", "POL",
    ]
    .into_iter()
    .find(|asset| code.starts_with(asset))
    .map(|asset| if asset == "POL" { "MATIC" } else { asset })
}

fn crypto_network(exchange: &ApiExchange) -> Option<&'static str> {
    let code = exchange
        .best_change_currency_name
        .as_deref()
        .unwrap_or(&exchange.currency)
        .to_ascii_uppercase();
    if code.contains("TRC20") || code == "TRX" {
        Some("tron")
    } else if code.contains("ERC20") || code == "ETH" || code == "SHIB" {
        Some("ethereum")
    } else if code.contains("BEP20") || code.contains("BEP2") || code == "BNB" {
        Some("bnb-smart-chain")
    } else if code.ends_with("SOL") || code == "SOL" {
        Some("solana")
    } else if code.ends_with("TON") || code == "TON" {
        Some("ton")
    } else if code.contains("ARBTM") || code == "ARB" {
        Some("arbitrum-one")
    } else if code.contains("OPTM") || code == "OP" {
        Some("optimism")
    } else if code.contains("POLYGON") || code == "POL" {
        Some("polygon-pos")
    } else if code == "AVAX" {
        Some("avalanche-c")
    } else if code == "BTC" {
        Some("bitcoin")
    } else if code == "BCH" {
        Some("bitcoin-cash")
    } else if code == "DOGE" {
        Some("dogecoin")
    } else if code == "LTC" {
        Some("litecoin")
    } else {
        None
    }
}

fn direction_url(
    public_endpoint: &str,
    base: &ApiExchange,
    target: &ApiExchange,
    amount: Option<f64>,
) -> String {
    let from = base
        .best_change_currency_name
        .as_deref()
        .unwrap_or(&base.currency);
    let to = target
        .best_change_currency_name
        .as_deref()
        .unwrap_or(&target.currency);
    let mut url = format!(
        "{}?from={from}&to={to}",
        public_endpoint.trim_end_matches('/')
    );
    if let Some(amount) = amount.and_then(positive) {
        url.push_str("&amt_from=");
        url.push_str(&number(amount));
    }
    url
}

fn positive(value: f64) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value)
}

fn number(value: f64) -> String {
    let rendered = format!("{value:.12}");
    rendered
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG: &str = r#"
    {
      "exchanges": {
        "24": {"id":24,"name":"СберБанк","nameEn":"Sberbank","currency":"RUB","type":"bank","bestChangeCurrencyName":"SBERRUB","balance":75000},
        "223": {"id":223,"name":"Tether TRC20","nameEn":"Tether TRC20","currency":"USDT","type":"crypto","bestChangeCurrencyName":"USDTTRC20","balance":500}
      },
      "availableDirections": [
        {"id":1,"isEnabled":true,"baseExchangeId":24,"targetExchangeId":223,"commissionPercent":5,"baseExchangeMinIncoming":5000,"baseExchangeMaxIncoming":100000,"targetExchangeMinOutgoing":10,"targetExchangeMaxOutgoing":1000,"DiscountRule":[]},
        {"id":2,"isEnabled":true,"baseExchangeId":223,"targetExchangeId":24,"commissionPercent":5,"baseExchangeMinIncoming":10,"baseExchangeMaxIncoming":1000,"targetExchangeMinOutgoing":5000,"targetExchangeMaxOutgoing":50000,"DiscountRule":[]}
      ]
    }
    "#;

    const RATES: &str = r#"
    {"rates":{"24":{"preferedExchangeRate":0.01},"223":{"preferedExchangeRate":1}}}
    "#;

    fn source() -> PapaChangeSource {
        PapaChangeSource::new(
            Client::new(),
            "papa-change".into(),
            PapaChangeAdapterConfig {
                directions_endpoint: "https://api.papa-change.biz/directions".into(),
                rates_endpoint: "https://api.papa-change.biz/rates".into(),
                public_endpoint: "https://papa-change.biz/".into(),
                timeout_ms: 10_000,
                cache_ttl_ms: 30_000,
                max_results: 100,
            },
        )
    }

    fn snapshot() -> Snapshot {
        let catalog: CatalogResponse = serde_json::from_str(CATALOG).unwrap();
        let rates: RatesResponse = serde_json::from_str(RATES).unwrap();
        Snapshot {
            exchanges: catalog.exchanges,
            directions: catalog.available_directions,
            rates: rates.rates,
        }
    }

    fn query(side: P2pSide) -> P2pSearchQuery {
        P2pSearchQuery {
            fiat: "RUB".into(),
            asset: "USDT".into(),
            side,
            amount: Some(10_000.0),
            payment_method: Some("Sberbank".into()),
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: None,
        }
    }

    #[test]
    fn maps_public_payload_to_buy_and_sell_offers() {
        let source = source();
        let snapshot = snapshot();
        let buy = source.offers(&query(P2pSide::BuyCrypto), &snapshot);
        let sell = source.offers(&query(P2pSide::SellCrypto), &snapshot);

        assert_eq!(buy.len(), 1);
        assert_eq!(buy[0].price, "105");
        assert_eq!(buy[0].network.as_deref(), Some("tron"));
        assert_eq!(buy[0].min_fiat, "5000");
        assert_eq!(buy[0].max_fiat, "52500");
        assert_eq!(buy[0].available_asset, "500");
        assert!(buy[0].payment_methods.iter().any(|name| name == "Sberbank"));

        assert_eq!(sell.len(), 1);
        assert_eq!(sell[0].price, "95.238095238095");
        assert_eq!(sell[0].min_fiat, "5000");
        assert_eq!(sell[0].max_fiat, "50000");
    }

    #[test]
    fn amount_tiers_override_the_default_rate() {
        let ranges = [
            VolumeRateRange {
                min: Some(0.0),
                max: Some(99.0),
                rate: 1.0,
                is_main: true,
            },
            VolumeRateRange {
                min: Some(100.0),
                max: None,
                rate: 1.1,
                is_main: false,
            },
        ];

        assert_eq!(volume_rate(&ranges, 50.0, 0.9), Some(1.0));
        assert_eq!(volume_rate(&ranges, 100.0, 0.9), Some(1.1));
    }

    #[test]
    fn recognizes_supported_asset_networks() {
        let cases = [
            ("USDTTRC20", "USDT", Some("tron")),
            ("USDCERC20", "USDC", Some("ethereum")),
            ("USDTSOL", "USDT", Some("solana")),
            ("POL", "MATIC", Some("polygon-pos")),
        ];
        for (code, asset, network) in cases {
            let exchange = ApiExchange {
                id: 1,
                name: code.into(),
                name_en: code.into(),
                currency: code.into(),
                kind: "crypto".into(),
                best_change_currency_name: Some(code.into()),
                balance: Some(1.0),
                exchange_comission_percent: None,
                exchange_comission_percent_income: None,
            };
            assert_eq!(crypto_asset(&exchange), Some(asset), "code: {code}");
            assert_eq!(crypto_network(&exchange), network, "code: {code}");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "requires Papa Change network access"]
    async fn live_api_returns_buy_and_sell_rates() {
        let provider = PapaChangeSource::new(
            Client::builder()
                .timeout(Duration::from_secs(15))
                .user_agent("Pay3Flow-Papa-Change-Test/0.1")
                .build()
                .unwrap(),
            "papa-change".into(),
            PapaChangeAdapterConfig {
                directions_endpoint: "https://api.papa-change.biz/users-directions/get-all-available".into(),
                rates_endpoint: "https://api.papa-change.biz/exchange-rates-api/get-all".into(),
                public_endpoint: "https://papa-change.biz/".into(),
                timeout_ms: 10_000,
                cache_ttl_ms: 30_000,
                max_results: 100,
            },
        );

        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = provider.search(&query(side)).await.unwrap();
            assert!(!offers.is_empty(), "no live {side:?} offers");
            assert!(offers.iter().all(|offer| {
                offer.price.parse::<f64>().is_ok_and(|price| price > 0.0)
                    && offer.min_fiat.parse::<f64>().is_ok_and(|value| value > 0.0)
                    && offer.max_fiat.parse::<f64>().is_ok_and(|value| value > 0.0)
            }));
        }
    }
}

