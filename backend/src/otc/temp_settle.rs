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

impl Service {
    pub(crate) async fn temp_settlement_tick(&self) -> Result<()> {
        if !self.cfg.temp_wallets {
            return Ok(());
        }
        let client = self.pool.get().await?;
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
        let fee = fee_split(gross, payout)?;
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
                let payee = row.body["usdt_payee"]
                    .as_str()
                    .ok_or_else(|| invalid("deal payee is missing"))?;
                let same_network = row.body["stablecoin_network"]
                    .as_str()
                    .unwrap_or("ethereum")
                    == row.chain;
                let result = if !same_network {
                    self.temp_intents_payout(&client, row, &material, &net, payee, gross, payout)
                        .await
                } else if self.cfg.temp_reserve_payout {
                    self.temp_reserve_payout(&client, row, &net, payee, payout)
                        .await
                } else {
                    self.evm_temp_token_transfer(
                        &material,
                        &net,
                        temp_token(&row.body)?,
                        payee,
                        payout,
                    )
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
                self.after_paid(&client, row, fee).await?;
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
                        self.after_paid(&client, row, fee).await?;
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
            "fee_submitted" => {
                let reference = row
                    .fee_reference
                    .as_deref()
                    .ok_or_else(|| invalid("fee reference is missing"))?;
                let receipt = self
                    .evm_agree(&net, "eth_getTransactionReceipt", json!([reference]))
                    .await?;
                if receipt.is_null() {
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

    /// Cross-network payout: exact-output quote to the payee, refund to the
    /// temp address we control, then a signed deposit from that address.
    async fn temp_intents_payout(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
        material: &WalletMaterial,
        net: &super::chain::EvmNet,
        payee: &str,
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
                recipient: payee.to_string(),
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

    /// Same-network payout from platform reserve inventory (spec §6.2): the desk
    /// fronts the net from the reserve on the payout network, so the payer's
    /// whole deposit stays withheld on the temporary wallet.
    async fn temp_reserve_payout(
        &self,
        client: &deadpool_postgres::Client,
        row: &repo::temp_wallets::SettlementRow,
        net: &super::chain::EvmNet,
        payee: &str,
        payout: u128,
    ) -> Result<String> {
        let reserve = self.ensure_reserve(client, &row.chain).await?;
        let material = self.reserve_material(&reserve)?;
        let latest = json!("latest");
        let balance = self
            .erc20_balance_of(net, &reserve.address, &latest)
            .await?;
        if balance < payout {
            return Err(invalid(
                "the same-network reserve cannot cover the agreed payout; fund it before retrying",
            ));
        }
        let gas = super::chain::number(
            &self
                .evm_agree(net, "eth_getBalance", json!([reserve.address, latest]))
                .await?,
        )?;
        if gas < amount::units(&self.cfg.temp_gas_reserve, 18)? {
            return Err(invalid(
                "the same-network reserve needs native gas before payout",
            ));
        }
        let reference = self
            .evm_temp_token_transfer(&material, net, temp_token(&row.body)?, payee, payout)
            .await?;
        repo::temp_wallets::set_reserve_payment(
            client,
            &row.trade_id,
            &reference,
            &payout.to_string(),
        )
        .await?;
        tracing::info!(trade=%row.trade_id, reserve=%reserve.address, payout, "otc.temp.reserve_payout_sent");
        Ok(reference)
    }

    /// Returns the chain's reserve row, generating and sealing one on first use.
    /// The address it reports is the one an operator has to fund.
    async fn ensure_reserve(
        &self,
        client: &deadpool_postgres::Client,
        chain: &str,
    ) -> Result<repo::temp_wallets::ReserveWallet> {
        if let Some(reserve) = repo::temp_wallets::reserve(client, chain).await? {
            return Ok(reserve);
        }
        let parsed = temp_wallet::Chain::parse(chain)?;
        let material = WalletMaterial::generate(parsed)?;
        let sealed = temp_wallet::seal(&self.secrets_key, material.seed())?;
        repo::temp_wallets::insert(
            client,
            &uuid::Uuid::new_v4(),
            None,
            "reserve",
            parsed.id(),
            &material.address()?,
            material.derivation_index(),
            &sealed.salt,
            &sealed.ciphertext,
            None,
        )
        .await?;
        repo::temp_wallets::reserve(client, chain)
            .await?
            .ok_or_else(|| invalid("the reserve wallet could not be provisioned"))
    }

    /// Accept-time guard for `OTC_TEMP_RESERVE_PAYOUT` (spec §6.3): a
    /// same-network deal that the reserve cannot front must fail before any
    /// address exists instead of parking in `awaiting_payment`. The reserve row
    /// is provisioned on first use so the conflict names the address to fund.
    pub(crate) async fn temp_reserve_preflight(
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
        let reserve = self.ensure_reserve(&client, chain).await?;
        let latest = json!("latest");
        let balance = self
            .erc20_balance_of(&net, &reserve.address, &latest)
            .await?;
        if balance < payout {
            return Err(super::error::Error::Conflict(format!(
                "the {chain} reserve wallet {} holds {} but the deal needs {}; fund it before accepting",
                reserve.address,
                amount::decimal(balance, 6),
                amount::decimal(payout, 6)
            )));
        }
        let gas = super::chain::number(
            &self
                .evm_agree(&net, "eth_getBalance", json!([reserve.address, latest]))
                .await?,
        )?;
        if gas < amount::units(&self.cfg.temp_gas_reserve, 18)? {
            return Err(super::error::Error::Conflict(format!(
                "the {chain} reserve wallet {} needs native gas before accepting reserve-payout deals",
                reserve.address
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
        let reference = self.ever_temp_transfer(&material, payee, units).await?;
        repo::temp_wallets::set_ever_state(
            &client,
            &row.trade_id,
            "submitted",
            Some(&reference),
            None,
        )
        .await?;
        tracing::info!(trade=%row.trade_id, reference, attempts=%row.ever_attempts, "otc.temp.ever_submitted");
        Ok(())
    }

    fn reserve_material(
        &self,
        reserve: &repo::temp_wallets::ReserveWallet,
    ) -> Result<WalletMaterial> {
        let seed = temp_wallet::open(&self.secrets_key, &reserve.salt, &reserve.key_ciphertext)?;
        let chain = temp_wallet::Chain::parse(&reserve.chain)?;
        let index = u32::try_from(reserve.derivation_index)
            .map_err(|_| invalid("reserve wallet derivation index is invalid"))?;
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
        fee: u128,
    ) -> Result<()> {
        repo::p2p::collect_fee(client, &row.trade_id).await?;
        let from_reserve = row.reserve_units.is_some();
        let Some(recipient) = self.cfg.fee_recipients.get(&row.chain).cloned() else {
            repo::temp_wallets::set_state(
                client,
                &row.trade_id,
                "fee_retained",
                Some("no fee recipient configured; fee stays on the temporary wallet"),
            )
            .await?;
            self.retire_reserved_roles(client, &row.trade_id).await?;
            return Ok(());
        };
        if fee == 0 && !from_reserve {
            self.recycle(client, row).await?;
            repo::temp_wallets::set_state(client, &row.trade_id, "settled", None).await?;
            self.retire_reserved_roles(client, &row.trade_id).await?;
            return Ok(());
        }
        let net = self.temp_net(&row.chain, &row.body)?;
        let material = self.material(row)?;
        // A cross-network route may consume part of the withheld fee; sweep what
        // is actually left instead of leaving dust that ops has to recover. A
        // reserve payout fronted the net, so the withheld deposit is desk funds
        // and the whole balance is swept.
        let balance = self
            .erc20_balance_of(&net, &row.address, &json!("latest"))
            .await?;
        let sweep = sweep_amount(fee, balance, from_reserve);
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
        if !from_reserve && balance < fee {
            tracing::warn!(trade=%row.trade_id, balance, fee, "otc.temp.fee_shortfall");
        }
        match self
            .evm_temp_token_transfer(&material, &net, temp_token(&row.body)?, &recipient, sweep)
            .await
        {
            Ok(reference) => {
                repo::temp_wallets::set_fee_reference(client, &row.trade_id, &reference).await?;
            }
            Err(error) => {
                // The net is already delivered; keep the fee for ops instead of
                // retrying a sweep that the chain just rejected.
                repo::temp_wallets::set_state(
                    client,
                    &row.trade_id,
                    "fee_retained",
                    Some(&error.to_string()),
                )
                .await?;
                self.retire_reserved_roles(client, &row.trade_id).await?;
            }
        }
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
fn sweep_amount(fee: u128, balance: u128, from_reserve: bool) -> u128 {
    if from_reserve {
        balance
    } else {
        fee.min(balance)
    }
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
    use super::{fee_split, intents_output, sweep_amount};
    use crate::route_engine::NearSwapStatus;
    use serde_json::json;

    #[test]
    fn reserve_payout_sweeps_the_whole_withheld_deposit() {
        assert_eq!(sweep_amount(3_000_000, 100_000_000, true), 100_000_000);
        assert_eq!(sweep_amount(3_000_000, 100_000_000, false), 3_000_000);
        assert_eq!(sweep_amount(3_000_000, 2_000_000, false), 2_000_000);
        assert_eq!(sweep_amount(3_000_000, 0, true), 0);
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
