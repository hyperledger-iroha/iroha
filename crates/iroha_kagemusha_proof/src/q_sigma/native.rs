//! Native preparation, key generation and proving for the fixed `Q_sigma` relation.
//!
//! A caller supplies the incoming mode after evaluating the complete operation's
//! soft checks. This module never chooses or authorizes a burn. It derives the
//! selected local claim, creates the exact local fold, binds every exported
//! instance and self-verifies each generated Q proof.

use core::fmt;

use ff::{Field, PrimeField};
use iroha_pasta::{Ep, Eq, Fp, Fq, PastaAffine, PastaField, msm::MemoryBudget};
use iroha_plonk::{
    DescriptorBinding, KeyError, ProverConfig, ProverError, ProverRandomness, ProvingKey,
    VerifyError, VerifyingKey, Witness, create_proof_owned,
    frontend::Error as LayoutError,
    keys::{CosetCachePolicy, KeygenConfigV2, keygen_pk_v2},
    pcs::{
        ipa::{IpaError, PinnedParams},
        multiopen::MultiopenError,
    },
    transcript::TranscriptError,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::bytes::tape::{ByteOrder, segment_value};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, create_fold, verifier::VerifierPlan,
};

use super::{
    IncomingSigmaWitness, QSigmaCircuit, QSigmaPlan, QSigmaWitness, SigmaClass, SigmaSlotWitness,
};
use crate::SigmaVerifier;

/// Failure to prepare, prove or verify a fixed Q relation.
#[derive(Debug)]
pub enum QSigmaError {
    /// Slot/frame metadata or assignment failure.
    Layout(LayoutError),
    /// The key or class is absent from the fixed allowlist.
    UnauthorizedKey,
    /// The selected incoming mode cannot be realized by the actual claim.
    IncomingMode,
    /// Own sigma has the wrong original carrier length.
    OwnLength,
    /// Outer parameters must cover exactly k16.
    Parameters,
    /// The installed descriptor is not the exact V2 Q profile for this source.
    Profile,
    /// The installed descriptor failed strict V2 decoding.
    Descriptor(iroha_plonk::cs::DescriptorError),
    /// Original proving-key source, allocation or commitment admission failed.
    Artifact(iroha_plonk::keys::pk::artifact::Error),
    /// Key-generation failure.
    Key(KeyError),
    /// Proof synthesis or creation failure.
    Prover(ProverError),
    /// Complete or succinct verification failure.
    Verify(VerifyError),
    /// Local accumulator or fold failure.
    Fold(iroha_plonk_recursion::Error),
}
impl fmt::Display for QSigmaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Q_sigma: {self:?}")
    }
}
impl std::error::Error for QSigmaError {}

/// The caller-selected global mode of the incoming sigma obligation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncomingMode {
    /// Keep a deciding original opening.
    Accept,
    /// Select the fixed full-length trivial claim.
    Trivial,
    /// Compute a distinct deciding commitment for the original challenges.
    Corrected,
}
impl IncomingMode {
    fn bits(self) -> [bool; 3] {
        match self {
            Self::Accept => [true, false, false],
            Self::Trivial => [false, true, false],
            Self::Corrected => [false, false, true],
        }
    }
}
/// Incoming proof bytes and the globally selected mode, before local folding.
#[derive(Clone, Debug)]
pub struct IncomingSigma {
    /// Descriptor-sized sigma carrier and statement.
    pub sigma: SigmaSlotWitness,
    /// Mode selected by the complete operation coordinator.
    pub mode: IncomingMode,
}
/// A fully prepared local circuit and its exact public frame.
#[must_use = "prove this Q relation and register its returned obligation in A"]
#[derive(Clone, Debug)]
pub struct PreparedQSigma {
    circuit: QSigmaCircuit,
    instances: Vec<Vec<Fq>>,
    part: FoldInput<Eq>,
}
impl PreparedQSigma {
    /// The fixed circuit and prepared witnesses.
    pub const fn circuit(&self) -> &QSigmaCircuit {
        &self.circuit
    }
    /// Public columns in the exact homogeneous Q schema.
    pub fn instances(&self) -> &[Vec<Fq>] {
        &self.instances
    }
    /// Selected forwarded or folded Vesta part.
    pub const fn part(&self) -> &FoldInput<Eq> {
        &self.part
    }
}

