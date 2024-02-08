// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::error::NymRewarderError;
use crate::rewarder::credential_issuance::types::CredentialIssuer;
use nym_coconut_bandwidth_contract_common::events::{
    COSMWASM_DEPOSITED_FUNDS_EVENT_TYPE, DEPOSIT_INFO, DEPOSIT_VALUE,
};
use nym_coconut_dkg_common::types::Epoch;
use nym_network_defaults::NymNetworkDetails;
use nym_validator_client::nyxd::contract_traits::{DkgQueryClient, PagedDkgQueryClient};
use nym_validator_client::nyxd::helpers::find_tx_attribute;
use nym_validator_client::nyxd::module_traits::staking::{
    QueryHistoricalInfoResponse, QueryValidatorsResponse,
};
use nym_validator_client::nyxd::{
    AccountId, Coin, CosmWasmClient, Hash, PageRequest, StakingQueryClient,
};
use nym_validator_client::{nyxd, DirectSigningHttpRpcNyxdClient};
use std::ops::Deref;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct NyxdClient {
    inner: Arc<RwLock<DirectSigningHttpRpcNyxdClient>>,
}

impl NyxdClient {
    pub(crate) fn new(config: &Config) -> Result<Self, NymRewarderError> {
        let client_config =
            nyxd::Config::try_from_nym_network_details(&NymNetworkDetails::new_from_env())?;
        let nyxd_url = config.base.upstream_nyxd.as_str();

        let mnemonic = config.base.mnemonic.clone();

        let inner = DirectSigningHttpRpcNyxdClient::connect_with_mnemonic(
            client_config,
            nyxd_url,
            mnemonic,
        )?;

        Ok(NyxdClient {
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    pub(crate) async fn balance(&self, denom: &str) -> Result<Coin, NymRewarderError> {
        let guard = self.inner.read().await;
        let address = guard.address();
        Ok(guard
            .get_balance(&address, denom.to_string())
            .await?
            .unwrap_or(Coin::new(0, denom)))
    }

    pub(crate) async fn send_rewards(
        &self,
        epoch: crate::rewarder::Epoch,
        amounts: Vec<(AccountId, Vec<Coin>)>,
    ) -> Result<Hash, NymRewarderError> {
        self.inner
            .write()
            .await
            .send_multiple(amounts, format!("sending rewards for {epoch:?}"), None)
            .await
            .map(|res| res.hash)
            .map_err(Into::into)
    }

    pub(crate) async fn historical_info(
        &self,
        height: i64,
    ) -> Result<QueryHistoricalInfoResponse, NymRewarderError> {
        Ok(self.inner.read().await.historical_info(height).await?)
    }

    pub(crate) async fn validators(
        &self,
        pagination: Option<PageRequest>,
    ) -> Result<QueryValidatorsResponse, NymRewarderError> {
        let guard = self.inner.read().await;
        Ok(StakingQueryClient::validators(guard.deref(), "".to_string(), pagination).await?)
    }

    pub(crate) async fn dkg_epoch(&self) -> Result<Epoch, NymRewarderError> {
        Ok(self.inner.read().await.get_current_epoch().await?)
    }

    pub(crate) async fn get_credential_issuers(
        &self,
        dkg_epoch: u64,
    ) -> Result<Vec<CredentialIssuer>, NymRewarderError> {
        self.inner
            .read()
            .await
            .get_all_verification_key_shares(dkg_epoch)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect()
    }

    pub(crate) async fn get_deposit_transaction_attributes(
        &self,
        tx_hash: Hash,
    ) -> Result<(String, String), NymRewarderError> {
        let tx = self.inner.read().await.get_tx(tx_hash).await?;

        // todo: we need to make it more concrete that the first attribute is the deposit value
        // and the second one is the deposit info
        let deposit_value =
            find_tx_attribute(&tx, COSMWASM_DEPOSITED_FUNDS_EVENT_TYPE, DEPOSIT_VALUE)
                .ok_or(NymRewarderError::DepositValueNotFound { tx_hash })?;

        let deposit_info =
            find_tx_attribute(&tx, COSMWASM_DEPOSITED_FUNDS_EVENT_TYPE, DEPOSIT_INFO)
                .ok_or(NymRewarderError::DepositInfoNotFound { tx_hash })?;

        Ok((deposit_value, deposit_info))
    }
}