//! Root-relative inclusion for the exact first-release retail policy and marker.
//!
//! This checks two values against one supplied accumulated contract-state root.
//! It does not authenticate that root, its finalized height, or its freshness.
//! In particular, Sumeragi-v2's per-block execution-witness `post_state_root`
//! must never be supplied as the accumulated map root.

use crate::{
    asset::{
        RetailDailyActivationV1, RetailDailyLimitPolicyV1,
        retail_daily_limit::{retail_activation_state_path_v1, retail_policy_state_path_v1},
    },
    contract_state_proof::ContractStateValueInclusionProofV1,
};
use iroha_crypto::Hash;

/// A root-relative check failed. None of these errors establishes root authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RetailStateInclusionErrorV1 {
    /// The independently expected first-release policy is malformed.
    #[error("expected first-release retail policy is invalid")]
    InvalidExpectedPolicy,
    /// The expected marker is not the policy digest and next UTC day.
    #[error("expected retail activation does not bind the policy and next UTC day")]
    InvalidExpectedActivation,
    /// The policy key/value is not a member of the supplied accumulated root.
    #[error("exact retail policy is not included under the supplied root")]
    PolicyNotIncluded,
    /// The activation key/value is not a member of the same supplied root.
    #[error("exact retail activation is not included under the supplied root")]
    ActivationNotIncluded,
    /// An independently expected value cannot be canonically encoded.
    #[error("expected retail value cannot be canonically encoded")]
    InvalidExpectedEncoding,
}