impl SigmaClass {
    /// Builds one descriptor class from already checked Sigma verifiers and
    /// their indices in the global allowlist; no profile fallback is attempted.
    ///
    /// # Errors
    /// Empty/mixed classes or invalid/duplicate global entries.
    pub fn from_verifiers(entries: &[(u8, &SigmaVerifier<Eq>)]) -> Result<Self, QSigmaError> {
        let (_, first) = entries.first().ok_or(QSigmaError::UnauthorizedKey)?;
        let mut digests = Vec::with_capacity(entries.len());
        for (index, verifier) in entries {
            if verifier.binding() != first.binding() {
                return Err(QSigmaError::UnauthorizedKey);
            }
            let digest = verifier
                .vk()
                .kagemusha_digest(verifier.binding())
                .map_err(|_| QSigmaError::UnauthorizedKey)?;
            digests.push((*index, digest));
        }
        let plan = VerifierPlan::new(first.binding().clone(), first.params().clone())
            .map_err(QSigmaError::Verify)?;
        Self::new(plan, digests).map_err(QSigmaError::Layout)
    }
    fn index(&self, witness: &SigmaSlotWitness) -> Result<u8, QSigmaError> {
        let digest = witness
            .key
            .kagemusha_digest(self.verifier.binding())
            .map_err(|_| QSigmaError::UnauthorizedKey)?;
        self.entries
            .iter()
            .find_map(|(index, allowed)| (*allowed == digest).then_some(*index))
            .ok_or(QSigmaError::UnauthorizedKey)
    }
    fn opening(
        &self,
        witness: &SigmaSlotWitness,
        budget: MemoryBudget,
    ) -> Result<FoldInput<Eq>, QSigmaError> {
        let opening = accumulate_generator(
            self.verifier.params(),
            self.verifier.binding(),
            &witness.key,
            &[vec![witness.statement]],
            &witness.proof,
            budget,
        )
        .map_err(QSigmaError::Verify)?;
        FoldInput::from_opening(*opening.g(), opening.challenges()).map_err(QSigmaError::Fold)
    }
}
fn proof_failure(error: &VerifyError) -> bool {
    match error {
        VerifyError::Transcript(error) => !matches!(error, TranscriptError::ProfileMismatch),
        VerifyError::DegenerateChallenge
        | VerifyError::ProofLength { .. }
        | VerifyError::Multiopen(
            MultiopenError::PointCollision
            | MultiopenError::ConflictingEvaluations { .. }
            | MultiopenError::DegenerateChallenge,
        ) => true,
        VerifyError::Ipa(error) | VerifyError::Multiopen(MultiopenError::Ipa(error)) => matches!(
            error,
            IpaError::ZeroChallenge { .. }
                | IpaError::OpeningFailed
                | IpaError::FoldedGeneratorMismatch
        ),
        _ => false,
    }
}
fn scalar(value: Fp) -> Fq {
    // Fp is strictly smaller than Fq, so this integer embedding is injective.
    Fq::from_canonical_limbs(value.to_canonical_limbs()).expect("Fp fits Fq")
}
fn chunks(witness: &SigmaSlotWitness) -> Vec<Fq> {
    let bytes: Vec<_> = witness
        .length
        .to_le_bytes()
        .into_iter()
        .chain(witness.proof.iter().copied())
        .collect();
    bytes
        .chunks(31)
        .map(|chunk| segment_value(chunk, ByteOrder::Little).expect("31-byte integer fits"))
        .collect()
}
impl QSigmaPlan {
    /// Prepares the exact local relation, including a fold when two sigmas are
    /// present. Own and Accept claims must decide. Corrected must change an
    /// undecidable original; Trivial uses the pinned k16 claim. The caller must
    /// authorize the selected mode in A's global branch rule.
    ///
    /// # Errors
    /// Wrong slots/lengths/classes, invalid own proof, an impossible incoming
    /// mode, or a local accumulator/fold failure.
    pub fn prepare(
        &self,
        own: SigmaSlotWitness,
        incoming: Option<IncomingSigma>,
        params: &PinnedParams<Eq>,
        salt: Fq,
        config: &FoldConfig,
    ) -> Result<PreparedQSigma, QSigmaError> {
        if own.proof.len() != self.own.verifier.proof_length()
            || self.incoming.is_some() != incoming.is_some()
            || self
                .incoming
                .as_ref()
                .zip(incoming.as_ref())
                .is_some_and(|(class, input)| {
                    input.sigma.proof.len() != class.verifier.proof_length()
                })
        {
            return Err(QSigmaError::Layout(LayoutError::Synthesis));
        }
        if usize::try_from(own.length).ok() != Some(own.proof.len()) {
            return Err(QSigmaError::OwnLength);
        }
        let own_index = self.own.index(&own)?;
        let own_claim = self.own.opening(&own, config.kernel_budget)?;
        own_claim
            .decide(params, config.kernel_budget)
            .map_err(QSigmaError::Fold)?;
        let mut statements = vec![scalar(own.statement)];
        let mut exported = chunks(&own);
        let mut indices = vec![Fq::from(u64::from(own_index))];
        let mut verdicts = vec![Fq::ONE];
        let (incoming, part) = if let Some(input) = incoming {
            let class = self.incoming.as_ref().ok_or(QSigmaError::IncomingMode)?;
            let index = class.index(&input.sigma)?;
            let original =
                if usize::try_from(input.sigma.length).ok() == Some(input.sigma.proof.len()) {
                    match class.opening(&input.sigma, config.kernel_budget) {
                        Ok(claim) => Some(claim),
                        Err(QSigmaError::Verify(error)) if proof_failure(&error) => None,
                        Err(error) => return Err(error),
                    }
                } else {
                    None
                };
            let valid = original.is_some();
            let trivial =
                AccumulatorT::trivial(params, config.kernel_budget).map_err(QSigmaError::Fold)?;
            let (selected, corrected) = match input.mode {
                IncomingMode::Accept => {
                    let original = original.ok_or(QSigmaError::IncomingMode)?;
                    original
                        .decide(params, config.kernel_budget)
                        .map_err(QSigmaError::Fold)?;
                    (original, Eq::from(*trivial.g()))
                }
                IncomingMode::Trivial => (trivial.as_input(), Eq::from(*trivial.g())),
                IncomingMode::Corrected => {
                    let original = original.ok_or(QSigmaError::IncomingMode)?;
                    let correction = original
                        .corrected(params, config.kernel_budget)
                        .map_err(QSigmaError::Fold)?;
                    (
                        correction.replacement().clone(),
                        Eq::from(*correction.replacement().g()),
                    )
                }
            };
            let (fold, part) = create_fold(
                params,
                &[own_claim, selected, trivial.as_input()],
                salt.to_repr(),
                config,
            )
            .map_err(QSigmaError::Fold)?;
            part.decide(params, config.kernel_budget)
                .map_err(QSigmaError::Fold)?;
            statements.push(scalar(input.sigma.statement));
            exported.extend(chunks(&input.sigma));
            indices.push(Fq::from(u64::from(index)));
            verdicts.push(Fq::from(u64::from(valid)));
            verdicts.extend(input.mode.bits().map(|bit| Fq::from(u64::from(bit))));
            (
                Some(IncomingSigmaWitness {
                    sigma: input.sigma,
                    mode: input.mode.bits(),
                    corrected,
                    fold: fold.to_bytes(),
                }),
                part.as_input(),
            )
        } else {
            (None, own_claim)
        };
        statements.extend(exported);
        statements.extend(part.challenges().iter().copied().map(scalar));
        let (x, y) =
            Option::<(Fq, Fq)>::from(part.g().coordinates()).ok_or(QSigmaError::IncomingMode)?;
        let instances = vec![
            statements,
            vec![x, y],
            indices,
            verdicts,
            vec![Fq::from(u64::from(part.source_k()))],
        ];
        let circuit = QSigmaCircuit::new(self.clone(), QSigmaWitness { own, incoming })
            .map_err(QSigmaError::Layout)?;
        Ok(PreparedQSigma {
            circuit,
            instances,
            part,
        })
    }
}

