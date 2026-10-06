//! Fixed native Receive and renewed-Receive A/W composition.
//!
//! Installed artifacts pin the complete owner schedule. Incoming decoded values,
//! five result bits and correction points are witness proposals: the mandatory
//! producers derive them from the exact original tapes and bind one common
//! context. Native preparation verifies every hard predecessor/Q proof and all
//! selected deciding claims; it never replaces a soft failure with acceptance.

mod circuit;
mod session;

#[cfg(test)]
mod tests;

pub use circuit::StageCircuit;
pub use session::{ACheckpoint, KeyArtifact, Prover, Session, Terminal, WCheckpoint};

use core::fmt;
use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_pasta::{
    Ep, EpAffine, Eq, EqAffine, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain,
};
use iroha_plonk::{
    DescriptorBinding, VerifyingKey,
    cs::InstanceType,
    pcs::ipa::PinnedParams,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::{
    bytes::{le_value, p_bytes_native},
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, K, create_fold, obligation::ledger::Variant,
    verifier::VerifierPlan,
};

use crate::{
    a_relation::{
        AProofPlan, QProofPlan,
        context::{ContextObjectSpec, ContextPlan},
        own::OwnPolicy,
        receive::{
            MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES, PAYMENT_PROOF_BUDGET, ReceiveStagePlan,
        },
        schedule::OperationTask,
    },
    admin_sigma::StateWitness,
    operation_relation::objects::ObjectKind,
    q_sigma::{QSigmaPlan, native::IncomingMode},
    tree::IndexedInsert,
};
use circuit::Stage;

/// Internal A circuits use the fixed tagged-four range layout.
pub const INTERNAL_RANGE_BUSES: usize = 4;
/// The operation terminal uses the common tagged-three catalog layout.
pub const TERMINAL_RANGE_BUSES: usize = 3;
/// Ten A stages separated by nine authenticated W continuations.
pub const A_STAGE_COUNT: usize = 10;

/// Native source/artifact/proof error. No failure changes monetary state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// An installed key, descriptor, parameter set or fixed schedule differs.
    Artifact,
    /// Exact source shape, canonical claim or admitted envelope is invalid.
    Input,
    /// A hard proof or selected accumulator fails full verification.
    Proof,
    /// Circuit assignment or proving fails.
    Prover,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native Receive: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Original verified Q proof and its exact descriptor-shaped public columns.
#[derive(Clone, Debug)]
pub struct QInput {
    /// Original unframed proof bytes.
    pub proof: Vec<u8>,
    /// Exact public columns; result and mode exports remain circuit-bound.
    pub instances: Vec<Vec<Fq>>,
}

/// Hard predecessor proof and both original full-k16 transported claims.
#[derive(Clone, Debug)]
pub struct PredecessorInput {
    /// Descriptor-sized unframed proof.
    pub proof: Vec<u8>,
    /// Canonical Pallas accumulator bytes.
    pub pallas: [u8; 544],
    /// Canonical Vesta accumulator bytes.
    pub vesta: [u8; 544],
}

/// Complete Receive state transition and exact depth32 map routes.
#[derive(Clone, Debug)]
pub struct Transition {
    /// Pre-Advance state and lineage public fields.
    pub before: StateWitness,
    /// Proposed successor and adjusted credit/burn lineage fields.
    pub after: StateWitness,
    /// Exact own statement26.
    pub statement: [Fp; 26],
    /// Consumed-credit authenticated lookup and insertion route.
    pub consumed: IndexedInsert<Fp>,
    /// Credit-record insertion route.
    pub credit: IndexedInsert<Fp>,
    /// Recorded-blacklist history search route; only its low opening is consumed.
    pub blacklist: IndexedInsert<Fp>,
    /// The OQ-3 insertion flag, derived again by terminal Effects.
    pub insert: bool,
}

/// Total incoming decoder and result proposals authenticated by the fixed owners.
/// Malformed original bytes use the decoder's fixed safe representation. These
/// fields are not native acceptance certificates: the circuits derive every
/// value and validity bit from the original objects and Q proofs.
#[derive(Clone, Debug)]
pub struct IncomingWitness {
    /// Exact decoded public fields, or the pinned safe fields on malformed input.
    pub public: [Fp; 18],
    /// Proposed total public-decoder validity bit.
    pub public_valid: bool,
    /// Original decoded Pallas claim, including a non-deciding commitment if present.
    pub pallas: AccumulatorT<Ep>,
    /// Original decoded Vesta claim, including a non-deciding commitment if present.
    pub vesta: AccumulatorT<Eq>,
    /// Actual soft-Omega verifier opening, or its fixed safe dummy on failure.
    pub opening: FoldInput<Ep>,
    /// Five proposed fixed-order owner results.
    pub results: [bool; 5],
    /// Original-P, Omega-opening, original-V, incoming-sigma modes.
    pub modes: [IncomingMode; 4],
    /// Proposed corrected commitments for original-P and Omega-opening slots.
    pub pallas_corrections: [EpAffine; 2],
    /// Proposed corrected commitment for the original-V slot.
    pub vesta_correction: EqAffine,
}

/// Exact originals retained after Advance for complete ordinary/renewed Receive.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Own state transition and local map witnesses.
    pub transition: Transition,
    /// Original incoming Send statement26.
    pub incoming_statement: [Fp; 26],
    /// Exact own hard sigma tape.
    pub sigma: Vec<u8>,
    /// Request, payer Credential, Send Receipt, Payment, raw Omega, raw sigma,
    /// current Credential, certificate, own Receipt, quoted Credential, certificate.
    pub objects: [Vec<u8>; 11],
    /// Q0 sigma, Q1 hard authorization, Q2 original incoming signatures.
    pub q: [QInput; 3],
    /// Actual receiver predecessor.
    pub predecessor: PredecessorInput,
    /// Source-bound soft decoder/results and obligation proposals.
    pub incoming: IncomingWitness,
}

