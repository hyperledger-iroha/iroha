//! Consume exact native grouping derivations before scoped canonical encoding.

use super::super::grouped_ownership::{
    CheckedAccountRekeys, CheckedAssetDefinitions, CheckedAssets, CheckedContractAliases,
    CheckedContractSubjects, CheckedEscrows, CheckedNfts, CheckedProofRecords,
    CheckedRepoAgreements, CheckedRwas, CheckedValidationFeeProposals, CheckedVerifyingKeys,
    GroupedOwnershipError,
};
use super::*;
use mv::{PublicationPreparationError, storage::StorageReadOnly};

// All concrete captures retain all original readers through encoding and the
// final native identity check. The State fence rejects mixed publication cuts.
// This remains scoped preparation, never a finalized execution anchor.
macro_rules! grouped_capture {
    ($function:ident, $checker:ident, $table:literal) => {
        grouped_capture!($function, $checker, $table, 32);
    };
    ($function:ident, $checker:ident, $table:literal, $work_per_row:literal) => {
        /// Capture exact canonical rows after checking every retained grouped index.
        pub(crate) fn $function(
            state: &State,
            limits: LeafLimits,
        ) -> Result<Option<CanonicalTablePairedSnapshot>, LeafError> {
            let generation = state.state_view_generation();
            if generation & 1 != 0 {
                return Ok(None);
            }
            let budget = state
                .pipeline_ivm_prepared_cache
                .read()
                .execution_budget()
                .clone();
            // Count physical scans, references and partition-range visits. This
            // is a local allowance; larger predecessor images can require a
            // larger admitted row allowance without changing ledger validity.
            let checked =
                $checker::capture(&state.world, limits.max_rows.saturating_mul($work_per_row));
            if !is_stable_state_view_generation(generation, state.state_view_generation()) {
                return Ok(None);
            }
            let checked = match checked {
                Ok(checked) => checked,
                Err(GroupedOwnershipError::Publication(PublicationPreparationError::Changed)) => {
                    return Ok(None)
                }
                Err(error) => return Err(error.into()),
            };
            let snapshot = CanonicalTableLeafSet::paired_table_from_rows(
                $table,
                limits,
                &budget,
                checked.rows().iter(),
            );
            // A refusal still belongs to the retained source cut. Observe its native owners,
            // release the readers, and let State publication win before reporting either error.
            let current = checked.matches_current();
            drop(checked);
            if !is_stable_state_view_generation(generation, state.state_view_generation()) {
                return Ok(None);
            }
            if !current? {
                return Ok(None);
            }
            Ok(Some(snapshot?))
        }
    };
}

grouped_capture!(capture_nfts_once, CheckedNfts, "world.nfts");
grouped_capture!(
    capture_contract_subject_bindings_once,
    CheckedContractSubjects,
    "world.contract_subject_bindings",
    16_384
);

grouped_capture!(
    capture_proofs_once,
    CheckedProofRecords,
    "world.proofs",
    512
);
grouped_capture!(
    capture_governance_proposals_once,
    CheckedValidationFeeProposals,
    "world.governance_proposals",
    8
);
grouped_capture!(
    capture_account_rekey_records_once,
    CheckedAccountRekeys,
    "world.account_rekey_records",
    256
);
grouped_capture!(capture_assets_once, CheckedAssets, "world.assets", 128);
grouped_capture!(
    capture_contract_alias_bindings_once,
    CheckedContractAliases,
    "world.contract_alias_bindings"
);
grouped_capture!(
    capture_asset_definitions_once,
    CheckedAssetDefinitions,
    "world.asset_definitions"
);
grouped_capture!(capture_rwas_once, CheckedRwas, "world.rwas");
grouped_capture!(capture_escrows_once, CheckedEscrows, "world.asset_escrows");
grouped_capture!(
    capture_repo_agreements_once,
    CheckedRepoAgreements,
    "world.repo_agreements"
);

grouped_capture!(
    capture_verifying_keys_once,
    CheckedVerifyingKeys,
    "world.verifying_keys",
    1024
);
