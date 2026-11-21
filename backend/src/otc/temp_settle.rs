//! Settlement of stablecoin temporary wallets.
//!
//! Phase D/E, EVM same-network v1: the payer's stablecoin lands on a platform
//! wallet, the worker pays the agreed net to the real payee and withholds the
//! service fee, then recycles the address. Cross-network payouts and non-EVM
//! sources keep the legacy direct/routed flow.
use super::{
    amount,
    error::{invalid, Result},
    repo,
    temp_wallet::{self, WalletMaterial},
    Service,
};
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
        let usdt = body["stablecoin_contract"]
            .as_str()
            .or_else(|| body["usdt_contract"].as_str())
            .ok_or_else(|| invalid("temporary wallet token is missing"))?;
        Ok(super::chain::EvmNet {
            rpcs: rpcs.to_vec(),
            chain_id,
            usdt: usdt.to_string(),
        })
    }

    fn material(&self, row: &repo::temp_wallets::SettlementRow) -> Result<WalletMaterial> {
        let seed = temp_wallet::open(&self.secrets_key, &row.salt, &row.key_ciphertext)?;
        let chain = temp_wallet::Chain::parse(&row.chain)?;
        let index = u32::try_from(row.derivation_index)
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
                let contract = contract_of(&row.body)?;
                match self
                    .evm_temp_token_transfer(&material, &net, contract, payee, payout)
                    .await
                {
                    Ok(reference) => {
                        repo::temp_wallets::set_payout_reference(
                            &client,
                            &row.trade_id,
                            &reference,
                        )
                        .await?;
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
                // The net reached the payee, so the withheld fee can count as
                // collected even before it is swept to the fee recipient.
                repo::p2p::collect_fee(&client, &row.trade_id).await?;
                let Some(recipient) = self.cfg.fee_recipients.get(&row.chain).cloned() else {
                    repo::temp_wallets::set_state(
                        &client,
                        &row.trade_id,
                        "fee_retained",
                        Some("no fee recipient configured; fee stays on the temporary wallet"),
                    )
                    .await?;
                    return Ok(());
                };
                if fee == 0 {
                    self.recycle(&client, row).await?;
                    repo::temp_wallets::set_state(&client, &row.trade_id, "settled", None).await?;
                    return Ok(());
                }
                let material = self.material(row)?;
                let reference = self
                    .evm_temp_token_transfer(
                        &material,
                        &net,
                        contract_of(&row.body)?,
                        &recipient,
                        fee,
                    )
                    .await?;
                repo::temp_wallets::set_fee_reference(&client, &row.trade_id, &reference).await?;
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
                tracing::info!(trade=%row.trade_id, "otc.temp.settled");
            }
            _ => {}
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
            "usd",
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

fn contract_of(body: &Value) -> Result<&str> {
    body["stablecoin_contract"]
        .as_str()
        .or_else(|| body["usdt_contract"].as_str())
        .ok_or_else(|| invalid("deal token is missing"))
}

/// Returns the withheld fee; the net must be positive and never exceed gross.
fn fee_split(gross: u128, payout: u128) -> Result<u128> {
    if payout == 0 || payout > gross {
        return Err(invalid("deal fee terms are inconsistent"));
    }
    Ok(gross - payout)
}

#[cfg(test)]
mod tests {
    use super::fee_split;

    #[test]
    fn fee_is_withheld_from_the_gross_deposit() {
        assert_eq!(fee_split(100_000_000, 97_000_000).unwrap(), 3_000_000);
        assert_eq!(fee_split(30_000_000, 29_100_000).unwrap(), 900_000);
        assert!(fee_split(1_000, 0).is_err());
        assert!(fee_split(1_000, 1_001).is_err());
    }
}
