//! Scoped canonical table encoding from the original frozen StateBlock.
//!
//! Native callbacks come from the same static declarations as committed readers.
//! They preserve actual native owners and modes, including prepaid storage, and
//! derive the encoding pool from the original State. Raw row encoding does not
//! validate a table's derived indexes, record invariants or cross-table relations.
//! Verifier, proof-status, validation-fee, contract-alias, contract-subject,
//! domain-owner, account-identity, account-alias, asset-definition, asset-balance,
//! escrow, repo-agreement, NFT/RWA and account-rekey adapters check bounded relations.
//!
//! TODO: adapt every other structural group, trigger Set, Musubi semantic source
//! and membership pair/frontier. Then retain every canonical cell and history
//! owner, jointly verify all original owners/modes/predecessors, and integrate
//! with the sole StatePublication owner. There is deliberately no aggregate
//! success, publication hook, complete root, finality token or alternate policy.

use super::{
    CanonicalTablePairedSnapshot, LeafError, LeafLimits, STATE_FIELDS, StateBlock,
    TABLE_MATERIALIZERS, TableCaptureError, TableMaterializer, require_exact_table_materializers,
};

/// Why a requested original table has no usable scoped encoder.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error(transparent)]
pub(in crate::state) struct FrozenTableCaptureError(#[from] Failure);

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
enum Failure {
    /// The requested identity is not a canonical table in the closed catalog.
    #[error("requested original State table is not declared in the canonical catalog")]
    UnknownTable,
    /// The table still needs its actual original structural/semantic/group owner.
    #[error("original frozen State table adapter is incomplete: {0}")]
    MissingAdapter(&'static str),
    /// Static catalog identity/group requirements failed before any source access.
    #[error(transparent)]
    Catalog(#[from] TableCaptureError),
    /// Actual canonical encoding, original-pool or checked relation refusal.
    #[error(transparent)]
    Leaf(#[from] LeafError),
}

impl FrozenTableCaptureError {
    /// Return the exact canonical identity still lacking its original adapter.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TODO: original StatePublication consumer must distinguish missing adapters from local capture refusal"
        )
    )]
    pub(in crate::state) fn missing_adapter(&self) -> Option<&'static str> {
        match self.0 {
            Failure::MissingAdapter(table) => Some(table),
            _ => None,
        }
    }
}

/// Encode one declared table from the caller's unchanged original frozen owner.
///
/// `None` preserves the existing scoped-capture convention: the World is not
/// completely frozen or the selected original field does not belong to its State.
/// A missing structural/semantic adapter is a separate exact error, never an
/// empty table. Native callbacks encode all current rows only; this does not
/// establish complete source validation. `max_relation_work` is used by the
/// checked relation only, and grants no new row or execution limits.
/// Every local encoding refusal leaves the same StateBlock available for retry.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "TODO: consume scoped original tables with all cells, relations and history in StatePublication"
    )
)]
pub(in crate::state) fn capture_original_table_once(
    block: &StateBlock<'_>,
    table: &str,
    limits: LeafLimits,
    max_relation_work: u64,
) -> Result<Option<CanonicalTablePairedSnapshot>, FrozenTableCaptureError> {
    require_exact_table_materializers(STATE_FIELDS, TABLE_MATERIALIZERS)
        .map_err(|error| FrozenTableCaptureError(Failure::Catalog(error)))?;
    let Some(owner) = TABLE_MATERIALIZERS
        .iter()
        .find(|owner| owner.table_ids().any(|identity| identity == table))
    else {
        return Err(Failure::UnknownTable.into());
    };
    let result = match *owner {
        TableMaterializer::Single {
            id: "world.account_rekey_records",
            ..
        } => super::super::frozen_account_rekeys::capture(block, limits, max_relation_work),
        TableMaterializer::Native { frozen, .. } => frozen(block, limits),
        TableMaterializer::Single {
            id: "world.domains",
            ..
        } => super::super::frozen_domain_ownership::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.accounts",
            ..
        } => super::super::frozen_account_identity::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.account_aliases",
            ..
        } => super::super::frozen_account_aliases::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.verifying_keys",
            ..
        } => super::super::frozen_verifying_keys::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.proofs", ..
        } => super::super::frozen_proofs::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.governance_proposals",
            ..
        } => {
            super::super::frozen_validation_fee_proposals::capture(block, limits, max_relation_work)
        }
        TableMaterializer::Single {
            id: "world.contract_subject_bindings",
            ..
        } => super::super::frozen_contract_subjects::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.contract_alias_bindings",
            ..
        } => super::super::frozen_contract_aliases::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.asset_definitions",
            ..
        } => super::super::frozen_asset_definitions::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.assets", ..
        } => super::super::frozen_assets::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.asset_escrows",
            ..
        } => super::super::frozen_escrows::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.repo_agreements",
            ..
        } => super::super::frozen_repo_agreements::capture(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.nfts", ..
        } => super::super::frozen_nfts_rwas::capture_nfts(block, limits, max_relation_work),
        TableMaterializer::Single {
            id: "world.rwas", ..
        } => super::super::frozen_nfts_rwas::capture_rwas(block, limits, max_relation_work),
        // The two membership outputs must eventually come from one original
        // owner together with its frontier, never independent raw callbacks.
        // The same rule applies to all semantic/structural checked groups.
        _ => {
            let identity = owner
                .table_ids()
                .find(|identity| *identity == table)
                .expect("requested table belongs to the original catalog entry");
            return Err(Failure::MissingAdapter(identity).into());
        }
    }
    .map_err(|error| FrozenTableCaptureError(Failure::Leaf(error)))?;
    if let Some(snapshot) = result.as_ref() {
        if snapshot.table_id() != table {
            let expected = owner
                .table_ids()
                .find(|identity| *identity == table)
                .expect("requested table belongs to the original catalog entry");
            return Err(Failure::Catalog(TableCaptureError::IdentityMismatch {
                expected,
                actual: snapshot.table_id(),
            })
            .into());
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "frozen/tests.rs"]
mod tests;
