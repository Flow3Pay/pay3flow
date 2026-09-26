use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use playwright_rs::{LaunchOptions, Page, Playwright};
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::p2p::service::{
    Advertiser, P2pOffer, P2pOfferMarket, P2pSearchQuery, P2pSide, P2pSource,
};
use crate::provider_adapter::{WorkflowConfig, WorkflowOperation, WorkflowRead, WorkflowStep};
use crate::providers::ProviderAdapterRecord;

const MAX_CONCURRENT_BROWSERS: usize = 2;
static BROWSER_SLOTS: OnceLock<Semaphore> = OnceLock::new();

pub(crate) struct WorkflowP2pSource {
    slug: String,
    display_name: String,
    config: WorkflowConfig,
    chromium_executable: Option<String>,
    debug_screenshot: Option<String>,
}

impl WorkflowP2pSource {
    pub(crate) fn from_record(
        record: &ProviderAdapterRecord,
        chromium_executable: Option<String>,
        debug_screenshot: Option<String>,
    ) -> Option<Self> {
        record.workflow.clone().map(|config| Self {
            slug: record.slug.clone(),
            display_name: record.display_name.clone(),
            config,
            chromium_executable,
            debug_screenshot,
        })
    }

    fn operation(&self, side: P2pSide) -> Result<&WorkflowOperation> {
        match side {
            P2pSide::BuyCrypto => self.config.buy.as_ref(),
            P2pSide::SellCrypto => self.config.sell.as_ref(),
        }
        .ok_or_else(|| anyhow!("{} does not support this workflow operation", self.slug))
    }

    fn template_values<'a>(
        &'a self,
        query: &'a P2pSearchQuery,
        operation: &WorkflowOperation,
    ) -> Result<WorkflowValues<'a>> {
        let asset = self
            .config
            .asset_codes
            .get(&query.asset)
            .map(String::as_str)
            .unwrap_or(&query.asset);
        let amount = match operation.amount_mode.as_str() {
            "query_or_empty" => query.amount,
            "fiat_probe" => query.amount.or(self.config.fiat_probe_amount),
            "asset_probe" => self.config.asset_probe_amount,
            mode => bail!("unsupported workflow amount mode `{mode}`"),
        };
        Ok(WorkflowValues {
            fiat: &query.fiat,
            asset,
            amount,
        })
    }

    async fn run_workflow(
        &self,
        operation: &WorkflowOperation,
        values: &WorkflowValues<'_>,
    ) -> Result<(f64, f64)> {
        let _slot = BROWSER_SLOTS
            .get_or_init(|| Semaphore::new(MAX_CONCURRENT_BROWSERS))
            .acquire()
            .await
            .context("Playwright browser limiter is closed")?;
        let playwright = Playwright::launch()
            .await
            .map_err(|error| anyhow!("cannot initialize playwright-rs: {error}"))?;
        let browser_type = playwright.chromium();
        let mut launch_options = LaunchOptions::new().headless(true).args(vec![
            "--disable-dev-shm-usage".into(),
            "--no-sandbox".into(),
        ]);
        if let Some(executable) = &self.chromium_executable {
            launch_options = launch_options.executable_path(executable.clone());
        }
        let browser = browser_type
            .launch_with_options(launch_options)
            .await
            .map_err(|error| anyhow!("cannot launch Chromium: {error}"))?;
        let result = async {
            let page = browser
                .new_page()
                .await
                .map_err(|error| anyhow!("cannot create browser page: {error}"))?;
            let failed_requests = Arc::new(Mutex::new(Vec::new()));
            let failed_requests_for_handler = Arc::clone(&failed_requests);
            page.on_request_failed(move |request| {
                let failed_requests = Arc::clone(&failed_requests_for_handler);
                async move {
                    let mut failed_requests =
                        failed_requests.lock().expect("request log mutex poisoned");
                    if failed_requests.len() < 8 {
                        failed_requests.push(format!(
                            "{}: {}",
                            request.url().split('?').next().unwrap_or(request.url()),
                            request.failure().unwrap_or_else(|| "unknown error".into())
                        ));
                    }
                    Ok(())
                }
            })
            .await
            .map_err(|error| anyhow!("cannot observe workflow requests: {error}"))?;
            page.set_default_timeout(self.config.timeout_ms as f64)
                .await;
            page.set_default_navigation_timeout(self.config.timeout_ms as f64)
                .await;
            page.goto(&self.config.get_exchange, None)
                .await
                .map_err(|error| anyhow!("workflow navigation failed: {error}"))?;
            for _ in 0..self.config.navigation_retries {
                tokio::time::sleep(Duration::from_millis(
                    self.config.navigation_retry_delay_ms,
                ))
                .await;
                page.reload(None)
                    .await
                    .map_err(|error| anyhow!("workflow navigation retry failed: {error}"))?;
                tokio::time::sleep(Duration::from_millis(
                    self.config.navigation_retry_delay_ms,
                ))
                .await;
            }
            for step in &operation.steps {
                if let Err(error) = execute_step(&page, step, values).await {
                    if let Some(path) = &self.debug_screenshot {
                        let _ = page.screenshot_to_file(std::path::Path::new(&path), None).await;
                    }
                    let page_url = page.url();
                    let page_errors = page.page_errors().join(" | ");
                    let failed_requests = failed_requests
                        .lock()
                        .expect("request log mutex poisoned")
                        .join(" | ");
                    return Err(error).with_context(|| {
                        format!(
                            "workflow page is `{page_url}`; page errors: `{page_errors}`; failed requests: `{failed_requests}`"
                        )
                    });
                }
            }
            let fiat_amount = read_number(&page, &operation.fiat_amount).await?;
            let asset_amount = read_number(&page, &operation.asset_amount).await?;
            Ok((fiat_amount, asset_amount))
        }
        .await;
        let close_result = browser
            .close()
            .await
            .map_err(|error| anyhow!("cannot close Chromium: {error}"));
        match (result, close_result) {
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Ok(amounts), Ok(())) => Ok(amounts),
        }
    }
}

