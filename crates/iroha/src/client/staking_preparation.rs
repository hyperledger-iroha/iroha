//! Request-bound observational staking plans with bounded canonical transport.

use super::*;
use iroha_data_model::nexus::{
    PUBLIC_LANE_PREPARATION_REQUEST_MAX_BYTES, PUBLIC_LANE_PREPARATION_RESPONSE_MAX_BYTES,
    PublicLaneMonetaryPreconditionV1, PublicLaneMonetaryScopeV1, PublicLanePreparationOperationV1,
    PublicLanePreparationRequestV1, PublicLanePreparationV1, PublicLanePreparedPlanV1,
};

impl Client {
    /// Prepare exact signing inputs using a coherent read-only server observation.
    ///
    /// This does not independently authenticate state. Inspect the exact effects
    /// before signing; execution checks expiry and recomputes every monetary leg.
    ///
    /// # Errors
    /// Rejects transport, codec, size, network, echoed intent and plan-shape mismatches.
    pub fn prepare_public_lane_plan(
        &self,
        request: &PublicLanePreparationRequestV1,
    ) -> Result<PublicLanePreparationV1> {
        let body =
            norito::encode_canonical(request).wrap_err("failed to encode staking preparation")?;
        eyre::ensure!(
            body.len() <= PUBLIC_LANE_PREPARATION_REQUEST_MAX_BYTES,
            "staking preparation request exceeds its bound"
        );
        self.ensure_activation_evidence_deadline()?;
        let response = self.send_builder(
            self.default_request(
                HttpMethod::POST,
                join_torii_url(
                    &self.torii_url,
                    iroha_torii_shared::uri::NEXUS_STAKING_PREPARATION,
                ),
            )
            .header("Content-Type", APPLICATION_NORITO)
            .header("Accept", APPLICATION_NORITO)
            .max_response_bytes(PUBLIC_LANE_PREPARATION_RESPONSE_MAX_BYTES)
            .body(body),
        )?;
        let prepared: PublicLanePreparationV1 = Self::decode_canonical_norito_response(
            &response,
            PUBLIC_LANE_PREPARATION_RESPONSE_MAX_BYTES,
            "Failed to prepare staking plan",
        )?;
        validate_response(&prepared, request, self.network_id)?;
        self.ensure_activation_evidence_deadline()?;
        Ok(prepared)
    }
}

