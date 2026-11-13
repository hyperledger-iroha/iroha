//! Authenticated root coverage for permanent runtime retirement receipts.
//!
//! The complete parent State publication selects membership and child genesis authority.
//! Missing registered roots block coverage. Proofs never select their own committee or scope.
//! TODO(N14): connect the reviewed native source inventory and physical deletion rehearsal
//! before exposing a code-deletion verdict. This module only authenticates
//! coverage of the supplied statements; it is not a release-removal authorization.

use std::num::NonZeroUsize;

mod producer;
pub(crate) use producer::{
    check_publication_capacity, check_transition, enabled as service_enabled, record_block,
    record_governed_exclusions, validate_persistence, validate_root,
};
mod code_coverage;
pub(crate) use code_coverage::{NativeCodeCoverage, verify_native_code_coverage};

/// Discover the latest local receipt and authenticate its original archived ordinary write.
/// Current World values locate evidence; they cannot replace its certified source bytes.
pub(crate) fn receipt_proof(
    view: &impl StateReadOnly,
    definition: DefinitionId,
) -> Result<Option<RuntimeRetirementProofV1>, String> {
    use mv::storage::StorageReadOnly as _;
    validate_persistence(view.world())?;
    let Some(receipt) = view
        .world()
        .runtime_retirements()
        .get(&definition)
        .and_then(|row| row.receipt.as_ref())
        .cloned()
    else {
        return Ok(None);
    };
    let bytes = norito::encode_canonical(&receipt).map_err(|error| error.to_string())?;
    let Some((block, path)) = crate::query::native_receipts::native_ordinary_write_proof(
        view,
        receipt.receipt_height,
        receipt.witness_key(),
        &bytes,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(RuntimeRetirementProofV1 {
        receipt,
        block,
        path,
    }))
}

use iroha_crypto::Hash;
use iroha_data_model::{
    block::consensus::SumeragiRootScope,
    private_dataspace::{
        PrivateDataspaceAnchorError, PrivateDataspaceRecord, PrivateDataspaceRegistry,
    },
    runtime::{
        definition::DefinitionId,
        retirement::{RuntimeRetirementProofV1, RuntimeRetirementRootV1},
    },
    sumeragi_amx::{AmxError, AmxForeignInstanceV1},
    sumeragi_finality::CertifiedResultScope,
};

use crate::{
    execution_attempt::{ExecutionAttemptError as Attempt, canonical_decode_attempt_error},
    state::{StateReadOnly, WorldReadOnly},
    sumeragi::certified_chain::CertifiedChain,
};

/// A completed evidence rejection, distinct from the original local resource refusal.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    /// Membership, source identity, finality, carrier, or write inclusion differs.
    #[error("runtime retirement evidence: {0}")]
    Invalid(String),
}

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}

fn proof_error(error: AmxError) -> Attempt<Error> {
    match error {
        AmxError::Resource(original) => canonical_decode_attempt_error(original, invalid),
        other => invalid(other).into(),
    }
}

fn registry_error(error: PrivateDataspaceAnchorError) -> Attempt<Error> {
    match error {
        PrivateDataspaceAnchorError::Resource(original) => {
            canonical_decode_attempt_error(original, invalid)
        }
        other => invalid(other).into(),
    }
}

/// Exact statement coverage of one authenticated parent registry cut, not deletion eligibility.
pub(crate) struct RetirementCoverage {
    definition: DefinitionId,
    roots: usize,
    evidence: Hash,
}

impl RetirementCoverage {
    /// Immutable definition covered on every registered root and the parent itself.
    pub(crate) fn definition(&self) -> DefinitionId {
        self.definition
    }
    /// Exact number of independently authenticated statements in this cut.
    pub(crate) fn roots(&self) -> usize {
        self.roots
    }
    /// Digest binding membership, the parent publication and all original evidence graphs.
    pub(crate) fn evidence(&self) -> Hash {
        self.evidence
    }
}

fn append_evidence(
    previous: Hash,
    proof: &RuntimeRetirementProofV1,
) -> Result<Hash, Attempt<Error>> {
    let bytes = norito::core::classify_decode_attempt(|| norito::encode_canonical(proof))
        .map_err(|original| canonical_decode_attempt_error(original, invalid))?;
    Ok(Hash::new_from_chunks(&[previous.as_ref(), &bytes]))
}

fn child_root(record: &PrivateDataspaceRecord) -> RuntimeRetirementRootV1 {
    let registration = record.anchor.registration();
    RuntimeRetirementRootV1 {
        network: registration.child_network_id,
        scope: registration.scope,
        instance: registration.instance,
    }
}

