use super::*;

use chrono::DateTime;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::p2p::service::P2pSource;
use crate::p2p::spot::CryptoTicker;
use crate::p2p::{
    Advertiser, FiatRouteQuote, P2pOffer, P2pOfferMarket, P2pSearchService,
    PublicFiatRouteProvider, SourceStatus,
};
use crate::route_engine::{Amount, PublicRouteProvider, PublicRouteQuote};
use async_trait::async_trait;

struct ProgressiveSource;

struct OutlierCycleSource;

struct DelayedRouteSource {
    name: &'static str,
    delay: Duration,
    price: &'static str,
}

struct FixedIntentProvider;

fn crypto_cycle_request(amount: f64) -> P2pRouteSearchQuery {
    P2pRouteSearchQuery {
        source_fiat: "USDT".into(),
        target_fiat: "USDT".into(),
        source_amount: amount,
        source_network: Some("BEP20".into()),
        target_network: Some("bnb-smart-chain".into()),
        bridge_fiat: None,
        assets: None,
        intermediary_assets: Some("USDC".into()),
        source_payment_method: None,
        target_payment_method: None,
        source_payment_fee_percent: None,
        target_payment_fee_percent: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        allow_cross_venue: Some(true),
        max_price_deviation_bps: None,
        limit: Some(40),
        sources: None,
        exchange_mode: ExchangeMode::Exchanger,
    }
}

#[test]
fn same_asset_on_same_network_is_allowed_for_cycle_search() {
    let query = normalize_query(
        crypto_cycle_request(100.0),
        &["USDT".into()],
        &crate::networks::NetworkCatalog::test_default(),
        &[Asset::new("USDT", Some("bnb-smart-chain")).unwrap()],
    )
    .expect("same-asset same-network request must search arbitrage cycles");
    assert_eq!(query.source_network.as_deref(), Some("bnb-smart-chain"));
    assert_eq!(query.source_network, query.target_network);
}

