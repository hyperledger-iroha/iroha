//! Read-only join of reviewed source owners and original all-root retirement evidence.
//!
//! Inputs are concrete immutable State publications, never caller-authored root inventories.
//! Children must match the permanent parent registry's exact current cursors. The result keeps
//! those views and the original source handles borrowed; it is neither a wire receipt nor
//! permission to delete code. TODO(N14): require a real source-removed oldest-floor startup
//! rehearsal, including retained codecs and proof verification, before any release verdict.

use std::collections::BTreeMap;

use iroha_data_model::runtime::{
    definition::{
        DEFINITION_FORMAT_VERSION, DEFINITION_ID_DOMAIN, DefinitionDescriptorV1,
        ImplementationCodeV1,
    },
    retirement::MAX_RETIREMENT_DEFINITIONS,
};
use mv::storage::StorageReadOnly as _;

use super::{source_ownership::CheckedSourceManifest, *};
use crate::state::{OriginalRetirementCut, StateQueryView};

/// A source observation failure or the original unfinished evidence attempt.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CandidateError {
    /// Original file acquisition, preimage, review, codec or allocation refusal.
    #[error(transparent)]
    Sources(#[from] source_ownership::Error),
    /// Original authority rejection or local execution refusal, without string conversion.
    #[error("{0}")]
    Evidence(Attempt<Error>),
    /// The original State pool refused the descriptor census counter owner.
    #[error(transparent)]
    Census(#[from] crate::state::RetirementCutError),
}

impl From<Attempt<Error>> for CandidateError {
    fn from(original: Attempt<Error>) -> Self {
        Self::Evidence(original)
    }
}

/// Exact original root publications, in permanent-registry order after the parent.
struct RootCuts<'view, 'state> {
    parent: &'view OriginalRetirementCut<'state>,
    children: &'view [OriginalRetirementCut<'state>],
    parent_root: RuntimeRetirementRootV1,
}

impl<'view, 'state> RootCuts<'view, 'state> {
    fn views(&self) -> impl Clone + Iterator<Item = &'view StateQueryView<'state>> {
        std::iter::once(self.parent)
            .chain(self.children.iter())
            .map(OriginalRetirementCut::view)
    }

    fn new(
        parent: &'view OriginalRetirementCut<'state>,
        children: &'view [OriginalRetirementCut<'state>],
    ) -> Result<Self, Attempt<Error>> {
        let registry = parent.view().world().private_dataspaces();
        registry.validate().map_err(registry_error)?;
        require_child_count(registry, children.len())?;
        let parent_root = original_root(parent.view())?;
        if parent_root.scope != SumeragiRootScope::Global {
            return Err(invalid("source candidate parent is not the global root").into());
        }
        for (record, view) in registry.records().iter().zip(children) {
            bind_child(parent_root.network, record, view.view())?;
        }
        Ok(Self {
            parent,
            children,
            parent_root,
        })
    }
}

fn require_child_count(
    registry: &PrivateDataspaceRegistry,
    supplied: usize,
) -> Result<(), Attempt<Error>> {
    if supplied != registry.records().len() {
        return Err(
            invalid("source candidate requires every registered child exactly once").into(),
        );
    }
    Ok(())
}

/// Require an original non-genesis executed publication and its exact certified tip.
fn original_root(view: &StateQueryView<'_>) -> Result<RuntimeRetirementRootV1, Attempt<Error>> {
    let tip = view
        .native_execution_tip()
        .ok_or_else(|| invalid("source candidate has no original native execution authority"))?;
    if tip.height() != view.height() as u64 || Some(tip.iroha_hash()) != view.latest_block_hash() {
        return Err(invalid("source candidate native tip differs from its State cut").into());
    }
    let height = NonZeroUsize::new(view.height())
        .ok_or_else(|| invalid("source candidate has no executed height"))?;
    let chain = CertifiedChain::new(view).map_err(|error| error.map_rejection(invalid))?;
    let certified = chain
        .certified_from_execution(height, |_, _| Ok(()))
        .map_err(|error| error.map_rejection(invalid))?;
    let original = certified.committed();
    if tip.root_scope() != chain.root_scope()
        || original.root_scope() != chain.root_scope()
        || original.block_hash() != tip.iroha_hash()
        || original.core_hash() != tip.core_hash()
        || original.result() != tip.result()
    {
        return Err(invalid("source candidate tip is not its original certified execution").into());
    }
    let root = RuntimeRetirementRootV1 {
        network: *view.network_id(),
        scope: chain.root_scope(),
        instance: chain.instance().0,
    };
    validate_root(view.world(), root, tip.height()).map_err(invalid)?;
    Ok(root)
}

