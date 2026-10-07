//! Offline compilation of the exact ordinary-finality source catalog.
//!
//! Compilation is explicit tooling, never part of wallet opening or proof
//! production. Originals are emitted to bounded storage and reimported against
//! their concrete compiled owners. A successful build grants no signed-genesis,
//! artifact-admission or finality authority and proves no live statement.

use std::collections::BTreeMap;

use ff::{Field, PrimeField};
use iroha_pasta::{Eq, Fp, PastaCurve};
use iroha_plonk::{
    DescriptorBinding, ProvingKey, VerifyingKey,
    cs::InstanceType,
    frontend::Circuit,
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2},
};
use iroha_plonk_recursion::FOLD_WITNESS_BYTES;

use super::{
    continuity::{
        SourceMergeCircuit, SourcePairPlan, SourceVerifier,
        producer::{Error, OriginalArtifact, Prover, SourceCircuit},
        tree::{OriginalBytes, SourceIdentity},
    },
    history::HistoryAnchor,
    native::{
        ArtifactId, ArtifactSource, Composition, ImportLimits, InstalledFinality, NodeId,
        Parameters, Program,
    },
};
use crate::omega::{OmegaCircuit, OmegaPlan, OmegaWitness};

mod builder;
mod recipe;
mod store;
mod streaming;
mod verification;
pub use builder::{Compilation, compile};
pub use recipe::OriginalRecipe;
pub use store::{ArtifactRecord, DirectoryCatalog};
pub use streaming::StreamingCatalog;
pub use verification::{ReceiptVerifier, VerifierBlobSource, VerifierLimits, qualify_receipt};

/// Exact offline compiler phase failure. It is retained for diagnosis and never
/// converted into a partial successful catalog or an alternate source profile.
#[derive(Debug)]
pub struct CompileError {
    /// Exact source node when failure occurred inside its compilation.
    pub node: Option<NodeId>,
    /// Fixed compiler phase, such as source key, wrapper key or strict import.
    pub phase: &'static str,
    /// Original diagnostic, including bounded original sizes when relevant.
    pub detail: String,
    cause: Failure,
}
impl CompileError {
    /// Whether the actual typed lower-level error was cancellation, never inferred
    /// from diagnostic text and never an invalid-source verdict.
    pub fn is_cancelled(&self) -> bool {
        self.cause.is_cancelled()
    }
}
#[derive(Debug)]
enum Failure {
    Source(Error),
    Key(iroha_plonk::keys::KeyError),
    Diagnostic(String),
}
impl Failure {
    fn is_cancelled(&self) -> bool {
        match self {
            Self::Source(error) => error.is_cancelled(),
            Self::Key(error) => error.is_cancelled(),
            Self::Diagnostic(_) => false,
        }
    }
}
impl core::fmt::Display for Failure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(f),
            Self::Key(error) => error.fmt(f),
            Self::Diagnostic(message) => message.fmt(f),
        }
    }
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Source(error)
    }
}
impl From<iroha_plonk::keys::KeyError> for Failure {
    fn from(error: iroha_plonk::keys::KeyError) -> Self {
        Self::Key(error)
    }
}
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::Diagnostic(message)
    }
}
impl core::fmt::Display for CompileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "finality catalog {:?} {}: {}",
            self.node, self.phase, self.detail
        )
    }
}
impl std::error::Error for CompileError {}
impl From<Error> for CompileError {
    fn from(error: Error) -> Self {
        failure(None, "graph", error)
    }
}
fn failure(node: Option<NodeId>, phase: &'static str, error: impl Into<Failure>) -> CompileError {
    let cause = error.into();
    CompileError {
        node,
        phase,
        detail: cause.to_string(),
        cause,
    }
}

/// Caller-recorded source provenance, not an authenticated software attestation.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagemusha_proof::finality::catalog::SourceProvenance")]
pub struct SourceProvenance {
    /// Source revision or explicit uncommitted-candidate label.
    pub revision: String,
    /// SHA-256 of the caller's canonical source-file manifest.
    pub source_manifest_sha256: [u8; 32],
}