#[async_trait]
impl P2pSource for WorkflowP2pSource {
    fn name(&self) -> &str {
        &self.slug
    }

    fn timeout(&self, _default: Duration) -> Duration {
        Duration::from_millis(self.config.timeout_ms)
    }

    async fn search(&self, query: &P2pSearchQuery) -> Result<Vec<P2pOffer>> {
        if !self.config.supported_assets.is_empty()
            && !self
                .config
                .supported_assets
                .iter()
                .any(|asset| asset == &query.asset)
        {
            return Ok(Vec::new());
        }
        if query.amount.is_some_and(|amount| {
            amount < self.config.default_min_fiat || amount > self.config.default_max_fiat
        }) {
            return Ok(Vec::new());
        }
        let operation = self.operation(query.side)?;
        let values = self.template_values(query, operation)?;
        let (fiat_amount, asset_amount) = self.run_workflow(operation, &values).await?;
        let price = fiat_amount / asset_amount;
        if !price.is_finite() || price <= 0.0 {
            bail!("{} workflow returned an invalid exchange rate", self.slug);
        }
        Ok(vec![P2pOffer {
            market: P2pOfferMarket::DirectExchange,
            source: self.slug.clone(),
            ad_id: format!(
                "{}-{}-{}-{}",
                self.slug,
                side_name(query.side),
                query.fiat,
                query.asset
            ),
            side: query.side,
            fiat: query.fiat.clone(),
            asset: query.asset.clone(),
            price: price.to_string(),
            available_asset: self.config.default_available_asset.to_string(),
            min_fiat: self.config.default_min_fiat.to_string(),
            max_fiat: self.config.default_max_fiat.to_string(),
            payment_methods: Vec::new(),
            pay_time_limit_minutes: None,
            advertiser: Advertiser {
                id: None,
                nickname: self.display_name.clone(),
                user_type: Some("service".into()),
                is_merchant: self.config.is_merchant,
                is_verified: self.config.is_verified,
                completed_orders_30d: None,
                completion_rate_30d: None,
                positive_rate: None,
            },
            advertiser_profile_url: None,
            source_url: self.config.source_url.clone(),
            source_url_is_exact: false,
        }])
    }
}

