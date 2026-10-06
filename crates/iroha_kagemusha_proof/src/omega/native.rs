//! Genuine final Omega original-key intake and proof production.
//!
//! The authenticated installation owner supplies the complete terminal A catalog,
//! exact source descriptor and fixed outer layout. This component imports the original
//! Pallas proving key against that compiled program and the installed VK. It verifies
//! the actual terminal A proof, derives all four Vesta fold slots from its canonical
//! frame, preserves the full original Pallas claim, and self-verifies the generated
//! Omega proof and both transported claims with complete native decisions.
//! It performs no runtime key generation and grants no wallet-open or custody authority.
//! Full catalog/profile authentication, original G1 preparation, source-marker admission
//! and exact G1 Payment/Credited byte qualification remain mandatory in the owning loader.

use core::fmt;

use ff::{Field, PrimeField};
use iroha_pasta::{
    Ep, Eq, EqAffine, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain,
};
use iroha_plonk::{
    DescriptorBinding, Protocol, ProverConfig, ProverRandomness, ProvingKey, VerifyingKey, Witness,
    create_proof_owned_with_claim,
    cs::{CurveV1, InstanceModeV1, ProofSuffixV1, TranscriptV2},
    frontend::Circuit,
    keys::pk::artifact::ReadConfig,
    pcs::ipa::PinnedParams,
    transcript::decode_point,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::{
    bytes::p_bytes_native, range::secondary::SecondaryPlan, statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    ACCUMULATOR_BYTES, AccumulatorT, FOLD_WITNESS_BYTES, FoldConfig, FoldInput, K,
    VESTA_TRIVIAL_GENERATOR, create_fold, verifier::CompactSpans,
};

use super::{OmegaCircuit, OmegaPlan, OmegaWitness};

#[cfg(test)]
#[path = "native/tests.rs"]
mod tests;

const DESCRIPTOR_MAX_BYTES: usize = 1 << 20;
const VERIFYING_KEY_MAX_BYTES: usize = 1 << 18;
const LINEAGE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwomg_1");
const CHECKPOINT_CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwoctx1");

/// Genuine original artifact/preparation/proof failure; no failure changes custody.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Installed descriptor/key/catalog/parameters/layout or imported source differs.
    Artifact,
    /// Original source frame, Pallas claim, incoming mode or field encoding differs.
    Input,
    /// Actual proof or complete native accumulator decision failed.
    Proof,
    /// Actual compiled circuit assignment or proof generation failed.
    Prover,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native final Omega: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Fixed outer source layout, supplied by the authenticated producer catalog owner.
/// The strict original-PK source import checks the exact resulting circuit. An operation
/// witness cannot choose this layout or switch it after installation.
#[derive(Clone, Debug)]
pub enum Layout {
    /// Existing complete verifier with its ordinary independent lanes.
    Ordinary,
    /// Complete verifier with fixed compact lane spans.
    Compact(CompactSpans),
    /// Complete verifier with fixed compact spans and the checked secondary range program.
    Secondary {
        /// Immutable compact lane spans of the installed source program.
        spans: CompactSpans,
        /// Exact checked secondary range schedule for that source program.
        schedule: SecondaryPlan,
    },
}

/// Immutable complete terminal catalog and native source program.
/// Construction checks genuine key syntax/profile; it does not authenticate release authority.
pub struct Program {
    plan: OmegaPlan,
    source_binding: DescriptorBinding,
    source_keys: Vec<VerifyingKey<Eq>>,
    vesta: PinnedParams<Eq>,
    pallas: PinnedParams<Ep>,
    layout: Layout,
}
impl Program {
    /// Pin the complete installed terminal A key list in its authenticated catalog order.
    /// Pins the complete key catalog in the compiled source in this exact authenticated order.
    /// # Errors
    /// Not k16, bad V2 A profile, empty/duplicate/oversized catalog or malformed key.
    pub fn new(
        source_descriptor: &[u8],
        terminal_keys: &[Vec<u8>],
        vesta: PinnedParams<Eq>,
        pallas: PinnedParams<Ep>,
        layout: Layout,
    ) -> Result<Self, Error> {
        if source_descriptor.is_empty()
            || source_descriptor.len() > DESCRIPTOR_MAX_BYTES
            || terminal_keys.is_empty()
            || terminal_keys.len() > 32
            || vesta.k() != 16
            || pallas.k() != 16
        {
            return Err(Error::Artifact);
        }
        let source_binding =
            DescriptorBinding::decode_v2(source_descriptor).map_err(|_| Error::Artifact)?;
        let descriptor = source_binding.descriptor();
        if descriptor.curve != CurveV1::Vesta
            || descriptor.k != 16
            || descriptor.transcript != TranscriptV2::KagemushaPoseidonRp57Base
            || descriptor.instance_mode != InstanceModeV1::Direct
            || descriptor.proof_suffix != ProofSuffixV1::FoldedGenerator
            || descriptor.instance_lengths != [69]
            || descriptor.instance_types.as_deref()
                != Some(&[iroha_plonk::cs::InstanceType::Bounded])
        {
            return Err(Error::Artifact);
        }
        let mut source_keys = Vec::with_capacity(terminal_keys.len());
        let mut digests = Vec::with_capacity(terminal_keys.len());
        for original in terminal_keys {
            if original.is_empty() || original.len() > VERIFYING_KEY_MAX_BYTES {
                return Err(Error::Artifact);
            }
            let key =
                VerifyingKey::<Eq>::read(original, &source_binding).map_err(|_| Error::Artifact)?;
            digests.push(
                key.kagemusha_digest(&source_binding)
                    .map_err(|_| Error::Artifact)?,
            );
            source_keys.push(key);
        }
        let plan = OmegaPlan::new(source_binding.clone(), vesta.clone(), digests)
            .and_then(|plan| plan.with_key_catalog(source_keys.clone()))
            .map_err(|_| Error::Artifact)?;
        Ok(Self {
            plan,
            source_binding,
            source_keys,
            vesta,
            pallas,
            layout,
        })
    }

    fn circuit(&self, witness: OmegaWitness) -> Result<OmegaCircuit, Error> {
        let circuit = OmegaCircuit::new(self.plan.clone(), witness).map_err(|_| Error::Input)?;
        Ok(match &self.layout {
            Layout::Ordinary => circuit,
            Layout::Compact(spans) => circuit.with_compact_layout(*spans),
            Layout::Secondary { spans, schedule } => {
                circuit.with_secondary_layout(*spans, schedule.clone())
            }
        })
    }

    fn blank(&self) -> Result<OmegaCircuit, Error> {
        let length = self.plan.verifier().proof_length();
        self.circuit(OmegaWitness {
            key: self.source_keys[0].clone(),
            instances: vec![Fp::ZERO; 69],
            proof: vec![0; length],
            length: u32::try_from(length).map_err(|_| Error::Artifact)?,
            fold: [0; FOLD_WITNESS_BYTES],
        })
        .map(|circuit| circuit.without_witnesses())
    }
}

/// Original installed outer key and fixed program. No method generates artifact keys.
pub struct Prover {
    program: Program,
    key: ProvingKey<Ep>,
}

/// Installed-key-derived canonical final checkpoint layout; no caller-supplied size or kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointLayout {
    /// Complete installed Omega key digest, including its actual descriptor binding.
    pub artifact_digest: [u8; 32],
    /// Exact canonical checkpoint bytes under this original key's proof layout.
    pub payload_bytes: u32,
    /// Exact proof || P544 || V544 transport length.
    pub transport_bytes: u32,
}

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.kagemusha.final_omega_checkpoint.v1")]
struct Checkpoint {
    version: u16,
    source_context: [u8; 32],
    salt: [u8; 32],
    transport: Vec<u8>,
}

