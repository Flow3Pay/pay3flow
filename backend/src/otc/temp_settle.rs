//! Settlement of stablecoin temporary wallets.
//!
//! Phase D/E: the payer's EVM stablecoin lands on a platform wallet. The worker
//! pays the agreed net to the real payee (directly on the same network, or
//! through an exact-output NEAR Intents route), withholds the service fee, then
//! recycles the address. Non-EVM sources still use the legacy direct/routed
//! flow because no temp-wallet signer exists for them.
use super::{
    amount,
    error::{invalid, Result},
    repo,
    temp_wallet::{self, WalletMaterial},
    Service,
};
use crate::route_engine::{Amount, Asset, NearQuoteRequest, NearSwapStatus};
use serde_json::{json, Value};

const FEE_SWEEP_THRESHOLD_UNITS: u128 = 1_000_000;

impl Service {
    /// Reports whether every configured EVM custody route has an existing,
    /// readable treasury with the configured minimum native gas reserve.
    /// This is an observation only; it does not provision wallets or sign.
    pub(crate) async fn temp_custody_readiness(&self) -> Value {
        if !self.cfg.temp_wallets {
            return json!({"enabled": false, "networks": {}});
        }
        let networks = ["ethereum", "polygon-pos", "avalanche-c"];
        let client = match self.pool.get().await {
            Ok(client) => client,
            Err(error) => {
                return json!({
                    "enabled": self.cfg.temp_wallets,
                    "networks": {},
                    "error": format!("treasury database unavailable: {error}"),
                });
            }
        };
        let mut status = serde_json::Map::new();
        let threshold = amount::units(&self.cfg.temp_gas_reserve, 18).unwrap_or(u128::MAX);
        for chain in networks {
            let Some(rpcs) = self.cfg.p2p_evm_rpcs(chain).filter(|rpcs| rpcs.len() >= 2) else {
                status.insert(
                    chain.to_string(),
                    json!({"ready": false, "reason": "two independent RPC endpoints are not configured"}),
                );
                continue;
            };
            let treasury = match repo::temp_wallets::treasury(&client, chain).await {
                Ok(Some(treasury)) => treasury,
                Ok(None) => {
                    status.insert(
                        chain.to_string(),
                        json!({"ready": false, "reason": "treasury wallet has not been provisioned"}),
                    );
                    continue;
                }
                Err(error) => {
                    status.insert(
                        chain.to_string(),
                        json!({"ready": false, "reason": format!("treasury lookup failed: {error}")}),
                    );
                    continue;
                }
            };
            let net = super::chain::EvmNet {
                rpcs: rpcs.to_vec(),
                chain_id: super::config::evm_chain_id(chain).unwrap_or_default(),
                usdt: self.cfg.p2p_usdt().to_string(),
            };
            let reading = async {
                self.evm_chain_matches(&net).await?;
                self.evm_agree(&net, "eth_getBalance", json!([treasury.address, "latest"]))
                    .await
                    .and_then(|value| super::chain::number(&value))
            }
            .await;
            match reading {
                Ok(balance) => {
                    status.insert(
                        chain.to_string(),
                        json!({
                            "ready": false,
                            "treasury_gas_ready": balance >= threshold,
                            "treasury_address": treasury.address,
                            "native_balance_wei": balance.to_string(),
                            "required_native_balance_wei": threshold.to_string(),
                            "reason": if balance < threshold {
                                json!("treasury native gas reserve is below the configured minimum")
                            } else {
                                json!("automatic treasury-to-deal-wallet gas funding is not implemented; custodial route is paused")
                            },
                        }),
                    );
                }
                Err(error) => {
                    status.insert(
                        chain.to_string(),
                        json!({"ready": false, "treasury_address": treasury.address, "reason": format!("independent RPC readiness check failed: {error}")}),
                    );
                }
            }
        }
        json!({
            "enabled": self.cfg.temp_wallets,
            "minimum_native_reserve": self.cfg.temp_gas_reserve,
            "networks": status,
            "note": "Readiness checks the permanent treasury's native balance against OTC_TEMP_GAS_RESERVE. It does not report token inventory or guarantee a route quote.",
        })
    }

    pub(crate) async fn temp_settlement_tick(&self) -> Result<()> {
        if !self.cfg.temp_wallets {
            return Ok(());
        }
        let client = self.pool.get().await?;
        // Provision durable treasuries for every EVM network whose temp-wallet
        // signer and independent RPC pair are configured. These rows are
        // operator-funded and never recycled or deleted.
        for chain in self.cfg.temp_wallet_networks() {
            self.ensure_treasury(&client, &chain).await?;
        }
        let rows = repo::temp_wallets::due_settlements(&client).await?;
        drop(client);
        for row in rows {
            if let Err(error) = self.settle_temp(&row).await {
                tracing::warn!(trade=%row.trade_id, error=%error, "otc.temp.settlement_failed");
                let client = self.pool.get().await?;
                repo::temp_wallets::note_error(&client, &row.trade_id, &error.to_string()).await?;
            }
        }
        if let Err(error) = self.ever_settlement_tick().await {
            tracing::warn!(error=%error, "otc.temp.ever_settlement_failed");
        }
        Ok(())
    }