struct CycleProvider {
    name: &'static str,
    buy_rate: f64,
    sell_rate: f64,
    invalid_return: bool,
    calls: Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl PublicRouteProvider for CycleProvider {
    fn name(&self) -> &str {
        self.name
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        ["USDT", "USDC"]
            .into_iter()
            .map(|symbol| Asset::new(symbol, Some("bnb-smart-chain")).unwrap())
            .collect()
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        assert_ne!(from, to, "cycles must never send a self-swap to a provider");
        self.calls
            .lock()
            .unwrap()
            .push((from.symbol.clone(), amount.value.clone()));
        let rate = if from.symbol == "USDT" {
            self.buy_rate
        } else {
            self.sell_rate
        };
        let input_value = amount.value.parse::<f64>().unwrap();
        let output = Amount::new(fixed(input_value * rate, 8), to.clone())?;
        let input = if self.invalid_return && from.symbol == "USDC" {
            Amount::new("1", from.clone())?
        } else {
            amount
        };
        Ok(PublicRouteQuote {
            provider: self.name.into(),
            quote_id: None,
            description: None,
            source_url: Some(format!("https://{}.example.test", self.name)),
            from: from.clone(),
            to: to.clone(),
            input,
            output,
            fees: vec![Amount::new("0.01", from.clone())?],
            expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(2)),
            path: vec![from, to],
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_cycles_quote_the_actual_return_amount_and_exclude_losses() {
    for (sell_rate, invalid_return, expected) in [
        (1.02, false, 1),
        (1.0, false, 0),
        (1.01, false, 0),
        (1.02, true, 0),
    ] {
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(CycleProvider {
                name: "cycle",
                buy_rate: 0.99,
                sell_rate,
                invalid_return,
                calls: calls.clone(),
            })]);
        let response = service
            .search_routes(crypto_cycle_request(100.0))
            .await
            .unwrap();
        assert_eq!(
            response.routes_found, expected,
            "sell_rate={sell_rate}, invalid_return={invalid_return}"
        );
        assert!(
            calls
                .lock()
                .unwrap()
                .contains(&("USDC".into(), "99.00000000".into())),
            "return quote must use the first leg's actual output"
        );
        if let Some(route) = response.routes.first() {
            assert_eq!(route.target_amount, "100.98000000");
            assert_eq!(route.route_kind, "crypto_cycle");
            assert_eq!(
                route.route_path,
                [
                    "USDT@bnb-smart-chain",
                    "USDC@bnb-smart-chain",
                    "USDT@bnb-smart-chain"
                ]
            );
            assert_eq!(route.cycle_legs.len(), 2);
            assert_eq!(route.profitability_decimals, Some(8));
            assert!(matches!(
                route.profitability,
                Some(RouteProfitability::Unconfirmed {
                    gross_profit_minor: 98_000_000,
                    ..
                })
            ));
            assert!(!route.transfer_fee_included);
            assert!(route.execution.is_none());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_cycles_respect_selected_providers_and_cross_venue_setting() {
    for (cross_venue, sources, expected) in
        [(true, None, 1), (false, None, 0), (true, Some("buy"), 0)]
    {
        let providers = [("buy", 1.02, 0.97), ("sell", 0.97, 1.02)]
            .into_iter()
            .map(|(name, buy_rate, sell_rate)| {
                Arc::new(CycleProvider {
                    name,
                    buy_rate,
                    sell_rate,
                    invalid_return: false,
                    calls: Arc::new(std::sync::Mutex::new(Vec::new())),
                }) as Arc<dyn PublicRouteProvider>
            })
            .collect();
        let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
            .with_route_providers(providers);
        let mut request = crypto_cycle_request(100.0);
        request.allow_cross_venue = Some(cross_venue);
        request.sources = sources.map(str::to_owned);
        let response = service.search_routes(request).await.unwrap();
        assert_eq!(
            response.routes_found, expected,
            "cross_venue={cross_venue}, sources={sources:?}"
        );
        if let Some(route) = response.routes.first() {
            assert_eq!(route.cycle_legs[0].provider, "buy");
            assert_eq!(route.cycle_legs[1].provider, "sell");
            assert!(!route.same_venue);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crypto_cycles_keep_gains_smaller_than_a_cent() {
    let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
        .with_route_providers(vec![Arc::new(CycleProvider {
            name: "cycle",
            buy_rate: 1.0,
            sell_rate: 1.02,
            invalid_return: false,
            calls: Arc::new(std::sync::Mutex::new(Vec::new())),
        })]);
    let response = service
        .search_routes(crypto_cycle_request(0.001))
        .await
        .unwrap();
    assert_eq!(response.routes_found, 1);
    assert!(matches!(
        response.routes[0].profitability,
        Some(RouteProfitability::Unconfirmed {
            gross_profit_minor: 2_000,
            gross_profit_bps: 200,
            ..
        })
    ));
}

struct FixedFiatRouteProvider;

struct BroadCycleProvider;

#[async_trait]
impl PublicRouteProvider for BroadCycleProvider {
    fn name(&self) -> &str {
        "broad-cycle"
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        let mut assets = vec![Asset::new("USDT", Some("bnb-smart-chain")).unwrap()];
        for symbol in [
            "USDC", "BTC", "ETH", "SOL", "BNB", "TRX", "TON", "DAI", "XRP", "AVAX", "NEAR", "AAA",
            "ZZZ",
        ] {
            for network in ["bnb-smart-chain", "ethereum"] {
                assets.push(Asset::new(symbol, Some(network)).unwrap());
            }
        }
        assets
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        self.quotes(from, to, amount)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no offer"))
    }

    async fn quotes(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<Vec<PublicRouteQuote>> {
        let profitable = from.symbol == "USDT"
            || from.symbol == "ZZZ"
            || from.location.as_deref() == Some("ethereum");
        Ok(["exchanger-a", "exchanger-b"]
            .into_iter()
            .map(|id| PublicRouteQuote {
                provider: self.name().into(),
                quote_id: Some(id.into()),
                description: Some(id.into()),
                source_url: Some(format!("https://{id}.example.test")),
                from: from.clone(),
                to: to.clone(),
                input: amount.clone(),
                output: Amount::new(
                    fixed(
                        amount.value.parse::<f64>().unwrap() * if profitable { 1.01 } else { 0.98 },
                        8,
                    ),
                    to.clone(),
                )
                .unwrap(),
                fees: vec![],
                expires_at: None,
                path: vec![from.clone(), to.clone()],
            })
            .collect())
    }
}

#[tokio::test]
async fn crypto_cycles_preserve_exchangers_and_search_beyond_twelve_assets_and_network_variants() {
    let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
        .with_route_providers(vec![Arc::new(BroadCycleProvider)]);
    let mut request = crypto_cycle_request(100.0);
    request.intermediary_assets = None;
    request.limit = Some(100);
    let response = service.search_routes(request).await.unwrap();
    assert!(
        response.routes.iter().any(|r| r.asset == "ZZZ"),
        "a profitable asset beyond the old twelve-asset cap must be searched"
    );
    assert!(
        response
            .routes
            .iter()
            .any(|r| r.route_path.contains(&"USDC@ethereum".into())),
        "additional networks must be searched"
    );
    let distinct = response
        .routes
        .iter()
        .filter(|r| r.asset == "ZZZ" && r.route_path.contains(&"ZZZ@bnb-smart-chain".into()))
        .count();
    assert_eq!(
        distinct, 4,
        "two entry and two exit exchangers are four distinct combinations"
    );
    assert!(response
        .provider_statuses
        .iter()
        .any(|s| s.source == "broad-cycle" && s.ok && s.offers_found > 0));
}

struct UnavailableCycleProvider;

#[async_trait]
impl PublicRouteProvider for UnavailableCycleProvider {
    fn name(&self) -> &str {
        "unavailable-cycle"
    }
    async fn supported_assets(&self) -> Vec<Asset> {
        ["USDT", "USDC"]
            .into_iter()
            .map(|s| Asset::new(s, Some("bnb-smart-chain")).unwrap())
            .collect()
    }
    async fn quote(&self, _: Asset, _: Asset, _: Amount) -> Result<PublicRouteQuote> {
        anyhow::bail!("HTTP 429: quota exceeded")
    }
}

#[tokio::test]
async fn crypto_cycles_report_provider_failures_instead_of_silently_hiding_sources() {
    let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
        .with_route_providers(vec![Arc::new(UnavailableCycleProvider)]);
    let response = service
        .search_routes(crypto_cycle_request(100.0))
        .await
        .unwrap();
    let status = response
        .provider_statuses
        .iter()
        .find(|s| s.source == "unavailable-cycle")
        .expect("failed provider must remain visible");
    assert!(!status.ok);
    assert!(status.error.as_deref().unwrap().contains("429"));
}

#[test]
fn route_snapshot_distinguishes_live_fallback_from_stale_cache() {
    let live_provider = SourceStatus {
        source: "bybit".into(),
        ok: true,
        cached: false,
        latency_ms: 10,
        offers_found: 20,
        error: None,
    };
    let status = |entry_sources| RouteAssetStatus {
        asset: "USDT".into(),
        entry_offers: 20,
        exit_offers: 0,
        routes_built: 20,
        routes_exhaustive: true,
        can_exchange_to_target: true,
        entry_sources,
        exit_sources: Vec::new(),
        entry_discovery_source: None,
        exit_discovery_source: None,
    };

    let mut fallback = status(vec![live_provider.clone()]);
    fallback.entry_discovery_source = Some("provider_fallback".into());
    assert_eq!(
        route_discovery_source(&[fallback]),
        ("provider_fallback", false)
    );
    let mut cached = status(vec![live_provider]);
    cached.entry_discovery_source = Some("database_cache".into());
    assert_eq!(route_discovery_source(&[cached]), ("database_cache", true));
}

#[async_trait]
impl PublicFiatRouteProvider for FixedFiatRouteProvider {
    fn name(&self) -> &str {
        "id-pay"
    }

    fn supports_pair(&self, source_currency: &str, target_currency: &str) -> bool {
        matches!(
            (source_currency, target_currency),
            ("AMD", "RUB") | ("RUB", "AMD")
        )
    }

    async fn quote(
        &self,
        source_currency: &str,
        target_currency: &str,
        source_amount: f64,
    ) -> Result<FiatRouteQuote> {
        let target_amount = if source_currency == "RUB" {
            source_amount * 4.0
        } else {
            source_amount / 4.0
        };
        Ok(FiatRouteQuote {
            provider: self.name().into(),
            source_url: "https://id-pay.ru/".into(),
            source_currency: source_currency.into(),
            target_currency: target_currency.into(),
            source_amount,
            target_amount,
        })
    }
}

struct PricedRouteProvider {
    name: &'static str,
    multiplier: f64,
    fee: &'static str,
    delay: Duration,
}

struct RecordingRouteProvider {
    name: &'static str,
    assets: Vec<Asset>,
    unsupported_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl PublicRouteProvider for RecordingRouteProvider {
    fn name(&self) -> &str {
        self.name
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        self.assets.clone()
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        if !self.assets.contains(&from) || !self.assets.contains(&to) {
            self.unsupported_calls.fetch_add(1, Ordering::Relaxed);
            anyhow::bail!("unsupported route dispatched to {}", self.name);
        }
        let output = amount.value.parse::<f64>().unwrap() * 0.99;
        Ok(PublicRouteQuote {
            provider: self.name.into(),
            quote_id: None,
            description: None,
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output: Amount::from_f64(output, to.clone())?,
            fees: Vec::new(),
            expires_at: None,
            path: vec![from, to],
        })
    }
}

#[async_trait]
impl PublicRouteProvider for PricedRouteProvider {
    fn name(&self) -> &str {
        self.name
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        ["USDT", "USDC"]
            .into_iter()
            .map(|symbol| Asset::new(symbol, Some("ethereum")).unwrap())
            .collect()
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        tokio::time::sleep(self.delay).await;
        let output = amount.value.parse::<f64>().unwrap() * self.multiplier;
        Ok(PublicRouteQuote {
            provider: self.name.into(),
            quote_id: None,
            description: None,
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output: Amount::from_f64(output, to.clone())?,
            fees: vec![Amount::new(self.fee, from.clone())?],
            expires_at: DateTime::from_timestamp(1_900_000_000, 0),
            path: vec![from, to],
        })
    }
}

#[async_trait]
impl PublicRouteProvider for FixedIntentProvider {
    fn name(&self) -> &str {
        "test-intents"
    }

    async fn supported_assets(&self) -> Vec<Asset> {
        [
            ("USDT", "tron"),
            ("USDT", "avalanche-c"),
            ("USDT", "ethereum"),
            ("USDT", "gnosis"),
            ("USDT", "near"),
            ("USDT", "optimism"),
            ("USDT", "scroll"),
            ("USDT", "ton"),
            ("USDC", "solana"),
            ("BTC", "bitcoin"),
            ("BTC", "near"),
        ]
        .into_iter()
        .map(|(symbol, network)| Asset::new(symbol, Some(network)).unwrap())
        .collect()
    }

    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        let value = amount.value.parse::<f64>().unwrap() * 0.98;
        Ok(PublicRouteQuote {
            provider: self.name().into(),
            quote_id: None,
            description: None,
            source_url: None,
            from: from.clone(),
            to: to.clone(),
            input: amount,
            output: Amount::from_f64(value, to.clone())?,
            fees: vec![Amount::new("0.5", from.clone())?],
            expires_at: None,
            path: vec![from, to],
        })
    }
}

#[async_trait]
impl P2pSource for ProgressiveSource {
    fn name(&self) -> &str {
        "bybit"
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let delay = if query.asset == "USDT" { 5 } else { 20 };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let mut result = offer(
            "bybit",
            query.side,
            if query.side == P2pSide::BuyCrypto {
                "400"
            } else {
                "80"
            },
            "1000",
            "200000",
        );
        result.fiat.clone_from(&query.fiat);
        result.asset.clone_from(&query.asset);
        result.ad_id = format!("{}-{:?}", query.asset, query.side);
        if query.payment_method.is_some() {
            result.payment_methods = vec!["Other Bank".into()];
        }
        Ok(vec![result])
    }
}

#[async_trait]
impl P2pSource for OutlierCycleSource {
    fn name(&self) -> &str {
        "outlier"
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        let prices: &[&str] = match query.side {
            P2pSide::BuyCrypto => &["195.49", "359", "360", "361"],
            P2pSide::SellCrypto => &["358", "359", "360"],
        };
        Ok(prices
            .iter()
            .map(|price| {
                let mut result = offer(self.name(), query.side, price, "1", "1000000");
                result.fiat.clone_from(&query.fiat);
                result.asset.clone_from(&query.asset);
                result
            })
            .collect())
    }
}

#[async_trait]
impl P2pSource for DelayedRouteSource {
    fn name(&self) -> &str {
        self.name
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        tokio::time::sleep(self.delay).await;
        let mut result = offer(self.name, query.side, self.price, "1", "1000000");
        result.fiat.clone_from(&query.fiat);
        result.asset.clone_from(&query.asset);
        result.ad_id = format!("{}-{:?}", self.name, query.side);
        Ok(vec![result])
    }
}

fn offer(source: &str, side: P2pSide, price: &str, min: &str, max: &str) -> P2pOffer {
    P2pOffer {
        market: crate::p2p::service::P2pOfferMarket::P2p,
        source: source.into(),
        ad_id: format!("{source}-{price}"),
        side,
        fiat: if side == P2pSide::BuyCrypto {
            "AMD"
        } else {
            "RUB"
        }
        .into(),
        asset: "USDT".into(),
        network: None,
        price: price.into(),
        available_asset: "10000".into(),
        min_fiat: min.into(),
        max_fiat: max.into(),
        payment_methods: Vec::new(),
        pay_time_limit_minutes: Some(15),
        advertiser: Advertiser {
            id: None,
            nickname: source.into(),
            user_type: None,
            is_merchant: true,
            is_verified: true,
            completed_orders_30d: Some(100),
            completion_rate_30d: Some(0.99),
            positive_rate: None,
        },
        advertiser_profile_url: None,
        source_url: "https://example.test".into(),
        source_url_is_exact: true,
    }
}

fn query(allow_cross_venue: bool) -> NormalizedRouteQuery {
    NormalizedRouteQuery {
        source_currency: "AMD".into(),
        target_currency: "RUB".into(),
        source_amount: 100_000.0,
        source_network: None,
        target_network: None,
        assets: vec!["USDT".into()],
        assets_explicit: false,
        source_payment_method: None,
        target_payment_method: None,
        source_payment_fee_bps: None,
        target_payment_fee_bps: None,
        merchant_only: false,
        min_orders: None,
        min_completion_rate: None,
        allow_cross_venue,
        max_price_deviation_bps: 1_000,
        limit: 20,
        sources: None,
        exchange_mode: ExchangeMode::All,
    }
}

#[test]
fn exchange_mode_selects_p2p_exchangers_or_both() {
    let p2p = offer("binance", P2pSide::BuyCrypto, "400", "1", "1000000");
    let mut exchanger = offer("whitebird", P2pSide::BuyCrypto, "401", "1", "1000000");
    exchanger.market = P2pOfferMarket::DirectExchange;
    let offers = [p2p, exchanger];
    let cases = [
        (ExchangeMode::P2p, vec!["binance"]),
        (ExchangeMode::Exchanger, vec!["whitebird"]),
        (ExchangeMode::All, vec!["binance", "whitebird"]),
    ];

    for (exchange_mode, expected_sources) in cases {
        let mut route_query = query(true);
        route_query.exchange_mode = exchange_mode;
        let actual_sources = matching_offers(&offers, &route_query)
            .into_iter()
            .map(|offer| offer.source)
            .collect::<Vec<_>>();
        assert_eq!(actual_sources, expected_sources, "mode: {exchange_mode:?}");
    }
}

#[test]
fn recognizes_assets_present_in_the_network_catalog_as_crypto() {
    let networks = crate::networks::NetworkCatalog::test_default();
    for asset in ["BTC", "ETH", "USDT", "USDC", "TRX", "TON"] {
        assert!(
            networks.is_supported_asset(asset),
            "asset should use crypto routing: {asset}"
        );
    }
    assert!(!networks.is_supported_asset("AMD"));
}

#[test]
fn network_validation_rejects_incompatible_assets() {
    let networks = crate::networks::NetworkCatalog::test_default();
    let cases = [
        ("ethereum", "ETH", true),
        ("ethereum", "BTC", false),
        ("bitcoin", "BTC", true),
        ("bitcoin", "ETH", false),
        ("tron", "USDT", true),
        ("tron", "USDC", false),
        ("unknown", "ETH", false),
    ];

    for (network, asset, expected) in cases {
        assert_eq!(
            validate_network(&networks, &Some(network.to_string()), asset, &[]).is_ok(),
            expected,
            "network={network}, asset={asset}"
        );
    }
}

#[test]
fn canonicalizes_common_network_aliases() {
    assert_eq!(canonical_network_id("avalanche"), "avalanche-c");
    assert_eq!(canonical_network_id("avax"), "avalanche-c");
    assert_eq!(canonical_network_id("sol"), "solana");
    assert_eq!(canonical_network_id("TRC20"), "tron");
    assert_eq!(canonical_network_id("ethereum"), "ethereum");
}

#[test]
fn same_asset_on_different_networks_is_allowed_for_provider_search() {
    let query = normalize_query(
        P2pRouteSearchQuery {
            source_fiat: "USDT".into(),
            target_fiat: "USDT".into(),
            source_amount: 125.0,
            source_network: Some("tron".into()),
            target_network: Some("ton".into()),
            bridge_fiat: None,
            assets: None,
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(false),
            max_price_deviation_bps: Some(1_000),
            limit: Some(20),
            sources: None,
            exchange_mode: ExchangeMode::All,
        },
        &["USDT".into()],
        &crate::networks::NetworkCatalog::test_default(),
        &[],
    )
    .expect("same-asset cross-network request should reach provider search");

    assert_eq!(query.source_network.as_deref(), Some("tron"));
    assert_eq!(query.target_network.as_deref(), Some("ton"));
}

#[test]
fn crypto_to_fiat_route_preserves_the_selected_source_network() {
    let query = normalize_query(
        P2pRouteSearchQuery {
            source_fiat: "USDT".into(),
            target_fiat: "AMD".into(),
            source_amount: 125.0,
            source_network: Some("ethereum".into()),
            target_network: None,
            bridge_fiat: None,
            assets: None,
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: Some("Ameriabank".into()),
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(false),
            max_price_deviation_bps: Some(1_000),
            limit: Some(20),
            sources: None,
            exchange_mode: ExchangeMode::All,
        },
        &["USDT".into()],
        &crate::networks::NetworkCatalog::test_default(),
        &[],
    )
    .unwrap();
    let mut routes = Vec::new();

    compose_crypto_to_fiat_routes(
        &mut routes,
        &query,
        "USDT",
        &[offer(
            "binance",
            P2pSide::SellCrypto,
            "359.12",
            "1000",
            "100000",
        )],
    );

    assert_eq!(routes.len(), 1);
    assert_eq!(
        serde_json::to_value(&routes[0]).unwrap()["entry_network"],
        "ethereum"
    );
}

#[test]
fn fixed_provider_network_rejects_incompatible_direct_and_composed_routes() {
    let mut direct_query = query(false);
    direct_query.target_currency = "USDT".into();
    direct_query.target_network = Some("tron".into());
    let mut solana_offer = offer(
        "bitcoin-center",
        P2pSide::BuyCrypto,
        "372",
        "50000",
        "10000000",
    );
    solana_offer.network = Some("solana".into());
    let mut routes = Vec::new();

    compose_fiat_to_crypto_routes(&mut routes, &direct_query, "USDT", &[solana_offer.clone()]);

    assert!(routes.is_empty());

    let mut tron_exit = offer("bncex", P2pSide::SellCrypto, "353", "10000", "50000000");
    tron_exit.network = Some("tron".into());
    compose_fiat_routes(
        &mut routes,
        &query(true),
        "USDT",
        &[solana_offer],
        &[tron_exit],
    );

    assert!(routes.is_empty());
}

#[test]
fn composes_two_feasible_legs() {
    let mut routes = Vec::new();
    compose_fiat_routes(
        &mut routes,
        &query(false),
        "USDT",
        &[offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000")],
        &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
    );
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].acquired_asset_amount, "250.00000000");
    assert_eq!(routes[0].target_amount, "20000.00");
    assert!(routes[0].same_venue);
}

#[test]
fn fiat_virtualization_builds_a_bounded_best_first_candidate_set() {
    let mut route_query = query(true);
    route_query.limit = 20;
    let entries = (0..60)
        .map(|index| {
            offer(
                &format!("entry-{index}"),
                P2pSide::BuyCrypto,
                &format!("{}", 400 + index),
                "1",
                "1000000",
            )
        })
        .collect::<Vec<_>>();
    let exits = (0..60)
        .map(|index| {
            offer(
                &format!("exit-{index}"),
                P2pSide::SellCrypto,
                &format!("{}", 100 - index),
                "1",
                "1000000",
            )
        })
        .collect::<Vec<_>>();
    let mut routes = Vec::new();

    let exhaustive = compose_fiat_routes(&mut routes, &route_query, "USDT", &entries, &exits);

    assert!(!exhaustive);
    assert_eq!(routes.len(), 60);
    assert!(routes
        .windows(2)
        .all(|pair| { route_target(&pair[0]) >= route_target(&pair[1]) }));
}

#[test]
fn fiat_virtualization_seeds_a_lower_ranked_venue() {
    let mut route_query = query(true);
    route_query.limit = 20;
    let entries = (0..60)
        .map(|index| {
            offer(
                "binance",
                P2pSide::BuyCrypto,
                &format!("{}", 400 + index),
                "1",
                "1000000",
            )
        })
        .collect::<Vec<_>>();
    let mut exits = (0..60)
        .map(|index| {
            offer(
                "binance",
                P2pSide::SellCrypto,
                &format!("{}", 100 - index),
                "1",
                "1000000",
            )
        })
        .collect::<Vec<_>>();
    exits.push(offer("whitebird", P2pSide::SellCrypto, "1", "1", "1000000"));
    let mut routes = Vec::new();

    compose_fiat_routes(&mut routes, &route_query, "USDT", &entries, &exits);

    assert_eq!(routes.len(), 60);
    assert!(routes.iter().any(|route| {
        route.exit_offer.as_ref().map(|offer| offer.source.as_str()) == Some("whitebird")
    }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_hundred_large_virtualizations_finish_within_one_second() {
    let mut route_query = query(true);
    route_query.limit = 40;
    let query = Arc::new(route_query);
    let entries = Arc::new(
        (0..60)
            .map(|index| {
                offer(
                    &format!("entry-{index}"),
                    P2pSide::BuyCrypto,
                    &format!("{}", 400 + index),
                    "1",
                    "1000000",
                )
            })
            .collect::<Vec<_>>(),
    );
    let exits = Arc::new(
        (0..60)
            .map(|index| {
                offer(
                    &format!("exit-{index}"),
                    P2pSide::SellCrypto,
                    &format!("{}", 100 - index),
                    "1",
                    "1000000",
                )
            })
            .collect::<Vec<_>>(),
    );
    let pipelines = (0..100).map(|_| {
        let query = query.clone();
        let entries = entries.clone();
        let exits = exits.clone();
        tokio::task::spawn_blocking(move || {
            let mut routes = Vec::new();
            compose_fiat_routes(&mut routes, &query, "USDT", &entries, &exits);
            routes.len()
        })
    });

    let counts = tokio::time::timeout(Duration::from_secs(1), futures::future::join_all(pipelines))
        .await
        .expect("100 concurrent virtualization pipelines exceeded the one-second SLO");

    assert!(counts
        .into_iter()
        .all(|count| count.expect("virtualization task panicked") == 120));
}

#[test]
fn composes_usd_bank_accounts_through_crypto() {
    let mut route_query = query(true);
    route_query.source_currency = "USD".into();
    route_query.target_currency = "USD".into();
    route_query.source_amount = 12_000.0;
    route_query.source_payment_method = Some("Bank Transfer".into());
    route_query.target_payment_method = Some("Bank Transfer".into());

    let mut entry = offer("skylabs", P2pSide::BuyCrypto, "1.01", "1", "1000000000");
    entry.fiat = "USD".into();
    entry.available_asset = "1000000000".into();
    entry.payment_methods = vec!["Bank Transfer".into()];
    entry.market = P2pOfferMarket::DirectExchange;

    let mut exit = offer("skylabs", P2pSide::SellCrypto, "0.98", "1", "1000000000");
    exit.fiat = "USD".into();
    exit.available_asset = "1000000000".into();
    exit.payment_methods = vec!["Bank Transfer".into()];
    exit.market = P2pOfferMarket::DirectExchange;

    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &route_query, "USDT", &[entry], &[exit]);

    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.source_fiat, "USD");
    assert_eq!(route.target_fiat, "USD");
    assert_eq!(route.asset, "USDT");
    assert!(route.same_venue);
    // A generic bank-transfer label does not confirm a specific payment rail.
    assert!(!route.payment_methods_verified);
    assert_eq!(
        route.entry_offer.as_ref().unwrap().payment_methods,
        ["Bank Transfer"]
    );
    assert_eq!(
        route.exit_offer.as_ref().unwrap().payment_methods,
        ["Bank Transfer"]
    );
}

#[test]
fn composes_armenian_usd_account_to_usd_bank_through_crypto() {
    let mut route_query = query(true);
    route_query.source_currency = "USD".into();
    route_query.target_currency = "USD".into();
    route_query.source_amount = 12_000.0;
    route_query.source_payment_method = Some("Ameriabank".into());
    route_query.target_payment_method = Some("Bank Transfer".into());

    let mut entry = offer("okx", P2pSide::BuyCrypto, "1.01", "1", "1000000000");
    entry.fiat = "USD".into();
    entry.available_asset = "1000000000".into();
    entry.payment_methods = vec!["Ameriabank".into()];

    let mut exit = offer("skylabs", P2pSide::SellCrypto, "0.98", "1", "1000000000");
    exit.fiat = "USD".into();
    exit.available_asset = "1000000000".into();
    exit.payment_methods = vec!["Bank Transfer".into()];
    exit.market = P2pOfferMarket::DirectExchange;

    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &route_query, "USDT", &[entry], &[exit]);

    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.source_fiat, "USD");
    assert_eq!(route.target_fiat, "USD");
    assert_eq!(route.asset, "USDT");
    assert_eq!(route.entry_offer.as_ref().unwrap().source, "okx");
    assert_eq!(route.exit_offer.as_ref().unwrap().source, "skylabs");
    assert!(!route.same_venue);
    assert!(route.requires_asset_transfer);
    assert!(!route.payment_methods_verified);
    assert_eq!(
        route.entry_offer.as_ref().unwrap().payment_methods,
        ["Ameriabank"]
    );
    assert_eq!(
        route.exit_offer.as_ref().unwrap().payment_methods,
        ["Bank Transfer"]
    );
}

#[test]
fn composes_rub_to_amd_with_bncex_as_the_exit() {
    let mut route_query = query(true);
    route_query.source_currency = "RUB".into();
    route_query.target_currency = "AMD".into();
    let mut entry = offer("bybit", P2pSide::BuyCrypto, "80", "1000", "1000000");
    entry.fiat = "RUB".into();
    let mut exit = offer("bncex", P2pSide::SellCrypto, "353.06", "10000", "50000000");
    exit.fiat = "AMD".into();
    exit.market = P2pOfferMarket::DirectExchange;

    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &route_query, "USDT", &[entry], &[exit]);

    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].source_fiat, "RUB");
    assert_eq!(routes[0].target_fiat, "AMD");
    assert_eq!(routes[0].entry_offer.as_ref().unwrap().source, "bybit");
    assert_eq!(routes[0].exit_offer.as_ref().unwrap().source, "bncex");
    assert!(!routes[0].same_venue);
    assert!(routes[0].requires_asset_transfer);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composes_fiat_provider_route_across_intermediary_networks() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "RUB".into(),
            target_fiat: "AMD".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();
    let route = response
        .routes
        .iter()
        .find(|route| route.route_provider.as_deref() == Some("test-intents"))
        .expect("provider route should be included");
    assert_eq!(route.route_path.first().map(String::as_str), Some("RUB"));
    assert_eq!(route.route_path.last().map(String::as_str), Some("AMD"));
    assert!(route.route_path.iter().any(|path| path == "USDT@tron"));
    assert_eq!(route.entry_offer.as_ref().unwrap().source, "bybit");
    assert_eq!(route.exit_offer.as_ref().unwrap().source, "bybit");
    assert!(route.route_fees[0].asset.starts_with("USDT@"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_cardano_to_amd_from_the_p2p_catalog() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1));
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "ADA".into(),
            target_fiat: "AMD".into(),
            source_amount: 100.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(false),
            max_price_deviation_bps: Some(1_000),
            limit: Some(20),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .expect("ADA to AMD search should complete");

    assert!(response.routes.iter().any(|route| {
        route.source_fiat == "ADA"
            && route.target_fiat == "AMD"
            && route.asset == "ADA"
            && route
                .exit_offer
                .as_ref()
                .is_some_and(|offer| offer.asset == "ADA")
    }));
    let encoded = serde_json::to_value(&response).expect("route response should serialize");
    let decoded: P2pRouteSearchResponse =
        serde_json::from_value(encoded).expect("cached route response should deserialize");
    assert_eq!(decoded.routes.len(), response.routes.len());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovers_bnd_to_amd_through_a_supported_intermediary() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1));
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "BND".into(),
            target_fiat: "AMD".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(false),
            max_price_deviation_bps: Some(1_000),
            limit: Some(20),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .expect("BND to AMD search should complete");

    assert!(response.routes.iter().any(|route| {
        route.source_fiat == "BND"
            && route.target_fiat == "AMD"
            && route.asset == "USDT"
            && route
                .entry_offer
                .as_ref()
                .is_some_and(|offer| offer.fiat == "BND")
            && route
                .exit_offer
                .as_ref()
                .is_some_and(|offer| offer.fiat == "AMD")
    }));
}

#[tokio::test]
async fn keeps_independent_quotes_from_multiple_route_providers() {
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![
            Arc::new(PricedRouteProvider {
                name: "near-intents",
                multiplier: 0.99,
                fee: "0.25",
                delay: Duration::ZERO,
            }),
            Arc::new(PricedRouteProvider {
                name: "cow-swap",
                multiplier: 0.97,
                fee: "2.5",
                delay: Duration::ZERO,
            }),
        ]);

    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "USDT".into(),
            target_fiat: "USDC".into(),
            source_amount: 100.0,
            source_network: Some("ethereum".into()),
            target_network: Some("ethereum".into()),
            bridge_fiat: None,
            assets: None,
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: None,
            max_price_deviation_bps: None,
            limit: Some(20),
            sources: Some("near-intents,cow-swap".into()),
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    let near = response
        .routes
        .iter()
        .find(|route| route.route_provider.as_deref() == Some("near-intents"))
        .unwrap();
    let cow = response
        .routes
        .iter()
        .find(|route| route.route_provider.as_deref() == Some("cow-swap"))
        .unwrap();
    assert_eq!(near.target_amount, "99");
    assert_eq!(cow.target_amount, "97");
    assert_eq!(near.route_fees[0].amount, "0.25");
    assert_eq!(cow.route_fees[0].amount, "2.5");
    assert_eq!(near.quote_expires_at, cow.quote_expires_at);
    assert_ne!(near.route_id, cow.route_id);

    let excluded = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "USDT".into(),
            target_fiat: "USDC".into(),
            source_amount: 100.0,
            source_network: Some("ethereum".into()),
            target_network: Some("ethereum".into()),
            bridge_fiat: None,
            assets: None,
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: None,
            max_price_deviation_bps: None,
            limit: Some(20),
            sources: Some("binance".into()),
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();
    assert!(excluded.routes.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn route_stream_publishes_each_provider_quote_batch_as_it_finishes() {
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![
            Arc::new(PricedRouteProvider {
                name: "fast-swap",
                multiplier: 0.99,
                fee: "0.25",
                delay: Duration::from_millis(10),
            }),
            Arc::new(PricedRouteProvider {
                name: "slow-swap",
                multiplier: 0.97,
                fee: "2.5",
                delay: Duration::from_millis(250),
            }),
        ]);
    let (updates, mut snapshots) = mpsc::channel(4);
    let search_id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        service
            .stream_routes(
                P2pRouteSearchQuery {
                    source_fiat: "USDT".into(),
                    target_fiat: "USDC".into(),
                    source_amount: 100.0,
                    source_network: Some("ethereum".into()),
                    target_network: Some("ethereum".into()),
                    bridge_fiat: None,
                    assets: None,
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    source_payment_fee_percent: None,
                    target_payment_fee_percent: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: None,
                    max_price_deviation_bps: None,
                    limit: Some(20),
                    sources: Some("fast-swap,slow-swap".into()),
                    exchange_mode: ExchangeMode::All,
                },
                search_id,
                updates,
            )
            .await
    });

