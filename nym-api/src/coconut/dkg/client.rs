// Copyright 2022 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: GPL-3.0-only

use crate::coconut::client::Client;
use crate::coconut::error::CoconutError;
use cw3::{ProposalResponse, Status};
use cw4::MemberResponse;
use nym_coconut_dkg_common::dealer::{DealerDetails, DealerDetailsResponse};
use nym_coconut_dkg_common::types::{
    DealingIndex, EncodedBTEPublicKeyWithProof, Epoch, EpochId, InitialReplacementData, NodeIndex,
    PartialContractDealing, State as ContractState,
};
use nym_coconut_dkg_common::verification_key::{ContractVKShare, VerificationKeyShare};
use nym_contracts_common::IdentityKey;
use nym_dkg::Threshold;
use nym_validator_client::nyxd::cosmwasm_client::logs::{find_attribute, NODE_INDEX};
use nym_validator_client::nyxd::cosmwasm_client::types::ExecuteResult;
use nym_validator_client::nyxd::AccountId;

pub(crate) struct DkgClient {
    inner: Box<dyn Client + Send + Sync>,
}

impl DkgClient {
    pub(crate) fn new<C>(nyxd_client: C) -> Self
    where
        C: Client + Send + Sync + 'static,
    {
        DkgClient {
            inner: Box::new(nyxd_client),
        }
    }

    pub(crate) async fn get_address(&self) -> AccountId {
        self.inner.address().await
    }

    pub(crate) async fn dkg_contract_address(&self) -> Result<AccountId, CoconutError> {
        self.inner.dkg_contract_address().await
    }

    pub(crate) async fn get_current_epoch(&self) -> Result<Epoch, CoconutError> {
        self.inner.get_current_epoch().await
    }

    pub(crate) async fn get_contract_state(&self) -> Result<ContractState, CoconutError> {
        self.inner.contract_state().await
    }

    pub(crate) async fn group_member(&self) -> Result<MemberResponse, CoconutError> {
        self.inner
            .group_member(self.get_address().await.to_string())
            .await
    }

    pub(crate) async fn get_current_epoch_threshold(
        &self,
    ) -> Result<Option<Threshold>, CoconutError> {
        self.inner.get_current_epoch_threshold().await
    }

    pub(crate) async fn get_initial_dealers(
        &self,
    ) -> Result<Option<InitialReplacementData>, CoconutError> {
        self.inner.get_initial_dealers().await
    }

    pub(crate) async fn get_self_registered_dealer_details(
        &self,
    ) -> Result<DealerDetailsResponse, CoconutError> {
        self.inner.get_self_registered_dealer_details().await
    }

    pub(crate) async fn get_current_dealers(&self) -> Result<Vec<DealerDetails>, CoconutError> {
        self.inner.get_current_dealers().await
    }

    pub(crate) async fn get_dealing_status(
        &self,
        epoch_id: EpochId,
        dealing_index: DealingIndex,
    ) -> Result<bool, CoconutError> {
        let address = self.inner.address().await.to_string();

        self.inner
            .get_dealing_status(epoch_id, address, dealing_index)
            .await
            .map(|r| r.dealing_submitted)
    }

    pub(crate) async fn get_dealings(
        &self,
        epoch_id: EpochId,
        dealer: String,
    ) -> Result<Vec<PartialContractDealing>, CoconutError> {
        self.inner.get_dealings(epoch_id, &dealer).await
    }

    pub(crate) async fn get_verification_key_share_status(
        &self,
        epoch_id: EpochId,
    ) -> Result<Option<bool>, CoconutError> {
        let address = self.inner.address().await.to_string();

        self.inner
            .get_verification_key_share(epoch_id, address)
            .await
            .map(|maybe_share| maybe_share.map(|s| s.verified))
    }

    pub(crate) async fn get_verification_key_shares(
        &self,
        epoch_id: EpochId,
    ) -> Result<Vec<ContractVKShare>, CoconutError> {
        self.inner.get_verification_key_shares(epoch_id).await
    }

    pub(crate) async fn list_proposals(&self) -> Result<Vec<ProposalResponse>, CoconutError> {
        self.inner.list_proposals().await
    }

    pub(crate) async fn get_proposal_status(
        &self,
        proposal_id: u64,
    ) -> Result<Status, CoconutError> {
        self.inner.get_proposal(proposal_id).await.map(|p| p.status)
    }

    pub(crate) async fn advance_epoch_state(&self) -> Result<(), CoconutError> {
        self.inner.advance_epoch_state().await
    }

    pub(crate) async fn register_dealer(
        &self,
        bte_key: EncodedBTEPublicKeyWithProof,
        identity_key: IdentityKey,
        announce_address: String,
        resharing: bool,
    ) -> Result<NodeIndex, CoconutError> {
        let res = self
            .inner
            .register_dealer(bte_key, identity_key, announce_address, resharing)
            .await?;
        let node_index = find_attribute(&res.logs, "wasm", NODE_INDEX)
            .ok_or(CoconutError::NodeIndexRecoveryError {
                reason: String::from("node index not found"),
            })?
            .value
            .parse::<NodeIndex>()
            .map_err(|_| CoconutError::NodeIndexRecoveryError {
                reason: String::from("node index could not be parsed"),
            })?;

        Ok(node_index)
    }

    pub(crate) async fn submit_dealing(
        &self,
        dealing: PartialContractDealing,
        resharing: bool,
    ) -> Result<(), CoconutError> {
        self.inner.submit_dealing(dealing, resharing).await?;
        Ok(())
    }

    pub(crate) async fn submit_verification_key_share(
        &self,
        share: VerificationKeyShare,
        resharing: bool,
    ) -> Result<ExecuteResult, CoconutError> {
        self.inner
            .submit_verification_key_share(share.clone(), resharing)
            .await
    }

    pub(crate) async fn vote_verification_key_share(
        &self,
        proposal_id: u64,
        vote_yes: bool,
    ) -> Result<(), CoconutError> {
        self.inner.vote_proposal(proposal_id, vote_yes, None).await
    }

    pub(crate) async fn execute_verification_key_share(
        &self,
        proposal_id: u64,
    ) -> Result<(), CoconutError> {
        self.inner.execute_proposal(proposal_id).await
    }
}
