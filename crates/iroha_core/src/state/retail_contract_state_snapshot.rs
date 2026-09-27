//! Local accumulated contract-state proofs for the exact retail policy and marker.
//!
//! A `StateView` captures the world and block-hash journal in one publication
//! generation. Its locally derived map root is not a consensus commitment and
//! must not be used for admission until finality authenticates that exact root.

use super::{StateView, WorldReadOnly};
use iroha_crypto::{Hash, HashOf, MerkleMapError};
use iroha_data_model::{
    asset::{
        RetailDailyActivationV1, RetailDailyLimitPolicyV1,
        retail_daily_limit::{retail_activation_state_path_v1, retail_policy_state_path_v1},
    },
    block::{
        BlockHeader,
        retail_state_map_inclusion::{
            RetailStateInclusionErrorV1, verify_retail_policy_activation_against_supplied_root_v1,
        },
    },
    contract_state_proof::{ContractStateMapV1, ContractStateValueInclusionProofV1},
};
use mv::storage::StorageReadOnly;

/// Why one local exact-value capture was refused.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RetailContractStateSnapshotErrorV1 {
    /// A pre-genesis view has no committed height to associate with the map.
    #[error("retail contract-state snapshot has no committed block")]
    NoCommittedBlock,
    /// The host height cannot be represented on the wire.
    #[error("retail contract-state snapshot height exceeds u64")]
    HeightOverflow,
    /// The exact physical policy entry is absent.
    #[error("retail contract-state snapshot has no exact policy entry")]
    MissingPolicy,
    /// The exact physical activation entry is absent.
    #[error("retail contract-state snapshot has no exact activation entry")]
    MissingActivation,
    /// The accumulated map could not capture every current physical entry.
    #[error("retail contract-state map capture failed: {0}")]
    Map(#[from] MerkleMapError),
    /// The exact entry cannot be represented within the selective proof bound.
    #[error("retail contract-state proof exceeds its bound or is absent")]
    ProofUnavailable,
    /// The candidate values do not match the independently expected policy.
    #[error("retail contract-state proof does not match expected values: {0}")]
    Inclusion(#[from] RetailStateInclusionErrorV1),
}

/// One local, unauthenticated map root and two selective exact-value proofs.
///
/// The height and hash are from the same committed `StateView`, but neither
/// authenticates `accumulated_root` to Sumeragi finality. This type is private
/// to Core so a response route cannot publish it without a reviewed finality
/// field and restricted-dataspace permission check.
pub(crate) struct LocalRetailContractStateSnapshotV1 {
    pub(crate) height: u64,
    pub(crate) block_hash: HashOf<BlockHeader>,
    pub(crate) accumulated_root: Hash,
    pub(crate) policy_proof: ContractStateValueInclusionProofV1,
    pub(crate) activation_proof: ContractStateValueInclusionProofV1,
}

impl StateView<'_> {
    /// Cold-capture the complete current contract-state map and prove only the
    /// expected retail policy and activation marker from this State generation.
    ///
    /// The caller must never treat this local root as finalized. A future
    /// selective route needs an exact finalized accumulated-root commitment and
    /// a restricted-dataspace read authorization before returning these bytes.
    ///
    /// # Errors
    /// Refuses pre-genesis, absent, malformed, mismatched or unprovable values.
    pub(crate) fn capture_local_retail_contract_state_v1(
        &self,
        expected_policy: &RetailDailyLimitPolicyV1,
        expected_activation: &RetailDailyActivationV1,
    ) -> Result<LocalRetailContractStateSnapshotV1, RetailContractStateSnapshotErrorV1> {
        let block_hash = self
            .latest_block_hash()
            .ok_or(RetailContractStateSnapshotErrorV1::NoCommittedBlock)?;
        let height = u64::try_from(self.height())
            .map_err(|_| RetailContractStateSnapshotErrorV1::HeightOverflow)?;
        let policy_path = retail_policy_state_path_v1(
            &expected_policy.asset_definition_id,
            expected_policy.physical_dataspace,
        );
        let activation_path = retail_activation_state_path_v1(&expected_policy.asset_definition_id);
        let state = self.world.smart_contract_state();
        let policy_bytes = state
            .get(&policy_path)
            .ok_or(RetailContractStateSnapshotErrorV1::MissingPolicy)?;
        let activation_bytes = state
            .get(&activation_path)
            .ok_or(RetailContractStateSnapshotErrorV1::MissingActivation)?;
        let map = ContractStateMapV1::capture(
            state.iter().map(|(path, value)| (path, value.as_slice())),
        )?;
        let accumulated_root = map.root();
        let policy_proof = map
            .proof(&policy_path, policy_bytes)
            .ok_or(RetailContractStateSnapshotErrorV1::ProofUnavailable)?;
        let activation_proof = map
            .proof(&activation_path, activation_bytes)
            .ok_or(RetailContractStateSnapshotErrorV1::ProofUnavailable)?;
        verify_retail_policy_activation_against_supplied_root_v1(
            accumulated_root,
            expected_policy,
            expected_activation,
            &policy_proof,
            &activation_proof,
        )?;
        Ok(LocalRetailContractStateSnapshotV1 {
            height,
            block_hash,
            accumulated_root,
            policy_proof,
            activation_proof,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{BlockHashes, State, World},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{account::AccountId, asset::AssetDefinitionId};
    use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
    use iroha_primitives::numeric::Quantity;
    use std::{collections::BTreeSet, num::NonZeroU64};

    fn fixture() -> (RetailDailyLimitPolicyV1, RetailDailyActivationV1) {
        let issuer = KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519).unwrap();
        let reserve = KeyPair::try_from_seed(vec![0x62; 32], Algorithm::Ed25519).unwrap();
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("kina", "bpng").unwrap(),
            "pgk".parse().unwrap(),
        );
        let policy = RetailDailyLimitPolicyV1 {
            asset_definition_id: definition.clone(),
            physical_dataspace: DataSpaceId::new(7),
            revision: 1,
            daily_cap: Quantity::from(5_u32),
            identity_issuer: AccountId::new(issuer.public_key().clone()),
            identity_issuer_public_key: issuer.public_key().clone(),
            monetary_issuer_account: AccountId::new(issuer.public_key().clone()),
            reserve_account: AccountId::new(reserve.public_key().clone()),
            institutional_exceptions: BTreeSet::new(),
        };
        let marker = super::super::retail_daily_limit_state::activation_for_policy(&policy, 1_000)
            .expect("valid activation marker");
        (policy, marker)
    }

    #[test]
    fn local_state_view_proves_full_current_map_but_does_not_claim_finality() {
        let (policy, marker) = fixture();
        let world = World::new();
        let policy_path =
            retail_policy_state_path_v1(&policy.asset_definition_id, policy.physical_dataspace);
        let marker_path = retail_activation_state_path_v1(&policy.asset_definition_id);
        let untouched_path: iroha_model_base::state_path::StatePath =
            "unrelated/native/untouched".parse().unwrap();
        let policy_bytes = norito::encode_canonical(&policy).unwrap();
        let marker_bytes = norito::encode_canonical(&marker).unwrap();
        let mut state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        // Test State installs native lane markers at startup; the committed
        // root must include those existing entries as well as our three keys.
        let baseline = state.world.smart_contract_state.view();
        let mut expected_full = ContractStateMapV1::capture(
            baseline
                .iter()
                .map(|(path, value)| (path, value.as_slice())),
        )
        .unwrap();
        drop(baseline);
        expected_full
            .replace(&policy_path, None, Some(&policy_bytes))
            .unwrap();
        expected_full
            .replace(&marker_path, None, Some(&marker_bytes))
            .unwrap();
        expected_full
            .replace(&untouched_path, None, Some(b"earlier block value"))
            .unwrap();
        state
            .world
            .smart_contract_state
            .insert(policy_path.clone(), policy_bytes.clone());
        state
            .world
            .smart_contract_state
            .insert(marker_path.clone(), marker_bytes.clone());
        state
            .world
            .smart_contract_state
            .insert(untouched_path.clone(), b"earlier block value".to_vec());
        assert!(matches!(
            state
                .view()
                .capture_local_retail_contract_state_v1(&policy, &marker),
            Err(RetailContractStateSnapshotErrorV1::NoCommittedBlock)
        ));
        let header = BlockHeader::new(NonZeroU64::new(1).unwrap(), None, None, 1_000, 0);
        state.block_hashes = BlockHashes::new(vec![header.hash()]);
        let view = state.view();
        let snapshot = view
            .capture_local_retail_contract_state_v1(&policy, &marker)
            .expect("exact local inclusion");
        assert_eq!(snapshot.height, 1);
        assert_eq!(snapshot.block_hash, header.hash());
        assert!(
            snapshot
                .policy_proof
                .verify(&policy_path, snapshot.accumulated_root)
        );
        assert!(
            snapshot
                .activation_proof
                .verify(&marker_path, snapshot.accumulated_root)
        );
        assert_eq!(snapshot.accumulated_root, expected_full.root());
        let mut wrong_policy = policy.clone();
        wrong_policy.daily_cap = Quantity::from(6_u32);
        assert!(matches!(
            view.capture_local_retail_contract_state_v1(&wrong_policy, &marker),
            Err(RetailContractStateSnapshotErrorV1::Inclusion(_))
        ));
    }
}
