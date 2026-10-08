//! Complete source verification without downloading server proving tables.

use std::{collections::BTreeSet, io::Read};

use iroha_pasta::msm::MemoryBudget;
use sha2::{Digest, Sha256};

use super::*;
use crate::finality::{
    continuity::producer::verification::{
        DerivedSource, compiled_wrapper, derive_wrapper_cancellable,
    },
    history::{GenesisSourceCircuit, HistoryAppendCircuit, HistoryAppendPlan},
};

/// Content-addressed verifier original access. The qualifier bounds every read.
/// A store cannot select the graph, source circuit, child keys or file paths.
pub trait VerifierBlobSource {
    /// Open exactly the identified original. PK addresses are never requested.
    /// # Errors
    /// Missing or unavailable content; neither may become an empty success.
    fn open(&mut self, sha256: &[u8; 32]) -> Result<Box<dyn Read + '_>, Error>;
}

/// Finite metadata storage and deterministic VK-derivation resources.
#[derive(Clone, Copy, Debug)]
pub struct VerifierLimits {
    /// Maximum exact source/wrapper records in the complete graph.
    pub maximum_artifacts: usize,
    /// Maximum sum of descriptor and VK bytes; server PK bytes are excluded.
    pub maximum_verifier_bytes: usize,
    /// Shared process-wide MSM scratch allocation policy.
    pub msm_budget: MemoryBudget,
}

/// Complete source-qualified receipt verifier under one exact genesis anchor.
/// This does not authenticate genesis or the inventory, prove a receipt, or
/// provide any server/wallet proving capability. The installation owner must
/// authenticate both inputs before using it for monetary acceptance.
pub struct ReceiptVerifier {
    anchor: HistoryAnchor,
    source: SourceVerifier,
    history: SourceVerifier,
    vesta: iroha_plonk::pcs::ipa::PinnedParams<Eq>,
}
impl ReceiptVerifier {
    /// Exact independently authenticated anchor the caller must bind to its installation.
    pub const fn anchor(&self) -> &HistoryAnchor {
        &self.anchor
    }

    /// Qualified compiled receipt source, including every graph dependency.
    pub const fn source(&self) -> &SourceVerifier {
        &self.source
    }

    /// Restore the opaque terminal history capability under this qualified fixed graph.
    /// The state authenticates only its terminal result, never an arbitrary earlier block.
    /// No proving key is opened and no caller checkpoint or acceptance verdict is trusted.
    /// # Errors
    /// Changed anchor/key/endpoints, malformed state/proof, failed curve claims or cancellation.
    pub fn restore_history(
        &self,
        state: &crate::finality::history::HistoryState,
        evidence: crate::finality::continuity::SourceNodeEvidence,
        budget: MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<crate::finality::native::HistoryPrefix, Error> {
        crate::finality::native::HistoryPrefix::restore(
            &self.anchor,
            &self.history,
            &self.vesta,
            state,
            evidence,
            budget,
            cancellation,
        )
    }

    /// Verify the complete singleton receipt statement under this exact source,
    /// including its original proof and both carried accumulator decisions.
    /// # Errors
    /// Foreign receipt/anchor/endpoints, malformed proof, wrong key or failed claim.
    pub fn verify_receipt_evidence(
        &self,
        receipt_digest: Fp,
        evidence: &crate::finality::continuity::SourceNodeEvidence,
        budget: MemoryBudget,
    ) -> Result<(), Error> {
        crate::finality::receipt_finality::verify_receipt_evidence(
            &self.anchor,
            &self.source,
            &self.vesta,
            receipt_digest,
            evidence,
            budget,
        )
    }
}

struct Qualification<'a> {
    originals: &'a mut dyn VerifierBlobSource,
    records: BTreeMap<Vec<u8>, &'a ArtifactRecord>,
    consumed: BTreeSet<Vec<u8>>,
    params: Parameters,
    limits: VerifierLimits,
    cache: BTreeMap<NodeId, SourceVerifier>,
    history: Option<SourceVerifier>,
    cancellation: Option<iroha_pasta::CancellationToken>,
    recipe_limits: Option<ImportLimits>,
    recipes: BTreeMap<Vec<u8>, (ArtifactRecord, OriginalRecipe)>,
}
impl<'a> Qualification<'a> {
    fn new(
        records: &'a [ArtifactRecord],
        originals: &'a mut dyn VerifierBlobSource,
        params: Parameters,
        limits: VerifierLimits,
    ) -> Result<Self, Error> {
        if params.pallas.k() != 16
            || params.vesta.k() != 16
            || records.is_empty()
            || records.len() > limits.maximum_artifacts
            || limits.maximum_artifacts > 65_536
            || limits.maximum_verifier_bytes == 0
        {
            return Err(Error::Artifact);
        }
        let mut total = 0usize;
        let mut previous: Option<&[u8]> = None;
        let mut inventory = BTreeMap::new();
        for record in records {
            record.validate_identity()?;
            if previous.is_some_and(|name| name >= record.name.as_slice()) {
                return Err(Error::Artifact);
            }
            previous = Some(&record.name);
            for (index, cap) in [1 << 20, 1 << 18, 1 << 30].into_iter().enumerate() {
                let length = usize::try_from(record.lengths[index]).map_err(|_| Error::Artifact)?;
                if length == 0 || length > cap || record.sha256[index] == [0; 32] {
                    return Err(Error::Artifact);
                }
                if index < 2 {
                    total = total
                        .checked_add(length)
                        .filter(|total| *total <= limits.maximum_verifier_bytes)
                        .ok_or(Error::Artifact)?;
                }
            }
            inventory.insert(record.name.clone(), record);
        }
        Ok(Self {
            originals,
            records: inventory,
            consumed: BTreeSet::new(),
            params,
            limits,
            cache: BTreeMap::new(),
            history: None,
            cancellation: None,
            recipe_limits: None,
            recipes: BTreeMap::new(),
        })
    }