/// Immutable complete ten-stage Receive metadata, pinned by artifact installation.
#[derive(Clone, Debug)]
pub struct Plan {
    stage: ReceiveStagePlan,
    policy: OwnPolicy,
    predecessor_key: VerifyingKey<Ep>,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin the exact full-envelope schedule for Receive or renewed Receive.
    /// # Errors
    /// Wrong variant, sigma/Q/source descriptor, key continuity or parameter profile.
    pub fn new(
        operation: AProofPlan,
        policy: OwnPolicy,
        predecessor_key: VerifyingKey<Ep>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
    ) -> Result<Self, Error> {
        let variant = operation.frame().variant();
        if !matches!(variant, Variant::Receive | Variant::ReceiveRenewed)
            || operation.frame().part_source_k() != 16
            || operation.q_count() != 3
            || operation.sigma.slot_count() != 2
            || !operation.frame().has_predecessor()
            || !operation.frame().has_incoming()
        {
            return Err(Error::Artifact);
        }
        for slot in 0..2 {
            let class = operation.sigma.class(slot).ok_or(Error::Artifact)?;
            if !matches!(class.verifier().binding().descriptor().k, 12 | 14)
                || class.verifier().proof_length() > MAX_SIGMA_RAW_BYTES
            {
                return Err(Error::Artifact);
            }
        }
        let incoming =
            crate::a_relation::incoming_transport::IncomingTransportPlan::new(&operation)
                .map_err(|_| Error::Artifact)?;
        if incoming.payload_length().map_err(|_| Error::Artifact)? > MAX_OMEGA_RAW_BYTES {
            return Err(Error::Artifact);
        }
        validate_transport_profile(
            incoming.payload_length().map_err(|_| Error::Artifact)?,
            operation
                .sigma
                .class(1)
                .ok_or(Error::Artifact)?
                .verifier()
                .proof_length(),
        )?;
        pallas.require_k(16).map_err(|_| Error::Artifact)?;
        vesta.require_k(16).map_err(|_| Error::Artifact)?;
        let predecessor = operation.omega().ok_or(Error::Artifact)?;
        let d = predecessor.binding().descriptor();
        if predecessor_key.descriptor_digest() != predecessor.binding().digest()
            || d.k != 16
            || d.instance_lengths != [1, 2, 16]
            || d.instance_types.as_deref()
                != Some(&[
                    InstanceType::Bounded,
                    InstanceType::Field,
                    InstanceType::Bounded,
                ])
        {
            return Err(Error::Artifact);
        }
        predecessor_key
            .kagemusha_digest(predecessor.binding())
            .map_err(|_| Error::Artifact)?;
        let context = ContextPlan::with_schedule(
            operation,
            vec![
                vec![],
                vec![0],
                vec![],
                vec![],
                vec![],
                vec![1],
                vec![2],
                vec![],
                vec![],
                vec![],
            ],
            Some(0),
            ReceiveStagePlan::context_specs(variant, MAX_OMEGA_RAW_BYTES, MAX_SIGMA_RAW_BYTES)
                .map_err(|_| Error::Artifact)?,
        )
        .and_then(|p| p.with_operation_tasks(task_schedule()))
        .map_err(|_| Error::Artifact)?;
        let stage = ReceiveStagePlan::new(context, policy).map_err(|_| Error::Artifact)?;
        Ok(Self {
            stage,
            policy,
            predecessor_key,
            pallas,
            vesta,
        })
    }
    /// Exact authenticated context, task owners and Q partitions.
    pub const fn context(&self) -> &ContextPlan {
        self.stage.context()
    }
    /// Verify hard originals and retain all soft proposals for their circuit owners.
    /// # Errors
    /// Over-envelope input, wrong exact tape shape, hard proof or deciding obligation.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Prepared, Error> {
        let program = self.context().operation();
        let omega = program.omega().ok_or(Error::Artifact)?;
        let sigma = &program.sigma;
        validate_envelope_lengths(input.objects[4].len(), input.objects[5].len())?;
        if input.sigma.len()
            != sigma
                .class(0)
                .ok_or(Error::Artifact)?
                .verifier()
                .proof_length()
        {
            return Err(Error::Input);
        }
        let specs = self.context().object_specs();
        for (index, raw) in input.objects.iter().enumerate() {
            if !matches!(index, 4 | 5)
                && raw.len()
                    != usize::try_from(specs[index].capacity).map_err(|_| Error::Artifact)?
            {
                return Err(Error::Input);
            }
        }
        let pallas =
            AccumulatorT::from_bytes(&input.predecessor.pallas).map_err(|_| Error::Input)?;
        let vesta = AccumulatorT::from_bytes(&input.predecessor.vesta).map_err(|_| Error::Input)?;
        pallas
            .decide(&self.pallas, budget)
            .map_err(|_| Error::Proof)?;
        vesta
            .decide(&self.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let key_digest = self
            .predecessor_key
            .kagemusha_digest(omega.binding())
            .map_err(|_| Error::Artifact)?;
        if input.transition.before.lineage[17] != key_digest
            || input.transition.after.lineage[17] != key_digest
        {
            return Err(Error::Input);
        }
        let public = omega_instances(
            terminal_digest(&input.transition.before.lineage, &pallas.as_input())?,
            &vesta,
        )?;
        verify_full(
            &self.pallas,
            omega.binding(),
            &self.predecessor_key,
            &public,
            &input.predecessor.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_pallas(
            &self.pallas,
            omega.binding(),
            &self.predecessor_key,
            &public,
            &input.predecessor.proof,
            budget,
        )?;
        let predecessor = Head {
            state: input.transition.before,
            key: self.predecessor_key.clone(),
            proof: input.predecessor.proof,
            pallas,
            vesta,
            opening,
        };
        let mut q = Vec::with_capacity(3);
        for (index, source) in input.q.into_iter().enumerate() {
            let fixed = program.q(index).ok_or(Error::Artifact)?;
            verify_full(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &source.instances,
                &source.proof,
                budget,
            )
            .map_err(|_| Error::Proof)?;
            let opening = opening_pallas(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &source.instances,
                &source.proof,
                budget,
            )?;
            q.push(QSource {
                plan: fixed.clone(),
                proof: source.proof,
                instances: source.instances,
                opening,
            });
        }
        let part = q_sigma_part(&q[0].instances)?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        validate_modes(&input.incoming)?;
        let selected_pallas = [
            select_pallas(
                &self.pallas,
                &input.incoming.pallas.as_input(),
                input.incoming.modes[0],
                input.incoming.pallas_corrections[0],
                budget,
            )?,
            select_pallas(
                &self.pallas,
                &input.incoming.opening,
                input.incoming.modes[1],
                input.incoming.pallas_corrections[1],
                budget,
            )?,
        ];
        // The terminal exports original V and its mode, while native preparation
        // separately decides the exact selected replacement before any proof work.
        let _selected_vesta = select_vesta(
            &self.vesta,
            &input.incoming.vesta.as_input(),
            input.incoming.modes[2],
            input.incoming.vesta_correction,
            budget,
        )?;
        let commitments = object_commitments(specs, &input.objects)?;
        let incoming_head = Head {
            state: StateWitness {
                core: [Fp::ZERO; 33],
                rest: [Fp::ZERO; 8],
                lineage: input.incoming.public,
            },
            key: self.predecessor_key.clone(),
            proof: vec![],
            pallas: input.incoming.pallas,
            vesta: input.incoming.vesta,
            opening: input.incoming.opening,
        };
        let source = Arc::new(Source {
            own: Own {
                witness: input.transition,
                send: SendStatement {
                    statement: input.incoming_statement,
                },
                sigma: input.sigma,
                incoming_sigma: input.objects[5].clone(),
                sigma_plan: sigma.clone(),
                part,
            },
            incoming: input.objects[4].clone(),
            objects: input.objects,
            commitments,
            predecessor,
            incoming_head,
            q,
            plan: self.stage.clone(),
            policy: self.policy,
            variant: program.frame().variant(),
            params: self.pallas.clone(),
            vparams: self.vesta.clone(),
            results: input.incoming.results,
            modes: input.incoming.modes,
            public_valid: input.incoming.public_valid,
            pallas_corrections: input.incoming.pallas_corrections,
            vesta_correction: input.incoming.vesta_correction,
            selected_pallas,
        });
        Ok(Prepared {
            plan: self.clone(),
            source,
        })
    }
}

fn task_schedule() -> Vec<Vec<OperationTask>> {
    use OperationTask::*;
    vec![
        vec![
            ReceiveOwnProof,
            ReceiveConsumedEffects,
            ReceiveCreditEffects,
        ],
        vec![],
        vec![ReceiveProofs],
        vec![ReceiveProofDigest],
        vec![ReceiveObjects],
        vec![ReceiveAuthorization],
        vec![],
        vec![ReceiveSignatures],
        vec![ReceiveNonmembership, ReceiveBlacklist],
        vec![ReceiveEffects],
    ]
}
#[derive(Clone)]
struct QSource {
    plan: QProofPlan,
    proof: Vec<u8>,
    instances: Vec<Vec<Fq>>,
    opening: FoldInput<Ep>,
}
#[derive(Clone)]
struct Head {
    state: StateWitness,
    key: VerifyingKey<Ep>,
    proof: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    vesta: AccumulatorT<Eq>,
    opening: FoldInput<Ep>,
}
#[derive(Clone)]
struct SendStatement {
    statement: [Fp; 26],
}
#[derive(Clone)]
struct Own {
    witness: Transition,
    send: SendStatement,
    sigma: Vec<u8>,
    incoming_sigma: Vec<u8>,
    sigma_plan: QSigmaPlan,
    part: FoldInput<Eq>,
}
struct Source {
    own: Own,
    predecessor: Head,
    incoming_head: Head,
    q: Vec<QSource>,
    plan: ReceiveStagePlan,
    policy: OwnPolicy,
    variant: Variant,
    objects: [Vec<u8>; 11],
    commitments: Vec<[Fp; 3]>,
    incoming: Vec<u8>,
    params: PinnedParams<Ep>,
    vparams: PinnedParams<Eq>,
    results: [bool; 5],
    modes: [IncomingMode; 4],
    public_valid: bool,
    pallas_corrections: [EpAffine; 2],
    vesta_correction: EqAffine,
    selected_pallas: [FoldInput<Ep>; 2],
}
impl Source {
    fn mode_words(&self, index: usize) -> [Fp; 3] {
        mode_words(self.modes[index])
    }
}
fn mode_words(mode: IncomingMode) -> [Fp; 3] {
    match mode {
        IncomingMode::Accept => [Fp::ONE, Fp::ZERO, Fp::ZERO],
        IncomingMode::Trivial => [Fp::ZERO, Fp::ONE, Fp::ZERO],
        IncomingMode::Corrected => [Fp::ZERO, Fp::ZERO, Fp::ONE],
    }
}

/// Checked hard originals coupled to this exact full-envelope owner plan.
#[derive(Clone)]
pub struct Prepared {
    plan: Plan,
    source: Arc<Source>,
}
impl Prepared {
    fn first(&self, salt: Fp, config: &FoldConfig) -> Result<Stage, Error> {
        let (fold, pallas) = create_fold(
            &self.plan.pallas,
            &[
                self.source.predecessor.pallas.as_input(),
                self.source.predecessor.opening.clone(),
            ],
            salt.to_repr(),
            config,
        )
        .map_err(|_| Error::Proof)?;
        pallas
            .decide(&self.plan.pallas, config.kernel_budget)
            .map_err(|_| Error::Proof)?;
        Ok(Stage {
            source: self.source.clone(),
            continuation: None,
            pallas: pallas.clone(),
            first: pallas,
            fold: fold.to_bytes().to_vec(),
            known: true,
        })
    }
    /// Construct the real A1 relation and its exact public frame for artifact tooling.
    /// # Errors
    /// A failed two-input predecessor fold or public-frame conversion.
    pub fn first_circuit(
        &self,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<(StageCircuit, Vec<Fp>), Error> {
        let inner = self.first(salt, config)?;
        let public = stage_public(&inner)?;
        Ok((StageCircuit { inner }, public))
    }
}

fn validate_transport_profile(omega: usize, sigma: usize) -> Result<(), Error> {
    let public = crate::a_relation::own::ConsumingProofCells::PUBLIC_BYTES;
    if omega
        .checked_sub(public)
        .and_then(|payload| payload.checked_add(sigma))
        .is_none_or(|payload| payload > PAYMENT_PROOF_BUDGET)
    {
        return Err(Error::Artifact);
    }
    Ok(())
}

fn validate_envelope_lengths(omega: usize, sigma: usize) -> Result<(), Error> {
    if omega > MAX_OMEGA_RAW_BYTES
        || sigma > MAX_SIGMA_RAW_BYTES
        || omega
            .checked_add(sigma)
            .is_none_or(|total| total > 320 + PAYMENT_PROOF_BUDGET)
    {
        return Err(Error::Input);
    }
    Ok(())
}

fn validate_modes(input: &IncomingWitness) -> Result<(), Error> {
    let corrections = input
        .modes
        .iter()
        .filter(|m| **m == IncomingMode::Corrected)
        .count();
    if corrections > 1 {
        return Err(Error::Input);
    }
    let valid = input.results.iter().all(|v| *v) && corrections == 0;
    if input
        .modes
        .iter()
        .any(|m| (*m == IncomingMode::Accept) != valid)
    {
        return Err(Error::Input);
    }
    Ok(())
}
fn select_pallas(
    params: &PinnedParams<Ep>,
    original: &FoldInput<Ep>,
    mode: IncomingMode,
    correction: EpAffine,
    budget: MemoryBudget,
) -> Result<FoldInput<Ep>, Error> {
    let selected = match mode {
        IncomingMode::Accept => original.clone(),
        IncomingMode::Trivial => AccumulatorT::trivial(params, budget)
            .map_err(|_| Error::Proof)?
            .as_input(),
        IncomingMode::Corrected => {
            if correction == *original.g() {
                return Err(Error::Input);
            }
            FoldInput::from_normalized(correction, original.source_k(), *original.challenges())
                .map_err(|_| Error::Input)?
        }
    };
    selected.decide(params, budget).map_err(|_| Error::Proof)?;
    Ok(selected)
}
fn select_vesta(
    params: &PinnedParams<Eq>,
    original: &FoldInput<Eq>,
    mode: IncomingMode,
    correction: EqAffine,
    budget: MemoryBudget,
) -> Result<FoldInput<Eq>, Error> {
    let selected = match mode {
        IncomingMode::Accept => original.clone(),
        IncomingMode::Trivial => AccumulatorT::trivial(params, budget)
            .map_err(|_| Error::Proof)?
            .as_input(),
        IncomingMode::Corrected => {
            if correction == *original.g() {
                return Err(Error::Input);
            }
            FoldInput::from_normalized(correction, original.source_k(), *original.challenges())
                .map_err(|_| Error::Input)?
        }
    };
    selected.decide(params, budget).map_err(|_| Error::Proof)?;
    Ok(selected)
}
fn q_sigma_part(columns: &[Vec<Fq>]) -> Result<FoldInput<Eq>, Error> {
    let [bounded, point, indices, verdicts, source] = columns else {
        return Err(Error::Input);
    };
    if point.len() != 2
        || indices.len() != 2
        || verdicts.len() != 5
        || verdicts[0] != Fq::ONE
        || source.as_slice() != [Fq::from(16)]
        || bounded.len() < K
    {
        return Err(Error::Input);
    }
    let g = Option::<EqAffine>::from(EqAffine::from_xy(point[0], point[1])).ok_or(Error::Input)?;
    let u = bounded[bounded.len() - K..]
        .iter()
        .map(|v| Option::<Fp>::from(Fp::from_repr(v.to_repr())).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    FoldInput::from_normalized(g, 16, u.try_into().map_err(|_| Error::Input)?)
        .map_err(|_| Error::Input)
}
fn frame(raw: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = u32::try_from(raw.len())
        .map_err(|_| Error::Input)?
        .to_le_bytes()
        .to_vec();
    out.extend(raw);
    Ok(out)
}
fn object_digest(kind: ObjectKind, raw: &[u8]) -> Result<Fp, Error> {
    let end = kind.body_len();
    if raw.len() != end + 64 {
        return Err(Error::Input);
    }
    let mut words = vec![p_bytes_native(kind.signing_domain(), &raw[..end])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            raw[end + offset..end + offset + 16]
                .try_into()
                .map_err(|_| Error::Input)?,
        )));
    }
    Ok(hash_with_domain(kind.object_domain(), &words))
}
fn object_commitments(
    specs: &[ContextObjectSpec],
    objects: &[Vec<u8>; 11],
) -> Result<Vec<[Fp; 3]>, Error> {
    if specs.len() != 11 {
        return Err(Error::Artifact);
    }
    let kinds = [
        Some(ObjectKind::Request),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Receipt),
        None,
        None,
        None,
        Some(ObjectKind::Credential),
        Some(ObjectKind::Certificate),
        Some(ObjectKind::Receipt),
        Some(ObjectKind::Credential),
        Some(ObjectKind::Certificate),
    ];
    let mut proof_bytes = frame(&objects[4])?;
    proof_bytes.extend(frame(&objects[5])?);
    let proof_digest = p_bytes_native(u64::from_le_bytes(*b"kgwprf_1"), &proof_bytes);
    specs
        .iter()
        .enumerate()
        .map(|(i, spec)| {
            let digest = match kinds[i] {
                Some(kind) => object_digest(kind, &objects[i])?,
                None => match i {
                    3 => p_bytes_native(u64::from_le_bytes(*b"kgwpay_1"), &objects[i]),
                    4 => proof_digest,
                    5 => p_bytes_native(u64::from_le_bytes(*b"kgwstep1"), &frame(&objects[i])?),
                    _ => return Err(Error::Input),
                },
            };
            let length = Fp::from(u64::try_from(objects[i].len()).map_err(|_| Error::Input)?);
            let mut words = vec![
                Fp::from(u64::from(spec.tag)),
                Fp::from(u64::from(spec.capacity)),
            ];
            let raw = frame(&objects[i])?;
            let domain = if matches!(i, 4 | 5) {
                words.extend([
                    length,
                    p_bytes_native(u64::from_le_bytes(*b"kgwcact1"), &raw),
                ]);
                u64::from_le_bytes(*b"kgwcact1")
            } else {
                for chunk in raw.chunks(31) {
                    words.push(le_value(chunk).ok_or(Error::Input)?);
                }
                u64::from_le_bytes(*b"kgwctap1")
            };
            Ok([digest, length, hash_with_domain(domain, &words)])
        })
        .collect()
}
fn push_pallas(words: &mut Vec<Fp>, claim: &FoldInput<Ep>) -> Result<(), Error> {
    if claim.source_k() != 16 {
        return Err(Error::Input);
    }
    let (x, y) = Option::<(Fp, Fp)>::from(claim.g().coordinates()).ok_or(Error::Input)?;
    words.extend([Fp::from(16), x, y]);
    for u in claim.challenges() {
        words.extend(foreign_limbs(u).map(Fp::from_u128));
    }
    Ok(())
}
fn vesta_words(claim: &FoldInput<Eq>) -> Result<Vec<Fp>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(claim.g().coordinates()).ok_or(Error::Input)?;
    let mut words = Vec::with_capacity(20);
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    words.extend(claim.challenges());
    Ok(words)
}
fn context_digest(source: &Source, first: &AccumulatorT<Ep>) -> Result<Fp, Error> {
    let mut words = source.plan.context().schema().to_vec();
    words.extend(source.own.witness.statement);
    words.extend(source.own.send.statement);
    let before = &source.own.witness.before;
    let after = &source.own.witness.after;
    words.extend(before.core);
    words.extend(before.rest);
    words.extend(before.lineage);
    push_pallas(&mut words, &source.predecessor.pallas.as_input())?;
    words.extend(vesta_words(&source.predecessor.vesta.as_input())?);
    words.extend(after.core);
    words.extend(after.rest);
    words.extend(after.lineage);
    words.extend(source.incoming_head.state.lineage);
    words.push(Fp::from(u64::from(source.public_valid)));
    push_pallas(&mut words, &source.incoming_head.pallas.as_input())?;
    words.extend(vesta_words(&source.incoming_head.vesta.as_input())?);
    for q in &source.q {
        let types = q
            .plan
            .verifier()
            .binding()
            .descriptor()
            .instance_types
            .as_ref()
            .ok_or(Error::Artifact)?;
        if types.len() != q.instances.len() {
            return Err(Error::Input);
        }
        for (column, ty) in q.instances.iter().zip(types) {
            for value in column {
                if matches!(*ty, InstanceType::Bounded | InstanceType::Bits(0..=253)) {
                    words.push(
                        Option::<Fp>::from(Fp::from_repr(value.to_repr())).ok_or(Error::Input)?,
                    );
                } else {
                    words.extend(foreign_limbs(value).map(Fp::from_u128));
                }
            }
        }
    }
    words.extend(source.commitments.iter().flatten().copied());
    words.extend(source.results.map(|v| Fp::from(u64::from(v))));
    push_pallas(&mut words, &source.incoming_head.opening)?;
    for mode in source.modes {
        words.extend(mode_words(mode));
    }
    for point in source.pallas_corrections {
        let (x, y) = Option::<(Fp, Fp)>::from(point.coordinates()).ok_or(Error::Input)?;
        words.extend([x, y]);
    }
    let (x, y) =
        Option::<(Fq, Fq)>::from(source.vesta_correction.coordinates()).ok_or(Error::Input)?;
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    push_pallas(&mut words, &first.as_input())?;
    Ok(hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words))
}
fn continued_digest(
    plan: &ContextPlan,
    stage: usize,
    previous: Fp,
    old: &AccumulatorT<Ep>,
    vesta: &AccumulatorT<Eq>,
    current: &AccumulatorT<Ep>,
) -> Result<Fp, Error> {
    let mut words = vec![
        Fp::ONE,
        plan.schema()[1],
        Fp::from(u64::try_from(stage + 1).map_err(|_| Error::Input)?),
        previous,
    ];
    push_pallas(&mut words, &old.as_input())?;
    words.extend(vesta_words(&vesta.as_input())?);
    push_pallas(&mut words, &current.as_input())?;
    Ok(hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words))
}
fn internal_public(source: &Source, digest: Fp, part: &FoldInput<Eq>) -> Result<Vec<Fp>, Error> {
    let mut words = vec![digest, Fp::from(u64::from(part.source_k()))];
    words.extend(vesta_words(part)?);
    let trivial =
        AccumulatorT::trivial(&source.vparams, MemoryBudget::DEFAULT).map_err(|_| Error::Proof)?;
    let trivial = vesta_words(&trivial.as_input())?;
    words.extend(&trivial);
    words.extend(&trivial);
    words.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    words.extend(&trivial[..4]);
    if words.len() != 69 {
        return Err(Error::Input);
    }
    Ok(words)
}
fn stage_public(c: &Stage) -> Result<Vec<Fp>, Error> {
    let root = context_digest(&c.source, &c.first)?;
    let Some(continuation) = &c.continuation else {
        return internal_public(&c.source, root, &c.source.own.part);
    };
    if continuation.history.len() + 1 != continuation.plan.stage() {
        return Err(Error::Input);
    }
    if !continuation.plan.is_terminal() {
        let mut digest = root;
        for (i, (p, v)) in continuation.history.iter().enumerate() {
            let next = continuation
                .history
                .get(i + 1)
                .map_or(&continuation.carried, |(p, _)| p);
            digest = continued_digest(c.source.plan.context(), i + 1, digest, p, v, next)?;
        }
        digest = continued_digest(
            c.source.plan.context(),
            continuation.plan.stage(),
            digest,
            &continuation.carried,
            &continuation.vesta,
            &c.pallas,
        )?;
        return internal_public(&c.source, digest, &continuation.vesta.as_input());
    }
    let mut words = vec![
        terminal_digest(&c.source.own.witness.after.lineage, &c.pallas.as_input())?,
        Fp::from(16),
    ];
    words.extend(vesta_words(&continuation.vesta.as_input())?);
    words.extend(vesta_words(&c.source.predecessor.vesta.as_input())?);
    words.extend(vesta_words(&c.source.incoming_head.vesta.as_input())?);
    words.extend(c.source.mode_words(2));
    let (x, y) =
        Option::<(Fq, Fq)>::from(c.source.vesta_correction.coordinates()).ok_or(Error::Input)?;
    for v in [x, y] {
        words.extend(foreign_limbs(&v).map(Fp::from_u128));
    }
    if words.len() != 69 {
        return Err(Error::Input);
    }
    Ok(words)
}
fn terminal_digest(public: &[Fp; 18], pallas: &FoldInput<Ep>) -> Result<Fp, Error> {
    let mut words = public.to_vec();
    let mut claim = vec![];
    push_pallas(&mut claim, pallas)?;
    words.extend(&claim[1..]);
    Ok(hash_with_domain(crate::a_relation::LINEAGE_DOMAIN, &words))
}
fn omega_instances(digest: Fp, vesta: &AccumulatorT<Eq>) -> Result<Vec<Vec<Fq>>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(vesta.g().coordinates()).ok_or(Error::Input)?;
    Ok(vec![
        vec![Option::<Fq>::from(Fq::from_repr(digest.to_repr())).ok_or(Error::Input)?],
        vec![x, y],
        vesta
            .challenges()
            .iter()
            .map(|v| Option::<Fq>::from(Fq::from_repr(v.to_repr())).ok_or(Error::Input))
            .collect::<Result<Vec<_>, _>>()?,
    ])
}
fn opening_pallas(
    params: &PinnedParams<Ep>,
    binding: &DescriptorBinding,
    key: &VerifyingKey<Ep>,
    instances: &[Vec<Fq>],
    proof: &[u8],
    budget: MemoryBudget,
) -> Result<FoldInput<Ep>, Error> {
    let claim = accumulate_generator(params, binding, key, instances, proof, budget)
        .map_err(|_| Error::Proof)?;
    let input =
        FoldInput::from_opening(*claim.g(), claim.challenges()).map_err(|_| Error::Proof)?;
    input.decide(params, budget).map_err(|_| Error::Proof)?;
    Ok(input)
}
fn opening_vesta(
    params: &PinnedParams<Eq>,
    binding: &DescriptorBinding,
    key: &VerifyingKey<Eq>,
    instances: &[Vec<Fp>],
    proof: &[u8],
    budget: MemoryBudget,
) -> Result<FoldInput<Eq>, Error> {
    let claim = accumulate_generator(params, binding, key, instances, proof, budget)
        .map_err(|_| Error::Proof)?;
    let input =
        FoldInput::from_opening(*claim.g(), claim.challenges()).map_err(|_| Error::Proof)?;
    input.decide(params, budget).map_err(|_| Error::Proof)?;
    Ok(input)
}
