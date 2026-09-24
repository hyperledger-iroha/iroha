//! Fail-closed work preflight for one native qPCS Merkle opening.
//!
//! This private prerequisite derives a conservative hash-work bound from the
//! canonical six-lane leaf and node frames. It deliberately does not authenticate
//! a proof, account for transcript/query-binding hashes or source arithmetic,
//! or grant a qPCS receipt. The original proof-session budget is not yet carried
//! into the source-bound qPCS verifier, so no production caller uses this cut.
//! TODO: retain the original session budget through the source/qPCS handoff and
//! add the remaining qPCS hash and arithmetic work before any authentication.

#![allow(
    dead_code,
    reason = "the original source-session budget has not reached the private qPCS verifier"
)]

use super::{
    rns_native_qpcs_leaf::RnsNativeOracleV1,
    rns_native_resource_budget::{RnsNativeProofResourceBudgetV1, RnsNativeResourceErrorV1},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeQpcsOpeningWorkErrorV1 {
    InvalidOracle,
    InvalidCount,
    ArithmeticOverflow,
    Resource(RnsNativeResourceErrorV1),
}

/// Upper bound for hashing one canonical multiproof's opened leaves and paths.
/// Each opened leaf is hashed once as a payload and once with its position; at
/// most one ancestor per tree level is hashed for each opened leaf. The bound
/// intentionally charges even if the one-entry public-leaf cache hits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RnsNativeQpcsOpeningHashWorkV1 {
    field_operations: u64,
}

impl RnsNativeQpcsOpeningHashWorkV1 {
    pub(super) fn for_opened_leaves_v1(
        parameter_digest: [u8; 32],
        oracle: RnsNativeOracleV1,
        opened_leaves: u64,
    ) -> Result<Self, RnsNativeQpcsOpeningWorkErrorV1> {
        let tree_leaves = u64::from(
            oracle
                .length()
                .map_err(|_| RnsNativeQpcsOpeningWorkErrorV1::InvalidOracle)?,
        );
        if opened_leaves == 0 || opened_leaves > tree_leaves {
            return Err(RnsNativeQpcsOpeningWorkErrorV1::InvalidCount);
        }
        let [payload, index, node] = oracle
            .full_tree_frame_work(parameter_digest)
            .map_err(|_| RnsNativeQpcsOpeningWorkErrorV1::InvalidOracle)?;
        let frame_operations = |multiplications: u64, additions: u64| {
            multiplications
                .checked_add(additions)
                .ok_or(RnsNativeQpcsOpeningWorkErrorV1::ArithmeticOverflow)
        };
        let leaf_operations =
            frame_operations(payload.field_multiplications, payload.field_additions)?
                .checked_add(frame_operations(
                    index.field_multiplications,
                    index.field_additions,
                )?)
                .ok_or(RnsNativeQpcsOpeningWorkErrorV1::ArithmeticOverflow)?;
        let node_operations = frame_operations(node.field_multiplications, node.field_additions)?;
        let depth = u64::from(tree_leaves.ilog2());
        let per_leaf = leaf_operations
            .checked_add(
                depth
                    .checked_mul(node_operations)
                    .ok_or(RnsNativeQpcsOpeningWorkErrorV1::ArithmeticOverflow)?,
            )
            .ok_or(RnsNativeQpcsOpeningWorkErrorV1::ArithmeticOverflow)?;
        Ok(Self {
            field_operations: checked_opening_work_v1(opened_leaves, per_leaf)?,
        })
    }

    pub(super) const fn field_operations_v1(self) -> u64 {
        self.field_operations
    }

    /// Debit the caller's existing original-session ledger. Work is permanent
    /// even when later proof authentication fails; no fresh budget is created.
    pub(super) fn admit_v1(
        self,
        original_budget: &mut RnsNativeProofResourceBudgetV1,
    ) -> Result<(), RnsNativeQpcsOpeningWorkErrorV1> {
        let _reservation = original_budget
            .admit(self.field_operations, 0, 0)
            .map_err(RnsNativeQpcsOpeningWorkErrorV1::Resource)?;
        Ok(())
    }
}

fn checked_opening_work_v1(
    opened_leaves: u64,
    per_leaf: u64,
) -> Result<u64, RnsNativeQpcsOpeningWorkErrorV1> {
    opened_leaves
        .checked_mul(per_leaf)
        .ok_or(RnsNativeQpcsOpeningWorkErrorV1::ArithmeticOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::zk_ams::mkhe::{
        rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1,
        rns_native_proof_hash::RnsNativeProofHashContextV1,
    };

    fn parameter_v1() -> [u8; 32] {
        RnsNativeProofHashContextV1::canonical()
            .unwrap()
            .parameter_digest()
    }

    #[test]
    fn canonical_frames_bound_opening_hashes_without_discounting_cache_hits() {
        let work = RnsNativeQpcsOpeningHashWorkV1::for_opened_leaves_v1(
            parameter_v1(),
            RnsNativeOracleV1::Initial,
            320,
        )
        .unwrap();
        assert_eq!(
            work.field_operations_v1(),
            320 * (4_819_350 + 487_008 + 19 * 476_862)
        );
        assert!(matches!(
            RnsNativeQpcsOpeningHashWorkV1::for_opened_leaves_v1(
                parameter_v1(),
                RnsNativeOracleV1::Fri { layer: 18 },
                1,
            ),
            Err(RnsNativeQpcsOpeningWorkErrorV1::InvalidOracle)
        ));
        assert!(matches!(
            RnsNativeQpcsOpeningHashWorkV1::for_opened_leaves_v1(
                parameter_v1(),
                RnsNativeOracleV1::Initial,
                0,
            ),
            Err(RnsNativeQpcsOpeningWorkErrorV1::InvalidCount)
        ));
    }

    #[test]
    fn exact_cap_and_one_over_use_the_same_original_ledger() {
        let work = RnsNativeQpcsOpeningHashWorkV1::for_opened_leaves_v1(
            parameter_v1(),
            RnsNativeOracleV1::Fri { layer: 17 },
            4,
        )
        .unwrap();
        let mut original = RnsNativeProofResourceBudgetV1::default();
        let unrelated = RnsNativeProofResourceBudgetV1::default();
        let retained = original.reserve_workspace_v1(8, 0).unwrap();
        assert!(retained.belongs_to_v1(&original));
        assert!(!retained.belongs_to_v1(&unrelated));
        original
            .charge(ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1 - work.field_operations_v1())
            .unwrap();
        work.admit_v1(&mut original).unwrap();
        assert_eq!(
            original.consumed().unwrap(),
            ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1
        );
        assert_eq!(original.live_bytes().unwrap(), 8);
        assert_eq!(unrelated.consumed().unwrap(), 0);

        let mut over = RnsNativeProofResourceBudgetV1::default();
        over.charge(ZK_AMS_MKHE_RNS_NATIVE_WORK_MAX_V1 - work.field_operations_v1() + 1)
            .unwrap();
        let prior = over.consumed().unwrap();
        assert!(matches!(
            work.admit_v1(&mut over),
            Err(RnsNativeQpcsOpeningWorkErrorV1::Resource(
                RnsNativeResourceErrorV1::WorkLimit
            ))
        ));
        assert_eq!(over.consumed().unwrap(), prior);
    }

    #[test]
    fn checked_work_multiplication_rejects_overflow() {
        assert_eq!(
            checked_opening_work_v1(u64::MAX, 2),
            Err(RnsNativeQpcsOpeningWorkErrorV1::ArithmeticOverflow)
        );
    }
}