    fn check<C: PastaCurve>(
        &mut self,
        id: &ArtifactId,
        binding: &DescriptorBinding,
        key: &VerifyingKey<C>,
    ) -> Result<(), Error> {
        let name = store::name(id)?;
        let record = self.records.get(&name).ok_or(Error::Artifact)?;
        for (index, expected) in [binding.encoded(), key.to_bytes()].into_iter().enumerate() {
            let length = usize::try_from(record.lengths[index]).map_err(|_| Error::Artifact)?;
            // Compare the derived identity before any storage read/allocation.
            if expected.len() != length
                || <[u8; 32]>::from(Sha256::digest(expected)) != record.sha256[index]
            {
                return Err(Error::Artifact);
            }
            iroha_pasta::CancellationToken::checkpoint(self.cancellation.as_ref())?;
            let mut reader = self.originals.open(&record.sha256[index])?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| Error::Artifact)?;
            let mut chunk = vec![0; 64 * 1024].into_boxed_slice();
            loop {
                iroha_pasta::CancellationToken::checkpoint(self.cancellation.as_ref())?;
                let remaining = length + 1 - bytes.len();
                let bound = remaining.min(chunk.len());
                let count = reader
                    .read(&mut chunk[..bound])
                    .map_err(|_| Error::Artifact)?;
                if count == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() > length {
                    return Err(Error::Artifact);
                }
            }
            iroha_pasta::CancellationToken::checkpoint(self.cancellation.as_ref())?;
            if bytes != expected {
                return Err(Error::Artifact);
            }
        }
        self.consumed.insert(name);
        Ok(())
    }

    fn derive<C: SourceCircuit + 'static>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<DerivedSource, CompileError> {
        let derived = DerivedSource::derive_cancellable(
            source,
            &self.params.vesta,
            self.limits.msm_budget,
            self.cancellation.as_ref(),
        )
        .map_err(|error| failure(Some(id.clone()), "source VK derivation", error))?;
        self.check(
            &ArtifactId::Source(id.clone()),
            derived.binding(),
            derived.key(),
        )
        .map_err(|error| failure(Some(id.clone()), "source VK original", error))?;
        if let Some(limits) = self.recipe_limits {
            self.retain_recipe(
                &ArtifactId::Source(id),
                OriginalRecipe::source(source, &self.params, limits),
            )?;
        }
        Ok(derived)
    }

    fn retain_recipe(&mut self, id: &ArtifactId, recipe: OriginalRecipe) -> Result<(), Error> {
        let name = store::name(id)?;
        let record = (*self.records.get(&name).ok_or(Error::Artifact)?).clone();
        if !self.consumed.contains(&name) {
            return Err(Error::Artifact);
        }
        self.recipes.entry(name).or_insert((record, recipe));
        Ok(())
    }

    fn wrapper(
        &mut self,
        id: &ArtifactId,
        derived: &[DerivedSource],
    ) -> Result<SourceVerifier, Error> {
        let (source, binding, key) = derive_wrapper_cancellable(
            derived,
            &self.params.pallas,
            &self.params.vesta,
            self.limits.msm_budget,
            self.cancellation.as_ref(),
        )?;
        self.check(id, &binding, &key)?;
        if let Some(limits) = self.recipe_limits {
            let circuit = compiled_wrapper(derived, &self.params.vesta)?;
            self.retain_recipe(id, OriginalRecipe::wrapper(&circuit, &self.params, limits))?;
        }
        Ok(source)
    }

    fn complete(&self) -> Result<(), Error> {
        if self.consumed.len() != self.records.len()
            || (self.recipe_limits.is_some() && self.recipes.len() != self.records.len())
        {
            return Err(Error::Artifact);
        }
        Ok(())
    }
}

