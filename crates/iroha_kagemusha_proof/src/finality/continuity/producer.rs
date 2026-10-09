//! Original-key production of internal source proofs and their two-curve wrappers.
//!
//! The accepted source types are sealed. Import checks the original proving
//! tables against their witnessless compiled source and exact installed VK; proof
//! production neither generates keys nor infers semantics from a public layout.
//! These are internal interval proofs. The finality owner must still pin the
//! complete source tree, require its full start/end states and join the schedule,
//! quorum, signature and receipt sources before granting Load authority.

use core::{fmt, marker::PhantomData};
use std::sync::Arc;

use ff::Field;
use iroha_pasta::{CancellationToken, Ep, Eq, Fp, msm::MemoryBudget};
use iroha_plonk::{
    DescriptorBinding, Protocol, ProverConfig, ProverRandomness, ProvingKey, VerifyingKey, Witness,
    create_proof_owned_with_claim,
    cs::{CurveV1, InstanceModeV1, InstanceType, ProofSuffixV1, TranscriptV2},
    frontend::Circuit,
    keys::pk::artifact::ReadConfig,
    pcs::ipa::PinnedParams,
    verifier::verify_full_cancellable,
};
use iroha_plonk_recursion::{
    AccumulatorT, FOLD_WITNESS_BYTES, FoldConfig, FoldInput, create_fold, verifier::VerifierPlan,
};

use super::{
    SourceMergeCircuit, SourceNodeEvidence, SourceVerifier, leaf_frame_native,
    pair::{native_binding, native_vesta},
};
use crate::{
    finality::{
        aggregate::AggregateLeafCircuit,
        bls::BlsBatchCircuit,
        certificate::CertificateCircuit,
        certified_result::CertifiedResultCircuit,
        history::{GenesisSourceCircuit, HistoryAppendCircuit, HistoryStepCircuit},
        load_source::LoadSourceCircuit,
        receipt_finality::ReceiptFinalityCircuit,
        result_scan::ResultScanBatchCircuit,
        schedule::{
            complete::ScheduleCircuit, context_hash::ContextHashBatchCircuit,
            source::ScheduleSourceCircuit,
        },
        scheduled_result::ScheduledResultCircuit,
    },
    omega::{OmegaCircuit, OmegaPlan, OmegaWitness},
};

pub(in crate::finality) mod verification;

const DESCRIPTOR_MAX_BYTES: usize = 1 << 20;
const VERIFYING_KEY_MAX_BYTES: usize = 1 << 18;

/// Internal source artifact, input or proof failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Wrong profile, original source tables, independently installed key or bounds.
    Artifact,
    /// Invalid endpoint or inconsistent source frame/claim opening.
    Input,
    /// Original proof, generator decision or fold failed.
    Proof,
    /// Circuit assignment or proof generation failed.
    Prover,
    /// The caller cancelled an in-flight operation; this is never proof invalidity.
    Cancelled,
}
impl Error {
    /// Whether the operation ended without a proof verdict because it was cancelled.
    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}
impl From<iroha_pasta::Cancelled> for Error {
    fn from(_: iroha_pasta::Cancelled) -> Self {
        Self::Cancelled
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "internal finality source: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Original artifact bytes independently selected by the installation owner.
#[derive(Clone, Copy)]
pub struct OriginalArtifact<'a> {
    /// Exact canonical native PIPA-R descriptor.
    pub descriptor: &'a [u8],
    /// Independently installed complete canonical verifying key.
    pub verifying_key: &'a [u8],
    /// Original proving tables; derived caches are not artifact authority.
    pub proving_key: &'a [u8],
}

pub(crate) use sealed::Exports;
pub(crate) mod sealed {
    use super::*;

    /// Exact internal source opening; private module prevents external construction.
    pub struct Exports {
        pub(crate) endpoints: [Fp; 6],
        pub(crate) frame: [Fp; 69],
        pub(crate) pallas: AccumulatorT<Ep>,
        pub(crate) parts: [AccumulatorT<Eq>; 2],
    }