fn bind_child(
    parent: iroha_data_model::NetworkId,
    record: &PrivateDataspaceRecord,
    view: &StateQueryView<'_>,
) -> Result<(), Attempt<Error>> {
    record.validate().map_err(registry_error)?;
    if !view.world().private_dataspaces().records().is_empty() {
        return Err(invalid("child census contains an unenumerated nested root registry").into());
    }
    let registration = record.anchor.registration();
    if !matches!(registration.scope, SumeragiRootScope::Dataspace { parent_network_id, .. }
        if parent_network_id == parent)
        || view.chain_id() != &registration.child_chain_id
        || view.network_id() != &registration.child_network_id
    {
        return Err(invalid("source candidate child has foreign parent, chain or network").into());
    }
    let tip = view
        .native_execution_tip()
        .ok_or_else(|| invalid("registered child has no original native execution authority"))?;
    let cursor = record.anchor.cursor();
    if (tip.height(), tip.core_hash().0, tip.result().0)
        != (cursor.height, cursor.consensus_hash, cursor.result)
    {
        return Err(
            invalid("source candidate child differs from the registered current decision").into(),
        );
    }
    if original_root(view)? != child_root(record) {
        return Err(invalid("source candidate child scope or instance differs").into());
    }
    Ok(())
}

fn descriptor_identity(value: &DefinitionDescriptorV1) -> Result<DefinitionId, Attempt<Error>> {
    // Structural validity comes from the original compiled catalog or authenticated World
    // publication. Re-running its legacy validator would allocate unowned schema scratch.
    // Stream the exact frozen identity, retaining the serializer's original refusal and
    // allocating no complete descriptor frame for this read-only census.
    norito::core::classify_decode_attempt(|| {
        let length = norito::canonical_frame_len(value)?;
        let length = u64::try_from(length).map_err(|_| norito::Error::LengthMismatch)?;
        let mut encoding = Ok(());
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(DEFINITION_ID_DOMAIN)?;
            writer.write_all(&[DEFINITION_FORMAT_VERSION, value.kind().tag()])?;
            writer.write_all(&length.to_le_bytes())?;
            encoding = norito::core::write_canonical_to_writer(value, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoding?;
        Ok(DefinitionId(hash.into()))
    })
    .map_err(|original| canonical_decode_attempt_error(original, invalid))
}

fn validate_definitions<'a>(
    definitions: impl Iterator<Item = (&'a DefinitionId, &'a DefinitionDescriptorV1)>,
) -> Result<(), Attempt<Error>> {
    for (index, (id, value)) in definitions.enumerate() {
        if index >= MAX_RETIREMENT_DEFINITIONS {
            return Err(invalid("source candidate definition census exceeds capacity").into());
        }
        if descriptor_identity(value)? != *id {
            return Err(
                invalid("source candidate descriptor differs from its immutable identity").into(),
            );
        }
    }
    Ok(())
}

fn code(value: &DefinitionDescriptorV1) -> Option<&ImplementationCodeV1> {
    match value {
        DefinitionDescriptorV1::Implementation(value) => Some(&value.code),
        _ => None,
    }
}

/// Stream the union without allocating a caller-selected alias inventory.
fn visit_aliases<'view, 'state: 'view>(
    owner: &ImplementationCodeV1,
    compiled: &BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    views: impl Clone + Iterator<Item = &'view StateQueryView<'state>>,
    mut visit: impl FnMut(DefinitionId) -> Result<(), Attempt<Error>>,
) -> Result<usize, Attempt<Error>> {
    if !matches!(owner, ImplementationCodeV1::Native { .. })
        || !compiled.values().any(|value| code(value) == Some(owner))
    {
        return Err(
            invalid("reviewed source owner has no exact compiled native implementation").into(),
        );
    }
    let mut aliases = 0usize;
    let mut observe = |id, value: &DefinitionDescriptorV1, duplicate| {
        if matches!(value, DefinitionDescriptorV1::Migration(migration) if &migration.code == owner)
        {
            return Err(invalid("reviewed native source also owns a retained migration").into());
        }
        if duplicate || code(value) != Some(owner) {
            return Ok(());
        }
        aliases = aliases
            .checked_add(1)
            .filter(|count| *count <= MAX_RETIREMENT_DEFINITIONS)
            .ok_or_else(|| invalid("source candidate native aliases exceed capacity"))?;
        visit(id)
    };
    for (id, value) in compiled {
        observe(*id, value, false)?;
    }
    for (index, view) in views.clone().enumerate() {
        for (id, value) in view.world().runtime_definitions().iter() {
            let duplicate = compiled.contains_key(id)
                || views
                    .clone()
                    .take(index)
                    .any(|previous| previous.world().runtime_definitions().get(id).is_some());
            observe(*id, value, duplicate)?;
        }
    }
    Ok(aliases)
}