impl builder::Assembler for Qualification<'_> {
    fn params(&self) -> &Parameters {
        &self.params
    }

    fn source<C: SourceCircuit + 'static>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<SourceVerifier, CompileError> {
        if let Some(previous) = self.cache.get(&id) {
            return Ok(previous.clone());
        }
        let derived = self.derive(id.clone(), source)?;
        let source = self
            .wrapper(&ArtifactId::Wrapper(id.clone()), &[derived])
            .map_err(|error| failure(Some(id.clone()), "wrapper VK original", error))?;
        self.cache.insert(id, source.clone());
        Ok(source)
    }

    fn history(
        &mut self,
        anchor: HistoryAnchor,
        body: SourceVerifier,
    ) -> Result<SourceVerifier, CompileError> {
        let genesis = self.derive(NodeId::Genesis, &GenesisSourceCircuit::for_source(anchor))?;
        let plan = layout(HistoryAppendPlan::new(
            anchor,
            body.binding().clone(),
            body,
            self.params.pallas.clone(),
        ))?;
        let append = self.derive(
            NodeId::Append,
            &layout(HistoryAppendCircuit::for_source(plan.clone()))?,
        )?;
        let source = self
            .wrapper(&ArtifactId::HistoryWrapper, &[genesis, append])
            .map_err(|error| failure(None, "history wrapper VK original", error))?;
        if source.binding() != plan.wrapper_binding() {
            return Err(Error::Artifact.into());
        }
        if self.history.replace(source.clone()).is_some() {
            return Err(Error::Artifact.into());
        }
        Ok(source)
    }
}

/// Reconstruct the complete fixed source graph with VK-only key derivation and
/// require exact descriptor/VK bytes for every committed source/wrapper record.
/// Server proving keys are never opened, generated or retained. The caller must
/// authenticate the inventory and exact signed-genesis anchor independently.
/// This is installation work, not a live proof or a readiness verdict.
/// # Errors
/// Bounds, malformed/unused/missing records, changed source/child/anchor keys,
/// unavailable/truncated/extended originals, or compiled source layout failure.
pub fn qualify_receipt(
    anchor: HistoryAnchor,
    records: &[ArtifactRecord],
    originals: &mut dyn VerifierBlobSource,
    params: Parameters,
    limits: VerifierLimits,
) -> Result<ReceiptVerifier, CompileError> {
    qualify_receipt_cancellable(anchor, records, originals, params, limits, None)
}

/// Reconstruct the verifier graph with typed cooperative cancellation.
/// # Errors
/// The same exact-source errors as [`qualify_receipt`], or cancellation.
pub fn qualify_receipt_cancellable(
    anchor: HistoryAnchor,
    records: &[ArtifactRecord],
    originals: &mut dyn VerifierBlobSource,
    params: Parameters,
    limits: VerifierLimits,
    cancellation: Option<&iroha_pasta::CancellationToken>,
) -> Result<ReceiptVerifier, CompileError> {
    iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(Error::from)?;
    let mut qualifier = Qualification::new(records, originals, params, limits)?;
    qualifier.cancellation = cancellation.cloned();
    let source = builder::assemble(&mut qualifier, anchor)?;
    qualifier
        .complete()
        .map_err(|error| failure(None, "unused verifier originals", error))?;
    Ok(ReceiptVerifier {
        anchor,
        source,
        history: qualifier.history.ok_or(Error::Artifact)?,
        vesta: qualifier.params.vesta.clone(),
    })
}