    let first = tokio::time::timeout(Duration::from_millis(100), snapshots.recv())
        .await
        .expect("fast provider quote should be published independently")
        .expect("first provider quote snapshot");
    assert_eq!(first.routes_found, 1);
    assert_eq!(first.routes[0].route_provider.as_deref(), Some("fast-swap"));
    assert!(!task.is_finished());

    let second = snapshots
        .recv()
        .await
        .expect("second provider quote snapshot");
    let final_response = task.await.unwrap().unwrap();
    assert_eq!(second.routes_found, 2);
    assert_eq!(final_response.routes_found, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fiat_to_crypto_stream_does_not_batch_fast_and_slow_providers() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(DelayedRouteSource {
            name: "p2p-entry",
            delay: Duration::ZERO,
            price: "1",
        })],
        Duration::from_secs(1),
    )
    .with_route_providers(vec![
        Arc::new(PricedRouteProvider {
            name: "fast-swap",
            multiplier: 0.99,
            fee: "0.25",
            delay: Duration::from_millis(10),
        }),
        Arc::new(PricedRouteProvider {
            name: "slow-swap",
            multiplier: 0.97,
            fee: "2.5",
            delay: Duration::from_millis(250),
        }),
    ]);
    let (updates, mut snapshots) = mpsc::channel(8);
    let search_id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        service
            .stream_routes(
                P2pRouteSearchQuery {
                    source_fiat: "AMD".into(),
                    target_fiat: "USDC".into(),
                    source_amount: 100.0,
                    source_network: None,
                    target_network: None,
                    bridge_fiat: None,
                    assets: Some("USDT".into()),
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    source_payment_fee_percent: None,
                    target_payment_fee_percent: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: None,
                    max_price_deviation_bps: None,
                    limit: Some(20),
                    sources: Some("p2p-entry,fast-swap,slow-swap".into()),
                    exchange_mode: ExchangeMode::All,
                },
                search_id,
                updates,
            )
            .await
    });

    let fast_provider_snapshot = tokio::time::timeout(Duration::from_millis(100), async {
        loop {
            let snapshot = snapshots.recv().await.expect("route snapshot");
            if snapshot
                .routes
                .iter()
                .any(|route| route.route_provider.as_deref() == Some("fast-swap"))
            {
                return snapshot;
            }
        }
    })
    .await
    .expect("fast fiat-to-crypto provider must not wait for a slow provider");
    assert!(fast_provider_snapshot
        .routes
        .iter()
        .all(|route| route.route_provider.as_deref() != Some("slow-swap")));
    assert!(!task.is_finished());

    let final_response = task.await.unwrap().unwrap();
    assert!(final_response
        .routes
        .iter()
        .any(|route| route.route_provider.as_deref() == Some("slow-swap")));
}