/// Bind an offered receipt to its permanent row and exact original local certificate.
pub(super) fn verify_local_receipt(
    view: &StateQueryView<'_>,
    root: RuntimeRetirementRootV1,
    definition: DefinitionId,
    proof: &RuntimeRetirementProofV1,
) -> Result<(), Attempt<Error>> {
    if view
        .world()
        .runtime_retirements()
        .get(&definition)
        .and_then(|row| row.receipt.as_ref())
        != Some(&proof.receipt)
    {
        return Err(
            invalid("source candidate receipt differs from its permanent native record").into(),
        );
    }
    proof.validate_bounds().map_err(proof_error)?;
    let height = usize::try_from(proof.receipt.receipt_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| invalid("source candidate receipt height is not representable"))?;
    let chain = CertifiedChain::new(view).map_err(|error| error.map_rejection(invalid))?;
    if root.network != *view.network_id()
        || root.scope != chain.root_scope()
        || root.instance != chain.instance().0
    {
        return Err(invalid("source candidate receipt belongs to another original root").into());
    }
    let certified = chain
        .certified_from_execution(height, |_, _| Ok(()))
        .map_err(|error| error.map_rejection(invalid))?;
    let certificate = certified
        .certificate()
        .ok_or_else(|| invalid("source candidate receipt has no original certificate"))?;
    if proof.block.consensus_header != certificate.consensus_header()
        || proof.block.commit_qc != certificate.commit_qc()
        || proof.block.result_preimage != certificate.result_preimage()
    {
        return Err(
            invalid("source candidate receipt carrier differs from its original frame").into(),
        );
    }
    proof
        .verify_inclusion(
            root,
            definition,
            certified.committed().height(),
            certified
                .committed()
                .commitment()
                .execution
                .ordinary_writes_root,
        )
        .map_err(proof_error)
}

/// Borrowed, read-only evidence of source custody and every original alias/root receipt.
/// No deletion, release eligibility, source-removed replay, or export authority is implied.
pub(crate) struct SourceCandidateEvidence<'view, 'state, 'review> {
    sources: &'view CheckedSourceManifest<'review>,
    cuts: RootCuts<'view, 'state>,
    aliases: usize,
}

impl SourceCandidateEvidence<'_, '_, '_> {
    /// Exact number of reviewed native source owners whose aliases were all checked.
    pub(crate) fn owners(&self) -> usize {
        self.sources.required_native_owners().len()
    }
    /// Exact number of distinct aliases across all reviewed owners.
    pub(crate) fn aliases(&self) -> usize {
        self.aliases
    }
    /// Exact number of original root publications covered for every alias.
    pub(crate) fn roots(&self) -> usize {
        self.cuts.children.len() + 1
    }
    /// Recheck the still-retained original source handles; this grants no removal authority.
    pub(crate) fn recheck_sources(&self) -> Result<(), source_ownership::Error> {
        self.sources.recheck_preimages()
    }
}

/// Join exact reviewed source ownership to compiled and all-root retained native aliases.
/// Missing roots, missing alias history, or a source change yields no evidence object.
/// Offered proofs locate bytes only; the original State cuts authenticate every carrier.
pub(crate) fn verify_source_candidate<'view, 'state, 'review>(
    sources: &'view CheckedSourceManifest<'review>,
    parent: &'view OriginalRetirementCut<'state>,
    children: &'view [OriginalRetirementCut<'state>],
    read: &mut dyn FnMut(
        DefinitionId,
        RuntimeRetirementRootV1,
    ) -> Result<Option<RuntimeRetirementProofV1>, Attempt<Error>>,
) -> Result<SourceCandidateEvidence<'view, 'state, 'review>, CandidateError> {
    sources.recheck_preimages()?;
    let cuts = RootCuts::new(parent, children)?;
    let catalog =
        crate::runtime::release_catalog().map_err(|error| Attempt::Rejected(invalid(error)))?;
    parent.with_census_scope(|| validate_definitions(catalog.definitions.iter()))??;
    for cut in std::iter::once(parent).chain(children.iter()) {
        cut.with_census_scope(|| {
            validate_definitions(cut.view().world().runtime_definitions().iter())
        })??;
    }
    let mut aliases = 0usize;
    for owner in sources.required_native_owners() {
        aliases += visit_aliases(owner, &catalog.definitions, cuts.views(), |alias| {
            for (index, view) in cuts.views().enumerate() {
                let root = if index == 0 {
                    cuts.parent_root
                } else {
                    child_root(&parent.view().world().private_dataspaces().records()[index - 1])
                };
                let proof = read(alias, root)?.ok_or_else(|| {
                    invalid(format!(
                        "reviewed native alias {alias} lacks an original root receipt"
                    ))
                })?;
                verify_local_receipt(view, root, alias, &proof)?;
            }
            Ok(())
        })?;
    }
    sources.recheck_preimages()?;
    Ok(SourceCandidateEvidence {
        sources,
        cuts,
        aliases,
    })
}

#[cfg(test)]
mod tests;