/// Explicit output boundary for offline key compilation.
/// Implementations must bound inventory writes and original reads; runtime
/// producers receive only the narrower import-only [`ArtifactSource`] interface.
pub trait ArtifactSink: ArtifactSource {
    /// Register an opaque offline regeneration recipe reconstructed from the
    /// compiled source and exact child keys. Ordinary archival sinks need not retain it.
    /// # Errors
    /// Finite recipe count or storage refusal.
    fn register_recipe(&mut self, _id: &ArtifactId, _recipe: OriginalRecipe) -> Result<(), Error> {
        Ok(())
    }
    /// Persist one original identity without replacing different existing bytes.
    /// # Errors
    /// Duplicate identity with changed bytes, storage failure or finite limit.
    fn store(&mut self, id: &ArtifactId, bytes: &OriginalBytes) -> Result<(), Error>;
}

fn borrowed(original: &OriginalBytes) -> OriginalArtifact<'_> {
    OriginalArtifact {
        descriptor: &original.descriptor,
        verifying_key: &original.verifying_key,
        proving_key: &original.proving_key,
    }
}

fn identity(source: &SourceVerifier) -> Result<SourceIdentity, Error> {
    Ok(SourceIdentity {
        descriptor: *source.binding().digest(),
        key: source.key_digest().map_err(|_| Error::Artifact)?.to_repr(),
    })
}

fn layout<T>(value: Result<T, iroha_plonk::frontend::Error>) -> Result<T, Error> {
    value.map_err(|_| Error::Artifact)
}

fn key_config(types: Vec<InstanceType>, source: bool, limits: ImportLimits) -> KeygenConfigV2 {
    let mut config = KeygenConfigV2::pipa_r(types);
    config.compress_selectors = !source;
    config.coset_cache = CosetCachePolicy::OnDemand;
    config.msm_budget = limits.key.msm_budget;
    config
}

fn original<C: PastaCurve>(
    key: &ProvingKey<C>,
    limits: ImportLimits,
) -> Result<OriginalBytes, String> {
    let bytes = OriginalBytes {
        descriptor: key.binding().encoded().to_vec(),
        verifying_key: key.vk().to_bytes().to_vec(),
        proving_key: key.artifact_bytes_v2().map_err(|error| error.to_string())?,
    };
    store::check_original(&bytes, limits).map_err(|_| {
        format!(
            "original lengths descriptor={} vk={} pk={} exceed bounds (PK cap {})",
            bytes.descriptor.len(),
            bytes.verifying_key.len(),
            bytes.proving_key.len(),
            limits.key.maximum_bytes,
        )
    })?;
    Ok(bytes)
}

fn wrapper(
    binding: DescriptorBinding,
    keys: Vec<VerifyingKey<Eq>>,
    params: &Parameters,
) -> Result<OmegaCircuit, Error> {
    let digests = keys
        .iter()
        .map(|key| key.kagemusha_digest(&binding).map_err(|_| Error::Artifact))
        .collect::<Result<Vec<_>, _>>()?;
    let first = keys.first().ok_or(Error::Artifact)?.clone();
    let plan = layout(OmegaPlan::new(binding, params.vesta.clone(), digests))?;
    let plan = layout(plan.with_key_catalog(keys))?;
    let length = plan.verifier().proof_length();
    Ok(layout(OmegaCircuit::new(
        plan,
        OmegaWitness {
            key: first,
            instances: vec![Fp::ZERO; 69],
            proof: vec![0; length],
            length: u32::try_from(length).map_err(|_| Error::Artifact)?,
            fold: [0; FOLD_WITNESS_BYTES],
        },
    ))?
    .without_witnesses())
}

