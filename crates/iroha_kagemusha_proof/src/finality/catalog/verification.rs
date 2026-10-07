//! Complete source verification without downloading server proving tables.

use std::{collections::BTreeSet, io::Read};

use iroha_pasta::msm::MemoryBudget;
use sha2::{Digest, Sha256};

use super::*;
use crate::finality::{
    continuity::producer::verification::{DerivedSource, derive_wrapper},
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
            let reader = self.originals.open(&record.sha256[index])?;
            let mut bytes = Vec::with_capacity(length);
            reader
                .take(u64::try_from(length).map_err(|_| Error::Artifact)? + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Artifact)?;
            if bytes != expected {
                return Err(Error::Artifact);
            }
        }
        self.consumed.insert(name);
        Ok(())
    }

    fn derive<C: SourceCircuit>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<DerivedSource, CompileError> {
        let derived = DerivedSource::derive(source, &self.params.vesta, self.limits.msm_budget)
            .map_err(|error| failure(Some(id.clone()), "source VK derivation", error))?;
        self.check(
            &ArtifactId::Source(id.clone()),
            derived.binding(),
            derived.key(),
        )
        .map_err(|error| failure(Some(id), "source VK original", error))?;
        Ok(derived)
    }

    fn complete(&self) -> Result<(), Error> {
        if self.consumed.len() != self.records.len() {
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
        let (source, binding, key) = derive_wrapper(
            &[derived],
            &self.params.pallas,
            &self.params.vesta,
            self.limits.msm_budget,
        )
        .map_err(|error| failure(Some(id.clone()), "wrapper VK derivation", error))?;
        self.check(&ArtifactId::Wrapper(id.clone()), &binding, &key)
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
        let (source, binding, key) = derive_wrapper(
            &[genesis, append],
            &self.params.pallas,
            &self.params.vesta,
            self.limits.msm_budget,
        )
        .map_err(|error| failure(None, "history wrapper VK derivation", error))?;
        if &binding != plan.wrapper_binding() {
            return Err(Error::Artifact.into());
        }
        self.check(&ArtifactId::HistoryWrapper, &binding, &key)
            .map_err(|error| failure(None, "history wrapper VK original", error))?;
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
    let mut qualifier = Qualification::new(records, originals, params, limits)?;
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

#[cfg(test)]
#[path = "verification/tests.rs"]
mod tests;