#[tokio::test]
async fn dispatches_only_provider_supported_network_pairs() {
    let ethereum_unsupported = Arc::new(AtomicUsize::new(0));
    let tron_unsupported = Arc::new(AtomicUsize::new(0));
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![
            Arc::new(RecordingRouteProvider {
                name: "record-ethereum",
                assets: ["USDT@ethereum", "USDC@ethereum"]
                    .into_iter()
                    .map(|asset| Asset::parse(asset).unwrap())
                    .collect(),
                unsupported_calls: ethereum_unsupported.clone(),
            }),
            Arc::new(RecordingRouteProvider {
                name: "record-tron",
                assets: ["USDT@tron", "USDC@tron"]
                    .into_iter()
                    .map(|asset| Asset::parse(asset).unwrap())
                    .collect(),
                unsupported_calls: tron_unsupported.clone(),
            }),
        ]);

    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "USDT".into(),
            target_fiat: "USDC".into(),
            source_amount: 100.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: None,
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: None,
            max_price_deviation_bps: None,
            limit: Some(20),
            sources: Some("record-ethereum,record-tron".into()),
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    assert_eq!(response.routes.len(), 2);
    assert!(response
        .routes
        .iter()
        .all(|route| route.source_network == route.target_network));
    assert_eq!(ethereum_unsupported.load(Ordering::Relaxed), 0);
    assert_eq!(tron_unsupported.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn adds_direct_fiat_quotes_in_both_amd_rub_directions() {
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_fiat_route_providers(vec![Arc::new(FixedFiatRouteProvider)]);
    assert!(service.route_provider_names().contains("id-pay"));
    for (source_fiat, target_fiat, source_amount, expected_target) in [
        ("AMD", "RUB", 400.0, "100.00"),
        ("RUB", "AMD", 100.0, "400.00"),
    ] {
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: source_fiat.into(),
                target_fiat: target_fiat.into(),
                source_amount,
                source_network: None,
                target_network: None,
                bridge_fiat: None,
                assets: None,
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                source_payment_fee_percent: None,
                target_payment_fee_percent: None,
                merchant_only: None,
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: None,
                max_price_deviation_bps: None,
                limit: Some(20),
                sources: Some("id-pay".into()),
                exchange_mode: ExchangeMode::All,
            })
            .await
            .unwrap();
        let route = response
            .routes
            .iter()
            .find(|route| {
                route
                    .entry_offer
                    .as_ref()
                    .map(|offer| offer.source.as_str())
                    == Some("id-pay")
            })
            .unwrap();
        assert_eq!(route.target_amount, expected_target);
        assert_eq!(route.route_kind, "fiat_to_fiat");
        assert!(route.same_venue);
        assert!(!route.requires_asset_transfer);
        assert!(route.route_provider.is_none());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composes_fiat_to_crypto_route_through_provider() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "RUB".into(),
            target_fiat: "USDT".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: Some("ton".into()),
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();
    let route = response
        .routes
        .iter()
        .find(|route| {
            route.route_kind == "fiat_to_crypto"
                && route.route_provider.as_deref() == Some("test-intents")
        })
        .expect("fiat-to-crypto provider route should be included");
    assert_eq!(route.route_provider.as_deref(), Some("test-intents"));
    assert_eq!(route.route_path.first().map(String::as_str), Some("RUB"));
    assert_eq!(
        route.route_path.last().map(String::as_str),
        Some("USDT@ton")
    );
    assert!(route.entry_offer.is_some());
    assert!(route.exit_offer.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composes_small_amd_to_btc_near_route_after_filter_fallback() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "BTC".into(),
            source_amount: 10_000.0,
            source_network: None,
            target_network: Some("near".into()),
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: Some("Ameriabank".into()),
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: Some(20),
            min_completion_rate: Some(0.9),
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    let paths = response
        .routes
        .iter()
        .filter(|route| route.route_provider.as_deref() == Some("test-intents"))
        .map(|route| {
            assert!(route.target_amount.parse::<f64>().unwrap() > 0.0);
            assert!(!route.payment_methods_verified);
            route.route_path.clone()
        })
        .collect::<std::collections::HashSet<_>>();
    let expected_paths = [
        "avalanche-c",
        "ethereum",
        "gnosis",
        "near",
        "optimism",
        "scroll",
    ]
    .into_iter()
    .map(|network| {
        vec![
            "AMD".to_string(),
            format!("USDT@{network}"),
            "BTC@near".to_string(),
        ]
    })
    .collect::<std::collections::HashSet<_>>();
    assert!(
        expected_paths.is_subset(&paths),
        "missing routes: {:?}",
        expected_paths.difference(&paths)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composes_crypto_to_crypto_provider_routes_for_supported_pairs() {
    let service = P2pSearchService::with_sources(vec![], Duration::from_secs(1))
        .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
    let cases = [
        ("USDT", "USDT", "ethereum", "avalanche-c"),
        ("USDT", "USDC", "ethereum", "solana"),
        ("USDT", "BTC", "ethereum", "near"),
    ];

    for (source_asset, target_asset, source_network, target_network) in cases {
        let response = service
            .search_routes(P2pRouteSearchQuery {
                source_fiat: source_asset.into(),
                target_fiat: target_asset.into(),
                source_amount: 1_000.0,
                source_network: Some(source_network.into()),
                target_network: Some(target_network.into()),
                bridge_fiat: None,
                assets: None,
                intermediary_assets: None,
                source_payment_method: None,
                target_payment_method: None,
                source_payment_fee_percent: None,
                target_payment_fee_percent: None,
                merchant_only: Some(false),
                min_orders: None,
                min_completion_rate: None,
                allow_cross_venue: Some(true),
                max_price_deviation_bps: Some(1_000),
                limit: Some(40),
                sources: None,
                exchange_mode: ExchangeMode::All,
            })
            .await
            .unwrap();
        let route = response
                .routes
                .iter()
                .find(|route| route.route_provider.as_deref() == Some("test-intents"))
                .unwrap_or_else(|| {
                    panic!(
                        "missing route for {source_asset}@{source_network} -> {target_asset}@{target_network}"
                    )
                });

        assert!(route.target_amount.parse::<f64>().unwrap() > 0.0);
        assert_eq!(
            route.route_path,
            vec![
                format!("{source_asset}@{source_network}"),
                format!("{target_asset}@{target_network}"),
            ]
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn composes_crypto_to_fiat_route_through_provider() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "USDT".into(),
            target_fiat: "AMD".into(),
            source_amount: 100.0,
            source_network: Some("ethereum".into()),
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();
    let route = response
        .routes
        .iter()
        .find(|route| {
            route.route_kind == "crypto_to_fiat"
                && route.route_provider.as_deref() == Some("test-intents")
        })
        .expect("crypto-to-fiat provider route should be included");
    assert_eq!(route.route_provider.as_deref(), Some("test-intents"));
    assert_eq!(
        route.route_path.first().map(String::as_str),
        Some("USDT@ethereum")
    );
    assert_eq!(route.route_path.last().map(String::as_str), Some("AMD"));
    assert!(route.entry_offer.is_none());
    assert!(route.exit_offer.is_some());
}

#[test]
fn route_limit_preserves_a_lower_ranked_provider_route() {
    let mut normalized = query(false);
    normalized.limit = 40;
    let mut discovered = Vec::new();
    compose_fiat_routes(
        &mut discovered,
        &normalized,
        "USDT",
        &[
            offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000"),
            offer("bybit", P2pSide::BuyCrypto, "410", "1000", "200000"),
        ],
        &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
    );
    let mut all_routes = HashMap::new();
    let bybit = discovered[0].clone();
    for index in 0..40 {
        let mut route = bybit.clone();
        route.route_id = format!("bybit-{index}");
        route.target_amount = format!("{}", 1_000 + index);
        all_routes.insert(route.route_id.clone(), route);
    }
    let mut id_pay = bybit;
    id_pay.route_id = "id-pay".into();
    id_pay.route_provider = Some("id-pay".into());
    id_pay.entry_offer = None;
    id_pay.exit_offer = None;
    id_pay.target_amount = "1".into();
    id_pay.warnings.clear();
    all_routes.insert(id_pay.route_id.clone(), id_pay);

    let snapshot = response_snapshot(Uuid::nil(), &normalized, &all_routes, &[], &HashMap::new());
    assert_eq!(snapshot.routes_found, 41);
    assert_eq!(snapshot.routes.len(), 40);
    assert!(snapshot
        .routes
        .iter()
        .any(|route| route.route_id == "id-pay"));
}

#[test]
fn route_limit_counts_every_provider_in_a_combined_route() {
    let normalized = query(true);
    let mut discovered = Vec::new();
    compose_fiat_routes(
        &mut discovered,
        &normalized,
        "USDT",
        &[offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000")],
        &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
    );
    let mut first = discovered.pop().unwrap();
    first.route_id = "first".into();
    first.route_provider = Some("shared".into());
    let mut second = first.clone();
    second.route_id = "second".into();
    let mut bitget = first.clone();
    bitget.route_id = "bitget".into();
    bitget.exit_offer.as_mut().unwrap().source = "bitget".into();
    let mut routes = vec![first, second, bitget];

    truncate_routes_preserving_providers(&mut routes, 2);

    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].route_id, "first");
    assert_eq!(routes[1].route_id, "bitget");
}

#[test]
fn route_limit_keeps_several_routes_per_provider() {
    let normalized = query(true);
    let mut discovered = Vec::new();
    compose_fiat_routes(
        &mut discovered,
        &normalized,
        "USDT",
        &[offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000")],
        &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
    );
    let first = discovered.pop().unwrap();
    let mut routes = (0..50)
        .map(|index| {
            let mut route = first.clone();
            route.route_id = format!("bybit-{index}");
            route
        })
        .collect::<Vec<_>>();
    routes.extend((0..10).map(|index| {
        let mut route = first.clone();
        route.route_id = format!("binance-{index}");
        route.entry_offer.as_mut().unwrap().source = "binance".into();
        route.exit_offer.as_mut().unwrap().source = "binance".into();
        route
    }));
    truncate_routes_preserving_providers(&mut routes, 20);

    assert_eq!(routes.len(), 20);
    assert_eq!(
        routes
            .iter()
            .filter(|route| route.route_id.starts_with("binance-"))
            .count(),
        8
    );
}

#[test]
fn route_sort_prefers_higher_payout_before_route_quality() {
    let normalized = query(true);
    let mut discovered = Vec::new();
    compose_fiat_routes(
        &mut discovered,
        &normalized,
        "USDT",
        &[offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000")],
        &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
    );
    let base = discovered.pop().expect("test route should be composed");

    let mut same_venue = base.clone();
    same_venue.route_id = "same-venue".into();
    same_venue.target_amount = "20000.00".into();

    let mut cross_venue = base;
    cross_venue.route_id = "cross-venue".into();
    cross_venue.same_venue = false;
    cross_venue.requires_asset_transfer = true;
    cross_venue.target_amount = "21000.00".into();

    let mut routes = vec![cross_venue, same_venue];
    sort_routes(&mut routes);

    assert_eq!(routes[0].route_id, "cross-venue");
    assert_eq!(routes[1].route_id, "same-venue");
}

#[test]
fn route_sort_uses_quality_as_a_tie_breaker() {
    let normalized = query(true);
    let mut discovered = Vec::new();
    compose_fiat_routes(
        &mut discovered,
        &normalized,
        "USDT",
        &[offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000")],
        &[offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000")],
    );
    let base = discovered.pop().expect("test route should be composed");

    let mut unverified = base.clone();
    unverified.route_id = "unverified".into();
    unverified.payment_methods_verified = false;
    unverified.target_amount = "20000.00".into();

    let mut verified = base;
    verified.route_id = "verified".into();
    verified.target_amount = "20000.00".into();

    let mut routes = vec![unverified, verified];
    sort_routes(&mut routes);

    assert_eq!(routes[0].route_id, "verified");
    assert_eq!(routes[1].route_id, "unverified");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn route_stream_reports_each_completed_asset_batch() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(ProgressiveSource)], Duration::from_secs(1));
    let (updates, mut snapshots) = mpsc::channel(4);
    let search_id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        service
            .stream_routes(
                P2pRouteSearchQuery {
                    source_fiat: "AMD".into(),
                    target_fiat: "RUB".into(),
                    source_amount: 100_000.0,
                    source_network: None,
                    target_network: None,
                    bridge_fiat: None,
                    assets: Some("USDT,USDC".into()),
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    source_payment_fee_percent: None,
                    target_payment_fee_percent: None,
                    merchant_only: Some(false),
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: Some(false),
                    max_price_deviation_bps: Some(1_000),
                    limit: Some(40),
                    sources: Some("bybit".into()),
                    exchange_mode: ExchangeMode::All,
                },
                search_id,
                updates,
            )
            .await
    });

    let first = snapshots.recv().await.expect("first asset snapshot");
    let second = snapshots.recv().await.expect("second asset snapshot");
    let final_response = task.await.unwrap().unwrap();
    assert_eq!(first.routes_found, 1);
    assert_eq!(second.routes_found, 2);
    assert_eq!(final_response.routes_found, 2);
    assert_eq!(final_response.search_id, search_id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn route_stream_publishes_a_fast_provider_before_slower_providers_finish() {
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(DelayedRouteSource {
                name: "fast",
                delay: Duration::from_millis(10),
                price: "84",
            }),
            Arc::new(DelayedRouteSource {
                name: "slow",
                delay: Duration::from_millis(250),
                price: "85",
            }),
        ],
        Duration::from_secs(1),
    );
    let (updates, mut snapshots) = mpsc::channel(4);
    let search_id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        service
            .stream_routes(
                P2pRouteSearchQuery {
                    source_fiat: "USDC".into(),
                    target_fiat: "RUB".into(),
                    source_amount: 100.0,
                    source_network: Some("ethereum".into()),
                    target_network: None,
                    bridge_fiat: None,
                    assets: None,
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: Some("Sberbank".into()),
                    source_payment_fee_percent: None,
                    target_payment_fee_percent: None,
                    merchant_only: Some(false),
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: Some(true),
                    max_price_deviation_bps: Some(1_000),
                    limit: Some(40),
                    sources: None,
                    exchange_mode: ExchangeMode::All,
                },
                search_id,
                updates,
            )
            .await
    });

    let first = tokio::time::timeout(Duration::from_millis(100), snapshots.recv())
        .await
        .expect("fast provider should stream before the slow provider finishes")
        .expect("first provider snapshot");
    assert_eq!(first.routes_found, 1);
    assert_eq!(first.routes[0].exit_offer.as_ref().unwrap().source, "fast");
    assert!(!task.is_finished());

    let second = snapshots.recv().await.expect("second provider snapshot");
    let final_response = task.await.unwrap().unwrap();
    assert_eq!(second.routes_found, 2);
    assert_eq!(final_response.routes_found, 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn route_stream_does_not_make_a_direct_provider_wait_for_local_legs() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(DelayedRouteSource {
            name: "slow-p2p",
            delay: Duration::from_millis(250),
            price: "400",
        })],
        Duration::from_secs(1),
    )
    .with_fiat_route_providers(vec![Arc::new(FixedFiatRouteProvider)]);
    let (updates, mut snapshots) = mpsc::channel(4);
    let search_id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        service
            .stream_routes(
                P2pRouteSearchQuery {
                    source_fiat: "AMD".into(),
                    target_fiat: "RUB".into(),
                    source_amount: 100_000.0,
                    source_network: None,
                    target_network: None,
                    bridge_fiat: None,
                    assets: Some("USDT".into()),
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    source_payment_fee_percent: None,
                    target_payment_fee_percent: None,
                    merchant_only: Some(false),
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: Some(true),
                    max_price_deviation_bps: Some(1_000),
                    limit: Some(40),
                    sources: None,
                    exchange_mode: ExchangeMode::All,
                },
                search_id,
                updates,
            )
            .await
    });

    let first = tokio::time::timeout(Duration::from_millis(100), snapshots.recv())
        .await
        .expect("direct provider should publish before slow local legs")
        .expect("direct provider snapshot");
    assert_eq!(first.routes_found, 1);
    assert_eq!(
        first.routes[0].entry_offer.as_ref().unwrap().source,
        "id-pay"
    );
    assert!(first.routes[0].exit_offer.is_none());
    assert!(!task.is_finished());

    let final_response = task.await.unwrap().unwrap();
    assert!(final_response.routes_found >= 2);
    assert!(final_response.routes.iter().any(|route| {
        route
            .entry_offer
            .as_ref()
            .map(|offer| offer.source.as_str())
            == Some("id-pay")
    }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_direct_provider_stops_waiting_before_slow_local_legs() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(DelayedRouteSource {
            name: "slow-p2p",
            delay: Duration::from_millis(250),
            price: "400",
        })],
        Duration::from_secs(1),
    )
    .with_fiat_route_providers(vec![Arc::new(FixedFiatRouteProvider)]);
    let (updates, mut snapshots) = mpsc::channel(4);
    let search_id = Uuid::new_v4();
    let task = tokio::spawn(async move {
        service
            .stream_routes(
                P2pRouteSearchQuery {
                    source_fiat: "AMD".into(),
                    target_fiat: "BYN".into(),
                    source_amount: 100_000.0,
                    source_network: None,
                    target_network: None,
                    bridge_fiat: None,
                    assets: Some("USDT".into()),
                    intermediary_assets: None,
                    source_payment_method: None,
                    target_payment_method: None,
                    source_payment_fee_percent: None,
                    target_payment_fee_percent: None,
                    merchant_only: Some(false),
                    min_orders: None,
                    min_completion_rate: None,
                    allow_cross_venue: Some(true),
                    max_price_deviation_bps: Some(1_000),
                    limit: Some(40),
                    sources: Some("slow-p2p,id-pay".into()),
                    exchange_mode: ExchangeMode::All,
                },
                search_id,
                updates,
            )
            .await
    });

    let first = tokio::time::timeout(Duration::from_millis(100), snapshots.recv())
        .await
        .expect("unsupported provider status should be published immediately")
        .expect("provider status snapshot");
    assert_eq!(first.provider_statuses.len(), 1);
    assert_eq!(first.provider_statuses[0].source, "id-pay");
    assert!(first.provider_statuses[0].ok);
    assert_eq!(first.provider_statuses[0].offers_found, 0);
    assert!(!task.is_finished());

    task.await.unwrap().unwrap();
}