    fn temp_net(&self, chain: &str, body: &Value) -> Result<super::chain::EvmNet> {
        let chain_id = super::config::evm_chain_id(chain)
            .ok_or_else(|| invalid("temporary wallet network is not EVM"))?;
        let rpcs = self
            .cfg
            .p2p_evm_rpcs(chain)
            .filter(|endpoints| endpoints.len() >= 2)
            .ok_or_else(|| invalid("two independent OTC EVM endpoints are required"))?;
        Ok(super::chain::EvmNet {
            rpcs: rpcs.to_vec(),
            chain_id,
            usdt: temp_token(body)?.to_string(),
        })
    }

    fn material(&self, row: &repo::temp_wallets::SettlementRow) -> Result<WalletMaterial> {
        self.material_for(
            &row.chain,
            row.derivation_index,
            &row.salt,
            &row.key_ciphertext,
        )
    }

    fn material_for(
        &self,
        chain: &str,
        derivation_index: i32,
        salt: &[u8],
        key_ciphertext: &[u8],
    ) -> Result<WalletMaterial> {
        let seed = temp_wallet::open(&self.secrets_key, salt, key_ciphertext)?;
        let chain = temp_wallet::Chain::parse(chain)?;
        let index = u32::try_from(derivation_index)
            .map_err(|_| invalid("temporary wallet derivation index is invalid"))?;
        WalletMaterial::from_seed(chain, &seed[..], index)
    }

