//! Closed frame geometry for original full-native finality admission.
//!
//! This selects finite physical corridors; it grants no native certificate or
//! allocation authority. Every destination still charges the original pool.

use std::alloc::Layout;

use iroha_allocation::AllocationCharge;
use iroha_data_model::{
    sumeragi::epoch::MAX_VALIDATORS,
    sumeragi_finality::{FinalityValidator, MAX_FINALITY_BLOCK_BYTES},
};

use super::proof_destination::OwnedProof;

/// Finite frame corridor for native intervals and their original HTTP responses.
///
/// `response_bytes` bounds the retained proof frames and the later encoded
/// response. `maximum_wire_bytes` also bounds the canonical resultless payload:
/// the native query constructor checks that relation before retaining it. The
/// live byte families therefore require `max(R + 2*B, 2*R)`. Exact destination
/// controls, compact BLS keys/PoPs and two legal source certificate sets are
/// additional. The prepared native decoder also charges DA commitments/policies,
/// penalty custody and other owned children to this unchanged pool. This formula
/// does not establish admission for every legal combination of those children;
/// their actual charges can refuse, as can malformed leaf extents.
///
/// Returns `None` for an invalid interval shape or arithmetic/layout overflow.
/// Neither this corridor nor the separate graph corridor promises acceptance of
/// every legal native wire shape. They retain finite physical and codec limits.
#[must_use]
pub fn native_finality_frame_budget_bytes(
    response_bytes: usize,
    maximum_wire_bytes: usize,
    maximum_proofs: usize,
) -> Option<usize> {
    if response_bytes == 0
        || !(1..=MAX_FINALITY_BLOCK_BYTES).contains(&maximum_wire_bytes)
        || !(1..=64).contains(&maximum_proofs)
    {
        return None;
    }
    let source_and_gap = maximum_wire_bytes
        .checked_mul(2)?
        .checked_add(response_bytes)?;
    let encoding = response_bytes.checked_mul(2)?;
    let compact_key = iroha_crypto::bls::consensus::PUBLIC_KEY_LEN.checked_add(1)?;
    let credentials = compact_key
        .checked_add(iroha_crypto::bls::consensus::SIGNATURE_LEN)?
        .checked_mul(MAX_VALIDATORS)?;
    let charge_count = MAX_VALIDATORS.checked_mul(2)?.checked_add(2)?;
    let proof_controls = Layout::array::<FinalityValidator>(MAX_VALIDATORS)
        .ok()?
        .size()
        .checked_add(Layout::array::<AllocationCharge>(charge_count).ok()?.size())?
        .checked_add(credentials)?
        .checked_mul(maximum_proofs)?
        .checked_add(Layout::array::<OwnedProof>(maximum_proofs).ok()?.size())?;
    source_and_gap
        .max(encoding)
        .checked_add(proof_controls)?
        .checked_add(crate::sumeragi::certified_chain::native_finality_source_control_bytes()?)
        .filter(|bytes| Layout::array::<u8>(*bytes).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_allocation::AllocationBudget;

    #[test]
    fn native_frame_profile_retains_both_source_and_encoding_high_waters() {
        let wire = MAX_FINALITY_BLOCK_BYTES;
        let controls = native_finality_frame_budget_bytes(1, wire, 64).unwrap() - (1 + 2 * wire);
        for response in [1, 10 * 1024 * 1024, 64_000_000, 3 * wire] {
            let expected = (response + 2 * wire).max(2 * response) + controls;
            let actual = native_finality_frame_budget_bytes(response, wire, 64).unwrap();
            assert_eq!(actual, expected);
            let pool = AllocationBudget::new(actual);
            let original = pool
                .try_reserve(Layout::array::<u8>(actual).unwrap())
                .unwrap();
            assert!(pool.try_reserve(Layout::new::<u8>()).is_err());
            drop(original);
            assert_eq!(pool.reserved_bytes(), 0);
        }
        assert!(
            native_finality_frame_budget_bytes(64_000_000, wire, 1).unwrap()
                < native_finality_frame_budget_bytes(64_000_000, wire, 64).unwrap()
        );
    }

    #[test]
    fn native_frame_profile_refuses_invalid_extents_without_saturating() {
        for (response, wire, count) in [
            (0, 1, 1),
            (1, 0, 1),
            (1, MAX_FINALITY_BLOCK_BYTES + 1, 1),
            (1, 1, 0),
            (1, 1, 65),
            (usize::MAX, 1, 1),
            (usize::MAX / 2, MAX_FINALITY_BLOCK_BYTES, 64),
        ] {
            assert!(native_finality_frame_budget_bytes(response, wire, count).is_none());
        }
    }

    #[test]
    fn native_compact_credential_geometry_matches_the_actual_key_owner() {
        let pair =
            iroha_crypto::KeyPair::from_seed(vec![7; 32], iroha_crypto::Algorithm::BlsNormal);
        assert_eq!(
            pair.public_key().retained_allocation_layout().size(),
            iroha_crypto::bls::consensus::PUBLIC_KEY_LEN + 1
        );
        assert_eq!(
            iroha_crypto::bls_normal_pop_prove(pair.private_key())
                .unwrap()
                .len(),
            iroha_crypto::bls::consensus::SIGNATURE_LEN
        );
    }
}
