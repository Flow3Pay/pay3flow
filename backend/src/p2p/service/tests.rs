use super::*;

#[test]
fn source_names_accept_providerfile_slug_characters() {
    assert_eq!(
        normalize_sources(Some("Cifra-Broker,foo_bar".into()))
            .expect("Providerfile slugs should be valid source names")
            .as_deref(),
        Some("cifra-broker,foo_bar")
    );
}

struct StubSource {
    name: &'static str,
    offers: Vec<P2pOffer>,
    delay: Duration,
}

struct DirectStubSource(StubSource);

struct StubRouteProvider;

#[async_trait]
impl PublicRouteProvider for StubRouteProvider {
    fn name(&self) -> &str {
        "test-intents"
    }

    async fn quote(
        &self,
        _from: crate::route_engine::Asset,
        _to: crate::route_engine::Asset,
        _amount: crate::route_engine::Amount,
    ) -> Result<crate::route_engine::PublicRouteQuote> {
        bail!("route quoting is not used by this test")
    }
}

#[async_trait]
impl P2pSource for StubSource {
    fn name(&self) -> &str {
        self.name
    }

    async fn search(&self, _query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        tokio::time::sleep(self.delay).await;
        Ok(self.offers.clone())
    }
}

#[async_trait]
impl P2pSource for DirectStubSource {
    fn name(&self) -> &str {
        self.0.name
    }

    fn market(&self) -> P2pOfferMarket {
        P2pOfferMarket::DirectExchange
    }

    async fn search(&self, _query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        Ok(self.0.offers.clone())
    }
}

fn offer(source: &str, price: &str, min: &str, max: &str, orders: u64) -> P2pOffer {
    P2pOffer {
        market: P2pOfferMarket::P2p,
        source: source.into(),
        ad_id: format!("{source}-{price}"),
        side: P2pSide::BuyCrypto,
        fiat: "AMD".into(),
        asset: "USDT".into(),
        network: None,
        price: price.into(),
        available_asset: "1000".into(),
        min_fiat: min.into(),
        max_fiat: max.into(),
        payment_methods: vec!["IDBank".into()],
        pay_time_limit_minutes: Some(15),
        advertiser: Advertiser {
            id: None,
            nickname: source.into(),
            user_type: Some("merchant".into()),
            is_merchant: true,
            is_verified: true,
            completed_orders_30d: Some(orders),
            completion_rate_30d: Some(0.99),
            positive_rate: Some(1.0),
        },
        advertiser_profile_url: None,
        source_url: "https://example.test".into(),
        source_url_is_exact: false,
    }
}

#[test]
fn parses_p2p_offer_candidates_from_fmatch() {
    let expected = offer("binance", "390.5", "100", "100000", 42);
    let reply = serde_json::json!({
        "candidates": [{
            "name": "binance-ad",
            "p2pOffer": serde_json::to_value(&expected).unwrap()
        }]
    });

    let parsed = P2pOffer::from_fmatch_reply(&reply, Some(P2pOfferMarket::P2p));

    assert_eq!(parsed, vec![expected]);
}

#[test]
fn all_market_fmatch_reply_preserves_each_offer_market() {
    let expected =
        offer("whitebird", "91.5", "10", "10000", 7).with_market(P2pOfferMarket::DirectExchange);
    let reply = serde_json::json!({
        "candidates": [{
            "name": "whitebird-offer",
            "attachment": [{
                "type": "PropertyValue",
                "name": "example:offer",
                "value": serde_json::to_value(&expected).unwrap()
            }]
        }]
    });

    let parsed = P2pOffer::from_fmatch_reply(&reply, None);

    assert_eq!(parsed, vec![expected]);
}

#[test]
fn all_market_query_uses_a_generic_wildcard_fact() {
    let query = P2pSearchQuery {
        fiat: "RUB".into(),
        asset: "USDT".into(),
        side: P2pSide::SellCrypto,
        amount: Some(100.0),
        payment_method: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        limit: Some(20),
        sources: None,
    };

    assert!(fmatch_p2p_content(&query, None).contains("market=all"));
}