/// Native outer Q key and pinned Pallas parameters. The profile and public
/// schema are fixed; resource policy uses an on-demand coset cache.
#[derive(Debug)]
pub struct QSigmaProver {
    params: PinnedParams<Ep>,
    key: ProvingKey<Ep>,
    serialized_buses: Option<usize>,
}
/// A Q proof and its exact public frame, retaining the Vesta part obligation.
#[must_use = "verify the Q proof in A and propagate the Vesta part to Omega"]
#[derive(Clone, Debug)]
pub struct QSigmaProof {
    /// Canonical PIPA-R proof, with its generator suffix.
    pub bytes: Vec<u8>,
    /// Five homogeneous public columns.
    pub instances: Vec<Vec<Fq>>,
    /// Forwarded or folded inner obligation, never implicitly omitted.
    pub part: FoldInput<Eq>,
}
impl QSigmaProver {
    /// Generates this concrete Q class key at k16. Key generation discards the
    /// prepared witness values and retains only fixed program/allowlist shape.
    ///
    /// # Errors
    /// Parameters are not k16, or synthesis/key generation fails.
    pub fn keygen(
        prepared: &PreparedQSigma,
        params: PinnedParams<Ep>,
    ) -> Result<Self, QSigmaError> {
        Self::keygen_profile(prepared, params, None)
    }
    /// Generates the explicitly selected shared-range serialized Q profile.
    /// The selected layout is retained by the prover and never inferred from
    /// supplied proof bytes or used as a verification fallback.
    ///
    /// # Errors
    /// Invalid fixed bus count, non-k16 parameters, or failed key generation.
    pub fn keygen_serialized_foreign(
        prepared: &PreparedQSigma,
        params: PinnedParams<Ep>,
        range_buses: usize,
    ) -> Result<Self, QSigmaError> {
        if !(1..=8).contains(&range_buses) {
            return Err(QSigmaError::Layout(LayoutError::Synthesis));
        }
        Self::keygen_profile(prepared, params, Some(range_buses))
    }
    fn keygen_profile(
        prepared: &PreparedQSigma,
        params: PinnedParams<Ep>,
        serialized_buses: Option<usize>,
    ) -> Result<Self, QSigmaError> {
        if params.k() != 16 {
            return Err(QSigmaError::Parameters);
        }
        let mut config = KeygenConfigV2::pipa_r(QSigmaPlan::instance_types().to_vec());
        config.coset_cache = CosetCachePolicy::OnDemand;
        let key = if let Some(buses) = serialized_buses {
            let circuit = prepared
                .circuit
                .clone()
                .with_serialized_foreign(buses)
                .map_err(QSigmaError::Layout)?;
            keygen_pk_v2(&params, &circuit, &config)
        } else {
            keygen_pk_v2(&params, &prepared.circuit, &config)
        }
        .map_err(QSigmaError::Key)?;
        Ok(Self {
            params,
            key,
            serialized_buses,
        })
    }
    /// Import original Q material for the default fixed source profile. The
    /// installed native owner authenticates descriptor/VK/PK originals, scheme
    /// scope and complete inventory before this call; witness input cannot select
    /// that authority or resource policy. Import generates no key and grants no
    /// `NativeProofs` owner, wallet-open capability or completed A/Omega relation.
    /// Original/domain bounds do not qualify total synthesis/prover memory.
    ///
    /// # Errors
    /// Non-k16 parameters, wrong V2 profile/schema, substituted installed VK, or
    /// a bounded source/commitment import failure. There is no profile fallback.
    pub fn from_original_artifact(
        prepared: &PreparedQSigma,
        params: PinnedParams<Ep>,
        descriptor: &[u8],
        installed_vk: &[u8],
        original: &[u8],
        config: iroha_plonk::keys::pk::artifact::ReadConfig,
    ) -> Result<Self, QSigmaError> {
        Self::from_original_profile(
            prepared,
            params,
            descriptor,
            installed_vk,
            original,
            config,
            None,
        )
    }