#[test]
fn rejects_cross_venue_and_impossible_target_limit() {
    let entry = offer("binance", P2pSide::BuyCrypto, "400", "1000", "200000");
    let exit = offer("bybit", P2pSide::SellCrypto, "80", "30000", "100000");
    let mut routes = Vec::new();
    compose_fiat_routes(
        &mut routes,
        &query(false),
        "USDT",
        std::slice::from_ref(&entry),
        std::slice::from_ref(&exit),
    );
    assert!(routes.is_empty());
    compose_fiat_routes(&mut routes, &query(true), "USDT", &[entry], &[exit]);
    assert!(routes.is_empty());
}

#[test]
fn removes_large_price_outlier() {
    let offers = vec![
        offer("a", P2pSide::SellCrypto, "80", "1", "100000"),
        offer("b", P2pSide::SellCrypto, "81", "1", "100000"),
        offer("c", P2pSide::SellCrypto, "200", "1", "100000"),
    ];
    let filtered = reject_price_outliers(offers, 1_000);
    assert_eq!(filtered.len(), 2);
}

#[tokio::test]
async fn direct_fiat_crypto_routes_keep_real_venue_price_outliers() {
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(DelayedRouteSource {
                name: "binance",
                delay: Duration::ZERO,
                price: "450",
            }),
            Arc::new(DelayedRouteSource {
                name: "bybit",
                delay: Duration::ZERO,
                price: "451",
            }),
            Arc::new(DelayedRouteSource {
                name: "bitget",
                delay: Duration::ZERO,
                price: "615",
            }),
        ],
        Duration::from_secs(1),
    );
    let mut query = P2pRouteSearchQuery {
        source_fiat: "KZT".into(),
        target_fiat: "USDT".into(),
        source_amount: 50_000.0,
        source_network: None,
        target_network: Some("tron".into()),
        bridge_fiat: None,
        assets: None,
        intermediary_assets: None,
        source_payment_method: None,
        target_payment_method: None,
        source_payment_fee_percent: None,
        target_payment_fee_percent: None,
        merchant_only: Some(false),
        min_orders: None,
        min_completion_rate: None,
        allow_cross_venue: Some(false),
        max_price_deviation_bps: Some(1_000),
        limit: Some(40),
        sources: None,
        exchange_mode: ExchangeMode::P2p,
    };
    for (source, target, amount) in [("KZT", "USDT", 50_000.0), ("USDT", "KZT", 100.0)] {
        query.source_fiat = source.into();
        query.target_fiat = target.into();
        query.source_amount = amount;
        query.source_network = (source == "USDT").then(|| "tron".into());
        query.target_network = (target == "USDT").then(|| "tron".into());
        let response = service.search_routes(query.clone()).await.unwrap();
        assert!(response.routes.iter().any(|route| {
            route
                .entry_offer
                .as_ref()
                .or(route.exit_offer.as_ref())
                .is_some_and(|offer| offer.source == "bitget")
        }));
    }
}