    /// Internal source framing contract, implemented only by compiled owners.
    pub trait Source {
        /// Export the exact complete frame and both carried curve obligations.
        fn exports(
            &self,
            pallas: &PinnedParams<Ep>,
            vesta: &PinnedParams<Eq>,
            budget: MemoryBudget,
            cancellation: Option<&CancellationToken>,
        ) -> Result<Exports, Error>;
    }
}

/// A compiled source circuit with exact endpoints and all carried obligations.
/// Sealing prevents a caller-defined circuit from acquiring source qualification
/// merely by using the same 69-word frame.
#[allow(private_bounds)]
pub trait SourceCircuit: Circuit<Fp> + sealed::Source {}

pub(super) fn leaf_exports(
    endpoints: [Fp; 6],
    pallas: &PinnedParams<Ep>,
    vesta: &PinnedParams<Eq>,
    budget: MemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<Exports, Error> {
    let classify = |error: iroha_plonk_recursion::Error| {
        if error.is_cancelled() {
            Error::Cancelled
        } else {
            Error::Proof
        }
    };
    let pallas =
        AccumulatorT::trivial_cancellable(pallas, budget, cancellation).map_err(classify)?;
    let trivial =
        AccumulatorT::trivial_cancellable(vesta, budget, cancellation).map_err(classify)?;
    Ok(Exports {
        endpoints,
        frame: leaf_frame_native(endpoints).map_err(|_| Error::Input)?,
        pallas,
        parts: [trivial.clone(), trivial],
    })
}

macro_rules! leaf_source {
    ($source:ty) => {
        impl SourceCircuit for $source {}
        impl sealed::Source for $source {
            fn exports(
                &self,
                pallas: &PinnedParams<Ep>,
                vesta: &PinnedParams<Eq>,
                budget: MemoryBudget,
                cancellation: Option<&CancellationToken>,
            ) -> Result<Exports, Error> {
                leaf_exports(self.endpoints(), pallas, vesta, budget, cancellation)
            }
        }
    };
}
leaf_source!(BlsBatchCircuit);
leaf_source!(AggregateLeafCircuit);
leaf_source!(ResultScanBatchCircuit);
leaf_source!(ContextHashBatchCircuit);
leaf_source!(ScheduleSourceCircuit);
leaf_source!(LoadSourceCircuit);
leaf_source!(GenesisSourceCircuit);
macro_rules! composed_source {
    ($source:ty) => {
        impl SourceCircuit for $source {}
        impl sealed::Source for $source {
            fn exports(
                &self,
                _pallas: &PinnedParams<Ep>,
                _vesta: &PinnedParams<Eq>,
                _budget: MemoryBudget,
                cancellation: Option<&CancellationToken>,
            ) -> Result<Exports, Error> {
                CancellationToken::checkpoint(cancellation)?;
                Ok(Exports {
                    endpoints: *self.endpoints(),
                    frame: *self.instances(),
                    pallas: self.pallas().clone(),
                    parts: self.carried_vesta(),
                })
            }
        }
    };
}
composed_source!(SourceMergeCircuit);
composed_source!(CertificateCircuit);
composed_source!(CertifiedResultCircuit);
composed_source!(ScheduleCircuit);
composed_source!(ScheduledResultCircuit);
composed_source!(HistoryStepCircuit);
composed_source!(HistoryAppendCircuit);
composed_source!(ReceiptFinalityCircuit);

fn artifact_binding(
    artifact: OriginalArtifact<'_>,
    curve: CurveV1,
    lengths: &[u32],
    types: &[InstanceType],
    config: ReadConfig,
) -> Result<DescriptorBinding, Error> {
    if artifact.descriptor.is_empty()
        || artifact.descriptor.len() > DESCRIPTOR_MAX_BYTES
        || artifact.verifying_key.is_empty()
        || artifact.verifying_key.len() > VERIFYING_KEY_MAX_BYTES
        || artifact.proving_key.is_empty()
        || artifact.proving_key.len() > config.maximum_bytes
    {
        return Err(Error::Artifact);
    }
    let binding = DescriptorBinding::decode_v2(artifact.descriptor).map_err(|_| Error::Artifact)?;
    let d = binding.descriptor();
    if d.k != 16
        || d.curve != curve
        || d.transcript != TranscriptV2::KagemushaPoseidonRp57Base
        || d.instance_mode != InstanceModeV1::Direct
        || d.proof_suffix != ProofSuffixV1::FoldedGenerator
        || d.instance_lengths != lengths
        || d.instance_types.as_deref() != Some(types)
        || binding.n() > config.maximum_rows
    {
        return Err(Error::Artifact);
    }
    Ok(binding)
}

/// Original tables qualified against their sealed compiled source.
pub(crate) struct ImportedSource<C: SourceCircuit> {
    key: ProvingKey<Eq>,
    source: PhantomData<C>,
}
/// A catalog entry obtainable only from actual sealed-source import or VK derivation.
#[derive(Clone)]
pub(crate) struct QualifiedSourceKey {
    binding: DescriptorBinding,
    key: VerifyingKey<Eq>,
}
impl<C: SourceCircuit> ImportedSource<C> {
    pub(crate) fn from_original_cancellable(
        circuit: &C,
        artifact: OriginalArtifact<'_>,
        vesta: &PinnedParams<Eq>,
        config: ReadConfig,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, Error> {
        CancellationToken::checkpoint(cancellation)?;
        if vesta.k() != 16 {
            return Err(Error::Artifact);
        }
        let binding = artifact_binding(
            artifact,
            CurveV1::Vesta,
            &[69],
            &[InstanceType::Bounded],
            config,
        )?;
        VerifyingKey::<Eq>::read_cancellable(artifact.verifying_key, &binding, cancellation)
            .map_err(verifying_key_error)?;
        let key = ProvingKey::from_artifact_v2_cancellable(
            artifact.proving_key,
            &binding,
            vesta,
            &circuit.without_witnesses(),
            config,
            cancellation,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Artifact
            }
        })?;
        if key.vk().to_bytes() != artifact.verifying_key {
            return Err(Error::Artifact);
        }
        Ok(Self {
            key,
            source: PhantomData,
        })
    }
    pub(crate) fn binding(&self) -> &DescriptorBinding {
        self.key.binding()
    }
    pub(crate) fn catalog_key(&self) -> QualifiedSourceKey {
        QualifiedSourceKey {
            binding: self.key.binding().clone(),
            key: self.key.vk().clone(),
        }
    }
}

/// Immutable imported source and wrapper keys. Each owner handles one fixed
/// source layout; the installed tree chooses its children and source ordinal.
pub struct Prover<C: SourceCircuit> {
    source: ProvingKey<Eq>,
    wrapper: ProvingKey<Ep>,
    plan: OmegaPlan,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
    source_type: PhantomData<C>,
}
impl<C: SourceCircuit> Prover<C> {
    /// Import both original keys against the exact compiled source and wrapper.
    /// The supplied circuit contributes only its witnessless fixed source layout.
    /// Authentication of the complete source tree remains the installation owner.
    /// # Errors
    /// Wrong native profile, bounds, source tables or installed VK commitments.
    pub fn from_original_artifacts(
        circuit: &C,
        source: OriginalArtifact<'_>,
        wrapper: OriginalArtifact<'_>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
        config: ReadConfig,
    ) -> Result<Self, Error> {
        Self::from_original_artifacts_cancellable(
            circuit, source, wrapper, pallas, vesta, config, None,
        )
    }