    async fn settle_temp(&self, row: &repo::temp_wallets::SettlementRow) -> Result<()> {
        let net = self.temp_net(&row.chain, &row.body)?;
        let gross = amount::integer(
            row.body["usdt_units"]
                .as_str()
                .ok_or_else(|| invalid("deal terms are malformed"))?,
        )?;
        let payout = amount::integer(
            row.body["fee_policy"]["net_units"]
                .as_str()
                .ok_or_else(|| invalid("deal fee terms are malformed"))?,
        )?;
        fee_split(gross, payout)?;
        let latest = json!("latest");
        let client = self.pool.get().await?;
        match row.state.as_str() {
            "awaiting_payment" => {
                if !repo::temp_wallets::claim_awaiting(&client, &row.trade_id).await? {
                    return Ok(());
                }
                let balance = self.erc20_balance_of(&net, &row.address, &latest).await?;
                if balance < gross {
                    repo::temp_wallets::set_state(&client, &row.trade_id, "awaiting_payment", None)
                        .await?;
                    return Ok(());
                }
                let gas = super::chain::number(
                    &self
                        .evm_agree(&net, "eth_getBalance", json!([row.address, latest]))
                        .await?,
                )?;
                if gas < amount::units(&self.cfg.temp_gas_reserve, 18)? {
                    repo::temp_wallets::set_state(
                        &client,
                        &row.trade_id,
                        "awaiting_payment",
                        Some("temporary wallet needs native gas before payout"),
                    )
                    .await?;
                    return Ok(());
                }
                let material = self.material(row)?;
                let same_network = row.body["stablecoin_network"]
                    .as_str()
                    .unwrap_or("ethereum")
                    == row.chain;
                let result = if !same_network {
                    self.temp_intents_payout(&client, row, &material, &net, gross, payout)
                        .await
                } else {
                    let payee = row.body["usdt_payee"]
                        .as_str()
                        .ok_or_else(|| invalid("deal payee is missing"))?;
                    self.temp_treasury_payout(&client, row, &net, payee, payout)
                        .await
                };
                match result {
                    Ok(reference) => {
                        tracing::info!(trade=%row.trade_id, reference, "otc.temp.payout_sent");
                    }
                    Err(error) => {
                        // Nothing is known to be on chain; release the claim so
                        // the balance gate can decide whether a retry is safe.
                        repo::temp_wallets::set_state(
                            &client,
                            &row.trade_id,
                            "awaiting_payment",
                            Some(&error.to_string()),
                        )
                        .await?;
                        return Err(error);
                    }
                }
            }
            "payout_claim" => {
                // Only stale claims reach this arm. The balance check in
                // `awaiting_payment` prevents a second payout if the first
                // transaction landed before the worker stopped.
                repo::temp_wallets::set_state(
                    &client,
                    &row.trade_id,
                    "awaiting_payment",
                    Some("recovered a stale payout claim"),
                )
                .await?;
            }
            "payout_submitted" => {
                let reference = row
                    .reference
                    .as_deref()
                    .ok_or_else(|| invalid("payout reference is missing"))?;
                let receipt = self
                    .evm_agree(&net, "eth_getTransactionReceipt", json!([reference]))
                    .await?;
                if receipt.is_null() {
                    if let Some(raw) = &row.payout_signed_transaction {
                        self.evm_send_raw(&net, raw).await?;
                    }
                    return Ok(());
                }
                if super::chain::number(&receipt["status"])? != 1 {
                    repo::temp_wallets::set_state(
                        &client,
                        &row.trade_id,
                        "failed",
                        Some("stablecoin payout reverted"),
                    )
                    .await?;
                    return Ok(());
                }
                self.after_paid(&client, row).await?;
            }
            "intents_submitted" => {
                let Some(provider) = &self.near_intents else {
                    return Err(invalid("NEAR Intents is not configured"));
                };
                let deposit = row
                    .deposit_address
                    .as_deref()
                    .ok_or_else(|| invalid("Intents deposit address is missing"))?;
                let status = provider
                    .status_with_memo(deposit, row.deposit_memo.as_deref())
                    .await
                    .map_err(|error| {
                        tracing::warn!(trade=%row.trade_id, error=%error, "otc.temp.intents_status_failed");
                        super::error::Error::RouteUnavailable
                    })?;
                match status.status.to_ascii_uppercase().as_str() {
                    "SUCCESS" => {
                        let delivered = intents_output(&status)
                            .ok_or_else(|| invalid("Intents success has no delivered amount"))?;
                        if delivered < payout {
                            tracing::warn!(trade=%row.trade_id, delivered, payout, "otc.temp.intents_shortfall");
                            return Ok(());
                        }
                        self.submit_intents_output_payout(&client, row, payout)
                            .await?;
                    }
                    "REFUNDED" => {
                        repo::temp_wallets::set_state(
                            &client,
                            &row.trade_id,
                            "intents_refunded",
                            Some("Intents refunded the deposit; funds are back on the temporary wallet"),
                        )
                        .await?;
                    }
                    "FAILED" => {
                        repo::temp_wallets::set_state(
                            &client,
                            &row.trade_id,
                            "failed",
                            Some("Intents failed the swap; check the temporary wallet balance"),
                        )
                        .await?;
                    }
                    _ => {}
                }
            }
            "intents_payout_claim" => {
                // The signed transaction is persisted before broadcast, so a
                // stale claim has not moved funds and can safely resume polling.
                repo::temp_wallets::set_state(
                    &client,
                    &row.trade_id,
                    "intents_submitted",
                    Some("recovered a stale cross-network payout claim"),
                )
                .await?;
            }
            "intents_payout_submitted" => {
                let net = self.output_temp_net(row)?;
                let reference = row
                    .reference
                    .as_deref()
                    .ok_or_else(|| invalid("cross-network payout reference is missing"))?;
                let receipt = self
                    .evm_agree(&net, "eth_getTransactionReceipt", json!([reference]))
                    .await?;
                if receipt.is_null() {
                    if let Some(raw) = &row.output_signed_transaction {
                        // Re-broadcasting the exact persisted envelope is
                        // idempotent even when the first response was lost.
                        self.evm_send_raw(&net, raw).await?;
                    }
                    return Ok(());
                }
                if super::chain::number(&receipt["status"])? != 1 {
                    repo::temp_wallets::reset_intents_payout(
                        &client,
                        &row.trade_id,
                        "cross-network temp payout reverted; safe to retry",
                    )
                    .await?;
                    return Ok(());
                }
                self.after_paid(&client, row).await?;
            }
            "fee_submitted" => {
                let reference = row
                    .fee_reference
                    .as_deref()
                    .ok_or_else(|| invalid("fee reference is missing"))?;
                let receipt = self
                    .evm_agree(&net, "eth_getTransactionReceipt", json!([reference]))
                    .await?;
                if receipt.is_null() {
                    if let Some(raw) = &row.fee_signed_transaction {
                        self.evm_send_raw(&net, raw).await?;
                    }
                    return Ok(());
                }
                if super::chain::number(&receipt["status"])? != 1 {
                    repo::temp_wallets::set_state(
                        &client,
                        &row.trade_id,
                        "fee_retained",
                        Some("service-fee sweep reverted; fee stays on the temporary wallet"),
                    )
                    .await?;
                    return Ok(());
                }
                self.recycle(&client, row).await?;
                repo::temp_wallets::set_state(&client, &row.trade_id, "settled", None).await?;
                self.retire_reserved_roles(&client, &row.trade_id).await?;
                tracing::info!(trade=%row.trade_id, "otc.temp.settled");
            }
            _ => {}
        }
        Ok(())
    }