/// Authenticate one exact current child decision from its parent-owned registration.
fn verify_child(
    record: &PrivateDataspaceRecord,
    definition: DefinitionId,
    proof: &RuntimeRetirementProofV1,
) -> Result<(), Attempt<Error>> {
    record.validate().map_err(registry_error)?;
    proof.validate_bounds().map_err(proof_error)?;
    let registration = record.anchor.registration();
    // This registered private-root profile fixes its genesis committee for its entire
    // lifetime. Parent validation rejects rotations before they can enter this registry.
    let epoch = norito::core::classify_decode_attempt(|| {
        norito::decode_canonical(&norito::encode_canonical(&registration.initial_epoch)?)
    })
    .map_err(|original| canonical_decode_attempt_error(original, invalid))?;
    let tracker = AmxForeignInstanceV1::new(
        registration.instance,
        epoch,
        CertifiedResultScope::NonGlobal,
    )
    .map_err(proof_error)?;
    let verified = tracker.verify_block(&proof.block).map_err(proof_error)?;
    let cursor = record.anchor.cursor();
    if verified.height != cursor.height
        || verified.block_hash != cursor.consensus_hash
        || verified.result != cursor.result
    {
        return Err(
            invalid("retirement carrier differs from the registered child decision").into(),
        );
    }
    proof
        .verify_inclusion(
            child_root(record),
            definition,
            verified.height,
            verified.commitment().execution.ordinary_writes_root,
        )
        .map_err(proof_error)
}

fn verify_children(
    parent_network: iroha_data_model::NetworkId,
    registry: &PrivateDataspaceRegistry,
    definition: DefinitionId,
    mut evidence: Hash,
    read_child: &mut dyn FnMut(
        RuntimeRetirementRootV1,
    ) -> Result<Option<RuntimeRetirementProofV1>, Attempt<Error>>,
) -> Result<(Hash, usize), Attempt<Error>> {
    registry.validate().map_err(registry_error)?;
    for record in registry.records() {
        let root = child_root(record);
        if !matches!(root.scope, SumeragiRootScope::Dataspace { parent_network_id, .. }
            if parent_network_id == parent_network)
        {
            return Err(invalid("supported child belongs to another parent network").into());
        }
        let proof = read_child(root)?
            .ok_or_else(|| invalid("registered root has no authenticated retirement receipt"))?;
        verify_child(record, definition, &proof)?;
        evidence = append_evidence(evidence, &proof)?;
    }
    Ok((evidence, registry.records().len()))
}

/// Verify the parent's receipt and one exact current receipt from every registered child.
/// Evidence is streamed one root at a time; an unreachable member is never skipped.
/// The original State cut supplies membership, child authority, and the parent's native tip.
/// # Errors
/// Missing roots, substituted identity, invalid finality/path, or original resource refusal.
pub(crate) fn verify_coverage(
    view: &impl StateReadOnly,
    definition: DefinitionId,
    parent: &RuntimeRetirementProofV1,
    read_child: &mut dyn FnMut(
        RuntimeRetirementRootV1,
    ) -> Result<Option<RuntimeRetirementProofV1>, Attempt<Error>>,
) -> Result<RetirementCoverage, Attempt<Error>> {
    validate_persistence(view.world()).map_err(invalid)?;
    use mv::storage::StorageReadOnly as _;
    if view
        .world()
        .runtime_retirements()
        .get(&definition)
        .and_then(|row| row.receipt.as_ref())
        != Some(&parent.receipt)
    {
        return Err(
            invalid("parent receipt differs from its permanent current native record").into(),
        );
    }
    parent.validate_bounds().map_err(proof_error)?;
    let tip = view.native_execution_tip().ok_or_else(|| {
        invalid("supported-root membership has no authenticated native State publication")
    })?;
    if tip.root_scope() != SumeragiRootScope::Global
        || tip.height() != view.height() as u64
        || Some(tip.iroha_hash()) != view.latest_block_hash()
    {
        return Err(
            invalid("supported-root membership differs from the original State cut").into(),
        );
    }
    let registry = view.world().private_dataspaces();
    registry.validate().map_err(registry_error)?;
    let chain = CertifiedChain::new(view).map_err(|error| error.map_rejection(invalid))?;
    let height = usize::try_from(parent.receipt.receipt_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| invalid("retirement height is not representable"))?;
    let certified = chain
        .certified_from_execution(height, |_, _| Ok(()))
        .map_err(|error| error.map_rejection(invalid))?;
    let certificate = certified
        .certificate()
        .ok_or_else(|| invalid("retirement carrier has no original certificate"))?;
    if parent.block.consensus_header != certificate.consensus_header()
        || parent.block.commit_qc != certificate.commit_qc()
        || parent.block.result_preimage != certificate.result_preimage()
    {
        return Err(
            invalid("retirement carrier differs from the original certified parent frame").into(),
        );
    }
    parent
        .verify_inclusion(
            RuntimeRetirementRootV1 {
                network: *view.network_id(),
                scope: SumeragiRootScope::Global,
                instance: chain.instance().0,
            },
            definition,
            certified.committed().height(),
            certified
                .committed()
                .commitment()
                .execution
                .ordinary_writes_root,
        )
        .map_err(proof_error)?;
    let membership = norito::core::classify_decode_attempt(|| norito::encode_canonical(registry))
        .map_err(|original| canonical_decode_attempt_error(original, invalid))?;
    let initial = Hash::new_from_chunks(&[
        b"iroha:runtime:retirement-coverage:v1\0",
        &definition.0,
        tip.iroha_hash().as_ref(),
        &tip.result().0,
        &membership,
    ]);
    let initial = append_evidence(initial, parent)?;
    let (evidence, children) = verify_children(
        *view.network_id(),
        registry,
        definition,
        initial,
        read_child,
    )?;
    Ok(RetirementCoverage {
        definition,
        roots: children + 1,
        evidence,
    })
}

#[cfg(test)]
mod tests;