impl Prover {
    /// Import one exact original PK under the installed VK and complete compiled source.
    /// # Errors
    /// Bad cap/profile/layout/rows, changed commitments or another original source/VK.
    pub fn from_original_artifact(
        program: Program,
        descriptor: &[u8],
        installed_vk: &[u8],
        original_pk: &[u8],
        config: ReadConfig,
    ) -> Result<Self, Error> {
        if descriptor.is_empty()
            || descriptor.len() > DESCRIPTOR_MAX_BYTES
            || installed_vk.is_empty()
            || installed_vk.len() > VERIFYING_KEY_MAX_BYTES
            || original_pk.is_empty()
            || original_pk.len() > config.maximum_bytes
        {
            return Err(Error::Artifact);
        }
        let binding = DescriptorBinding::decode_v2(descriptor).map_err(|_| Error::Artifact)?;
        let d = binding.descriptor();
        if d.curve != CurveV1::Pallas
            || d.k != 16
            || d.transcript != TranscriptV2::KagemushaPoseidonRp57Base
            || d.instance_mode != InstanceModeV1::Direct
            || d.proof_suffix != ProofSuffixV1::FoldedGenerator
            || d.instance_lengths != [1, 2, K as u32]
            || d.instance_types.as_deref() != Some(&OmegaPlan::instance_types())
            || binding.n() > config.maximum_rows
        {
            return Err(Error::Artifact);
        }
        VerifyingKey::<Ep>::read(installed_vk, &binding).map_err(|_| Error::Artifact)?;
        let blank = program.blank()?;
        let key =
            ProvingKey::from_artifact_v2(original_pk, &binding, &program.pallas, &blank, config)
                .map_err(|_| Error::Artifact)?;
        if key.vk().to_bytes() != installed_vk {
            return Err(Error::Artifact);
        }
        Ok(Self { program, key })
    }