async fn execute_step(page: &Page, step: &WorkflowStep, values: &WorkflowValues<'_>) -> Result<()> {
    match step {
        WorkflowStep::Fill { selector, value } => page
            .locator(selector)
            .fill(&render(value, values), None)
            .await
            .map_err(|error| anyhow!("workflow fill `{selector}` failed: {error}")),
        WorkflowStep::Click { selector } => page
            .locator(selector)
            .click(None)
            .await
            .map_err(|error| anyhow!("workflow click `{selector}` failed: {error}")),
        WorkflowStep::Press { selector, key } => page
            .locator(selector)
            .press(key, None)
            .await
            .map_err(|error| anyhow!("workflow key press on `{selector}` failed: {error}")),
        WorkflowStep::SelectOption { selector, value } => page
            .locator(selector)
            .select_option(render(value, values), None)
            .await
            .map(|_| ())
            .map_err(|error| anyhow!("workflow select `{selector}` failed: {error}")),
        WorkflowStep::ReactSelect { selector, value } => {
            let value = render(value, values);
            page.locator(selector)
                .click(None)
                .await
                .map_err(|error| anyhow!("workflow React select `{selector}` failed: {error}"))?;
            let options = page.locator("[role=option]");
            options.first().wait_for(None).await.map_err(|error| {
                anyhow!("workflow options for `{selector}` did not open: {error}")
            })?;
            let option_texts = options
                .all_text_contents()
                .await
                .map_err(|error| anyhow!("cannot read workflow options: {error}"))?;
            let index = option_texts
                .iter()
                .position(|text| text.trim() == value || text.contains(&value))
                .ok_or_else(|| {
                    anyhow!(
                        "workflow option `{value}` was not found; available options: {option_texts:?}"
                    )
                })?;
            options
                .nth(index as i32)
                .click(None)
                .await
                .map_err(|error| {
                    anyhow!("workflow option `{value}` could not be selected: {error}")
                })?;
            Ok(())
        }
        WorkflowStep::WaitFor { selector } => page
            .locator(selector)
            .wait_for(None)
            .await
            .map(|_| ())
            .map_err(|error| anyhow!("workflow wait for `{selector}` failed: {error}")),
        WorkflowStep::Wait { milliseconds } => {
            tokio::time::sleep(Duration::from_millis(*milliseconds)).await;
            Ok(())
        }
        WorkflowStep::Evaluate { script, value } => {
            let value = value
                .as_deref()
                .map(|value| Value::String(render(value, values)))
                .unwrap_or(Value::Null);
            page.evaluate::<_, Value>(script, Some(&value))
                .await
                .map(|_| ())
                .map_err(|error| anyhow!("workflow JavaScript failed: {error}"))
        }
    }
}

async fn read_number(page: &Page, read: &WorkflowRead) -> Result<f64> {
    let value = if read.property == "value" {
        page.locator(&read.selector)
            .input_value(None)
            .await
            .map_err(|error| anyhow!("cannot read value from `{}`: {error}", read.selector))?
    } else if read.property == "text" {
        page.locator(&read.selector)
            .text_content()
            .await
            .map_err(|error| anyhow!("cannot read text from `{}`: {error}", read.selector))?
            .unwrap_or_default()
    } else {
        let attribute = read
            .property
            .strip_prefix("attribute:")
            .expect("validated workflow read property");
        page.locator(&read.selector)
            .get_attribute(attribute)
            .await
            .map_err(|error| {
                anyhow!(
                    "cannot read `{attribute}` from `{}`: {error}",
                    read.selector
                )
            })?
            .unwrap_or_default()
    };
    parse_localized_number(&value)
        .with_context(|| format!("invalid numeric workflow result `{value}`"))
}

struct WorkflowValues<'a> {
    fiat: &'a str,
    asset: &'a str,
    amount: Option<f64>,
}

fn render(template: &str, values: &WorkflowValues<'_>) -> String {
    template
        .replace("{{fiat}}", values.fiat)
        .replace("{{asset}}", values.asset)
        .replace(
            "{{amount}}",
            &values
                .amount
                .map(|value| value.to_string())
                .unwrap_or_default(),
        )
}