    /// Import exact original source and wrapper tables with bounded cancellation.
    /// # Errors
    /// As [`Self::from_original_artifacts`], or cancellation without an import verdict.
    pub fn from_original_artifacts_cancellable(
        circuit: &C,
        source: OriginalArtifact<'_>,
        wrapper: OriginalArtifact<'_>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
        config: ReadConfig,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, Error> {
        CancellationToken::checkpoint(cancellation)?;
        let source = ImportedSource::from_original_cancellable(
            circuit,
            source,
            &vesta,
            config,
            cancellation,
        )?;
        let catalog = [source.catalog_key()];
        Self::from_qualified_catalog_cancellable(
            source,
            wrapper,
            &catalog,
            pallas,
            vesta,
            config,
            cancellation,
        )
    }

    /// Mount one shared wrapper against only original source-qualified catalog entries.
    /// The finite history owner fixes the exact Genesis/Append pair and ordering.
    pub(crate) fn from_qualified_catalog_cancellable(
        source: ImportedSource<C>,
        wrapper: OriginalArtifact<'_>,
        catalog: &[QualifiedSourceKey],
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
        config: ReadConfig,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, Error> {
        CancellationToken::checkpoint(cancellation)?;
        if pallas.k() != 16
            || vesta.k() != 16
            || catalog.is_empty()
            || catalog
                .iter()
                .any(|entry| entry.binding.encoded() != source.binding().encoded())
            || !catalog
                .iter()
                .any(|entry| entry.key.to_bytes() == source.key.vk().to_bytes())
        {
            return Err(Error::Artifact);
        }
        let wrapper_binding = artifact_binding(
            wrapper,
            CurveV1::Pallas,
            &[1, 2, 16],
            &OmegaPlan::instance_types(),
            config,
        )?;
        VerifyingKey::<Ep>::read_cancellable(wrapper.verifying_key, &wrapper_binding, cancellation)
            .map_err(verifying_key_error)?;
        let (plan, blank) = wrapper_source(catalog, vesta.clone())?;
        let wrapper_key = ProvingKey::from_artifact_v2_cancellable(
            wrapper.proving_key,
            &wrapper_binding,
            &pallas,
            &blank,
            config,
            cancellation,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Artifact
            }
        })?;
        if wrapper_key.vk().to_bytes() != wrapper.verifying_key {
            return Err(Error::Artifact);
        }
        Ok(Self {
            source: source.key,
            wrapper: wrapper_key,
            plan,
            pallas,
            vesta,
            source_type: PhantomData,
        })
    }