/// Server-only compiled regeneration capability for one completely matched graph.
/// Construction verifies D/V identity, but grants neither signed installation authority
/// nor strict proving-key admission. Every regenerated original still requires import.
pub struct ServerRecipes {
    receipt: ReceiptVerifier,
    params: Parameters,
    imports: ImportLimits,
    sources: BTreeMap<NodeId, SourceVerifier>,
    recipes: BTreeMap<Vec<u8>, (ArtifactRecord, OriginalRecipe)>,
}
impl ServerRecipes {
    /// Construct the fixed proving graph from this complete source-qualified owner.
    /// No proving key is read or generated. Every actual proof still strictly imports
    /// its exact selected originals; this does not assert resident-key readiness.
    /// The application separately authenticates this owner's inventory and anchor.
    /// # Errors
    /// Missing fixed graph metadata, inconsistent topology or cancellation.
    pub fn installed_graph(
        &self,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<InstalledFinality, Error> {
        InstalledFinality::from_qualified_catalog(self, cancellation)
    }

    pub(in crate::finality) fn selected_source(
        &self,
        id: &NodeId,
    ) -> Result<SourceVerifier, Error> {
        self.sources.get(id).cloned().ok_or(Error::Artifact)
    }

    pub(in crate::finality) fn history_source(&self) -> SourceVerifier {
        self.receipt.history.clone()
    }

    pub(in crate::finality) const fn parameters(&self) -> &Parameters {
        &self.params
    }

    pub(in crate::finality) const fn import_limits(&self) -> ImportLimits {
        self.imports
    }
    /// Complete verifier graph whose exact metadata selected these recipes.
    pub const fn receipt(&self) -> &ReceiptVerifier {
        &self.receipt
    }

    /// Regenerate one selected compiled source and compare all canonical original bytes
    /// against its inventory. The caller must strictly import before proof production.
    /// # Errors
    /// Unknown source, cancellation, resource refusal or differing exact original identity.
    pub fn regenerate(
        &self,
        id: &ArtifactId,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<OriginalBytes, Error> {
        let (record, recipe) = self.recipes.get(&store::name(id)?).ok_or(Error::Artifact)?;
        let originals = recipe.regenerate_cancellable(cancellation)?;
        for (role, bytes) in [
            &originals.descriptor,
            &originals.verifying_key,
            &originals.proving_key,
        ]
        .into_iter()
        .enumerate()
        {
            iroha_pasta::CancellationToken::checkpoint(cancellation)?;
            if u64::try_from(bytes.len()).map_err(|_| Error::Artifact)? != record.lengths[role] {
                return Err(Error::Artifact);
            }
            let mut hash = Sha256::new();
            for chunk in bytes.chunks(64 * 1024) {
                iroha_pasta::CancellationToken::checkpoint(cancellation)?;
                hash.update(chunk);
            }
            if <[u8; 32]>::from(hash.finalize()) != record.sha256[role] {
                return Err(Error::Artifact);
            }
        }
        Ok(originals)
    }
}

/// Reconstruct exact D/V and compiled recipes without reading or generating any PK.
/// The installation owner authenticates records, genesis and parameter pins first.
/// No recipe escapes until every expected record is consumed by the fixed graph.
/// # Errors
/// Invalid bounds, missing/unused/duplicate records, source mismatch or cancellation.
pub fn qualify_server_recipes(
    anchor: HistoryAnchor,
    records: &[ArtifactRecord],
    originals: &mut dyn VerifierBlobSource,
    params: Parameters,
    limits: VerifierLimits,
    imports: ImportLimits,
    cancellation: Option<&iroha_pasta::CancellationToken>,
) -> Result<ServerRecipes, CompileError> {
    iroha_pasta::CancellationToken::checkpoint(cancellation).map_err(Error::from)?;
    if imports.maximum_artifacts == 0
        || imports.maximum_artifacts == usize::MAX
        || imports.maximum_original_bytes == 0
        || imports.maximum_original_bytes == usize::MAX
        || imports.maximum_artifacts < records.len()
        || imports.key.maximum_rows != 1 << 16
        || imports.key.maximum_bytes == 0
        || imports.key.maximum_bytes > 1 << 30
    {
        return Err(Error::Artifact.into());
    }
    if records
        .iter()
        .any(|record| record.lengths[2] > imports.key.maximum_bytes as u64)
    {
        return Err(Error::Artifact.into());
    }
    let total = records.iter().try_fold(0usize, |sum, record| {
        record.lengths.iter().try_fold(sum, |sum, &length| {
            sum.checked_add(usize::try_from(length).map_err(|_| Error::Artifact)?)
                .ok_or(Error::Artifact)
        })
    })?;
    if total > imports.maximum_original_bytes {
        return Err(Error::Artifact.into());
    }
    let mut qualifier = Qualification::new(records, originals, params, limits)?;
    qualifier.cancellation = cancellation.cloned();
    qualifier.recipe_limits = Some(imports);
    let source = builder::assemble(&mut qualifier, anchor)?;
    qualifier
        .complete()
        .map_err(|error| failure(None, "server recipe closure", error))?;
    let receipt = ReceiptVerifier {
        anchor,
        source,
        history: qualifier.history.ok_or(Error::Artifact)?,
        vesta: qualifier.params.vesta.clone(),
    };
    Ok(ServerRecipes {
        receipt,
        params: qualifier.params,
        imports,
        sources: qualifier.cache,
        recipes: qualifier.recipes,
    })
}

#[cfg(test)]
#[path = "verification/tests.rs"]
mod tests;