    /// Cross-network payout: Intents delivers the exact output to our separate
    /// USD temp. A later worker step verifies and forwards it to the payee.
    async fn temp_intents_payout(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
        material: &WalletMaterial,
        net: &super::chain::EvmNet,
        gross: u128,
        payout: u128,
    ) -> Result<String> {
        let provider = self
            .near_intents
            .as_ref()
            .ok_or(super::error::Error::RouteUnavailable)?;
        let output_network = row.body["stablecoin_network"]
            .as_str()
            .ok_or_else(|| invalid("deal network is missing"))?;
        let output_temp = row.body["temp_wallets"]
            .as_array()
            .and_then(|wallets| wallets.iter().find(|wallet| wallet["role"] == "usd_out"))
            .and_then(|wallet| wallet["address"].as_str())
            .ok_or_else(|| invalid("cross-network USD temporary wallet is missing"))?;
        let symbol = row.body["stablecoin"].as_str().unwrap_or("USDT");
        let from = Asset::new(symbol, Some(&row.chain))?;
        let to = Asset::new(symbol, Some(output_network))?;
        let quote = provider
            .executable_quote(NearQuoteRequest {
                from,
                to,
                origin_asset_id: row.body["temp_wallet"]["asset_id"]
                    .as_str()
                    .map(str::to_string),
                destination_asset_id: row.body["stablecoin_asset_id"].as_str().map(str::to_string),
                amount: Amount::new(
                    amount::decimal(payout, 6),
                    Asset::new(symbol, Some(output_network))?,
                )?,
                recipient: output_temp.to_string(),
                refund_to: row.address.clone(),
                slippage_bps: 100,
                app_fees: Vec::new(),
                dry: false,
            })
            .await
            .map_err(|error| {
                tracing::warn!(trade=%row.trade_id, error=%error, "otc.temp.intents_quote_failed");
                super::error::Error::RouteUnavailable
            })?;
        if amount::units(&quote.output.value, 6)? != payout {
            return Err(super::error::Error::Conflict(
                "the cross-network quote does not deliver the exact deal amount".into(),
            ));
        }
        let source_units = amount::units(&quote.input.value, 6)?;
        if source_units > gross {
            return Err(super::error::Error::Conflict(
                "the cross-network route costs exceed the withheld service fee".into(),
            ));
        }
        let deposit = quote
            .deposit_address
            .as_deref()
            .filter(|address| !address.trim().is_empty())
            .ok_or(super::error::Error::RouteUnavailable)?;
        let memo = crate::route_engine::first_string(&quote.raw, &["depositMemo", "deposit_memo"]);
        let reference = self
            .evm_temp_token_transfer(material, net, temp_token(&row.body)?, deposit, source_units)
            .await?;
        repo::temp_wallets::set_intents_quote(
            client,
            &row.trade_id,
            &reference,
            deposit,
            memo.as_deref(),
            &quote.quote_id,
            &source_units.to_string(),
            &payout.to_string(),
        )
        .await?;
        Ok(reference)
    }

    fn output_temp_net(
        &self,
        row: &repo::temp_wallets::SettlementRow,
    ) -> Result<super::chain::EvmNet> {
        let chain = row.body["stablecoin_network"]
            .as_str()
            .ok_or_else(|| invalid("deal output network is missing"))?;
        let chain_id = super::config::evm_chain_id(chain)
            .ok_or_else(|| invalid("temporary output wallet is not on a supported EVM network"))?;
        let rpcs = self
            .cfg
            .p2p_evm_rpcs(chain)
            .filter(|endpoints| endpoints.len() >= 2)
            .ok_or_else(|| invalid("two independent output-network EVM endpoints are required"))?;
        let token = row.body["stablecoin_contract"]
            .as_str()
            .ok_or_else(|| invalid("deal output token is missing"))?;
        Ok(super::chain::EvmNet {
            rpcs: rpcs.to_vec(),
            chain_id,
            usdt: token.to_string(),
        })
    }