    /// Import the independently installed shared-range serialized Q profile.
    /// The fixed bus count is selected by native installation metadata, never
    /// inferred from original/proof bytes or tried as a fallback. Descriptor,
    /// VK and complete original PK authentication remain the installation owner's
    /// duty, as in [`Self::from_original_artifact`]; no monetary authority is granted.
    ///
    /// # Errors
    /// Invalid fixed bus count or any default-profile import refusal above.
    pub fn from_original_artifact_serialized_foreign(
        prepared: &PreparedQSigma,
        params: PinnedParams<Ep>,
        descriptor: &[u8],
        installed_vk: &[u8],
        original: &[u8],
        config: iroha_plonk::keys::pk::artifact::ReadConfig,
        range_buses: usize,
    ) -> Result<Self, QSigmaError> {
        if !(1..=8).contains(&range_buses) {
            return Err(QSigmaError::Layout(LayoutError::Synthesis));
        }
        Self::from_original_profile(
            prepared,
            params,
            descriptor,
            installed_vk,
            original,
            config,
            Some(range_buses),
        )
    }

    fn from_original_profile(
        prepared: &PreparedQSigma,
        params: PinnedParams<Ep>,
        descriptor: &[u8],
        installed_vk: &[u8],
        original: &[u8],
        config: iroha_plonk::keys::pk::artifact::ReadConfig,
        serialized_buses: Option<usize>,
    ) -> Result<Self, QSigmaError> {
        use iroha_plonk::cs::{InstanceModeV1, ProofSuffixV1, TranscriptV2};
        use iroha_plonk::keys::pk::artifact::Error as ArtifactError;
        if params.k() != 16 {
            return Err(QSigmaError::Parameters);
        }
        let binding = DescriptorBinding::decode_v2(descriptor).map_err(QSigmaError::Descriptor)?;
        let d = binding.descriptor();
        let lengths = prepared.circuit.plan.instance_lengths();
        if d.k != 16
            || d.transcript != TranscriptV2::KagemushaPoseidonRp57Base
            || d.instance_mode != InstanceModeV1::Direct
            || d.proof_suffix != ProofSuffixV1::FoldedGenerator
            || d.instance_types.as_deref() != Some(&QSigmaPlan::instance_types())
            || d.instance_lengths.len() != lengths.len()
            || !d
                .instance_lengths
                .iter()
                .zip(lengths)
                .all(|(found, expected)| usize::try_from(*found).ok() == Some(expected))
        {
            return Err(QSigmaError::Profile);
        }
        // Reject the installed allocation bounds before cloning a serialized source.
        if original.len() > config.maximum_bytes || binding.n() > config.maximum_rows {
            return Err(QSigmaError::Artifact(ArtifactError::Length));
        }
        VerifyingKey::<Ep>::read(installed_vk, &binding).map_err(|error| {
            QSigmaError::Artifact(ArtifactError::Key(KeyError::VerifyingKey(error)))
        })?;
        let key = if let Some(buses) = serialized_buses {
            let circuit = prepared
                .circuit
                .clone()
                .with_serialized_foreign(buses)
                .map_err(QSigmaError::Layout)?;
            ProvingKey::from_artifact_v2(original, &binding, &params, &circuit, config)
        } else {
            ProvingKey::from_artifact_v2(original, &binding, &params, &prepared.circuit, config)
        }
        .map_err(QSigmaError::Artifact)?;
        if key.vk().to_bytes() != installed_vk {
            return Err(QSigmaError::UnauthorizedKey);
        }
        Ok(Self {
            params,
            key,
            serialized_buses,
        })
    }

