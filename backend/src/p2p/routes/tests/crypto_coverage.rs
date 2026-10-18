use super::*;
use std::sync::Mutex;

#[test]
fn implicit_market_search_discovers_shared_markets_outside_the_default_asset_list() {
    let tickers = vec![
        CryptoTicker {
            symbol: "ASTRETH".into(),
            bid: 0.0001,
            ask: 0.00011,
        },
        CryptoTicker {
            symbol: "ASTRBTC".into(),
            bid: 0.000003,
            ask: 0.000004,
        },
        CryptoTicker {
            symbol: "ETHUSD".into(),
            bid: 2400.0,
            ask: 2401.0,
        },
        CryptoTicker {
            symbol: "BTCUSD".into(),
            bid: 80000.0,
            ask: 80001.0,
        },
    ];
    let mut request = crypto_cycle_request(0.05);
    request.source_fiat = "ETH".into();
    request.target_fiat = "BTC".into();
    request.source_network = Some("ethereum".into());
    request.target_network = Some("bitcoin".into());
    request.intermediary_assets = None;
    let networks = crate::networks::NetworkCatalog::test_default();
    let query = normalize_query(request.clone(), &["USDT".into()], &networks, &[]).unwrap();
    let mut routes = Vec::new();
    compose_crypto_market_routes(&mut routes, &query, "gate", "ETH", "BTC", &tickers);
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].bridge_currency.as_deref(), Some("ASTR"));

    request.intermediary_assets = Some("USDT".into());
    let query = normalize_query(request, &["USDT".into()], &networks, &[]).unwrap();
    let mut routes = Vec::new();
    compose_crypto_market_routes(&mut routes, &query, "gate", "ETH", "BTC", &tickers);
    assert!(
        routes.is_empty(),
        "explicit intermediate selection must still restrict search"
    );
}

#[test]
fn gram_spot_routes_accept_legacy_ton_requests_without_renaming_the_network() {
    let tickers = vec![
        CryptoTicker {
            symbol: "ETHUSDT".into(),
            bid: 2400.0,
            ask: 2401.0,
        },
        CryptoTicker {
            symbol: "GRAMUSDT".into(),
            bid: 1.2,
            ask: 1.25,
        },
    ];
    for legacy_or_current in ["TON", "GRAM"] {
        let mut request = crypto_cycle_request(1.0);
        request.source_fiat = "ETH".into();
        request.target_fiat = legacy_or_current.into();
        request.source_network = Some("ethereum".into());
        request.target_network = Some("ton".into());
        request.intermediary_assets = Some("USDT".into());
        let query = normalize_query(
            request,
            &["USDT".into()],
            &crate::networks::NetworkCatalog::test_default(),
            &[],
        )
        .unwrap();
        assert_eq!(query.target_currency, "GRAM");
        assert_eq!(query.target_network.as_deref(), Some("ton"));
        let mut routes = Vec::new();
        compose_crypto_market_routes(
            &mut routes,
            &query,
            "binance",
            "ETH",
            &query.target_currency,
            &tickers,
        );
        assert_eq!(
            routes.len(),
            1,
            "missing market route for {legacy_or_current}"
        );
        assert_eq!(routes[0].target_fiat, "GRAM");
        assert_eq!(routes[0].target_amount, "1916.161920000000");
        assert_eq!(routes[0].target_network.as_deref(), Some("ton"));
    }
}

struct AlternativesProvider(DirectedProvider);

#[async_trait]
impl PublicRouteProvider for AlternativesProvider {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        self.0.supports_pair(from, to)
    }
    async fn supported_assets(&self) -> Vec<Asset> {
        self.0.supported_assets().await
    }
    async fn quote(&self, from: Asset, to: Asset, amount: Amount) -> Result<PublicRouteQuote> {
        self.0.quote(from, to, amount).await
    }
    async fn quotes(
        &self,
        from: Asset,
        to: Asset,
        amount: Amount,
    ) -> Result<Vec<PublicRouteQuote>> {
        let mut first = self.quote(from, to, amount).await?;
        first.quote_id = Some("exchanger-one".into());
        let mut second = first.clone();
        second.quote_id = Some("exchanger-two".into());
        Ok(vec![first, second])
    }
}

struct DirectedProvider {
    name: &'static str,
    from: Asset,
    to: Asset,
    rate: f64,
    inputs: Arc<Mutex<Vec<Amount>>>,
}