    /// Pays the agreed stablecoin amount from the USD output temp after
    /// Intents has delivered and the exact output balance is verified.
    async fn submit_intents_output_payout(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
        payout: u128,
    ) -> Result<()> {
        if !repo::temp_wallets::claim_intents_payout(client, &row.trade_id).await? {
            return Ok(());
        }
        let net = self.output_temp_net(row)?;
        let output_wallet = row.body["temp_wallets"]
            .as_array()
            .and_then(|wallets| wallets.iter().find(|wallet| wallet["role"] == "usd_out"))
            .and_then(|wallet| wallet["address"].as_str())
            .ok_or_else(|| invalid("cross-network USD temporary wallet is missing"))?;
        let balance = self
            .erc20_balance_of(&net, output_wallet, &json!("latest"))
            .await?;
        if balance < payout {
            repo::temp_wallets::set_state(
                client,
                &row.trade_id,
                "intents_submitted",
                Some("Intents output is not yet visible on the USD temporary wallet"),
            )
            .await?;
            return Ok(());
        }
        let gas = super::chain::number(
            &self
                .evm_agree(&net, "eth_getBalance", json!([output_wallet, "latest"]))
                .await?,
        )?;
        if gas < amount::units(&self.cfg.temp_gas_reserve, 18)? {
            repo::temp_wallets::set_state(
                client,
                &row.trade_id,
                "intents_submitted",
                Some("USD output temporary wallet needs native gas before payout"),
            )
            .await?;
            return Ok(());
        }
        let secret = repo::temp_wallets::wallet_secret_for_role(client, &row.trade_id, "usd_out")
            .await?
            .ok_or_else(|| invalid("cross-network USD wallet key is missing"))?;
        let material = self.material_for(
            &secret.chain,
            secret.derivation_index,
            &secret.salt,
            &secret.key_ciphertext,
        )?;
        let payee = row.body["usdt_payee"]
            .as_str()
            .ok_or_else(|| invalid("deal payee is missing"))?;
        let signed = self
            .evm_sign_temp_token_transfer(&material, &net, &net.usdt, payee, payout)
            .await?;
        let reference = super::chain::evm_transaction_hash(&signed);
        // Persist the exact envelope before broadcasting it. A restart or a
        // lost RPC response can safely retry the same transaction hash.
        repo::temp_wallets::set_intents_payout_reference(
            client,
            &row.trade_id,
            &reference,
            &signed,
        )
        .await?;
        self.evm_send_raw(&net, &signed).await?;
        Ok(())
    }

    /// Same-network payout from the permanent treasury (spec §6.2), leaving
    /// the payer's entire deposit on the temporary wallet until fee sweep.
    async fn temp_treasury_payout(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
        net: &super::chain::EvmNet,
        payee: &str,
        payout: u128,
    ) -> Result<String> {
        let treasury = self.ensure_treasury(client, &row.chain).await?;
        let material = self.treasury_material(&treasury)?;
        let latest = json!("latest");
        let balance = self
            .erc20_balance_of(net, &treasury.address, &latest)
            .await?;
        if balance < payout {
            return Err(invalid(
                "the same-network treasury cannot cover the agreed payout; fund it before retrying",
            ));
        }
        let gas = super::chain::number(
            &self
                .evm_agree(net, "eth_getBalance", json!([treasury.address, latest]))
                .await?,
        )?;
        if gas < amount::units(&self.cfg.temp_gas_reserve, 18)? {
            return Err(invalid(
                "the same-network treasury needs native gas before payout",
            ));
        }
        let signed = self
            .evm_sign_temp_token_transfer(&material, net, temp_token(&row.body)?, payee, payout)
            .await?;
        let reference = super::chain::evm_transaction_hash(&signed);
        repo::temp_wallets::set_treasury_payment(
            client,
            &row.trade_id,
            &reference,
            &payout.to_string(),
            &signed,
        )
        .await?;
        self.evm_send_raw(net, &signed).await?;
        tracing::info!(trade=%row.trade_id, treasury=%treasury.address, payout, "otc.temp.treasury_payout_sent");
        Ok(reference)
    }

    /// Returns the chain's permanent treasury, generating and sealing it on
    /// first use. Its native balance funds outgoing treasury transactions.
    async fn ensure_treasury(
        &self,
        client: &deadpool_postgres::Client,
        chain: &str,
    ) -> Result<repo::temp_wallets::TreasuryWallet> {
        if let Some(treasury) = repo::temp_wallets::treasury(client, chain).await? {
            return Ok(treasury);
        }
        let parsed = temp_wallet::Chain::parse(chain)?;
        let material = WalletMaterial::generate(parsed)?;
        let sealed = temp_wallet::seal(&self.secrets_key, material.seed())?;
        repo::temp_wallets::insert(
            client,
            &uuid::Uuid::new_v4(),
            None,
            "treasury",
            parsed.id(),
            &material.address()?,
            material.derivation_index(),
            &sealed.salt,
            &sealed.ciphertext,
            None,
        )
        .await?;
        repo::temp_wallets::treasury(client, chain)
            .await?
            .ok_or_else(|| invalid("the treasury wallet could not be provisioned"))
    }

    /// Blocks new custody deals until the treasury-to-temp gas transfer has a
    /// persisted and recoverable signer path.
    pub(crate) async fn temp_treasury_gas_preflight(&self, chain: &str) -> Result<()> {
        Err(super::error::Error::Conflict(format!(
            "custodial OTC on {chain} is paused until automatic treasury-to-deal-wallet gas funding is enabled"
        )))
    }

