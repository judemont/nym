// Copyright 2023 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::epoch_operations::error::RewardingError;
use crate::epoch_operations::helpers::MixnodeWithPerformance;
use crate::RewardedSetUpdater;
use nym_mixnet_contract_common::{EpochState, Interval, MixId};

impl RewardedSetUpdater {
    pub(super) async fn reward_current_rewarded_set(
        &self,
        to_reward: &[MixnodeWithPerformance],
        current_interval: Interval,
    ) -> Result<(), RewardingError> {
        let epoch_status = self.nyxd_client.get_current_epoch_status().await?;
        match epoch_status.state {
            EpochState::InProgress => {
                // hard error, this shouldn't have happened!
                error!("tried to perform node rewarding while the epoch is still in progress!");
                Err(RewardingError::InvalidEpochState {
                    current_state: EpochState::InProgress,
                    operation: "mix rewarding".to_string(),
                })
            }
            EpochState::ReconcilingEvents | EpochState::AdvancingEpoch => {
                warn!("we seem to have crashed mid epoch operations... no need to reward mixnodes as we've already done that! (or this could be a false positive if there were no nodes to reward - to fix this warning later)");
                Ok(())
            }
            EpochState::Rewarding { last_rewarded, .. } => {
                log::info!("Rewarding the current rewarded set...");

                // with how the nym-api is currently coded, this should never happen as we're always
                // rewarding ALL mixnodes at once, but who knows what we might decide to do in the future...
                if last_rewarded != 0 {
                    return Err(RewardingError::MidMixRewarding { last_rewarded });
                }

                if let Err(err) = self
                    ._reward_current_rewarded_set(to_reward, current_interval)
                    .await
                {
                    log::error!("FAILED to reward rewarded set - {err}");
                    Err(err)
                } else {
                    log::info!("Rewarded current rewarded set... SUCCESS");
                    Ok(())
                }
            }
        }
    }

    async fn _reward_current_rewarded_set(
        &self,
        to_reward: &[MixnodeWithPerformance],
        current_interval: Interval,
    ) -> Result<(), RewardingError> {
        if to_reward.is_empty() {
            error!("There are no nodes to reward in this epoch - we shouldn't have been in the 'Rewarding' state!");
        } else if let Err(err) = self.nyxd_client.send_rewarding_messages(to_reward).await {
            error!(
                "failed to perform mixnode rewarding for epoch {}! Error encountered: {err}",
                current_interval.current_epoch_absolute_id(),
            );
            return Err(err.into());
        }

        log::info!("rewarded {} mixnodes...", to_reward.len());

        Ok(())
    }

    pub(crate) async fn nodes_to_reward(&self, interval: Interval) -> Vec<MixnodeWithPerformance> {
        // try to get current up to date view of the network bypassing the cache
        // in case the epochs were significantly shortened for the purposes of testing
        let rewarded_set: Vec<MixId> = match self.nyxd_client.get_rewarded_set_mixnodes().await {
            Ok(nodes) => nodes.into_iter().map(|(id, _)| id).collect::<Vec<_>>(),
            Err(err) => {
                warn!("failed to obtain the current rewarded set - {err}. falling back to the cached version");
                self.nym_contract_cache
                    .rewarded_set()
                    .await
                    .into_inner()
                    .into_iter()
                    .map(|node| node.mix_id())
                    .collect::<Vec<_>>()
            }
        };

        self.load_nodes_performance(&interval, &rewarded_set).await
    }
}