#[test]
fn amd_crypto_cycle_reports_confirmed_profit_in_minor_units() {
    let mut query = query(false);
    query.target_currency = "AMD".into();
    query.source_payment_fee_bps = Some(0);
    query.target_payment_fee_bps = Some(0);
    let entry = offer("binance", P2pSide::BuyCrypto, "350", "1", "200000");
    let exit = offer("binance", P2pSide::SellCrypto, "360", "1", "200000");
    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &query, "USDT", &[entry], &[exit]);

    assert_eq!(routes[0].route_kind, "crypto_cycle");
    assert_eq!(
        profitability_for_route(&routes[0], &query),
        Some(RouteProfitability::Confirmed {
            net_profit_minor: 285_714,
            profit_bps: 285,
        })
    );
}

#[test]
fn cross_venue_cycle_does_not_claim_profit_without_network_fee() {
    let mut query = query(true);
    query.target_currency = "AMD".into();
    query.source_payment_fee_bps = Some(0);
    query.target_payment_fee_bps = Some(0);
    let entry = offer("binance", P2pSide::BuyCrypto, "350", "1", "200000");
    let exit = offer("bybit", P2pSide::SellCrypto, "360", "1", "200000");
    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &query, "USDT", &[entry], &[exit]);

    assert!(matches!(
        profitability_for_route(&routes[0], &query),
        Some(RouteProfitability::Unconfirmed { missing_costs, .. })
            if missing_costs == vec![RouteCostKind::NetworkFee]
    ));
}