#[test]
fn cached_p2p_answer_round_trips_with_market_restored_by_query() {
    let response = P2pSearchResponse {
        query: P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(1000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: None,
        },
        searched_at: Utc::now(),
        cached: false,
        offers: vec![offer("binance", "390.5", "100", "100000", 42)],
        sources: Vec::new(),
        source: "fmatch".into(),
        stale: false,
        observed_at: Some(Utc::now()),
    };
    let value = serde_json::to_value(&response).unwrap();
    let decoded: P2pSearchResponse = serde_json::from_value(value).unwrap();

    assert_eq!(decoded.offers.len(), 1);
    assert_eq!(decoded.offers[0].source, "binance");
    assert_eq!(decoded.source, "fmatch");
}

#[test]
fn provider_fallback_does_not_expose_fmatch_as_a_venue() {
    let response = build_search_response(
        P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(1000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: None,
        },
        &[offer("binance", "390.5", "100", "100000", 42)],
        vec![SourceStatus {
            source: "binance".into(),
            ok: true,
            cached: false,
            latency_ms: 10,
            offers_found: 1,
            error: None,
        }],
        false,
        "provider",
        false,
        Some(Utc::now()),
    );

    let fallback = mark_provider_fallback(response, "no active P2P offers");

    assert_eq!(fallback.source, "provider_fallback");
    assert_eq!(fallback.sources.len(), 1);
    assert_eq!(fallback.sources[0].source, "binance");
    assert!(fallback.sources[0].ok);
}

#[test]
fn fmatch_offer_statuses_name_the_actual_venues() {
    let offers = vec![
        offer("bybit", "390.5", "100", "100000", 42),
        offer("bybit", "391.0", "100", "100000", 43),
        offer("whitebird", "392.0", "100", "100000", 44),
    ];

    let statuses = source_statuses_from_offers(&offers);

    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[0].source, "bybit");
    assert_eq!(statuses[0].offers_found, 2);
    assert_eq!(statuses[1].source, "whitebird");
    assert_eq!(statuses[1].offers_found, 1);
    assert!(statuses.iter().all(|status| status.source != "fmatch"));
}

#[test]
fn empty_fmatch_response_is_not_usable() {
    let response = build_search_response(
        P2pSearchQuery {
            fiat: "RUB".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(10_000.0),
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: Some(20),
            sources: None,
        },
        &[],
        Vec::new(),
        false,
        "fmatch",
        false,
        Some(Utc::now()),
    );

    assert!(!usable_fmatch_response(&response));
}

#[test]
fn reputation_filters_keep_offers_when_metrics_are_not_published() {
    let query = P2pSearchQuery {
        fiat: "RUB".into(),
        asset: "USDC".into(),
        side: P2pSide::SellCrypto,
        amount: Some(100.0),
        payment_method: Some("Sberbank".into()),
        merchant_only: None,
        min_orders: Some(20),
        min_completion_rate: Some(0.9),
        limit: Some(10),
        sources: Some("whitebird".into()),
    };
    let mut direct_exchange = offer("whitebird", "84.7", "1", "100000", 100);
    direct_exchange.side = P2pSide::SellCrypto;
    direct_exchange.fiat = "RUB".into();
    direct_exchange.asset = "USDC".into();
    direct_exchange.payment_methods.clear();
    direct_exchange.advertiser.completed_orders_30d = None;
    direct_exchange.advertiser.completion_rate_30d = None;

    assert!(direct_exchange.matches(&query));

    direct_exchange.advertiser.completed_orders_30d = Some(19);
    assert!(!direct_exchange.matches(&query));

    direct_exchange.advertiser.completed_orders_30d = None;
    direct_exchange.advertiser.completion_rate_30d = Some(0.89);
    assert!(!direct_exchange.matches(&query));
}