    /// Accept-time guard: a same-network deal the treasury cannot front fails
    /// before any temporary address exists.
    pub(crate) async fn temp_treasury_preflight(
        &self,
        chain: &str,
        token: &str,
        payout: u128,
    ) -> Result<()> {
        let chain_id = super::config::evm_chain_id(chain)
            .ok_or_else(|| invalid("temporary wallet network is not EVM"))?;
        let rpcs = self
            .cfg
            .p2p_evm_rpcs(chain)
            .filter(|endpoints| endpoints.len() >= 2)
            .ok_or_else(|| invalid("two independent OTC EVM endpoints are required"))?;
        let net = super::chain::EvmNet {
            rpcs: rpcs.to_vec(),
            chain_id,
            usdt: token.to_string(),
        };
        let client = self.pool.get().await?;
        let treasury = self.ensure_treasury(&client, chain).await?;
        let latest = json!("latest");
        let balance = self
            .erc20_balance_of(&net, &treasury.address, &latest)
            .await?;
        if balance < payout {
            return Err(super::error::Error::Conflict(format!(
                "the {chain} treasury {} holds {} but the deal needs {}; fund it before accepting",
                treasury.address,
                amount::decimal(balance, 6),
                amount::decimal(payout, 6)
            )));
        }
        let gas = super::chain::number(
            &self
                .evm_agree(&net, "eth_getBalance", json!([treasury.address, latest]))
                .await?,
        )?;
        if gas < amount::units(&self.cfg.temp_gas_reserve, 18)? {
            return Err(super::error::Error::Conflict(format!(
                "the {chain} treasury {} needs native gas before accepting same-network deals",
                treasury.address
            )));
        }
        Ok(())
    }

    /// EVER leg (spec §6.1 step 11): the payer deposits EVER on the deal's
    /// `ever` temp wallet; after the USD net is delivered the worker forwards
    /// the booked EVER amount to `ever_payee` and only then completes the deal.
    async fn ever_settlement_tick(&self) -> Result<()> {
        if !self.cfg.temp_wallets {
            return Ok(());
        }
        let client = self.pool.get().await?;
        let rows = repo::temp_wallets::due_ever_payouts(&client).await?;
        drop(client);
        for row in rows {
            if let Err(error) = self.settle_ever(&row).await {
                tracing::warn!(trade=%row.trade_id, error=%error, "otc.temp.ever_failed");
                let client = self.pool.get().await?;
                repo::temp_wallets::note_ever_error(&client, &row.trade_id, &error.to_string())
                    .await?;
            }
        }
        Ok(())
    }

    async fn settle_ever(&self, row: &repo::temp_wallets::EverPayoutRow) -> Result<()> {
        let client = self.pool.get().await?;
        if row.ever_state.as_deref() == Some("submitted") {
            let reference = row
                .ever_reference
                .as_deref()
                .ok_or_else(|| invalid("EVER payout reference is missing"))?;
            let data = self
                .ever_agree(
                    "query($id:String!){transactions(filter:{in_msg:{eq:$id}},limit:1){id aborted compute{success}}}",
                    json!({"id": reference}),
                )
                .await?;
            let Some(transaction) = data["transactions"]
                .as_array()
                .and_then(|rows| rows.first())
            else {
                if let Some(signed) = row.ever_signed_message.as_deref() {
                    let boc = std::str::from_utf8(signed)
                        .map_err(|_| invalid("persisted EVER message is malformed"))?;
                    // The exact signed message is durable before this call;
                    // replaying it is safe if the first RPC response was lost.
                    self.ever_send_boc(boc).await?;
                }
                repo::temp_wallets::note_ever_error(
                    &client,
                    &row.trade_id,
                    "EVER payout message is not included yet",
                )
                .await?;
                return Ok(());
            };
            if transaction["aborted"] == true || transaction["compute"]["success"] != true {
                repo::temp_wallets::set_ever_state(
                    &client,
                    &row.trade_id,
                    "failed",
                    None,
                    Some("EVER payout transaction aborted"),
                )
                .await?;
                return Ok(());
            }
            repo::temp_wallets::delete_role(&client, &row.trade_id, "ever").await?;
            repo::p2p::complete_trade(&client, &row.trade_id).await?;
            repo::temp_wallets::set_ever_state(&client, &row.trade_id, "paid", None, None).await?;
            tracing::info!(trade=%row.trade_id, reference, "otc.temp.ever_paid");
            return Ok(());
        }
        if !repo::temp_wallets::claim_ever_payout(&client, &row.trade_id).await? {
            return Ok(());
        }
        let units = amount::integer(
            row.body["ever_units"]
                .as_str()
                .ok_or_else(|| invalid("deal EVER terms are missing"))?,
        )?;
        if units == 0 {
            return Err(invalid("deal EVER amount must be positive"));
        }
        let payee = row.body["ever_payee"]
            .as_str()
            .ok_or_else(|| invalid("deal EVER payee is missing"))?;
        let balance = super::chain::number(&self.ever_account(&row.address).await?["balance"])?;
        // The payout handoff adds exactly `p2p_ever_fee_reserve` on top of the
        // booked amount, so that is the gas buffer this address can rely on.
        let gas = amount::units(&self.cfg.p2p_ever_fee_reserve, 9)?;
        let needed = units
            .checked_add(gas)
            .ok_or_else(|| invalid("EVER payout amount overflow"))?;
        if balance < needed {
            repo::temp_wallets::set_ever_state(
                &client,
                &row.trade_id,
                "pending",
                None,
                Some("temporary EVER wallet is not funded enough for payout"),
            )
            .await?;
            return Ok(());
        }
        let material = self.material_for(
            &row.chain,
            row.derivation_index,
            &row.salt,
            &row.key_ciphertext,
        )?;
        let (reference, boc) = self.ever_sign_temp_transfer(&material, payee, units)?;
        if !repo::temp_wallets::set_ever_signed(&client, &row.trade_id, &reference, boc.as_bytes())
            .await?
        {
            return Ok(());
        }
        self.ever_send_boc(&boc).await?;
        tracing::info!(trade=%row.trade_id, reference, attempts=%row.ever_attempts, "otc.temp.ever_submitted");
        Ok(())
    }