#[test]
fn sub_minor_unit_cycle_does_not_divide_by_zero() {
    let mut query = query(false);
    query.target_currency = "AMD".into();
    query.source_payment_fee_bps = Some(0);
    query.target_payment_fee_bps = Some(0);
    let entry = offer("binance", P2pSide::BuyCrypto, "350", "1", "200000");
    let exit = offer("binance", P2pSide::SellCrypto, "360", "1", "200000");
    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &query, "USDT", &[entry], &[exit]);
    routes[0].source_amount = "0.00".into();

    assert_eq!(profitability_for_route(&routes[0], &query), None);
}

#[tokio::test]
async fn composes_amd_fiat_cycle_from_catalog_intermediary() {
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_fiat_route_providers(vec![Arc::new(FixedFiatRouteProvider)]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "AMD".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: Some(0.0),
            target_payment_fee_percent: Some(0.0),
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(20),
            sources: Some("id-pay".into()),
            exchange_mode: ExchangeMode::Exchanger,
        })
        .await
        .unwrap();

    let route = response
        .routes
        .iter()
        .find(|route| route.route_kind == "fiat_cycle")
        .expect("AMD fiat cycle should be returned");
    assert_eq!(route.route_path, ["AMD", "RUB", "AMD"]);
    assert!(matches!(
        route.profitability,
        Some(RouteProfitability::Unconfirmed { ref missing_costs, .. })
            if missing_costs == &[RouteCostKind::ProviderFee]
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_cycle_rejects_an_amd_price_outlier_before_quoting() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(OutlierCycleSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(FixedIntentProvider)]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "AMD".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: Some(0.0),
            target_payment_fee_percent: Some(0.0),
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    let provider_routes = response
        .routes
        .iter()
        .filter(|route| route.route_provider.as_deref() == Some("test-intents"))
        .collect::<Vec<_>>();
    assert!(!provider_routes.is_empty());
    assert!(provider_routes.iter().all(|route| {
        route
            .entry_offer
            .as_ref()
            .is_some_and(|offer| offer.price != "195.49")
            && route.target_amount.parse::<f64>().unwrap() < 110_000.0
    }));
}

#[tokio::test]
async fn composes_an_amd_cycle_through_a_same_chain_cow_swap() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(OutlierCycleSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(PricedRouteProvider {
                name: "cow-swap",
                multiplier: 0.99,
                fee: "0.1",
                delay: Duration::ZERO,
            })]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "AMD".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT,USDC".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: Some(0.0),
            target_payment_fee_percent: Some(0.0),
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: Some("outlier,cow-swap".into()),
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    let route = response
        .routes
        .iter()
        .find(|route| route.route_provider.as_deref() == Some("cow-swap"))
        .expect("same-chain CoW cycle should be returned");
    let entry = route.entry_offer.as_ref().unwrap();
    let exit = route.exit_offer.as_ref().unwrap();
    assert_ne!(entry.asset, exit.asset);
    assert_eq!(route.source_network.as_deref(), Some("ethereum"));
    assert_eq!(route.target_network.as_deref(), Some("ethereum"));
    assert!(route.route_path.iter().any(|step| step == "USDT@ethereum"));
    assert!(route.route_path.iter().any(|step| step == "USDC@ethereum"));
}

#[tokio::test]
async fn composes_an_amd_cycle_through_a_cross_chain_symbiosis_swap() {
    let service =
        P2pSearchService::with_sources(vec![Arc::new(OutlierCycleSource)], Duration::from_secs(1))
            .with_route_providers(vec![Arc::new(RecordingRouteProvider {
                name: "symbiosis",
                assets: vec![
                    Asset::new("USDT", Some("ethereum")).unwrap(),
                    Asset::new("USDC", Some("solana")).unwrap(),
                ],
                unsupported_calls: Arc::new(AtomicUsize::new(0)),
            })]);
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "AMD".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT,USDC".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: Some(0.0),
            target_payment_fee_percent: Some(0.0),
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(40),
            sources: Some("outlier,symbiosis".into()),
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    let route = response
        .routes
        .iter()
        .find(|route| route.route_provider.as_deref() == Some("symbiosis"))
        .expect("cross-chain Symbiosis cycle should be returned");
    let route_networks = HashSet::from([
        route.source_network.as_deref(),
        route.target_network.as_deref(),
    ]);
    assert_eq!(
        route_networks,
        HashSet::from([Some("ethereum"), Some("solana")])
    );
    assert!(route.route_path.iter().any(|step| step == "USDT@ethereum"));
    assert!(route.route_path.iter().any(|step| step == "USDC@solana"));
}

#[tokio::test]
async fn refreshes_initially_empty_route_provider_capabilities() {
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![
            Arc::new(FixedIntentProvider),
            Arc::new(PricedRouteProvider {
                name: "cow-swap",
                multiplier: 0.99,
                fee: "0.1",
                delay: Duration::ZERO,
            }),
            Arc::new(PricedRouteProvider {
                name: "symbiosis",
                multiplier: 0.98,
                fee: "0.2",
                delay: Duration::ZERO,
            }),
        ]);
    *service.provider_capabilities_cache.write().unwrap() = Some(HashMap::from([
        ("test-intents".to_string(), HashSet::new()),
        ("cow-swap".to_string(), HashSet::new()),
        ("symbiosis".to_string(), HashSet::new()),
    ]));

    service
        .refresh_provider_capabilities(&["test-intents", "cow-swap", "symbiosis", "missing"])
        .await;

    let cache = service.provider_capabilities_cache.read().unwrap();
    let snapshot = cache.as_ref().expect("provider capability snapshot");
    let intent_assets = snapshot
        .get("test-intents")
        .expect("refreshed intent capability");
    assert!(intent_assets.contains(&Asset::new("USDT", Some("tron")).unwrap()));
    assert!(intent_assets.contains(&Asset::new("USDC", Some("solana")).unwrap()));
    let cow_assets = snapshot.get("cow-swap").expect("refreshed CoW capability");
    assert!(cow_assets.contains(&Asset::new("USDT", Some("ethereum")).unwrap()));
    assert!(cow_assets.contains(&Asset::new("USDC", Some("ethereum")).unwrap()));
    let symbiosis_assets = snapshot
        .get("symbiosis")
        .expect("refreshed Symbiosis capability");
    assert!(symbiosis_assets.contains(&Asset::new("USDT", Some("ethereum")).unwrap()));
    assert!(symbiosis_assets.contains(&Asset::new("USDC", Some("ethereum")).unwrap()));
    assert!(snapshot.get("missing").is_none());
}