#[test]
fn fmatch_boundary_rejects_wrong_pair_side_and_unrequested_source() {
    let query = P2pSearchQuery {
        fiat: "AMD".into(),
        asset: "USDT".into(),
        side: P2pSide::BuyCrypto,
        amount: Some(100_000.0),
        payment_method: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        limit: Some(20),
        sources: Some("binance".into()),
    };
    let valid = offer("binance", "360", "1000", "200000", 10);
    let mut wrong_fiat = valid.clone();
    wrong_fiat.fiat = "RUB".into();
    let mut wrong_asset = valid.clone();
    wrong_asset.asset = "USDC".into();
    let mut wrong_side = valid.clone();
    wrong_side.side = P2pSide::SellCrypto;
    let mut wrong_source = valid.clone();
    wrong_source.source = "mexc".into();

    let response = build_search_response(
        query,
        &[
            valid.clone(),
            wrong_fiat,
            wrong_asset,
            wrong_side,
            wrong_source,
        ],
        Vec::new(),
        false,
        "fmatch",
        false,
        Some(Utc::now()),
    );

    assert_eq!(response.offers, vec![valid]);
}

#[test]
fn global_limit_keeps_the_best_offer_from_each_source() {
    let query = P2pSearchQuery {
        fiat: "RUB".into(),
        asset: "USDC".into(),
        side: P2pSide::SellCrypto,
        amount: None,
        payment_method: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        limit: Some(60),
        sources: Some("bybit,whitebird".into()),
    };
    let mut offers = (0..60)
        .map(|index| {
            let mut offer = offer(
                "bybit",
                &format!("{}", 90.0 - f64::from(index) / 100.0),
                "1",
                "100000",
                100,
            );
            offer.side = P2pSide::SellCrypto;
            offer.fiat = "RUB".into();
            offer.asset = "USDC".into();
            offer
        })
        .collect::<Vec<_>>();
    let mut whitebird = offer("whitebird", "84.7", "1", "100000", 0);
    whitebird.side = P2pSide::SellCrypto;
    whitebird.fiat = "RUB".into();
    whitebird.asset = "USDC".into();
    whitebird.payment_methods.clear();
    whitebird.advertiser.completed_orders_30d = None;
    whitebird.advertiser.completion_rate_30d = None;
    offers.push(whitebird);

    let response = build_search_response(
        query,
        &offers,
        Vec::new(),
        false,
        "provider",
        false,
        Some(Utc::now()),
    );

    assert_eq!(response.offers.len(), 60);
    assert!(response
        .offers
        .iter()
        .any(|offer| offer.source == "whitebird"));
}

#[test]
fn merging_market_partitions_keeps_a_lower_ranked_direct_source() {
    let query = P2pSearchQuery {
        fiat: "AMD".into(),
        asset: "USDT".into(),
        side: P2pSide::BuyCrypto,
        amount: Some(10_000.0),
        payment_method: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        limit: Some(5),
        sources: None,
    };
    let p2p_offers = (0..5)
        .map(|index| offer("binance", &format!("{}", 390 + index), "100", "100000", 42))
        .collect::<Vec<_>>();
    let direct_offers =
        vec![offer("whitebird", "500", "100", "100000", 0)
            .with_market(P2pOfferMarket::DirectExchange)];
    let p2p = build_search_response(
        query.clone(),
        &p2p_offers,
        source_statuses_from_offers(&p2p_offers),
        false,
        "fmatch",
        false,
        Some(Utc::now()),
    );
    let direct = build_search_response(
        query.clone(),
        &direct_offers,
        source_statuses_from_offers(&direct_offers),
        false,
        "fmatch",
        false,
        Some(Utc::now()),
    );

    let response = merge_market_responses(query, p2p, direct);

    assert_eq!(response.offers.len(), 5);
    assert!(
        response
            .offers
            .iter()
            .any(|offer| offer.source == "whitebird"
                && offer.market == P2pOfferMarket::DirectExchange)
    );
}