    fn treasury_material(
        &self,
        treasury: &repo::temp_wallets::TreasuryWallet,
    ) -> Result<WalletMaterial> {
        let seed = temp_wallet::open(&self.secrets_key, &treasury.salt, &treasury.key_ciphertext)?;
        let chain = temp_wallet::Chain::parse(&treasury.chain)?;
        let index = u32::try_from(treasury.derivation_index)
            .map_err(|_| invalid("treasury wallet derivation index is invalid"))?;
        WalletMaterial::from_seed(chain, &seed[..], index)
    }

    /// A terminal USD payout retires the roles that were reserved at Accept
    /// but never funded. The `ever` row is not retired here: the EVER worker
    /// forwards it after the USD net is delivered and then deletes it
    /// (spec §6.1 step 11, §7 rule 4).
    async fn retire_reserved_roles(
        &self,
        client: &deadpool_postgres::Client,
        trade_id: &uuid::Uuid,
    ) -> Result<()> {
        repo::temp_wallets::delete_role(client, trade_id, "usd_out").await
    }

    /// The net reached the payee: record the fee as collected, sweep the desk's
    /// funds, then recycle the wallet once the sweep settles.
    async fn after_paid(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
    ) -> Result<()> {
        repo::p2p::collect_fee(client, &row.trade_id).await?;
        let treasury = self.ensure_treasury(client, &row.chain).await?;
        let recipient = treasury.address;
        let net = self.temp_net(&row.chain, &row.body)?;
        let material = self.material(row)?;
        // A cross-network route may consume part of the withheld fee; sweep what
        // is actually left instead of leaving dust that ops has to recover. A
        // reserve payout fronted the net, so the withheld deposit is desk funds
        // and the whole balance is swept.
        let balance = self
            .erc20_balance_of(&net, &row.address, &json!("latest"))
            .await?;
        if !fee_sweep_required(balance) {
            repo::temp_wallets::return_to_pool(client, &row.wallet_id).await?;
            repo::temp_wallets::set_state(client, &row.trade_id, "settled", None).await?;
            self.retire_reserved_roles(client, &row.trade_id).await?;
            tracing::info!(trade=%row.trade_id, balance, "otc.temp.dust_slot_reused");
            return Ok(());
        }
        // Once accumulated fees reach $1, sweep the entire remaining token
        // balance. On cross-network settlements the Intents input itself can
        // consume part of the withheld fee, so the current balance is the
        // authoritative amount owned by the platform.
        let sweep = sweep_amount(balance);
        if sweep == 0 {
            repo::temp_wallets::set_state(
                client,
                &row.trade_id,
                "fee_retained",
                Some("nothing is left on the temporary wallet for the service fee"),
            )
            .await?;
            self.retire_reserved_roles(client, &row.trade_id).await?;
            return Ok(());
        }
        let signed = match self
            .evm_sign_temp_token_transfer(
                &material,
                &net,
                temp_token(&row.body)?,
                &recipient,
                sweep,
            )
            .await
        {
            Ok(signed) => signed,
            Err(error) => {
                // The net is already delivered; retain the fee for ops if the
                // treasury sweep cannot be prepared safely.
                repo::temp_wallets::set_state(
                    client,
                    &row.trade_id,
                    "fee_retained",
                    Some(&error.to_string()),
                )
                .await?;
                self.retire_reserved_roles(client, &row.trade_id).await?;
                return Ok(());
            }
        };
        let reference = super::chain::evm_transaction_hash(&signed);
        repo::temp_wallets::set_fee_reference(client, &row.trade_id, &reference, &signed).await?;
        self.evm_send_raw(&net, &signed).await?;
        Ok(())
    }