    /// Exact immutable outer descriptor bound by the installed key.
    #[must_use]
    pub fn binding(&self) -> &DescriptorBinding {
        self.key.binding()
    }
    /// Exact installed native outer verifying key.
    #[must_use]
    pub fn verifying_key(&self) -> &VerifyingKey<Ep> {
        self.key.vk()
    }

    /// Derive the exact canonical durable checkpoint size from the installed descriptor.
    /// # Errors
    /// Invalid native protocol/key digest or canonical size/codec overflow.
    pub fn checkpoint_layout(&self) -> Result<CheckpointLayout, Error> {
        let proof = Protocol::new(self.binding().descriptor())
            .map_err(|_| Error::Artifact)?
            .proof_length();
        let transport = proof
            .checked_add(2 * ACCUMULATOR_BYTES)
            .ok_or(Error::Artifact)?;
        let counting = Checkpoint {
            version: 1,
            source_context: [0; 32],
            salt: [0; 32],
            transport: vec![0; transport],
        };
        let encoded = norito::encode_canonical(&counting).map_err(|_| Error::Artifact)?;
        Ok(CheckpointLayout {
            artifact_digest: self
                .verifying_key()
                .kagemusha_digest(self.binding())
                .map_err(|_| Error::Artifact)?
                .to_repr(),
            payload_bytes: u32::try_from(encoded.len()).map_err(|_| Error::Artifact)?,
            transport_bytes: u32::try_from(transport).map_err(|_| Error::Artifact)?,
        })
    }