    /// Exact imported wrapper descriptor to pin in a parent source merge.
    pub fn binding(&self) -> &DescriptorBinding {
        self.wrapper.binding()
    }

    /// Exact source-qualified wrapper key to pin in a parent source merge.
    pub fn verifying_key(&self) -> &VerifyingKey<Ep> {
        self.wrapper.vk()
    }

    /// Retain this successfully source-imported wrapper for a parent merge.
    /// The token contains its exact immutable verifier, never a proof verdict.
    /// # Errors
    /// An invalid wrapper profile or parameter set.
    pub fn qualified_source(&self) -> Result<SourceVerifier, Error> {
        Ok(SourceVerifier {
            verifier: Arc::new(
                VerifierPlan::new(self.binding().clone(), self.pallas.clone())
                    .map_err(|_| Error::Artifact)?,
            ),
            key: Arc::new(self.verifying_key().clone()),
        })
    }

    /// Prove the source and its wrapper, checking every original and full claim.
    /// Randomness for the two actual proofs and fold salt are supplied explicitly.
    /// # Errors
    /// Source layout substitution, inconsistent frame, failed proof/fold/decision.
    pub fn prove(
        &self,
        circuit: &C,
        salt: [u8; 32],
        source_randomness: ProverRandomness<'_>,
        wrapper_randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<SourceNodeEvidence, Error> {
        let budget = config.msm_budget;
        let cancellation = config.cancellation;
        CancellationToken::checkpoint(cancellation)?;
        let exports = circuit.exports(&self.pallas, &self.vesta, budget, cancellation)?;
        // Check the exact source-bound endpoints and hard frame before proving.
        // The actual circuit independently computes and authenticates them.
        leaf_frame_native(exports.endpoints).map_err(|_| Error::Input)?;
        exports
            .pallas
            .decide_cancellable(&self.pallas, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        let trivial = AccumulatorT::trivial_cancellable(&self.vesta, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        let mut expected = vec![
            native_binding(&exports.endpoints, &exports.pallas).map_err(|_| Error::Input)?,
            Fp::from(16),
        ];
        for claim in exports.parts.iter().chain(core::iter::once(&trivial)) {
            claim
                .decide_cancellable(&self.vesta, budget, cancellation)
                .map_err(|error| {
                    if error.is_cancelled() {
                        Error::Cancelled
                    } else {
                        Error::Proof
                    }
                })?;
            expected.extend(native_vesta(claim).map_err(|_| Error::Input)?);
        }
        expected.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
        expected.extend_from_slice(&native_vesta(&trivial).map_err(|_| Error::Input)?[..4]);
        if expected != exports.frame {
            return Err(Error::Input);
        }
        let public = [exports.frame.to_vec()];
        let witness =
            Witness::from_circuit_cancellable(&self.source, circuit, &public, cancellation)
                .map_err(|error| {
                    if error.is_cancelled() {
                        Error::Cancelled
                    } else {
                        Error::Prover
                    }
                })?;
        let output = create_proof_owned_with_claim(
            &self.vesta,
            &self.source,
            witness,
            source_randomness,
            config,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Prover
            }
        })?;
        output
            .opening
            .decide_cancellable(&self.vesta, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        verify_full_cancellable(
            &self.vesta,
            self.source.binding(),
            self.source.vk(),
            &public,
            &output.proof,
            budget,
            cancellation,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Proof
            }
        })?;
        let opening = FoldInput::from_opening(*output.opening.g(), output.opening.challenges())
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        let (fold, vesta) = create_fold(
            &self.vesta,
            &[
                exports.parts[0].as_input(),
                opening,
                exports.parts[1].as_input(),
                trivial.as_input(),
            ],
            salt,
            &FoldConfig {
                kernel_budget: budget,
                cancellation: cancellation.cloned(),
                ..FoldConfig::default()
            },
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Proof
            }
        })?;
        let wrapper = OmegaCircuit::new(
            self.plan.clone(),
            OmegaWitness {
                key: self.source.vk().clone(),
                instances: exports.frame.to_vec(),
                length: u32::try_from(output.proof.len()).map_err(|_| Error::Input)?,
                proof: output.proof,
                fold: fold.to_bytes(),
            },
        )
        .map_err(|_| Error::Input)?;
        let mut evidence = SourceNodeEvidence {
            endpoints: exports.endpoints,
            proof: vec![],
            pallas: exports.pallas,
            vesta,
        };
        let instances = evidence.instances().map_err(|_| Error::Input)?;
        let witness =
            Witness::from_circuit_cancellable(&self.wrapper, &wrapper, &instances, cancellation)
                .map_err(|error| {
                    if error.is_cancelled() {
                        Error::Cancelled
                    } else {
                        Error::Prover
                    }
                })?;
        let output = create_proof_owned_with_claim(
            &self.pallas,
            &self.wrapper,
            witness,
            wrapper_randomness,
            config,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Prover
            }
        })?;
        output
            .opening
            .decide_cancellable(&self.pallas, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        evidence.proof = output.proof;
        self.verify_evidence_cancellable(&evidence, budget, cancellation)?;
        CancellationToken::checkpoint(cancellation)?;
        Ok(evidence)
    }

    /// Reverify a retained source interval under this exact installed wrapper.
    /// This checks both transported curves; it grants no monetary authority.
    /// # Errors
    /// Wrong length, endpoints/key/proof/claim substitution or failed decision.
    pub fn verify_evidence(
        &self,
        evidence: &SourceNodeEvidence,
        budget: MemoryBudget,
    ) -> Result<(), Error> {
        self.verify_evidence_cancellable(evidence, budget, None)
    }

    /// Reverify exact evidence with the caller's operation cancellation signal.
    /// A cancelled verification produces no valid or invalid proof verdict.
    /// # Errors
    /// As [`Self::verify_evidence`], or cancellation.
    pub fn verify_evidence_cancellable(
        &self,
        evidence: &SourceNodeEvidence,
        budget: MemoryBudget,
        cancellation: Option<&CancellationToken>,
    ) -> Result<(), Error> {
        CancellationToken::checkpoint(cancellation)?;
        let length = Protocol::new(self.binding().descriptor())
            .map_err(|_| Error::Artifact)?
            .proof_length();
        if evidence.proof.len() != length {
            return Err(Error::Input);
        }
        leaf_frame_native(evidence.endpoints).map_err(|_| Error::Input)?;
        let instances = evidence.instances().map_err(|_| Error::Input)?;
        verify_full_cancellable(
            &self.pallas,
            self.binding(),
            self.verifying_key(),
            &instances,
            &evidence.proof,
            budget,
            cancellation,
        )
        .map_err(|error| {
            if error.is_cancelled() {
                Error::Cancelled
            } else {
                Error::Proof
            }
        })?;
        evidence
            .pallas
            .decide_cancellable(&self.pallas, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })?;
        evidence
            .vesta
            .decide_cancellable(&self.vesta, budget, cancellation)
            .map_err(|error| {
                if error.is_cancelled() {
                    Error::Cancelled
                } else {
                    Error::Proof
                }
            })
    }
}