    /// `create_based_on`: same seed, next index, fresh salt and ciphertext. The
    /// child is kept in the reusable pool until a future deal attaches it.
    async fn recycle(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
    ) -> Result<()> {
        let child = self.material(row)?.derive_next()?;
        let sealed = temp_wallet::seal(&self.secrets_key, child.seed())?;
        repo::temp_wallets::insert(
            client,
            &uuid::Uuid::new_v4(),
            None,
            row.role.as_str(),
            &row.chain,
            &child.address()?,
            child.derivation_index(),
            &sealed.salt,
            &sealed.ciphertext,
            Some(&row.wallet_id),
        )
        .await?;
        repo::temp_wallets::set_status(client, &row.wallet_id, "recycled").await
    }
}

/// The source ERC-20 contract of the temporary wallet.
fn temp_token(body: &Value) -> Result<&str> {
    body["temp_wallet"]["token"]
        .as_str()
        .or_else(|| body["stablecoin_contract"].as_str())
        .or_else(|| body["usdt_contract"].as_str())
        .ok_or_else(|| invalid("temporary wallet token is missing"))
}

/// Returns the withheld fee; the net must be positive and never exceed gross.
fn fee_split(gross: u128, payout: u128) -> Result<u128> {
    if payout == 0 || payout > gross {
        return Err(invalid("deal fee terms are inconsistent"));
    }
    Ok(gross - payout)
}

/// Units swept to the fee/treasury recipient once the net is delivered.
///
/// A direct or Intents payout leaves the withheld fee on the temporary wallet,
/// so only the fee is swept. A reserve payout fronted the net from platform
/// inventory, so the whole withheld deposit belongs to the desk and is swept.
fn sweep_amount(balance: u128) -> u128 {
    balance
}

fn fee_sweep_required(balance: u128) -> bool {
    balance >= FEE_SWEEP_THRESHOLD_UNITS
}

/// Reads the delivered atomic amount from a 1Click status payload.
fn intents_output(status: &NearSwapStatus) -> Option<u128> {
    let details = status.raw.get("swapDetails").unwrap_or(&Value::Null);
    let value = status
        .output_amount
        .clone()
        .or_else(|| {
            crate::route_engine::first_string(details, &["amountOut", "amount_out", "outputAmount"])
        })
        .or_else(|| {
            crate::route_engine::first_string(
                &status.raw,
                &["amountOut", "amount_out", "outputAmount"],
            )
        })
        .or_else(|| {
            crate::route_engine::first_string(
                details,
                &["amountOutFormatted", "amount_out_formatted"],
            )
        })?;
    if value.contains('.') {
        amount::units(&value, 6).ok()
    } else {
        value.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{fee_split, fee_sweep_required, intents_output, sweep_amount};
    use crate::route_engine::NearSwapStatus;
    use serde_json::json;

    #[test]
    fn sweep_moves_the_full_accumulated_treasury_balance() {
        assert_eq!(sweep_amount(100_000_000), 100_000_000);
        assert_eq!(sweep_amount(2_000_000), 2_000_000);
        assert_eq!(sweep_amount(0), 0);
    }

    #[test]
    fn dust_below_one_usd_keeps_the_same_wallet_slot_available() {
        assert!(!fee_sweep_required(999_999));
        assert!(fee_sweep_required(1_000_000));
    }

    #[test]
    fn fee_is_withheld_from_the_gross_deposit() {
        assert_eq!(fee_split(100_000_000, 97_000_000).unwrap(), 3_000_000);
        assert_eq!(fee_split(30_000_000, 29_100_000).unwrap(), 900_000);
        assert!(fee_split(1_000, 0).is_err());
        assert!(fee_split(1_000, 1_001).is_err());
    }

    #[test]
    fn intents_output_reads_atomic_and_formatted_amounts() {
        let atomic = NearSwapStatus {
            status: "SUCCESS".into(),
            output_amount: Some("29000000".into()),
            raw: json!({}),
        };
        assert_eq!(intents_output(&atomic), Some(29_000_000));
        let formatted = NearSwapStatus {
            status: "SUCCESS".into(),
            output_amount: None,
            raw: json!({"swapDetails":{"amountOutFormatted":"29.0"}}),
        };
        assert_eq!(intents_output(&formatted), Some(29_000_000));
        let details = NearSwapStatus {
            status: "SUCCESS".into(),
            output_amount: None,
            raw: json!({"swapDetails":{"amountOut":"29000000"}}),
        };
        assert_eq!(intents_output(&details), Some(29_000_000));
    }
}