    /// Prepare the actual final wrapper from the verified terminal A frame. Every Vesta
    /// slot, source-k and incoming selection is decoded from that same 69-word frame.
    /// # Errors
    /// Key not in installed catalog, wrong original/frame/source, failed A/P/selected-slot
    /// full verification, D_A or installed outer-key binding, or actual local fold failure.
    pub fn prepare(
        &self,
        input: Input,
        salt: [u8; 32],
        budget: MemoryBudget,
    ) -> Result<Session<'_>, Error> {
        let program = &self.program;
        let original = input.key.to_bytes();
        let key = program
            .source_keys
            .iter()
            .find(|key| key.to_bytes() == original)
            .ok_or(Error::Artifact)?;
        if input.proof.len() != program.plan.verifier().proof_length() {
            return Err(Error::Input);
        }
        let source_public = [input.frame.to_vec()];
        verify_full(
            &program.vesta,
            &program.source_binding,
            key,
            &source_public,
            &input.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = accumulate_generator(
            &program.vesta,
            &program.source_binding,
            key,
            &source_public,
            &input.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = FoldInput::from_opening(*opening.g(), opening.challenges())
            .map_err(|_| Error::Proof)?;
        input
            .pallas
            .decide(&program.pallas, budget)
            .map_err(|_| Error::Proof)?;
        let omega_key_digest = self
            .key
            .vk()
            .kagemusha_digest(self.key.binding())
            .map_err(|_| Error::Artifact)?;
        if input.public[17] != omega_key_digest
            || lineage_digest(&input.public, &input.pallas)? != input.frame[0]
        {
            return Err(Error::Input);
        }
        let slots = frame_slots(&input.frame, opening.clone())?;
        for slot in &slots {
            slot.decide(&program.vesta, budget)
                .map_err(|_| Error::Proof)?;
        }
        let fold_config = FoldConfig {
            kernel_budget: budget,
            ..FoldConfig::default()
        };
        let (fold, vesta) =
            create_fold(&program.vesta, &slots, salt, &fold_config).map_err(|_| Error::Proof)?;
        vesta
            .decide(&program.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let public = outer_instances(input.frame[0], &vesta)?;
        let mut context = omega_key_digest.to_repr().to_vec();
        context.extend_from_slice(
            &key.kagemusha_digest(&program.source_binding)
                .map_err(|_| Error::Artifact)?
                .to_repr(),
        );
        context.extend_from_slice(&salt);
        context.extend_from_slice(
            &u32::try_from(input.proof.len())
                .map_err(|_| Error::Input)?
                .to_le_bytes(),
        );
        context.extend_from_slice(&input.proof);
        for value in input.frame.iter().chain(&input.public) {
            context.extend_from_slice(&value.to_repr());
        }
        context.extend_from_slice(&input.pallas.to_bytes());
        let source_context = p_bytes_native::<Fp>(CHECKPOINT_CONTEXT_DOMAIN, &context).to_repr();
        let witness = OmegaWitness {
            key: key.clone(),
            instances: input.frame.to_vec(),
            length: u32::try_from(input.proof.len()).map_err(|_| Error::Input)?,
            proof: input.proof,
            fold: fold.to_bytes(),
        };
        let circuit = program.circuit(witness)?;
        Ok(Session {
            owner: self,
            circuit,
            public,
            pallas: input.pallas,
            vesta,
            salt,
            source_context,
        })
    }
}

/// Actual output of a genuine terminal A producer plus its matching native public18/P claim.
/// No host acceptance verdict, caller-computed Omega instances or optional slot objects.
pub struct Input {
    /// Actual terminal A key; must exactly equal a key in the installed complete catalog.
    pub key: VerifyingKey<Eq>,
    /// Exact original terminal A proof.
    pub proof: Vec<u8>,
    /// Exact original terminal A frame.
    pub frame: [Fp; 69],
    /// Exact source-bound eighteen-field native lineage prefix from canonical preparation.
    pub public: [Fp; 18],
    /// Full original Pallas output, bound through frame D_A and completely decided.
    pub pallas: AccumulatorT<Ep>,
}

/// Prepared session tied to its immutable imported owner; private fields prevent forged state.
pub struct Session<'a> {
    owner: &'a Prover,
    circuit: OmegaCircuit,
    public: Vec<Vec<Fq>>,
    pallas: AccumulatorT<Ep>,
    vesta: AccumulatorT<Eq>,
    salt: [u8; 32],
    source_context: [u8; 32],
}
impl Session<'_> {
    /// Exact retained fold salt; restore recreates this same source session with this salt.
    #[must_use]
    pub const fn fold_salt(&self) -> [u8; 32] {
        self.salt
    }

    /// Encode a canonical durable final checkpoint only after full native restoration.
    /// # Errors
    /// Failed original proof/decides, canonical codec or installed layout mismatch.
    pub fn encode_checkpoint(
        &self,
        transport: &[u8],
        budget: MemoryBudget,
    ) -> Result<Vec<u8>, Error> {
        self.restore_transport(transport, budget)?;
        let bytes = norito::encode_canonical(&Checkpoint {
            version: 1,
            source_context: self.source_context,
            salt: self.salt,
            transport: transport.to_vec(),
        })
        .map_err(|_| Error::Input)?;
        if bytes.len() != self.owner.checkpoint_layout()?.payload_bytes as usize {
            return Err(Error::Input);
        }
        Ok(bytes)
    }