#[tokio::test]
async fn market_filter_only_queries_matching_sources() {
    let mut direct_offer = offer("direct", "361", "1", "100000", 0);
    direct_offer.market = P2pOfferMarket::DirectExchange;
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(StubSource {
                name: "p2p",
                offers: vec![offer("p2p", "360", "1", "100000", 0)],
                delay: Duration::ZERO,
            }),
            Arc::new(DirectStubSource(StubSource {
                name: "direct",
                offers: vec![direct_offer],
                delay: Duration::ZERO,
            })),
        ],
        Duration::from_secs(1),
    );
    let query = P2pSearchQuery {
        fiat: "AMD".into(),
        asset: "USDT".into(),
        side: P2pSide::BuyCrypto,
        amount: None,
        payment_method: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        limit: Some(10),
        sources: None,
    };

    let p2p = service
        .search_market(query.clone(), Some(P2pOfferMarket::P2p))
        .await
        .unwrap();
    let direct = service
        .search_market(query, Some(P2pOfferMarket::DirectExchange))
        .await
        .unwrap();

    assert_eq!(p2p.sources.len(), 1);
    assert_eq!(p2p.sources[0].source, "p2p");
    assert_eq!(direct.sources.len(), 1);
    assert_eq!(direct.sources[0].source, "direct");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fans_out_filters_and_sorts() {
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(StubSource {
                name: "one",
                offers: vec![offer("one", "362", "10000", "200000", 100)],
                delay: Duration::from_millis(30),
            }),
            Arc::new(StubSource {
                name: "two",
                offers: vec![
                    offer("two", "360", "10000", "200000", 200),
                    offer("too-small", "350", "100", "1000", 200),
                ],
                delay: Duration::from_millis(30),
            }),
        ],
        Duration::from_secs(1),
    );
    let started = Instant::now();
    let response = service
        .search(P2pSearchQuery {
            fiat: "amd".into(),
            asset: "usdt".into(),
            side: P2pSide::BuyCrypto,
            amount: Some(50_000.0),
            payment_method: None,
            merchant_only: Some(true),
            min_orders: Some(50),
            min_completion_rate: Some(0.95),
            limit: Some(10),
            sources: None,
        })
        .await
        .unwrap();

    assert!(started.elapsed() < Duration::from_millis(55));
    assert_eq!(response.offers.len(), 2);
    assert_eq!(response.offers[0].source, "two");
    assert!(response.sources.iter().all(|source| source.ok));
}

#[tokio::test]
async fn times_out_one_source_without_losing_other_results() {
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(StubSource {
                name: "fast",
                offers: vec![offer("fast", "360", "1", "100000", 20)],
                delay: Duration::ZERO,
            }),
            Arc::new(StubSource {
                name: "slow",
                offers: Vec::new(),
                delay: Duration::from_millis(100),
            }),
        ],
        Duration::from_millis(10),
    );
    let response = service
        .search(P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: None,
            sources: None,
        })
        .await
        .unwrap();

    assert_eq!(response.offers.len(), 1);
    assert_eq!(
        response.sources.iter().filter(|source| source.ok).count(),
        1
    );
}

#[tokio::test]
async fn queries_only_requested_sources() {
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(StubSource {
                name: "one",
                offers: vec![offer("one", "362", "1", "100000", 20)],
                delay: Duration::ZERO,
            }),
            Arc::new(StubSource {
                name: "two",
                offers: vec![offer("two", "360", "1", "100000", 20)],
                delay: Duration::ZERO,
            }),
        ],
        Duration::from_secs(1),
    );

    let response = service
        .search(P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: None,
            sources: Some("TWO".into()),
        })
        .await
        .unwrap();

    assert_eq!(response.sources.len(), 1);
    assert_eq!(response.sources[0].source, "two");
    assert_eq!(response.offers.len(), 1);
    assert_eq!(response.offers[0].source, "two");
}

#[tokio::test]
async fn unavailable_requested_source_returns_an_empty_result() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(StubSource {
            name: "one",
            offers: vec![offer("one", "362", "1", "100000", 20)],
            delay: Duration::ZERO,
        })],
        Duration::from_secs(1),
    );

    let response = service
        .search(P2pSearchQuery {
            fiat: "AMD".into(),
            asset: "USDT".into(),
            side: P2pSide::BuyCrypto,
            amount: None,
            payment_method: None,
            merchant_only: None,
            min_orders: None,
            min_completion_rate: None,
            limit: None,
            sources: Some("whitebird".into()),
        })
        .await
        .expect("an unavailable catalog source must not fail the whole route search");

    assert!(response.offers.is_empty());
    assert!(response.sources.is_empty());
}