fn validate_response(
    prepared: &PublicLanePreparationV1,
    request: &PublicLanePreparationRequestV1,
    network_id: NetworkId,
) -> Result<()> {
    eyre::ensure!(
        &prepared.request == request
            && prepared.network_id == network_id
            && prepared.observed_height > 0
            && prepared.observed_height.checked_add(1) == Some(prepared.assumed_execution_height),
        "staking preparation differs from the requested intent, network or height"
    );
    let expiry = prepared
        .observed_height
        .checked_add(request.valid_for_blocks)
        .ok_or_else(|| eyre!("staking preparation expiry overflow"))?;
    let mut assets = std::collections::BTreeSet::new();
    match (&request.operation, &prepared.plan) {
        (operation, PublicLanePreparedPlanV1::Monetary(plan)) => {
            eyre::ensure!(
                plan.has_canonical_shape()
                    && plan.network_scope == PublicLaneMonetaryScopeV1::Network(network_id)
                    && plan.valid_until_height == expiry,
                "staking preparation returned an invalid monetary plan"
            );
            let matches = match (operation, &plan.precondition) {
                (
                    PublicLanePreparationOperationV1::Registration(intent),
                    PublicLaneMonetaryPreconditionV1::Registration(_),
                ) => {
                    plan.source_asset.account() == &intent.validator && plan.amount == intent.amount
                }
                (
                    PublicLanePreparationOperationV1::Bond(intent),
                    PublicLaneMonetaryPreconditionV1::Bond(_),
                ) => plan.source_asset.account() == &intent.staker && plan.amount == intent.amount,
                (
                    PublicLanePreparationOperationV1::FinalizeUnbond(intent),
                    PublicLaneMonetaryPreconditionV1::Unbond(_),
                ) => plan.destination_asset.account() == &intent.staker,
                _ => false,
            };
            eyre::ensure!(
                matches,
                "staking preparation monetary legs differ from operator intent"
            );
            assets.insert(plan.source_asset.clone());
            assets.insert(plan.destination_asset.clone());
        }
        (
            PublicLanePreparationOperationV1::ClaimRewards(intent),
            PublicLanePreparedPlanV1::Claim(plan),
        ) => {
            eyre::ensure!(
                plan.has_canonical_shape(&intent.recipient)
                    && plan.network_scope == PublicLaneMonetaryScopeV1::Network(network_id)
                    && plan.valid_until_height == expiry
                    && plan.records.len() <= usize::from(intent.max_records)
                    && plan
                        .records
                        .iter()
                        .all(|record| intent.upto_epoch.is_none_or(|cut| record.epoch <= cut)),
                "staking preparation returned an invalid reward claim"
            );
            for source in &plan.sources {
                assets.insert(source.source_asset.clone());
                assets.insert(source.destination_asset.clone());
            }
        }
        _ => return Err(eyre!("staking preparation returned the wrong plan kind")),
    }
    eyre::ensure!(
        prepared.balances.len() == assets.len()
            && prepared
                .balances
                .iter()
                .map(|entry| &entry.asset)
                .eq(assets.iter())
            && assets
                .iter()
                .all(|asset| asset.definition() == &prepared.xor_asset_definition_id),
        "staking preparation balances or canonical XOR identity differ from its exact legs"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::nexus::{PublicLanePrepareClaimV1, PublicLaneRewardClaimPlanV1};
    use iroha_model_base::topology::LaneId;

    #[test]
    fn observational_plan_rejects_network_intent_expiry_and_balance_substitution() {
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"client-staking-preparation",
        )));
        let request = PublicLanePreparationRequestV1 {
            lane_id: LaneId::SINGLE,
            valid_for_blocks: 10,
            operation: PublicLanePreparationOperationV1::ClaimRewards(PublicLanePrepareClaimV1 {
                recipient: iroha_test_samples::ALICE_ID.clone(),
                upto_epoch: None,
                max_records: 64,
                accrued_sources: vec![],
            }),
        };
        let response = PublicLanePreparationV1 {
            request: request.clone(),
            network_id,
            observed_height: 5,
            observed_block_hash: Hash::new(b"block5"),
            observed_ledger_time_ms: 123,
            assumed_execution_height: 6,
            xor_asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw".parse().unwrap(),
            plan: PublicLanePreparedPlanV1::Claim(PublicLaneRewardClaimPlanV1 {
                network_scope: PublicLaneMonetaryScopeV1::Network(network_id),
                valid_until_height: 15,
                expected_state: None,
                records: vec![],
                sources: vec![],
            }),
            balances: vec![],
        };
        validate_response(&response, &request, network_id).unwrap();
        let other = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"other-network",
        )));
        assert!(validate_response(&response, &request, other).is_err());
        let mut changed = response.clone();
        changed.request.valid_for_blocks += 1;
        assert!(validate_response(&changed, &request, network_id).is_err());
        let mut changed = response.clone();
        changed.assumed_execution_height += 1;
        assert!(validate_response(&changed, &request, network_id).is_err());
        let mut changed = response.clone();
        changed
            .balances
            .push(iroha_data_model::nexus::PublicLanePreparationBalanceV1 {
                asset: iroha_data_model::asset::AssetId::new(
                    changed.xor_asset_definition_id.clone(),
                    iroha_test_samples::ALICE_ID.clone(),
                ),
                balance: Quantity::zero(),
                stake_reserved: Quantity::zero(),
                rewards_reserved: Quantity::zero(),
            });
        assert!(validate_response(&changed, &request, network_id).is_err());
        let mut changed = response;
        let PublicLanePreparedPlanV1::Claim(plan) = &mut changed.plan else {
            unreachable!()
        };
        plan.valid_until_height += 1;
        assert!(validate_response(&changed, &request, network_id).is_err());
    }
}