/// Verify the exact owner-approved policy and finalized historical activation
/// marker as entries in one *caller-supplied* accumulated contract-state root.
///
/// The policy and marker must be independently authenticated before this call.
/// A caller must separately authenticate the accumulated root to the current
/// finalized chain head through a consensus field or reviewed validator quorum
/// attestation before making any current-state claim. Neither this function nor
/// `ContractStateValueInclusionProofV1` performs that missing step.
///
/// # Errors
/// Rejects malformed expected values, a wrong physical path, wrong bytes, a
/// substituted root, or any invalid Merkle map branch.
pub fn verify_retail_policy_activation_against_supplied_root_v1(
    supplied_accumulated_root: Hash,
    expected_policy: &RetailDailyLimitPolicyV1,
    expected_activation: &RetailDailyActivationV1,
    policy_proof: &ContractStateValueInclusionProofV1,
    activation_proof: &ContractStateValueInclusionProofV1,
) -> Result<(), RetailStateInclusionErrorV1> {
    if expected_policy.validate_shape().is_err()
        || expected_policy.revision != 1
        || !expected_policy.institutional_exceptions.is_empty()
    {
        return Err(RetailStateInclusionErrorV1::InvalidExpectedPolicy);
    }
    let policy_bytes = norito::encode_canonical(expected_policy)
        .map_err(|_| RetailStateInclusionErrorV1::InvalidExpectedEncoding)?;
    let following_day = expected_activation
        .activated_at_ms
        .checked_div(86_400_000)
        .and_then(|day| day.checked_add(1))
        .and_then(|day| day.checked_mul(86_400_000))
        .ok_or(RetailStateInclusionErrorV1::InvalidExpectedActivation)?;
    if expected_activation.asset_definition_id != expected_policy.asset_definition_id
        || expected_activation.physical_dataspace != expected_policy.physical_dataspace
        || expected_activation.policy_digest != *Hash::new(&policy_bytes).as_ref()
        || expected_activation.enforce_from_day_start_ms != following_day
    {
        return Err(RetailStateInclusionErrorV1::InvalidExpectedActivation);
    }
    let policy_path = retail_policy_state_path_v1(
        &expected_policy.asset_definition_id,
        expected_policy.physical_dataspace,
    );
    if policy_proof.value != policy_bytes
        || !policy_proof.verify(&policy_path, supplied_accumulated_root)
    {
        return Err(RetailStateInclusionErrorV1::PolicyNotIncluded);
    }
    let activation_bytes = norito::encode_canonical(expected_activation)
        .map_err(|_| RetailStateInclusionErrorV1::InvalidExpectedEncoding)?;
    let activation_path = retail_activation_state_path_v1(&expected_policy.asset_definition_id);
    if activation_proof.value != activation_bytes
        || !activation_proof.verify(&activation_path, supplied_accumulated_root)
    {
        return Err(RetailStateInclusionErrorV1::ActivationNotIncluded);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{account::AccountId, asset::AssetDefinitionId};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::{domain::DomainId, topology::DataSpaceId};
    use iroha_primitives::numeric::Quantity;
    use std::collections::BTreeSet;

    fn fixture() -> (RetailDailyLimitPolicyV1, RetailDailyActivationV1) {
        let issuer =
            KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Ed25519).expect("fixture issuer");
        let reserve =
            KeyPair::try_from_seed(vec![0x52; 32], Algorithm::Ed25519).expect("fixture reserve");
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("kina", "bpng").expect("fixture domain"),
            "pgk".parse().expect("fixture name"),
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
        let bytes = norito::encode_canonical(&policy).expect("canonical policy");
        let marker = RetailDailyActivationV1 {
            asset_definition_id: definition,
            physical_dataspace: policy.physical_dataspace,
            policy_digest: *Hash::new(bytes).as_ref(),
            activated_at_ms: 1000,
            enforce_from_day_start_ms: 86_400_000,
        };
        (policy, marker)
    }

    #[test]
    fn paired_inclusion_binds_exact_paths_values_and_one_accumulated_root() {
        use crate::contract_state_proof::ContractStateMapV1;
        let (policy, marker) = fixture();
        let policy_path =
            retail_policy_state_path_v1(&policy.asset_definition_id, policy.physical_dataspace);
        let marker_path = retail_activation_state_path_v1(&policy.asset_definition_id);
        let unrelated_path = "unrelated/native/state".parse().expect("state path");
        let policy_bytes = norito::encode_canonical(&policy).expect("policy bytes");
        let marker_bytes = norito::encode_canonical(&marker).expect("marker bytes");
        let mut map = ContractStateMapV1::new();
        map.replace(&policy_path, None, Some(&policy_bytes))
            .unwrap();
        let old_root = map.root();
        map.replace(&marker_path, None, Some(&marker_bytes))
            .unwrap();
        map.replace(&unrelated_path, None, Some(b"untouched history"))
            .unwrap();
        let root = map.root();
        let policy_proof = map.proof(&policy_path, &policy_bytes).unwrap();
        let marker_proof = map.proof(&marker_path, &marker_bytes).unwrap();
        assert_eq!(
            verify_retail_policy_activation_against_supplied_root_v1(
                root,
                &policy,
                &marker,
                &policy_proof,
                &marker_proof,
            ),
            Ok(())
        );
        assert_eq!(
            verify_retail_policy_activation_against_supplied_root_v1(
                old_root,
                &policy,
                &marker,
                &policy_proof,
                &marker_proof,
            ),
            Err(RetailStateInclusionErrorV1::PolicyNotIncluded)
        );
        let mut wrong_policy = policy.clone();
        wrong_policy.daily_cap = Quantity::from(6_u32);
        assert_eq!(
            verify_retail_policy_activation_against_supplied_root_v1(
                root,
                &wrong_policy,
                &marker,
                &policy_proof,
                &marker_proof,
            ),
            Err(RetailStateInclusionErrorV1::InvalidExpectedActivation)
        );
        let mut wrong_marker = marker.clone();
        wrong_marker.enforce_from_day_start_ms += 86_400_000;
        assert_eq!(
            verify_retail_policy_activation_against_supplied_root_v1(
                root,
                &policy,
                &wrong_marker,
                &policy_proof,
                &marker_proof,
            ),
            Err(RetailStateInclusionErrorV1::InvalidExpectedActivation)
        );
        let swapped = map.proof(&unrelated_path, b"untouched history").unwrap();
        assert_eq!(
            verify_retail_policy_activation_against_supplied_root_v1(
                root,
                &policy,
                &marker,
                &swapped,
                &marker_proof,
            ),
            Err(RetailStateInclusionErrorV1::PolicyNotIncluded)
        );
    }
}