/// Compile one concrete sealed source and its pinned one-key wrapper. Only
/// verifier metadata survives; all generated originals pass the same strict
/// importer as independently installed artifacts. This is not proof evidence.
struct Compiler<'a> {
    sink: &'a mut dyn ArtifactSink,
    params: Parameters,
    limits: ImportLimits,
    cache: BTreeMap<NodeId, SourceVerifier>,
    emitted: BTreeMap<ArtifactId, usize>,
    total: usize,
}
impl Compiler<'_> {
    fn emit(&mut self, id: ArtifactId, bytes: &OriginalBytes) -> Result<(), Error> {
        let length = store::check_original(bytes, self.limits)?;
        if self.emitted.contains_key(&id) || self.emitted.len() >= self.limits.maximum_artifacts {
            return Err(Error::Artifact);
        }
        let total = self
            .total
            .checked_add(length)
            .filter(|n| *n <= self.limits.maximum_original_bytes)
            .ok_or(Error::Artifact)?;
        self.sink.store(&id, bytes)?;
        self.emitted.insert(id, length);
        self.total = total;
        Ok(())
    }
    fn source<C: SourceCircuit + 'static>(
        &mut self,
        id: NodeId,
        source: &C,
    ) -> Result<SourceVerifier, CompileError> {
        if let Some(previous) = self.cache.get(&id) {
            return Ok(previous.clone());
        }
        self.sink.register_recipe(
            &ArtifactId::Source(id.clone()),
            OriginalRecipe::source(source, &self.params, self.limits),
        )?;
        let key = keygen_pk_v2(
            &self.params.vesta,
            &source.without_witnesses(),
            &key_config(vec![InstanceType::Bounded], true, self.limits),
        )
        .map_err(|error| failure(Some(id.clone()), "source key", error))?;
        let binding = key.binding().clone();
        let vk = key.vk().clone();
        self.emit(
            ArtifactId::Source(id.clone()),
            &original(&key, self.limits)
                .map_err(|error| failure(Some(id.clone()), "source original", error))?,
        )
        .map_err(|error| failure(Some(id.clone()), "source storage", error))?;
        drop(key);
        let blank = wrapper(binding, vec![vk], &self.params)?;
        self.sink.register_recipe(
            &ArtifactId::Wrapper(id.clone()),
            OriginalRecipe::wrapper(&blank, &self.params, self.limits),
        )?;
        let key = keygen_pk_v2(
            &self.params.pallas,
            &blank,
            &key_config(OmegaPlan::instance_types().to_vec(), false, self.limits),
        )
        .map_err(|error| failure(Some(id.clone()), "wrapper key", error))?;
        self.emit(
            ArtifactId::Wrapper(id.clone()),
            &original(&key, self.limits)
                .map_err(|error| failure(Some(id.clone()), "wrapper original", error))?,
        )
        .map_err(|error| failure(Some(id.clone()), "wrapper storage", error))?;
        drop(key);
        let source_original = self.sink.load(&ArtifactId::Source(id.clone()))?;
        let outer_original = self.sink.load(&ArtifactId::Wrapper(id.clone()))?;
        let producer = Prover::from_original_artifacts(
            source,
            borrowed(&source_original),
            borrowed(&outer_original),
            self.params.pallas.clone(),
            self.params.vesta.clone(),
            self.limits.key,
        )
        .map_err(|error| failure(Some(id.clone()), "strict source import", error))?;
        let qualified = producer.qualified_source()?;
        self.cache.insert(id, qualified.clone());
        Ok(qualified)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    #[test]
    fn exact_cancellation_survives_phase_wrapping_without_parsing_messages() {
        let error = failure(None, "strict source import", Error::Cancelled);
        assert!(error.is_cancelled());
        assert!(CompileError::from(Error::Cancelled).is_cancelled());
        assert!(failure(None, "source key", iroha_plonk::keys::KeyError::Cancelled).is_cancelled());
        for error in [Error::Artifact, Error::Input, Error::Proof, Error::Prover] {
            assert!(!CompileError::from(error).is_cancelled());
        }
        assert!(!failure(None, "diagnostic", "Cancelled".to_owned()).is_cancelled());
    }
}