#[test]
fn reports_only_configured_search_sources() {
    let service = P2pSearchService::with_sources(
        vec![
            Arc::new(StubSource {
                name: "one",
                offers: Vec::new(),
                delay: Duration::ZERO,
            }),
            Arc::new(StubSource {
                name: "two",
                offers: Vec::new(),
                delay: Duration::ZERO,
            }),
        ],
        Duration::from_secs(1),
    );

    assert_eq!(
        service.searchable_sources(),
        HashSet::from(["one".to_string(), "two".to_string()])
    );
    assert!(!service.searchable_sources().contains("whitebird"));
}

#[test]
fn reports_route_provider_names_separately() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(StubSource {
            name: "one",
            offers: Vec::new(),
            delay: Duration::ZERO,
        })],
        Duration::from_secs(1),
    )
    .with_route_providers(vec![Arc::new(StubRouteProvider)]);

    assert_eq!(
        service.route_provider_names(),
        HashSet::from(["test-intents".to_string()])
    );
    assert!(!service.searchable_sources().contains("test-intents"));
}

#[tokio::test]
async fn reuses_short_lived_search_cache() {
    let service = P2pSearchService::with_sources(
        vec![Arc::new(StubSource {
            name: "cached",
            offers: vec![offer("cached", "360", "1", "100000", 20)],
            delay: Duration::ZERO,
        })],
        Duration::from_secs(1),
    )
    .with_cache_ttl(Duration::from_secs(1));
    let query = P2pSearchQuery {
        fiat: "AMD".into(),
        asset: "USDT".into(),
        side: P2pSide::BuyCrypto,
        amount: None,
        payment_method: None,
        merchant_only: None,
        min_orders: None,
        min_completion_rate: None,
        limit: None,
        sources: None,
    };

    let first = service.search(query.clone()).await.unwrap();
    let second = service.search(query).await.unwrap();
    assert!(!first.cached);
    assert!(second.cached);
    assert_eq!(first.offers, second.offers);
}

#[test]
fn payment_filter_keeps_opaque_ids_but_rejects_known_mismatch() {
    let mut numeric = offer("bybit", "360", "1", "100000", 20);
    numeric.payment_methods = vec!["18".into(), "40".into()];
    assert_eq!(
        numeric.payment_method_match("Sberbank"),
        PaymentMethodMatch::Unknown
    );

    let known = offer("bitget", "360", "1", "100000", 20);
    assert_eq!(
        known.payment_method_match("Ameriabank"),
        PaymentMethodMatch::No
    );
    assert_eq!(
        known.payment_method_match("ID Bank"),
        PaymentMethodMatch::Exact
    );

    let mut cash = offer("skylabs", "360", "1", "100000", 20);
    cash.payment_methods = vec!["SkyLabs ATM".into()];
    assert_eq!(cash.payment_method_match("Cash"), PaymentMethodMatch::No);
    canonicalize_offer_payment_methods(
        &mut cash,
        &BTreeMap::from([("Cash".into(), vec!["SkyLabs ATM".into()])]),
    );
    assert_eq!(cash.payment_method_match("Cash"), PaymentMethodMatch::Exact);

    let mut bncex = offer("bncex", "360", "1", "100000", 20);
    bncex.payment_methods = vec!["CASH".into()];
    assert_eq!(
        bncex.payment_method_match("Cash"),
        PaymentMethodMatch::Exact
    );
    assert_eq!(
        bncex.payment_method_match("Non-cash"),
        PaymentMethodMatch::No
    );
    assert_eq!(
        bncex.payment_method_match("Ameriabank"),
        PaymentMethodMatch::Unknown
    );

    bncex.payment_methods = vec!["NON_CASH".into()];
    assert_eq!(
        bncex.payment_method_match("Non-cash"),
        PaymentMethodMatch::Exact
    );
    assert_eq!(bncex.payment_method_match("Cash"), PaymentMethodMatch::No);
    assert_eq!(
        bncex.payment_method_match("Ameriabank"),
        PaymentMethodMatch::Unknown
    );
}