#[test]
fn keeps_direct_exchange_quotes_outside_the_p2p_median() {
    let mut direct = offer("whitebird", P2pSide::SellCrypto, "70", "1", "100000");
    direct.market = crate::p2p::service::P2pOfferMarket::DirectExchange;
    let offers = vec![
        offer("a", P2pSide::SellCrypto, "80", "1", "100000"),
        offer("b", P2pSide::SellCrypto, "81", "1", "100000"),
        offer("c", P2pSide::SellCrypto, "82", "1", "100000"),
        direct,
    ];

    let filtered = reject_price_outliers(offers, 1_000);

    assert_eq!(filtered.len(), 4);
    assert!(filtered.iter().any(|offer| offer.source == "whitebird"));
}

#[test]
fn keeps_offers_without_exact_deep_links_for_manual_fallback() {
    let mut entry = offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000");
    entry.source_url_is_exact = false;
    let exit = offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000");

    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &query(false), "USDT", &[entry], &[exit]);

    assert_eq!(routes.len(), 1);
    assert!(!routes[0].entry_offer.as_ref().unwrap().source_url_is_exact);
    assert!(routes[0]
        .warnings
        .iter()
        .any(|warning| warning.contains("deep-link")));
}

#[test]
fn labels_route_when_venue_payment_ids_are_opaque() {
    let mut query = query(true);
    query.source_payment_method = Some("Ameriabank".into());
    query.target_payment_method = Some("Sberbank".into());
    let mut entry = offer("bybit", P2pSide::BuyCrypto, "400", "1000", "200000");
    let mut exit = offer("bybit", P2pSide::SellCrypto, "80", "1000", "100000");
    entry.payment_methods = vec!["18".into()];
    exit.payment_methods = vec!["40".into()];

    let mut routes = Vec::new();
    compose_fiat_routes(&mut routes, &query, "USDT", &[entry], &[exit]);

    assert_eq!(routes.len(), 1);
    assert!(!routes[0].payment_methods_verified);
    assert_eq!(routes[0].warnings.len(), 3);
}

#[test]
fn composes_crypto_to_crypto_route_without_fiat() {
    let query = NormalizedRouteQuery {
        source_currency: "ETH".into(),
        target_currency: "USDC".into(),
        source_amount: 0.04,
        source_network: Some("ethereum".into()),
        target_network: Some("ethereum".into()),
        assets: vec!["USDT".into()],
        assets_explicit: false,
        source_payment_method: None,
        target_payment_method: None,
        source_payment_fee_bps: None,
        target_payment_fee_bps: None,
        merchant_only: false,
        min_orders: None,
        min_completion_rate: None,
        allow_cross_venue: true,
        max_price_deviation_bps: 1_000,
        limit: 20,
        sources: None,
        exchange_mode: ExchangeMode::All,
    };
    let tickers = vec![
        CryptoTicker {
            symbol: "ETHUSDT".into(),
            bid: 2_500.0,
            ask: 2_501.0,
        },
        CryptoTicker {
            symbol: "USDCUSDT".into(),
            bid: 0.999,
            ask: 1.001,
        },
    ];
    let mut routes = Vec::new();

    compose_crypto_market_routes(&mut routes, &query, "bybit", "ETH", "USDC", &tickers);

    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.source_fiat, "ETH");
    assert_eq!(route.target_fiat, "USDC");
    assert_eq!(route.source_network.as_deref(), Some("ethereum"));
    assert_eq!(route.target_network.as_deref(), Some("ethereum"));
    assert_eq!(route.bridge_currency.as_deref(), Some("USDT"));
    assert!(route.entry_offer.is_none());
    assert!(route.exit_offer.is_none());
    assert_eq!(route.market_path.as_ref().unwrap().venue, "bybit");
    assert!(route.target_amount.parse::<f64>().unwrap() > 99.0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "calls live Binance, Bybit, OKX, Bitget and Rapira public P2P endpoints"]
async fn live_amd_to_rub_route_search() {
    let config = Config::from_env().unwrap();
    let service =
        P2pSearchService::from_config(&config, crate::networks::NetworkCatalog::test_default())
            .unwrap();
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "RUB".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT,USDC,BTC,ETH".into()),
            intermediary_assets: None,
            source_payment_method: None,
            target_payment_method: None,
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(false),
            max_price_deviation_bps: Some(1_000),
            limit: Some(10),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
    assert!(response.routes.iter().all(|route| route.same_venue));
    assert_eq!(response.asset_statuses.len(), 4);
    assert!(response.asset_statuses.iter().all(|status| status
        .entry_sources
        .iter()
        .any(|source| source.ok)
        && status.exit_sources.iter().any(|source| source.ok)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "calls live Binance, Bybit, OKX, Bitget and Rapira public P2P endpoints"]
async fn live_bank_filtered_amd_to_rub_route_search() {
    let config = Config::from_env().unwrap();
    let service =
        P2pSearchService::from_config(&config, crate::networks::NetworkCatalog::test_default())
            .unwrap();
    let response = service
        .search_routes(P2pRouteSearchQuery {
            source_fiat: "AMD".into(),
            target_fiat: "RUB".into(),
            source_amount: 100_000.0,
            source_network: None,
            target_network: None,
            bridge_fiat: None,
            assets: Some("USDT,USDC,BTC,ETH".into()),
            intermediary_assets: None,
            source_payment_method: Some("Ameriabank".into()),
            target_payment_method: Some("Sberbank".into()),
            source_payment_fee_percent: None,
            target_payment_fee_percent: None,
            merchant_only: Some(false),
            min_orders: None,
            min_completion_rate: None,
            allow_cross_venue: Some(true),
            max_price_deviation_bps: Some(1_000),
            limit: Some(10),
            sources: None,
            exchange_mode: ExchangeMode::All,
        })
        .await
        .unwrap();

    eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
    assert!(response.routes.iter().all(|route| {
        route
            .entry_offer
            .as_ref()
            .is_some_and(|offer| offer.side == P2pSide::BuyCrypto)
            && route
                .exit_offer
                .as_ref()
                .is_some_and(|offer| offer.side == P2pSide::SellCrypto)
    }));
}

#[test]
fn crypto_market_cycles_compare_venues_and_find_three_trade_paths_after_fees() {
    let mut request = crypto_cycle_request(100.0);
    request.intermediary_assets = None;
    let origin = Asset::new("USDT", Some("bnb-smart-chain")).unwrap();
    let mut query = normalize_query(
        request,
        &["USDT".into()],
        &crate::networks::NetworkCatalog::test_default(),
        &[origin],
    )
    .unwrap();
    let ticker = |symbol: &str, bid, ask| CryptoTicker {
        symbol: symbol.into(),
        bid,
        ask,
    };
    let cross = HashMap::from([
        ("bybit".into(), vec![ticker("ETHUSDT", 99.0, 100.0)]),
        ("binance".into(), vec![ticker("ETHUSDT", 103.0, 104.0)]),
    ]);
    let routes = compose_crypto_market_cycles(&query, &cross);
    assert_eq!(
        routes.len(),
        1,
        "cross-venue spread should create one positive round trip"
    );
    assert_eq!(
        routes[0]
            .cycle_legs
            .iter()
            .map(|l| l.provider.as_str())
            .collect::<Vec<_>>(),
        ["bybit", "binance"]
    );
    assert!((routes[0].target_amount.parse::<f64>().unwrap() - 102.794103).abs() < 1e-7);
    query.allow_cross_venue = false;
    assert!(compose_crypto_market_cycles(&query, &cross).is_empty());
    let triangles = HashMap::from([(
        "bybit".into(),
        vec![
            ticker("ETHUSDT", 99.0, 100.0),
            ticker("BTCETH", 2.0, 2.01),
            ticker("BTCUSDT", 205.0, 206.0),
        ],
    )]);
    let routes = compose_crypto_market_cycles(&query, &triangles);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].cycle_legs.len(), 3);
    assert!(routes[0].target_amount.parse::<f64>().unwrap() > 100.0);
    assert_eq!(
        routes[0].route_path,
        ["USDT@bnb-smart-chain", "ETH", "BTC", "USDT@bnb-smart-chain"]
    );
    assert!(routes[0]
        .warnings
        .iter()
        .any(|w| w.contains("order-book depth")));
    query.assets_explicit = true;
    query.assets = vec!["USDC".into()];
    assert!(compose_crypto_market_cycles(&query, &triangles).is_empty());
    query.assets_explicit = false;
    query.sources = Some("binance".into());
    assert!(compose_crypto_market_cycles(&query, &triangles).is_empty());
}

#[test]
fn crypto_market_cycles_reject_large_price_outliers_and_fiat_intermediaries() {
    let mut request = crypto_cycle_request(100.0);
    request.intermediary_assets = None;
    let query = normalize_query(
        request,
        &["USDT".into()],
        &crate::networks::NetworkCatalog::test_default(),
        &[Asset::new("USDT", Some("bnb-smart-chain")).unwrap()],
    )
    .unwrap();
    let ticker = |symbol: &str, bid, ask| CryptoTicker {
        symbol: symbol.into(),
        bid,
        ask,
    };
    let markets = HashMap::from([
        (
            "bybit".into(),
            vec![ticker("ETHUSDT", 99.0, 100.0), ticker("USDTEUR", 0.9, 0.91)],
        ),
        (
            "okx".into(),
            vec![
                ticker("ETHUSDT", 200.0, 201.0),
                ticker("USDTEUR", 0.88, 0.89),
            ],
        ),
    ]);
    assert!(
        compose_crypto_market_cycles(&query, &markets).is_empty(),
        "unverified 100% spreads and bank-money transfers must not be offered as crypto arbitrage"
    );
}