// Shared source factory for original-PK import and verifier-only derivation.
// Catalog entries are opaque and can only come from sealed-source qualification.
fn wrapper_source(
    catalog: &[QualifiedSourceKey],
    vesta: PinnedParams<Eq>,
) -> Result<(OmegaPlan, OmegaCircuit), Error> {
    let first = catalog.first().ok_or(Error::Artifact)?;
    if vesta.k() != 16 || catalog.iter().any(|entry| entry.binding != first.binding) {
        return Err(Error::Artifact);
    }
    let digests = catalog
        .iter()
        .map(|entry| {
            entry
                .key
                .kagemusha_digest(&entry.binding)
                .map_err(|_| Error::Artifact)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let plan = OmegaPlan::new(first.binding.clone(), vesta, digests)
        .and_then(|plan| {
            plan.with_key_catalog(catalog.iter().map(|entry| entry.key.clone()).collect())
        })
        .map_err(|_| Error::Artifact)?;
    let length = plan.verifier().proof_length();
    let blank = OmegaCircuit::new(
        plan.clone(),
        OmegaWitness {
            key: first.key.clone(),
            instances: vec![Fp::ZERO; 69],
            proof: vec![0; length],
            length: u32::try_from(length).map_err(|_| Error::Artifact)?,
            fold: [0; FOLD_WITNESS_BYTES],
        },
    )
    .map_err(|_| Error::Artifact)?
    .without_witnesses();
    Ok((plan, blank))
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;

    #[test]
    fn cancellation_has_no_proof_failure_verdict() {
        let token = CancellationToken::new();
        token.cancel();
        let error = CancellationToken::checkpoint(Some(&token))
            .map_err(Error::from)
            .unwrap_err();
        assert!(error.is_cancelled());
        for error in [Error::Artifact, Error::Input, Error::Proof, Error::Prover] {
            assert!(!error.is_cancelled());
        }
    }
}

fn verifying_key_error(error: iroha_plonk::keys::VkError) -> Error {
    if error.is_cancelled() {
        Error::Cancelled
    } else {
        Error::Artifact
    }
}

#[cfg(test)]
mod verifier_read_cancellation_tests {
    use super::*;
    #[test]
    fn verifier_original_cancellation_is_never_an_artifact_verdict() {
        assert_eq!(
            verifying_key_error(iroha_plonk::keys::VkError::Cancelled),
            Error::Cancelled
        );
        assert_eq!(
            verifying_key_error(iroha_plonk::keys::VkError::Version { found: 0 }),
            Error::Artifact
        );
    }
}