#[async_trait]
impl PublicRouteProvider for DirectedProvider {
    fn name(&self) -> &str {
        self.name
    }
    fn supports_pair(&self, from: &Asset, to: &Asset) -> bool {
        *from == self.from && *to == self.to
    }
    async fn supported_assets(&self) -> Vec<Asset> {
        vec![self.from.clone(), self.to.clone()]
    }
    async fn quote(&self, from: Asset, to: Asset, input: Amount) -> Result<PublicRouteQuote> {
        assert!(self.supports_pair(&from, &to));
        self.inputs.lock().unwrap().push(input.clone());
        Ok(PublicRouteQuote {
            provider: self.name.into(),
            quote_id: Some(format!("{}-{}", self.name, input.value)),
            description: None,
            source_url: Some(format!("https://{}.example", self.name)),
            from: from.clone(),
            to: to.clone(),
            output: Amount::from_f64(input.value.parse::<f64>()? * self.rate, to.clone())?,
            input,
            fees: Vec::new(),
            expires_at: None,
            path: vec![from, to],
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn finds_btc_to_ever_through_an_exact_shared_token_and_network() {
    assert!(same_crypto_amount("00100.000", "100"));
    assert!(!same_crypto_amount(
        "0.100000000000000001",
        "0.100000000000000002"
    ));
    let btc = Asset::parse("BTC@bitcoin").unwrap();
    let middle = Asset::parse("USDC@ethereum").unwrap();
    let target = Asset::parse("EVER@ethereum").unwrap();
    let exit_inputs = Arc::new(Mutex::new(Vec::new()));
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![
            Arc::new(DirectedProvider {
                name: "entry-swap",
                from: btc,
                to: middle.clone(),
                rate: 100_000.0,
                inputs: Default::default(),
            }),
            Arc::new(DirectedProvider {
                name: "exit-swap",
                from: middle.clone(),
                to: target,
                rate: 1_000.0,
                inputs: exit_inputs.clone(),
            }),
        ]);
    let mut request = crypto_cycle_request(0.001);
    request.source_fiat = "BTC".into();
    request.source_network = Some("bitcoin".into());
    request.target_fiat = "EVER".into();
    request.target_network = Some("ethereum".into());
    request.allow_cross_venue = Some(true);
    request.sources = None;
    request.intermediary_assets = Some("USDC".into());
    request.limit = Some(1);
    let response = service.search_routes(request.clone()).await.unwrap();
    assert_eq!(response.routes_found, 1);
    assert!(!response.routes_exhaustive);
    let route = &response.routes[0];
    assert_eq!(route.route_kind, "crypto_to_crypto");
    assert_eq!(route.asset, "USDC");
    assert_eq!(
        route.route_path,
        ["BTC@bitcoin", "USDC@ethereum", "EVER@ethereum"]
    );
    assert_eq!(route.target_amount.parse::<f64>().unwrap(), 100_000.0);
    assert_eq!(route.cycle_legs.len(), 2);
    assert!(route.execution.is_none());
    assert!(!route.transfer_fee_included);
    let inputs = exit_inputs.lock().unwrap().clone();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].asset, middle);
    assert_eq!(inputs[0].value.parse::<f64>().unwrap(), 100.0);

    request.allow_cross_venue = Some(false);
    assert_eq!(
        service
            .search_routes(request.clone())
            .await
            .unwrap()
            .routes_found,
        0
    );
    request.allow_cross_venue = Some(true);
    request.sources = Some("entry-swap".into());
    assert_eq!(
        service.search_routes(request).await.unwrap().routes_found,
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fresh_direct_quotes_preserve_exchangers_when_a_background_coefficient_exists() {
    let from = Asset::parse("BTC@bitcoin").unwrap();
    let to = Asset::parse("EVER@ethereum").unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![Arc::new(AlternativesProvider(DirectedProvider {
            name: "alternatives",
            from: from.clone(),
            to: to.clone(),
            rate: 100_000.0,
            inputs: calls.clone(),
        }))]);
    service.provider_coefficient_cache.write().unwrap().insert(
        provider_coefficient_key("alternatives", &from, &to),
        crate::p2p::service::CachedProviderCoefficient {
            inserted_at: std::time::Instant::now(),
            output_per_input: Some(999_999.0),
        },
    );
    let mut request = crypto_cycle_request(0.001);
    request.source_fiat = "BTC".into();
    request.source_network = Some("bitcoin".into());
    request.target_fiat = "EVER".into();
    request.target_network = Some("ethereum".into());
    request.limit = Some(1);
    let response = service.search_routes(request).await.unwrap();
    assert_eq!(response.routes_found, 2);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(
        response.routes[0].target_amount.parse::<f64>().unwrap(),
        100.0
    );
    assert!(response.routes[0].provider_quote_id.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn does_not_join_the_same_intermediate_symbol_on_different_networks() {
    let service = P2pSearchService::with_sources(Vec::new(), Duration::from_secs(1))
        .with_route_providers(vec![
            Arc::new(DirectedProvider {
                name: "entry-swap",
                from: Asset::parse("BTC@bitcoin").unwrap(),
                to: Asset::parse("USDC@base").unwrap(),
                rate: 100_000.0,
                inputs: Default::default(),
            }),
            Arc::new(DirectedProvider {
                name: "exit-swap",
                from: Asset::parse("USDC@ethereum").unwrap(),
                to: Asset::parse("EVER@ethereum").unwrap(),
                rate: 1_000.0,
                inputs: Default::default(),
            }),
        ]);
    let mut request = crypto_cycle_request(0.001);
    request.source_fiat = "BTC".into();
    request.source_network = Some("bitcoin".into());
    request.target_fiat = "EVER".into();
    request.target_network = Some("ethereum".into());
    request.allow_cross_venue = Some(true);
    request.sources = None;
    assert_eq!(
        service.search_routes(request).await.unwrap().routes_found,
        0
    );
}