    /// Restore canonical bytes under the actual rederived source session and installed key.
    /// Metadata cannot select another salt, terminal original, artifact or transport size.
    /// # Errors
    /// Wrong exact bound/canonical codec/context/salt or failed native proof/claim decisions.
    pub fn restore_checkpoint(
        &self,
        original: &[u8],
        budget: MemoryBudget,
    ) -> Result<Output, Error> {
        let layout = self.owner.checkpoint_layout()?;
        if original.len() != layout.payload_bytes as usize {
            return Err(Error::Input);
        }
        let checkpoint: Checkpoint = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(|_| Error::Input)?;
        if checkpoint.version != 1
            || checkpoint.source_context != self.source_context
            || checkpoint.salt != self.salt
            || checkpoint.transport.len() != layout.transport_bytes as usize
        {
            return Err(Error::Input);
        }
        self.restore_transport(&checkpoint.transport, budget)
    }
    /// Restore the exact canonical final transport after interruption under this same
    /// rederived source session. Claim bytes cannot replace the prepared original P/V.
    /// # Errors
    /// Wrong total length, noncanonical/different claims, changed source public or a failed
    /// original Omega opening/full Pallas/Vesta native decision.
    pub fn restore_transport(
        &self,
        original: &[u8],
        budget: MemoryBudget,
    ) -> Result<Output, Error> {
        let owner = self.owner;
        let length = Protocol::new(owner.key.binding().descriptor())
            .map_err(|_| Error::Artifact)?
            .proof_length();
        if length.checked_add(2 * ACCUMULATOR_BYTES) != Some(original.len()) {
            return Err(Error::Input);
        }
        let (proof, claims) = original.split_at(length);
        let (pallas, vesta) = claims.split_at(ACCUMULATOR_BYTES);
        let pallas = AccumulatorT::<Ep>::from_bytes(pallas).map_err(|_| Error::Input)?;
        let vesta = AccumulatorT::<Eq>::from_bytes(vesta).map_err(|_| Error::Input)?;
        if pallas != self.pallas || vesta != self.vesta {
            return Err(Error::Input);
        }
        verify_full(
            &owner.program.pallas,
            owner.key.binding(),
            owner.key.vk(),
            &self.public,
            proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = accumulate_generator(
            &owner.program.pallas,
            owner.key.binding(),
            owner.key.vk(),
            &self.public,
            proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = FoldInput::from_opening(*opening.g(), opening.challenges())
            .map_err(|_| Error::Proof)?;
        opening
            .decide(&owner.program.pallas, budget)
            .map_err(|_| Error::Proof)?;
        pallas
            .decide(&owner.program.pallas, budget)
            .map_err(|_| Error::Proof)?;
        vesta
            .decide(&owner.program.vesta, budget)
            .map_err(|_| Error::Proof)?;
        Ok(Output {
            proof: proof.to_vec(),
            instances: self.public.clone(),
            pallas,
            vesta,
            opening,
        })
    }

    /// Prove the actual compiled Omega and fully verify own opening plus both transported claims.
    /// # Errors
    /// Actual witness assignment, installed proof generation or full native decisions fail.
    pub fn prove(
        &self,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<Output, Error> {
        let owner = self.owner;
        let witness = Witness::from_circuit(&owner.key, &self.circuit, &self.public)
            .map_err(|_| Error::Prover)?;
        let output = create_proof_owned_with_claim(
            &owner.program.pallas,
            &owner.key,
            witness,
            randomness,
            config,
        )
        .map_err(|_| Error::Prover)?;
        let expected = Protocol::new(owner.key.binding().descriptor())
            .map_err(|_| Error::Artifact)?
            .proof_length();
        if output.proof.len() != expected {
            return Err(Error::Proof);
        }
        verify_full(
            &owner.program.pallas,
            owner.key.binding(),
            owner.key.vk(),
            &self.public,
            &output.proof,
            config.msm_budget,
        )
        .map_err(|_| Error::Proof)?;
        output
            .opening
            .decide(&owner.program.pallas, config.msm_budget)
            .map_err(|_| Error::Proof)?;
        self.pallas
            .decide(&owner.program.pallas, config.msm_budget)
            .map_err(|_| Error::Proof)?;
        self.vesta
            .decide(&owner.program.vesta, config.msm_budget)
            .map_err(|_| Error::Proof)?;
        let opening = FoldInput::from_opening(*output.opening.g(), output.opening.challenges())
            .map_err(|_| Error::Proof)?;
        Ok(Output {
            proof: output.proof,
            instances: self.public.clone(),
            pallas: self.pallas.clone(),
            vesta: self.vesta.clone(),
            opening,
        })
    }
}

/// Genuine fully self-verified final native proof and distinct preserved obligations.
pub struct Output {
    /// Exact outer Omega proof.
    pub proof: Vec<u8>,
    /// Native public columns derived from source D_A and actual four-slot fold.
    pub instances: Vec<Vec<Fq>>,
    /// Original source full Pallas claim, distinct from the outer proof's own opening.
    pub pallas: AccumulatorT<Ep>,
    /// Actual full Vesta result of the four-slot local fold.
    pub vesta: AccumulatorT<Eq>,
    /// Outer proof's own opening, fully decided and retained separately.
    pub opening: FoldInput<Ep>,
}
impl Output {
    /// Canonical native unframed transport: original Omega proof || P544 || V544.
    /// The owning complete G1 allowlist enforces its exact admitted length and wire caps.
    #[must_use]
    pub fn transport(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.proof.len() + 2 * ACCUMULATOR_BYTES);
        bytes.extend_from_slice(&self.proof);
        bytes.extend_from_slice(&self.pallas.to_bytes());
        bytes.extend_from_slice(&self.vesta.to_bytes());
        bytes
    }
}

fn native_u128(value: Fp) -> Result<u128, Error> {
    let bytes = value.to_repr();
    if bytes[16..].iter().any(|byte| *byte != 0) {
        return Err(Error::Input);
    }
    Ok(u128::from_le_bytes(
        bytes[..16].try_into().map_err(|_| Error::Input)?,
    ))
}
fn foreign_coordinate(words: &[Fp]) -> Result<Fq, Error> {
    let [low, high] = words else {
        return Err(Error::Input);
    };
    let mut bytes = [0; 32];
    bytes[..16].copy_from_slice(&native_u128(*low)?.to_le_bytes());
    bytes[16..].copy_from_slice(&native_u128(*high)?.to_le_bytes());
    Option::<Fq>::from(Fq::from_repr(bytes)).ok_or(Error::Input)
}
fn point(words: &[Fp]) -> Result<EqAffine, Error> {
    if words.len() != 4 {
        return Err(Error::Input);
    }
    let point = Option::<EqAffine>::from(EqAffine::from_xy(
        foreign_coordinate(&words[..2])?,
        foreign_coordinate(&words[2..])?,
    ))
    .ok_or(Error::Input)?;
    // Pasta encodes the identity as (0, 0); a transported fold claim requires
    // a nonidentity affine point, including in an unselected incoming slot.
    if bool::from(point.coordinates().is_none()) {
        return Err(Error::Input);
    }
    Ok(point)
}
fn claim(point_words: &[Fp], challenges: &[Fp], source_k: u32) -> Result<FoldInput<Eq>, Error> {
    FoldInput::from_normalized(
        point(point_words)?,
        source_k,
        challenges.try_into().map_err(|_| Error::Input)?,
    )
    .map_err(|_| Error::Input)
}
fn frame_slots(frame: &[Fp; 69], opening: FoldInput<Eq>) -> Result<[FoldInput<Eq>; 4], Error> {
    let source_k = u32::try_from(native_u128(frame[1])?).map_err(|_| Error::Input)?;
    if !matches!(source_k, 12 | 14 | 16) || opening.source_k() != 16 {
        return Err(Error::Input);
    }
    let part = claim(&frame[2..6], &frame[6..22], source_k)?;
    let predecessor = claim(&frame[22..26], &frame[26..42], 16)?;
    let incoming = claim(&frame[42..46], &frame[46..62], 16)?;
    let corrected = point(&frame[65..69])?;
    let selected = match [frame[62], frame[63], frame[64]] {
        modes if modes == [Fp::ONE, Fp::ZERO, Fp::ZERO] => incoming,
        modes if modes == [Fp::ZERO, Fp::ONE, Fp::ZERO] => FoldInput::from_normalized(
            decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).map_err(|_| Error::Artifact)?,
            16,
            [Fp::ONE; K],
        )
        .map_err(|_| Error::Artifact)?,
        modes if modes == [Fp::ZERO, Fp::ZERO, Fp::ONE] => {
            if &corrected == incoming.g() {
                return Err(Error::Input);
            }
            FoldInput::from_normalized(corrected, 16, *incoming.challenges())
                .map_err(|_| Error::Input)?
        }
        _ => return Err(Error::Input),
    };
    Ok([part, opening, predecessor, selected])
}
fn lineage_digest(public: &[Fp; 18], pallas: &AccumulatorT<Ep>) -> Result<Fp, Error> {
    let (x, y) = Option::<(Fp, Fp)>::from(pallas.g().coordinates()).ok_or(Error::Input)?;
    let mut fields = public.to_vec();
    fields.extend([x, y]);
    for challenge in pallas.challenges() {
        fields.extend(foreign_limbs(challenge).map(Fp::from_u128));
    }
    if fields.len() != 52 {
        return Err(Error::Input);
    }
    Ok(hash_with_domain(LINEAGE_DOMAIN, &fields))
}
fn outer_instances(context: Fp, vesta: &AccumulatorT<Eq>) -> Result<Vec<Vec<Fq>>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(vesta.g().coordinates()).ok_or(Error::Input)?;
    let context = Option::<Fq>::from(Fq::from_repr(context.to_repr())).ok_or(Error::Input)?;
    let challenges = vesta
        .challenges()
        .iter()
        .map(|value| Option::<Fq>::from(Fq::from_repr(value.to_repr())).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![vec![context], vec![x, y], challenges])
}