fn parse_localized_number(value: &str) -> Result<f64> {
    let mut value = value
        .trim()
        .replace(['\u{00a0}', ' '], "")
        .replace(',', ".");
    if value.matches('.').count() > 1 {
        let decimal = value.rfind('.').expect("at least one decimal separator");
        value = value
            .chars()
            .enumerate()
            .filter_map(|(index, character)| {
                (character != '.' || index == decimal).then_some(character)
            })
            .collect();
    }
    let numeric = value
        .chars()
        .take_while(|character| {
            character.is_ascii_digit() || matches!(character, '.' | '-' | '+' | 'e' | 'E')
        })
        .collect::<String>();
    numeric
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| anyhow!("not a positive number"))
}

fn side_name(side: P2pSide) -> &'static str {
    match side {
        P2pSide::BuyCrypto => "buy",
        P2pSide::SellCrypto => "sell",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::routes::P2pRouteSearchQuery;
    use crate::p2p::service::P2pSearchService;

    fn whitebird_source() -> WorkflowP2pSource {
        let document: toml::Value =
            toml::from_str(include_str!("../../providers/whitebird/Providerfile")).unwrap();
        let workflow: WorkflowConfig = document
            .get("workflow")
            .unwrap()
            .clone()
            .try_into()
            .unwrap();
        let record = ProviderAdapterRecord {
            slug: "whitebird".into(),
            source_url: "https://whitebird.io/".into(),
            display_name: "Whitebird".into(),
            config: None,
            workflow: Some(workflow),
        };
        WorkflowP2pSource::from_record(&record, None, None).unwrap()
    }

    #[test]
    fn parses_decimal_comma_and_grouped_numbers() {
        assert_eq!(parse_localized_number("1,416969 TRX").unwrap(), 1.416969);
        assert_eq!(
            parse_localized_number("1 190 654,07").unwrap(),
            1_190_654.07
        );
    }

    #[tokio::test]
    #[ignore = "calls the live Whitebird website and requires Chromium"]
    async fn live_whitebird_workflow_returns_buy_and_sell_quotes() {
        let source = whitebird_source();
        for side in [P2pSide::BuyCrypto, P2pSide::SellCrypto] {
            let offers = source
                .search(&P2pSearchQuery {
                    fiat: "RUB".into(),
                    asset: "TRX".into(),
                    side,
                    amount: Some(1_000.0),
                    payment_method: None,
                    merchant_only: None,
                    min_orders: None,
                    min_completion_rate: None,
                    limit: Some(1),
                    sources: Some("whitebird".into()),
                })
                .await
                .unwrap();

            assert_eq!(offers.len(), 1);
            assert_eq!(offers[0].side, side);
            assert!(offers[0].price.parse::<f64>().unwrap() > 0.0);
        }
    }

    #[tokio::test]
    #[ignore = "calls the live Whitebird website and requires Chromium"]
    async fn live_whitebird_builds_user_selected_usdc_to_sberbank_route() {
        let service = P2pSearchService::with_sources(
            vec![Arc::new(whitebird_source())],
            Duration::from_secs(30),
        );
        let response = service
            .search_routes(P2pRouteSearchQuery {
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
                merchant_only: None,
                min_orders: Some(20),
                min_completion_rate: Some(0.9),
                allow_cross_venue: None,
                max_price_deviation_bps: None,
                limit: Some(10),
                sources: Some("whitebird".into()),
            })
            .await
            .unwrap();

        assert_eq!(
            response.routes_found,
            1,
            "{}",
            serde_json::to_string_pretty(&response).unwrap()
        );
        let route = &response.routes[0];
        assert_eq!(route.asset, "USDC");
        assert_eq!(route.target_fiat, "RUB");
        assert_eq!(route.route_kind, "crypto_to_fiat");
        assert!(route.target_amount.parse::<f64>().unwrap() > 0.0);
        assert_eq!(route.exit_offer.as_ref().unwrap().source, "whitebird");
        assert_eq!(route.source_network.as_deref(), Some("ethereum"));
        assert!(!route.payment_methods_verified);
    }
}