    /// Original proving material for the native package producer. Export alone
    /// confers no signed inventory or scheme authority.
    pub const fn proving_key(&self) -> &ProvingKey<Ep> {
        &self.key
    }

    /// Validated V2 descriptor, suitable for A's fixed verifier program.
    pub fn binding(&self) -> &DescriptorBinding {
        self.key.binding()
    }
    /// Fixed Q verifying key for A's class registry.
    pub fn verifying_key(&self) -> &VerifyingKey<Ep> {
        self.key.vk()
    }
    /// The outer pinned Pallas prefix.
    pub const fn params(&self) -> &PinnedParams<Ep> {
        &self.params
    }
    /// Proves and completely self-verifies Q before returning its bytes.
    /// Witness construction rejects a different fixed plan/allowlist/copy graph.
    ///
    /// # Errors
    /// Circuit mismatch, synthesis/proving failure or failed self-verification.
    pub fn prove(
        &self,
        prepared: &PreparedQSigma,
        randomness: ProverRandomness,
        config: ProverConfig,
    ) -> Result<QSigmaProof, QSigmaError> {
        let witness = if let Some(buses) = self.serialized_buses {
            let circuit = prepared
                .circuit
                .clone()
                .with_serialized_foreign(buses)
                .map_err(QSigmaError::Layout)?;
            Witness::from_circuit(&self.key, &circuit, &prepared.instances)
        } else {
            Witness::from_circuit(&self.key, &prepared.circuit, &prepared.instances)
        }
        .map_err(QSigmaError::Prover)?;
        let bytes = create_proof_owned(&self.params, &self.key, witness, randomness, config)
            .map_err(QSigmaError::Prover)?;
        verify_full(
            &self.params,
            self.key.binding(),
            self.key.vk(),
            &prepared.instances,
            &bytes,
            config.msm_budget,
        )
        .map_err(QSigmaError::Verify)?;
        Ok(QSigmaProof {
            bytes,
            instances: prepared.instances.clone(),
            part: prepared.part.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn soft_failure_classification_retains_configuration_and_resource_errors() {
        assert!(proof_failure(&VerifyError::Transcript(
            TranscriptError::InvalidPoint
        )));
        assert!(proof_failure(&VerifyError::Ipa(IpaError::OpeningFailed)));
        assert!(proof_failure(&VerifyError::Multiopen(MultiopenError::Ipa(
            IpaError::ZeroChallenge { round: 0 }
        ))));
        assert!(!proof_failure(&VerifyError::Transcript(
            TranscriptError::ProfileMismatch
        )));
        assert!(!proof_failure(&VerifyError::KeyMismatch));
        assert!(!proof_failure(&VerifyError::Ipa(
            IpaError::ParamsTooSmall {
                needed: 16,
                available: 12
            }
        )));
    }
}
