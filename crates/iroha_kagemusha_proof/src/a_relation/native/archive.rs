//! Genuine original-key native ArchiveSent composition for both canonical evidence forms.
//!
//! Own predecessor and own sigma are hard. Receive evidence consumes the original
//! Receive sigma and receipt without an Omega; Status evidence consumes its full
//! original folded-head Omega, statement, receipt and depth32 membership opening.
//! The final A relation derives every soft predicate and all mode selections.
//! Invalid evidence retains the canonical adjusted-map NoOp; software core removal
//! still has genuine depth32 paths. No cleanup authority or monetary refund is exposed.
//! Installation synthesizes the actual fixed source with unknown witnesses, imports
//! every original PIPAPK01 against that source and requires exact installed VK equality.
//! No prepared operation, accepted claim or checkpoint is constructed during import.
//! TODO: qualify this fixed schedule with real compact installed artifacts before intake.

#[path = "archive/checkpoint.rs"]
mod checkpoint;
pub use checkpoint::{CheckpointKind, CheckpointLayout};

use core::fmt;
use std::sync::Arc;

use ff::{Field, PrimeField};
use group::Curve;
use iroha_pasta::{
    Ep, Eq, EqAffine, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain,
};
use iroha_plonk::{
    DescriptorBinding, ProverConfig, ProverRandomness, ProvingKey, VerifyingKey, Witness,
    create_proof_owned_with_claim,
    cs::{
        Column, ConstraintSystem, CurveV1, Instance, InstanceModeV1, InstanceType, ProofSuffixV1,
        TranscriptV2,
    },
    frontend::{Circuit, Error as LayoutError, Layouter, Region, SimpleFloorPlanner, Value},
    keys::pk::artifact::ReadConfig,
    pcs::ipa::PinnedParams,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::{
    UintChip, Word,
    bytes::{
        element::le_message_segments,
        le_value,
        tape::{BytesChip, BytesConfig, SegmentSpec},
        variable::ActiveBytes,
    },
    imt::{LeafCells, OpeningCells, PathCells},
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, K,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_fold,
    obligation::{ModeCells, ledger::Variant},
    verifier::{VerifierChip, VerifierConfig, VerifierKeyCells},
};

use super::super::{
    AProofPlan, IncomingVestaCells, LineagePublicCells, ProofMessageCells, SigmaBindingCells,
    SignatureQCells, VestaClaimCells,
    context::{
        ContextIncomingProof, ContextInputs, ContextObjectCells, ContextObjectSpec, ContextPlan,
        ContextPredecessor, ContextState,
    },
    incoming_transport::{IncomingTransportCells, IncomingTransportPlan},
    own::{CurrentAuthorization, OwnPolicy, authenticate_current},
    schedule::OperationTask,
    split::{SplitPlan, WCircuit, WKey, close_first},
    verify_predecessor, verify_sigma,
};
use crate::{
    admin_sigma::StateWitness,
    omega::OmegaWitness,
    operation_relation::{
        administrative,
        incoming_statement::{DynamicStatementCells, IncomingStatementCells},
        map_effects::{ArchiveMapWitness, MapEffectsChip, MapState, MapTransition, RemoveCells},
        objects::{
            ObjectKind, SignedObjectCells,
            credential::CredentialCells,
            credit_opening::CreditOpeningCells,
            payment::{PaymentCells, PaymentInputs},
            receipt::{self, ReceiptContext},
            request::RequestCells,
            status::{
                CreditedCells, EvidenceKind, ReceiveEvidenceCells, ReceiveEvidenceInputs,
                StatusCells, StatusInputs,
            },
        },
        state::StateCells,
        statement::StatementCells,
    },
    q_signature::{QSignaturePlan, SignatureKey, SignatureSlot},
    tree::IndexedLeaf,
};
use iroha_plonk_gadgets::{Bit, GlueChip, p256::VerifyMode};

#[cfg(test)]
#[path = "archive/tests.rs"]
mod tests;

/// Fixed native source profile; private inputs cannot choose another range-bus count.
pub const SOURCE_RANGE_BUSES: usize = 4;
/// Frozen initial four-A/three-W schedule; actual synthesis/proof qualification remains required.
pub const A_STAGE_COUNT: usize = 4;
/// Exact internal wrapper count; terminal A cannot become another W.
pub const W_STAGE_COUNT: usize = A_STAGE_COUNT - 1;
const DESCRIPTOR_MAX_BYTES: usize = 1 << 20;
const VERIFYING_KEY_MAX_BYTES: usize = 1 << 18;
/// Joint raw Ω(public320 included) plus σ capacity from the wire10,000 cap.
pub const MAX_PAYMENT_ORIGINAL_BYTES: usize = 10_000 - 1_723 + 320;

/// Native preparation/proof failure. No failure changes a monetary head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Installed descriptor/key/schema or fixed policy differs.
    Artifact,
    /// Original shape, exact tape, history or source checkpoint differs.
    Input,
    /// Native proof, accumulator or complete decide failed.
    Proof,
    /// Installed circuit assignment or proof production failed.
    Prover,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native ArchiveSent: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Exact original Q proof and columns, retained by the source native producer.
#[derive(Clone, Debug)]
pub struct QInput {
    /// Descriptor-sized proof bytes.
    pub proof: Vec<u8>,
    /// Exact descriptor-shaped public columns; never a native acceptance verdict.
    pub instances: Vec<Vec<Fq>>,
}

/// Actual predecessor Omega proof and both canonical transported accumulator originals.
#[derive(Clone, Debug)]
pub struct PredecessorInput {
    /// Unframed, descriptor-sized predecessor Omega proof.
    pub proof: Vec<u8>,
    /// Exact544 canonical Pallas claim bytes.
    pub pallas: [u8; 544],
    /// Exact544 canonical Vesta claim bytes.
    pub vesta: [u8; 544],
}

/// Original own committed state openings and ArchiveSent statement.
#[derive(Clone, Copy, Debug)]
pub struct ArchiveState {
    /// Actual selected predecessor state opening and public lineage fields.
    pub before: StateWitness,
    /// Actual Archive successor state opening and public lineage fields.
    pub after: StateWitness,
    /// Exact canonical Archive statement field order.
    pub statement: [Fp; 26],
}
/// All original proof and statement tapes of the retained immutable Send Payment.
#[derive(Clone, Debug)]
pub struct RetainedPayment {
    /// Exact retained Send statement, including its seven pending descriptor fields.
    pub statement: [Fp; 26],
    /// Original retained payer Omega transport.
    pub omega: Vec<u8>,
    /// Original retained Send step proof.
    pub sigma: Vec<u8>,
}
/// Selected evidence's exact original incoming tapes. The fixed Plan chooses its form.
#[derive(Clone, Debug)]
pub struct IncomingInput {
    /// Original Receive statement or deterministic Status-form compact statement.
    pub statement: [Fp; 26],
    /// Status form only: public320 || original Omega proof || P544 || V544.
    pub omega: Vec<u8>,
    /// Receive form only: the original Receive sigma; no Omega is admitted.
    pub sigma: Vec<u8>,
    /// Status form only: all162 canonical CreditStatus transcript bytes.
    pub status: Vec<u8>,
    /// Status form only: all1125 canonical credit-opening transcript bytes.
    pub opening: Vec<u8>,
}
/// One original authenticated depth32 tree route, never a claimed membership bit.
#[derive(Clone, Copy, Debug)]
pub struct Search {
    /// Exact original authenticated indexed leaf.
    pub leaf: IndexedLeaf<Fp>,
    /// Actual depth32 leaf position.
    pub slot: u32,
    /// Original bottom-up depth32 Merkle route.
    pub siblings: [Fp; 32],
}
/// Relink predecessor followed by clearing the removed leaf.
#[derive(Clone, Copy, Debug)]
pub struct Removal {
    /// Original predecessor leaf and route for the relink step.
    pub predecessor: Search,
    /// Removed leaf and its route after that relink.
    pub removed: Search,
}
/// Committed core removal and distinct adjusted-lineage removal paths.
#[derive(Clone, Copy, Debug)]
pub struct Effects {
    /// Exact seven retained Send effect fields17..24, not a caller digest.
    pub descriptor: [Fp; 7],
    /// Removal paths authenticated under the committed core pending root.
    pub core: Removal,
    /// Distinct removal paths under the folded adjusted-lineage pending root.
    pub lineage: Removal,
}
/// Native G1's authenticated retained originals and exact released Archive state.
/// No evidence, signature or proof acceptance boolean can be supplied.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Actual own predecessor/successor openings and exact Archive statement.
    pub state: ArchiveState,
    /// Original released Archive step proof.
    pub sigma: Vec<u8>,
    /// Current credential, Enrollment, own receipt; held Request, retained payer
    /// credential, retained Send receipt, Payment163, quoted receiver credential,
    /// incoming receipt and Credited99, in this exact order.
    pub objects: [Vec<u8>; 10],
    /// Hard original preimages of the immutable own Send Payment.
    pub retained: RetainedPayment,
    /// Original selected Receive or Status evidence tapes, constrained by the fixed Plan.
    pub incoming: IncomingInput,
    /// Exact descriptor and both independently authenticated pending-map removals.
    pub effects: Effects,
    /// Original Q0 own/(Receive incoming) sigma, Q1 hard own2V1F, Q2 soft receipt1V.
    pub q: [QInput; 3],
    /// Actual own predecessor Omega proof and canonical transported claims.
    pub predecessor: PredecessorInput,
}

/// Fixed four-A/three-W profile, original-key catalog and complete source schema.
#[derive(Clone, Debug)]
pub struct Plan {
    context: ContextPlan,
    recorded_blacklist: bool,
    policy: OwnPolicy,
    signatures: [QSignaturePlan; 2],
    predecessor_key: VerifyingKey<Ep>,
    incoming: Option<IncomingTransportPlan>,
    omega_capacity: usize,
    sigma_capacity: usize,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin the actual canonical evidence ledger, Q schemas and all original tapes.
    /// The two capacities are authenticated artifact metadata, never witness choices.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        operation: AProofPlan,
        recorded_blacklist: bool,
        policy: OwnPolicy,
        signatures: [QSignaturePlan; 2],
        predecessor_key: VerifyingKey<Ep>,
        omega_capacity: usize,
        sigma_capacity: usize,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
    ) -> Result<Self, Error> {
        let receive = operation.frame().variant() == Variant::ArchiveReceive;
        if !matches!(
            operation.frame().variant(),
            Variant::ArchiveReceive | Variant::ArchiveStatus
        ) || operation.q_count() != 3
            || !operation.frame().has_predecessor()
            || operation.frame().has_incoming() == receive
            || operation.sigma.slot_count() != if receive { 2 } else { 1 }
            || operation.frame().part_source_k() != if receive { 16 } else { 12 }
            || (!receive && recorded_blacklist)
        {
            return Err(Error::Artifact);
        }
        let own = operation.sigma.class(0).ok_or(Error::Artifact)?;
        if own.verifier().binding().descriptor().k != 12 || own.selector_key_digest(12).is_none() {
            return Err(Error::Artifact);
        }
        if receive
            && operation
                .sigma
                .class(1)
                .ok_or(Error::Artifact)?
                .selector_key_digest(10 + u8::from(recorded_blacklist))
                .is_none()
        {
            return Err(Error::Artifact);
        }
        pallas.require_k(16).map_err(|_| Error::Artifact)?;
        vesta.require_k(16).map_err(|_| Error::Artifact)?;
        let fixed = operation.omega().ok_or(Error::Artifact)?;
        let d = fixed.binding().descriptor();
        if predecessor_key.descriptor_digest() != fixed.binding().digest()
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
            .kagemusha_digest(fixed.binding())
            .map_err(|_| Error::Artifact)?;
        let incoming = if receive {
            None
        } else {
            Some(IncomingTransportPlan::new(&operation).map_err(|_| Error::Artifact)?)
        };
        if omega_capacity < 320 + fixed.proof_length() + 1088
            || omega_capacity > 10_000
            || sigma_capacity == 0
            || sigma_capacity > 10_000
            || (receive
                && sigma_capacity
                    < operation
                        .sigma
                        .class(1)
                        .ok_or(Error::Artifact)?
                        .verifier()
                        .proof_length())
        {
            return Err(Error::Artifact);
        }
        let expected = signature_schemas(policy)?;
        if signatures
            .iter()
            .zip(&expected)
            .any(|(a, b)| a.slots() != b.slots())
        {
            return Err(Error::Artifact);
        }
        for index in 1..3 {
            let d = operation
                .q(index)
                .ok_or(Error::Artifact)?
                .verifier()
                .binding()
                .descriptor();
            if d.instance_lengths != [signatures[index - 1].instance_length() as u32]
                || d.instance_types.as_deref() != Some(&QSignaturePlan::instance_types())
            {
                return Err(Error::Artifact);
            }
        }
        let variant = operation.frame().variant();
        let (parts, tasks) = operation_schedule();
        let context = ContextPlan::with_schedule(
            operation,
            parts,
            Some(0),
            context_specs(variant, omega_capacity, sigma_capacity)?,
        )
        .and_then(|p| p.with_operation_tasks(tasks))
        .map_err(|_| Error::Artifact)?;
        Ok(Self {
            context,
            recorded_blacklist,
            policy,
            signatures,
            predecessor_key,
            incoming,
            omega_capacity,
            sigma_capacity,
            pallas,
            vesta,
        })
    }
    /// Immutable complete operation context and ordered proof schedule.
    #[must_use]
    pub const fn context(&self) -> &ContextPlan {
        &self.context
    }
    /// Exact hard-own and soft-receiver signature schemas under the fixed policy.
    pub fn signature_schemas(policy: OwnPolicy) -> Result<[QSignaturePlan; 2], Error> {
        signature_schemas(policy)
    }
    /// Fully verify all hard original sources, then derive witness data by the same
    /// production evidence gadget and actual independent original P/opening/V decides.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Prepared, Error> {
        check_original_shapes(self, &input)?;
        check_sigma_tape(self, &input)?;
        let pallas =
            AccumulatorT::<Ep>::from_bytes(&input.predecessor.pallas).map_err(|_| Error::Input)?;
        let vesta =
            AccumulatorT::<Eq>::from_bytes(&input.predecessor.vesta).map_err(|_| Error::Input)?;
        pallas
            .decide(&self.pallas, budget)
            .map_err(|_| Error::Proof)?;
        vesta
            .decide(&self.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let fixed = self.context.operation().omega().ok_or(Error::Artifact)?;
        let digest = self
            .predecessor_key
            .kagemusha_digest(fixed.binding())
            .map_err(|_| Error::Artifact)?;
        if input.state.before.lineage[17] != digest || input.state.after.lineage[17] != digest {
            return Err(Error::Input);
        }
        let public = omega_instances(
            terminal_digest(&input.state.before.lineage, &pallas.as_input())?,
            &vesta,
        )?;
        verify_full(
            &self.pallas,
            fixed.binding(),
            &self.predecessor_key,
            &public,
            &input.predecessor.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_pallas(
            &self.pallas,
            fixed.binding(),
            &self.predecessor_key,
            &public,
            &input.predecessor.proof,
            budget,
        )?;
        let mut q_openings = Vec::with_capacity(3);
        for (index, q) in input.q.iter().enumerate() {
            let fixed = self.context.operation().q(index).ok_or(Error::Artifact)?;
            verify_full(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &q.instances,
                &q.proof,
                budget,
            )
            .map_err(|_| Error::Proof)?;
            q_openings.push(opening_pallas(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &q.instances,
                &q.proof,
                budget,
            )?);
        }
        let part = q_sigma_part(
            &input.q[0],
            self.context.operation().frame().part_source_k(),
        )?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        let tp = AccumulatorT::<Ep>::trivial(&self.pallas, budget)
            .map_err(|_| Error::Proof)?
            .as_input();
        let tv = AccumulatorT::<Eq>::trivial(&self.vesta, budget)
            .map_err(|_| Error::Proof)?
            .as_input();
        let predproof = input.predecessor.proof.clone();
        let source = Sources {
            original: input,
            q_openings,
            part,
            predecessor: Predecessor {
                proof: predproof,
                pallas,
                opening,
                vesta,
            },
            evidence_valid: false,
            incoming: NativeIncoming {
                public: [Fp::ZERO; 18],
                pallas: tp.clone(),
                opening: tp.clone(),
                vesta: tv.clone(),
                selected: [tp.clone(), tp.clone()],
                selected_vesta: tv.clone(),
                modes: [[false, true, false]; 4],
                pallas_corrections: [Ep::from(*tp.g()); 2],
                vesta_correction: Eq::from(*tv.g()),
            },
        };
        let mut prepared = Prepared {
            plan: self.clone(),
            source: Arc::new(source),
        };
        let values = evaluate_evidence(&prepared)?;
        let valid = field_bit(values[0])?;
        let mut incoming = if self.incoming.is_some() {
            incoming_from_fields(&values[1..])?
        } else {
            prepared.source.incoming.clone()
        };
        select_native_incoming(
            self,
            &prepared.source.original,
            valid,
            &mut incoming,
            budget,
        )?;
        let source = Arc::get_mut(&mut prepared.source).ok_or(Error::Input)?;
        source.evidence_valid = valid;
        source.incoming = incoming;
        prepared.verify_sources(budget)?;
        Ok(prepared)
    }
}

#[derive(Clone)]
struct Predecessor {
    proof: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    opening: FoldInput<Ep>,
    vesta: AccumulatorT<Eq>,
}
#[derive(Clone)]
struct Sources {
    original: Inputs,
    q_openings: Vec<FoldInput<Ep>>,
    part: FoldInput<Eq>,
    predecessor: Predecessor,
    evidence_valid: bool,
    incoming: NativeIncoming,
}
#[derive(Clone)]
struct NativeIncoming {
    public: [Fp; 18],
    pallas: FoldInput<Ep>,
    opening: FoldInput<Ep>,
    vesta: FoldInput<Eq>,
    selected: [FoldInput<Ep>; 2],
    selected_vesta: FoldInput<Eq>,
    modes: [[bool; 3]; 4],
    pallas_corrections: [Ep; 2],
    vesta_correction: Eq,
}

#[derive(Clone)]
struct First {
    source: Arc<Sources>,
    plan: Plan,
    pallas: AccumulatorT<Ep>,
    fold: Vec<u8>,
    known: bool,
}
// Circuit-only source views preserve unknown optional claims and create no Prepared.
#[derive(Clone)]
struct CircuitPredecessor {
    proof: Vec<u8>,
    pallas: Option<FoldInput<Ep>>,
    vesta: Option<AccumulatorT<Eq>>,
}
#[derive(Clone)]
struct CircuitIncoming {
    modes: [[bool; 3]; 4],
    pallas_corrections: [Option<Ep>; 2],
    vesta_correction: Option<Eq>,
}
#[derive(Clone)]
struct CircuitSources {
    original: Inputs,
    predecessor: CircuitPredecessor,
    incoming: CircuitIncoming,
}
#[derive(Clone)]
struct FirstCircuit {
    source: Arc<CircuitSources>,
    plan: Plan,
    pallas: Option<FoldInput<Ep>>,
    fold: Vec<u8>,
    known: bool,
}
impl First {
    fn circuit(&self) -> FirstCircuit {
        let source = &self.source;
        FirstCircuit {
            source: Arc::new(CircuitSources {
                original: source.original.clone(),
                predecessor: CircuitPredecessor {
                    proof: source.predecessor.proof.clone(),
                    pallas: Some(source.predecessor.pallas.as_input()),
                    vesta: Some(source.predecessor.vesta.clone()),
                },
                incoming: CircuitIncoming {
                    modes: source.incoming.modes,
                    pallas_corrections: source.incoming.pallas_corrections.map(Some),
                    vesta_correction: Some(source.incoming.vesta_correction),
                },
            }),
            plan: self.plan.clone(),
            pallas: Some(self.pallas.as_input()),
            fold: self.fold.clone(),
            known: self.known,
        }
    }
}
impl FirstCircuit {
    fn blank(plan: &Plan) -> Result<Self, Error> {
        let operation = plan.context.operation();
        let own = operation.sigma.class(0).ok_or(Error::Artifact)?;
        let predecessor = operation.omega().ok_or(Error::Artifact)?;
        let state = StateWitness {
            core: [Fp::ZERO; 33],
            rest: [Fp::ZERO; 8],
            lineage: [Fp::ZERO; 18],
        };
        let q = (0..3)
            .map(|i| {
                let q = operation.q(i).ok_or(Error::Artifact)?;
                Ok(QInput {
                    proof: vec![0; q.verifier().proof_length()],
                    instances: q
                        .verifier()
                        .binding()
                        .descriptor()
                        .instance_lengths
                        .iter()
                        .map(|length| vec![Fq::ZERO; *length as usize])
                        .collect(),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?
            .try_into()
            .map_err(|_| Error::Artifact)?;
        let search = Search {
            leaf: IndexedLeaf::default(),
            slot: 0,
            siblings: [Fp::ZERO; 32],
        };
        let removal = Removal {
            predecessor: search,
            removed: search,
        };
        let receive = plan.incoming.is_none();
        let omega_length = predecessor
            .proof_length()
            .checked_add(320 + 2 * 544)
            .ok_or(Error::Artifact)?;
        let incoming = if receive {
            IncomingInput {
                statement: [Fp::ZERO; 26],
                omega: vec![],
                sigma: vec![
                    0;
                    operation
                        .sigma
                        .class(1)
                        .ok_or(Error::Artifact)?
                        .verifier()
                        .proof_length()
                ],
                status: vec![],
                opening: vec![],
            }
        } else {
            IncomingInput {
                statement: [Fp::ZERO; 26],
                omega: vec![
                    0;
                    plan.incoming
                        .as_ref()
                        .ok_or(Error::Artifact)?
                        .payload_length()
                        .map_err(|_| Error::Artifact)?
                ],
                sigma: vec![],
                status: vec![0; 162],
                opening: vec![0; 1125],
            }
        };
        let original = Inputs {
            state: ArchiveState {
                before: state,
                after: state,
                statement: [Fp::ZERO; 26],
            },
            sigma: vec![0; own.verifier().proof_length()],
            objects: core::array::from_fn(|i| vec![0; operation_object_length(i)]),
            retained: RetainedPayment {
                statement: [Fp::ZERO; 26],
                omega: vec![0; omega_length],
                // Retained Send proof length is private active data; capacity is fixed metadata.
                // Its complete unknown active tape is synthesized without an acceptance object.
                sigma: vec![],
            },
            incoming,
            effects: Effects {
                descriptor: [Fp::ZERO; 7],
                core: removal,
                lineage: removal,
            },
            q,
            predecessor: PredecessorInput {
                proof: vec![0; predecessor.proof_length()],
                pallas: [0; 544],
                vesta: [0; 544],
            },
        };
        Ok(Self {
            source: Arc::new(CircuitSources {
                original,
                predecessor: CircuitPredecessor {
                    proof: vec![0; predecessor.proof_length()],
                    pallas: None,
                    vesta: None,
                },
                incoming: CircuitIncoming {
                    modes: [[false; 3]; 4],
                    pallas_corrections: [None; 2],
                    vesta_correction: None,
                },
            }),
            plan: plan.clone(),
            pallas: None,
            fold: vec![0; 1120],
            known: false,
        })
    }
}
fn operation_object_length(index: usize) -> usize {
    object_kind(index).map_or(if index == 6 { 163 } else { 99 }, |k| k.body_len() + 64)
}

#[derive(Clone, Debug)]
/// Fixed source-stage columns and range buses.
pub struct StageConfig {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
impl FirstCircuit {
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn scalar(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        v: Fq,
    ) -> Result<ScalarCells<Ep>, LayoutError> {
        self.scalar_value(chip, region, self.value(v))
    }
    fn scalar_value(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        value: Value<Fq>,
    ) -> Result<ScalarCells<Ep>, LayoutError> {
        let lo = chip
            .uint()
            .assign::<128>(region, value.map(|v| foreign_limbs(&v)[0]))?;
        let hi = chip
            .uint()
            .assign::<127>(region, value.map(|v| foreign_limbs(&v)[1]))?;
        ScalarCells::from_limbs(&mut chip.uint(), region, &lo, &hi)
    }
    fn pallas(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: Option<&FoldInput<Ep>>,
    ) -> Result<FoldInputCells<Ep>, LayoutError> {
        let g = chip.witness_point(
            region,
            input.map_or(Value::unknown(), |v| self.value(Ep::from(*v.g()))),
        )?;
        let challenges = (0..K)
            .map(|i| {
                self.scalar_value(
                    chip,
                    region,
                    input.map_or(Value::unknown(), |v| self.value(v.challenges()[i])),
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        FoldInputCells::from_normalized(chip, region, 16, g, challenges)
    }
    fn vesta(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: Option<&AccumulatorT<Eq>>,
    ) -> Result<VestaClaimCells, LayoutError> {
        let coordinates = input
            .map(|v| Option::<(Fq, Fq)>::from(v.g().coordinates()).ok_or(LayoutError::Synthesis))
            .transpose()?;
        let coordinates = [
            self.scalar_value(
                chip,
                region,
                coordinates.map_or(Value::unknown(), |(x, _)| self.value(x)),
            )?,
            self.scalar_value(
                chip,
                region,
                coordinates.map_or(Value::unknown(), |(_, y)| self.value(y)),
            )?,
        ];
        let values = core::array::from_fn::<_, K, _>(|i| {
            input.map_or(Value::unknown(), |v| self.value(v.challenges()[i]))
        });
        let challenges = chip
            .uint()
            .glue()
            .witnesses(region, &values)?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        VestaClaimCells::constrain(chip, region, 16, coordinates, challenges)
    }
    // Omega's descriptor is fixed metadata; its VK is a witness. Pinning the VK as an A
    // constant would require an A<->Omega artifact commitment fixed point. The actual hard
    // predecessor verifier binds this witness key's computed digest to predecessor public17
    // and successor public17. Incoming transport uses that same carried root and full verify.
    fn omega_key(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<VerifierKeyCells<Ep>, LayoutError> {
        let key = &self.plan.predecessor_key;
        let iroha_plonk::transcript::TranscriptRepr::Base(repr) = *key.transcript_repr() else {
            return Err(LayoutError::Synthesis);
        };
        let fixed = key
            .fixed_commitments()
            .iter()
            .map(|point| self.value(Ep::from(*point)))
            .collect::<Vec<_>>();
        let permutation = key
            .permutation_commitments()
            .iter()
            .map(|point| self.value(Ep::from(*point)))
            .collect::<Vec<_>>();
        chip.witness_key(region, self.value(repr), &fixed, &permutation)
    }
    fn carrier(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        body: &[u8],
    ) -> Result<ProofMessageCells, LayoutError> {
        let length = u32::try_from(body.len()).map_err(|_| LayoutError::BoundsFailure)?;
        let source = length
            .to_le_bytes()
            .into_iter()
            .chain(body.iter().copied())
            .map(|v| self.value(v))
            .collect::<Vec<_>>();
        let mut segments = vec![SegmentSpec::little(0, 4)];
        segments.extend(le_message_segments(4, body.len() / 32));
        let run = bytes.run(
            region,
            &source,
            &source
                .chunks(31)
                .map(<[Value<u8>]>::len)
                .collect::<Vec<_>>(),
            &segments,
        )?;
        ProofMessageCells::from_run(chip, region, &run, 0, body.len())
    }
}

struct Cells {
    old: StateCells,
    new: StateCells,
    pred: LineagePublicCells,
    next: LineagePublicCells,
    statement: StatementCells,
    retained_statement: StatementCells,
    incoming_statement: Option<IncomingStatementCells>,
    dynamic_statement: Option<DynamicStatementCells>,
    transport: Option<IncomingTransportCells>,
    sigma: Vec<SigmaBindingCells>,
    objects: Vec<Option<SignedObjectCells>>,
    runs: Vec<iroha_plonk_gadgets::bytes::tape::ByteRun<Fp>>,
    context_objects: Vec<ContextObjectCells>,
    q: Vec<Vec<Vec<ScalarCells<Ep>>>>,
    pp: FoldInputCells<Ep>,
    pv: VestaClaimCells,
    modes: Vec<ModeCells<Fp>>,
    pc: Vec<iroha_plonk_gadgets::ecc::NonIdentityPoint<Fp>>,
    vc: Vec<[ScalarCells<Ep>; 2]>,
    retained_digest: Word<Fp>,
}
impl Cells {
    fn object(&self, i: usize) -> Result<&SignedObjectCells, LayoutError> {
        self.objects
            .get(i)
            .and_then(Option::as_ref)
            .ok_or(LayoutError::Synthesis)
    }
    fn context(&self) -> ContextInputs<'_> {
        ContextInputs {
            own_statement: &self.statement,
            incoming_statement: self.incoming_statement.as_ref(),
            predecessor: Some(ContextPredecessor {
                state: &self.old,
                public: &self.pred,
                pallas: &self.pp,
                vesta: &self.pv,
            }),
            successor: ContextState {
                state: &self.new,
                public: &self.next,
            },
            incoming: self.transport.as_ref().map(|t| {
                crate::a_relation::context::ContextIncoming {
                    public: t.public(),
                    pallas: t.pallas(),
                    vesta: t.vesta(),
                    proof: ContextIncomingProof::Messages(t.proof()),
                }
            }),
            q_instances: &self.q,
            objects: &self.context_objects,
            modes: &self.modes,
            pallas_corrections: &self.pc,
            vesta_corrections: &self.vc,
            receive_results: None,
        }
    }
}
impl FirstCircuit {
    fn state(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        w: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), LayoutError> {
        let core = chip
            .uint()
            .glue()
            .witnesses(region, &w.core.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let rest = chip
            .uint()
            .glue()
            .witnesses(region, &w.rest.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let state = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = chip
            .uint()
            .glue()
            .witnesses(region, &w.lineage.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        Ok((
            state,
            LineagePublicCells::constrain(&mut chip.uint(), region, &public)?,
        ))
    }
    fn search(
        &self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        w: &Search,
    ) -> Result<OpeningCells<Fp>, LayoutError> {
        let leaf = uint
            .glue()
            .witnesses(
                region,
                &[w.leaf.key, w.leaf.value, w.leaf.next_key].map(|v| self.value(v)),
            )?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let index = uint
            .glue()
            .witness(region, self.value(Fp::from(u64::from(w.slot))))?;
        let siblings = uint
            .glue()
            .witnesses(region, &w.siblings.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        Ok(OpeningCells {
            leaf: LeafCells::from_words(leaf),
            path: PathCells::from_words(uint, region, &index, siblings)?,
        })
    }
    fn removal(
        &self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        w: &Removal,
    ) -> Result<RemoveCells, LayoutError> {
        Ok(RemoveCells {
            predecessor: self.search(uint, region, &w.predecessor)?,
            removed: self.search(uint, region, &w.removed)?,
        })
    }
    fn active(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        raw: &[u8],
        capacity: usize,
        segments: &[SegmentSpec],
    ) -> Result<ActiveBytes<Fp>, LayoutError> {
        ActiveBytes::assign(
            &mut chip.uint(),
            bytes,
            region,
            capacity,
            &if self.known {
                Value::known(raw.to_vec())
            } else {
                Value::unknown()
            },
            segments,
        )
    }
    fn setup(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Cells, LayoutError> {
        let source = &self.source.original;
        let (old, pred) = self.state(chip, region, &source.state.before)?;
        let (new, next) = self.state(chip, region, &source.state.after)?;
        let words = chip
            .uint()
            .glue()
            .witnesses(region, &source.state.statement.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let statement = StatementCells::constrain_with_verifier(
            chip,
            region,
            self.plan.context.operation().frame().variant(),
            &words,
        )?;
        let words = chip
            .uint()
            .glue()
            .witnesses(region, &source.retained.statement.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let retained_statement =
            StatementCells::constrain_with_verifier(chip, region, Variant::Send, &words)?;
        let incoming_words = chip
            .uint()
            .glue()
            .witnesses(region, &source.incoming.statement.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let receive = self.plan.incoming.is_none();
        let (incoming_statement, dynamic_statement) = {
            let lanes = chip.operation_lanes()?;
            let mut uint = UintChip::new(lanes.glue, lanes.range);
            if receive {
                (
                    Some(IncomingStatementCells::constrain(
                        &mut uint,
                        lanes.hash,
                        region,
                        Variant::Receive,
                        &incoming_words,
                    )?),
                    None,
                )
            } else {
                (
                    None,
                    Some(DynamicStatementCells::constrain(
                        &mut uint,
                        lanes.hash,
                        region,
                        &incoming_words,
                    )?),
                )
            }
        };
        let own_raw = frame(&source.sigma)?;
        let run = bytes.run(
            region,
            &own_raw.iter().map(|b| self.value(*b)).collect::<Vec<_>>(),
            &iroha_plonk_gadgets::bytes::chunk_segments(0, own_raw.len()),
            &[SegmentSpec::little(0, 4)],
        )?;
        let own_index = chip.uint().glue().constant(region, Fp::from(12))?;
        let mut sigma = vec![SigmaBindingCells::from_run(
            chip, region, &statement, own_index, &run,
        )?];
        let mut context_objects = Vec::new();
        let mut objects = Vec::new();
        let mut runs = Vec::new();
        for (i, raw) in source.objects.iter().enumerate() {
            let spec = self.plan.context.object_specs()[i];
            let (primary, secondary) = if let Some(kind) = object_kind(i) {
                (kind.primary_segments(), kind.secondary_segments())
            } else if i == 6 {
                (
                    PaymentCells::primary_segments(),
                    PaymentCells::secondary_segments(),
                )
            } else {
                (
                    CreditedCells::primary_segments(),
                    CreditedCells::secondary_segments(),
                )
            };
            let run = bytes.run(
                region,
                &raw.iter().map(|b| self.value(*b)).collect::<Vec<_>>(),
                &primary,
                &secondary,
            )?;
            let object = if let Some(kind) = object_kind(i) {
                let lanes = chip.operation_lanes()?;
                let mut uint = UintChip::new(lanes.glue, lanes.range);
                Some(if i == 8 {
                    SignedObjectCells::decode_soft(&mut uint, lanes.hash, region, kind, &run)?.0
                } else {
                    SignedObjectCells::from_run(&mut uint, lanes.hash, region, kind, &run)?
                })
            } else {
                None
            };
            let digest = if let Some(object) = &object {
                object.digest().clone()
            } else {
                raw_digest(
                    chip,
                    region,
                    &run,
                    if i == 6 { *b"kgwpay_1" } else { *b"kgwcrdd1" },
                )?
            };
            context_objects.push(ContextObjectCells::from_exact_run(
                chip, region, spec, &digest, &run,
            )?);
            objects.push(object);
            runs.push(run);
        }
        // The retained proof digest is from the full immutable original Omega and sigma strings.
        // No caller digest or regenerated package can replace either tape.
        let omega = self.active(
            chip,
            bytes,
            region,
            &source.retained.omega,
            self.plan.omega_capacity,
            &[],
        )?;
        let retained_sigma = self.active(
            chip,
            bytes,
            region,
            &source.retained.sigma,
            self.plan.sigma_capacity,
            &[],
        )?;
        let expected_omega = 320
            + self
                .plan
                .context
                .operation()
                .omega()
                .ok_or(LayoutError::Synthesis)?
                .proof_length()
            + 1088;
        GlueChip::assert_constant(
            region,
            omega.length().word(),
            Fp::from(u64::try_from(expected_omega).map_err(|_| LayoutError::BoundsFailure)?),
        )?;
        let retained_digest = {
            let lanes = chip.operation_lanes()?;
            let mut uint = UintChip::new(lanes.glue, lanes.range);
            let a = omega.packed().length_prefixed(&mut uint, region)?;
            let b = retained_sigma.packed().length_prefixed(&mut uint, region)?;
            a.concat(&mut uint, region, &b)?.digest(
                &mut uint,
                lanes.hash.sponge_mut()?,
                region,
                u64::from_le_bytes(*b"kgwprf_1"),
            )?
        };
        for raw in [&omega, &retained_sigma] {
            let spec = self.plan.context.object_specs()[context_objects.len()];
            context_objects.push(ContextObjectCells::from_active(
                chip,
                region,
                spec,
                &retained_digest,
                raw,
            )?);
        }
        let transport = if let Some(plan) = &self.plan.incoming {
            let raw = self.active(
                chip,
                bytes,
                region,
                &source.incoming.omega,
                self.plan.omega_capacity,
                &plan.active_segments()?,
            )?;
            let t = plan.decode_active(chip, region, &raw, next.omega_key_digest())?;
            let digest = active_digest(chip, region, &raw, *b"kgwlin_1")?;
            context_objects.push(ContextObjectCells::from_active(
                chip,
                region,
                self.plan.context.object_specs()[context_objects.len()],
                &digest,
                &raw,
            )?);
            for (raw, primary, secondary, domain) in [
                (
                    &source.incoming.status,
                    StatusCells::primary_segments(),
                    StatusCells::secondary_segments(),
                    *b"kgwcsts1",
                ),
                (
                    &source.incoming.opening,
                    CreditOpeningCells::primary_segments(),
                    CreditOpeningCells::secondary_segments(),
                    *b"kgwcopn1",
                ),
            ] {
                let run = bytes.run(
                    region,
                    &raw.iter().map(|b| self.value(*b)).collect::<Vec<_>>(),
                    &primary,
                    &secondary,
                )?;
                let digest = raw_digest(chip, region, &run, domain)?;
                context_objects.push(ContextObjectCells::from_exact_run(
                    chip,
                    region,
                    self.plan.context.object_specs()[context_objects.len()],
                    &digest,
                    &run,
                )?);
                runs.push(run);
            }
            Some(t)
        } else {
            let proofbytes = self
                .plan
                .context
                .operation()
                .sigma
                .class(1)
                .ok_or(LayoutError::Synthesis)?
                .verifier()
                .proof_length();
            let raw = self.active(
                chip,
                bytes,
                region,
                &source.incoming.sigma,
                self.plan.sigma_capacity,
                &SigmaBindingCells::incoming_segments(proofbytes)?,
            )?;
            let index = chip.uint().glue().constant(
                region,
                Fp::from(10 + u64::from(self.plan.recorded_blacklist)),
            )?;
            let incoming = SigmaBindingCells::from_incoming_active(
                chip,
                region,
                incoming_statement.as_ref().ok_or(LayoutError::Synthesis)?,
                index,
                &raw,
                proofbytes,
            )?;
            let digest = incoming.step_digest()?.clone();
            context_objects.push(ContextObjectCells::from_active(
                chip,
                region,
                self.plan.context.object_specs()[context_objects.len()],
                &digest,
                &raw,
            )?);
            sigma.push(incoming);
            None
        };
        let q = source
            .q
            .iter()
            .map(|q| {
                q.instances
                    .iter()
                    .map(|c| c.iter().map(|v| self.scalar(chip, region, *v)).collect())
                    .collect()
            })
            .collect::<Result<Vec<Vec<Vec<_>>>, LayoutError>>()?;
        let pp = self.pallas(chip, region, self.source.predecessor.pallas.as_ref())?;
        let pv = self.vesta(chip, region, self.source.predecessor.vesta.as_ref())?;
        let mode_source = if receive {
            &self.source.incoming.modes[3..]
        } else {
            &self.source.incoming.modes[..3]
        };
        let modes = mode_source
            .iter()
            .map(|bits| {
                let words = chip
                    .uint()
                    .glue()
                    .witnesses(region, &bits.map(|v| self.value(Fp::from(u64::from(v)))))?;
                ModeCells::constrain(
                    chip.uint().glue(),
                    region,
                    &words.try_into().map_err(|_| LayoutError::Synthesis)?,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let pc = if receive {
            vec![]
        } else {
            self.source
                .incoming
                .pallas_corrections
                .iter()
                .map(|g| chip.witness_point(region, g.map_or(Value::unknown(), |g| self.value(g))))
                .collect::<Result<Vec<_>, _>>()?
        };
        let vc = if receive {
            vec![]
        } else {
            let coordinates = self
                .source
                .incoming
                .vesta_correction
                .map(|g| {
                    Option::<(Fq, Fq)>::from(g.to_affine().coordinates())
                        .ok_or(LayoutError::Synthesis)
                })
                .transpose()?;
            vec![[
                self.scalar_value(
                    chip,
                    region,
                    coordinates.map_or(Value::unknown(), |(x, _)| self.value(x)),
                )?,
                self.scalar_value(
                    chip,
                    region,
                    coordinates.map_or(Value::unknown(), |(_, y)| self.value(y)),
                )?,
            ]]
        };
        Ok(Cells {
            old,
            new,
            pred,
            next,
            statement,
            retained_statement,
            incoming_statement,
            dynamic_statement,
            transport,
            sigma,
            objects,
            runs,
            context_objects,
            q,
            pp,
            pv,
            modes,
            pc,
            vc,
            retained_digest,
        })
    }
    fn own_proof(&self, region: &mut Region<'_, Fp>, c: &Cells) -> Result<(), LayoutError> {
        GlueChip::assert_equal(region, c.object(2)?.word(9)?, c.sigma[0].step_digest()?)
    }
    fn own_auth(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        c: &Cells,
        bundle: &SignatureQCells,
    ) -> Result<(), LayoutError> {
        bundle.bind_context(region, self.plan.context.operation(), 1, &c.q[1])?;
        let slots = bundle.slots();
        if slots.len() != 3 {
            return Err(LayoutError::Synthesis);
        }
        authenticate_current(
            chip,
            region,
            self.plan.policy,
            CurrentAuthorization {
                credential: c.object(0)?,
                certificate: c.object(1)?,
                credential_proof: &slots[1],
                certificate_proof: &slots[2],
                current: MapState {
                    state: &c.old,
                    lineage: &c.pred,
                },
                statement: &c.statement,
            },
        )?;
        let key = core::array::from_fn(|i| c.pred.fields()[9 + i].clone());
        let signed = c.object(2)?.bind_signature(region, &slots[0], &key)?;
        GlueChip::assert_constant(region, signed.word(), Fp::ONE)?;
        let scope = self.plan.policy.scope(chip, region)?;
        let wallet = core::array::from_fn(|i| c.pred.fields()[6 + i].clone());
        let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
        let lanes = chip.operation_lanes()?;
        let valid = receipt::bind(
            &mut UintChip::new(lanes.glue, lanes.range),
            lanes.hash,
            region,
            c.object(2)?,
            &ReceiptContext {
                wallet: &wallet,
                provider: &scope.provider,
                statement: &c.statement,
                proof_digest: c.sigma[0].step_digest()?,
                payment_digest: &zero,
            },
        )?;
        GlueChip::assert_constant(region, valid.word(), Fp::ONE)
    }
    fn evidence(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        c: &Cells,
        bundle: &SignatureQCells,
    ) -> Result<(Bit<Fp>, Option<crate::a_relation::IncomingOmegaCells>), LayoutError> {
        bundle.bind_context(region, self.plan.context.operation(), 2, &c.q[2])?;
        if bundle.slots().len() != 1 || bundle.slots()[0].mode() != VerifyMode::Soft {
            return Err(LayoutError::Synthesis);
        }
        let scope = self.plan.policy.scope(chip, region)?;
        let (request, payer, receiver) = {
            let lanes = chip.operation_lanes()?;
            let mut uint = UintChip::new(lanes.glue, lanes.range);
            (
                RequestCells::check(&mut uint, lanes.hash, region, c.object(3)?)?,
                CredentialCells::check(&mut uint, region, c.object(4)?)?,
                CredentialCells::check(&mut uint, region, c.object(7)?)?,
            )
        };
        let payment = {
            let lanes = chip.operation_lanes()?;
            PaymentCells::from_run(
                &mut UintChip::new(lanes.glue, lanes.range),
                lanes.hash,
                region,
                &c.runs[6],
                &PaymentInputs {
                    request: &request,
                    payer: &payer,
                    statement: &c.retained_statement,
                    receipt: c.object(5)?,
                    provider: &scope.provider,
                    proof_digest: &c.retained_digest,
                },
            )?
        };
        // The retained own Payment and descriptor are hard original preimages.
        GlueChip::assert_constant(region, payment.valid().word(), Fp::ONE)?;
        for (actual, expected) in c.retained_statement.fields()[1..7].iter().zip(
            [
                &c.pred.fields()[3..5],
                &c.pred.fields()[1..3],
                &c.statement.fields()[5..7],
            ]
            .concat()
            .iter(),
        ) {
            GlueChip::assert_equal(region, actual, expected)?;
        }
        for (actual, expected) in payer.payment_key()?.iter().zip(&c.pred.fields()[9..13]) {
            GlueChip::assert_equal(region, actual, expected)?;
        }
        for (actual, expected) in payer
            .object()
            .identifier(3)?
            .iter()
            .zip(&c.pred.fields()[6..8])
        {
            GlueChip::assert_equal(region, actual, expected)?;
        }
        let signature =
            c.object(8)?
                .bind_signature(region, &bundle.slots()[0], receiver.payment_key()?)?;
        let mut incoming = None;
        let (digest, mut checks) = if let Some(t) = &c.transport {
            let key = self.omega_key(chip, region)?;
            let verified = t.verify(chip, region, self.plan.context.operation(), &key)?;
            let original_lineage_digest =
                active_digest(chip, region, t.active_carrier()?, *b"kgwlin_1")?;
            let opening = {
                let lanes = chip.operation_lanes()?;
                CreditOpeningCells::from_run(
                    lanes.glue,
                    lanes.range,
                    lanes.hash,
                    region,
                    &c.runs[11],
                )?
            };
            let status = {
                let lanes = chip.operation_lanes()?;
                StatusCells::from_run(
                    &mut UintChip::new(lanes.glue, lanes.range),
                    lanes.hash,
                    region,
                    &c.runs[10],
                    &StatusInputs {
                        statement: c.dynamic_statement.as_ref().ok_or(LayoutError::Synthesis)?,
                        receipt: c.object(8)?,
                        lineage: t.public().checked(),
                        lineage_digest: &original_lineage_digest,
                        proof_digest: c.object(8)?.word(9)?,
                        opening: &opening,
                        provider: &scope.provider,
                        relation: c.pred.fields()[3..5]
                            .try_into()
                            .map_err(|_| LayoutError::Synthesis)?,
                        request: &request,
                        receiver: &receiver,
                        payment_digest: payment.digest(),
                    },
                )?
            };
            let checks = vec![status.valid().clone(), verified.valid.clone(), signature];
            incoming = Some(verified);
            (status.digest().clone(), checks)
        } else {
            let bound = super::super::binding::bind_sigma(
                chip,
                region,
                &self.plan.context.operation().sigma,
                &c.q[0],
                &c.sigma,
            )?;
            super::super::split::bind_mode(
                region,
                bound.incoming_mode.as_ref().ok_or(LayoutError::Synthesis)?,
                &c.modes[0],
            )?;
            let evidence = {
                let lanes = chip.operation_lanes()?;
                ReceiveEvidenceCells::bind(
                    &mut UintChip::new(lanes.glue, lanes.range),
                    lanes.hash,
                    region,
                    &ReceiveEvidenceInputs {
                        statement: c
                            .incoming_statement
                            .as_ref()
                            .ok_or(LayoutError::Synthesis)?,
                        receipt: c.object(8)?,
                        request: &request,
                        receiver: &receiver,
                        relation: c.pred.fields()[3..5]
                            .try_into()
                            .map_err(|_| LayoutError::Synthesis)?,
                        provider: &scope.provider,
                        payment_digest: payment.digest(),
                        proof_digest: c.sigma[1].step_digest()?,
                    },
                )?
            };
            (
                evidence.digest().clone(),
                vec![
                    evidence.valid().clone(),
                    bound.incoming_valid.ok_or(LayoutError::Synthesis)?,
                    signature,
                ],
            )
        };
        let credited = {
            let lanes = chip.operation_lanes()?;
            CreditedCells::from_run(
                &mut UintChip::new(lanes.glue, lanes.range),
                lanes.hash,
                region,
                &c.runs[9],
                if c.transport.is_some() {
                    EvidenceKind::Status
                } else {
                    EvidenceKind::Receive
                },
                &[
                    request.credit_id().clone(),
                    payment.digest().clone(),
                    digest,
                ],
            )?
        };
        GlueChip::assert_equal(region, &c.statement.fields()[17], request.credit_id())?;
        GlueChip::assert_equal(region, &c.statement.fields()[18], credited.digest())?;
        checks.push(credited.valid().clone());
        let valid = crate::operation_relation::objects::predicates::all(
            chip.uint().glue(),
            region,
            &checks,
        )?;
        Ok((valid, incoming))
    }
    fn maps(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        c: &Cells,
        valid: &Bit<Fp>,
    ) -> Result<(), LayoutError> {
        let effects = &self.source.original.effects;
        let descriptor: [Word<Fp>; 7] = chip
            .uint()
            .glue()
            .witnesses(region, &effects.descriptor.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        for (actual, expected) in descriptor
            .iter()
            .zip(&c.retained_statement.fields()[17..24])
        {
            GlueChip::assert_equal(region, actual, expected)?;
        }
        let witness = ArchiveMapWitness {
            descriptor,
            core: self.removal(&mut chip.uint(), region, &effects.core)?,
            lineage: self.removal(&mut chip.uint(), region, &effects.lineage)?,
        };
        let transition = MapTransition {
            statement: &c.statement,
            predecessor: MapState {
                state: &c.old,
                lineage: &c.pred,
            },
            successor: MapState {
                state: &c.new,
                lineage: &c.next,
            },
        };
        administrative::archive(&mut chip.uint(), region, &transition)?;
        let lanes = chip.operation_lanes()?;
        MapEffectsChip::new(lanes.glue, lanes.range, lanes.hash).archive(
            region,
            &transition,
            &witness,
            valid,
        )
    }
}
fn raw_digest(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    run: &iroha_plonk_gadgets::bytes::tape::ByteRun<Fp>,
    domain: [u8; 8],
) -> Result<Word<Fp>, LayoutError> {
    let mut tape = iroha_plonk_gadgets::bytes::PBytes::new();
    for segment in run.primary() {
        tape.push_bounded(segment.bounded().ok_or(LayoutError::Synthesis)?)?;
    }
    if tape.len() != run.len() {
        return Err(LayoutError::Synthesis);
    }
    let lanes = chip.operation_lanes()?;
    tape.digest(lanes.glue, lanes.hash, region, u64::from_le_bytes(domain))
}
fn active_digest(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    raw: &ActiveBytes<Fp>,
    domain: [u8; 8],
) -> Result<Word<Fp>, LayoutError> {
    let lanes = chip.operation_lanes()?;
    raw.packed().digest(
        &mut UintChip::new(lanes.glue, lanes.range),
        lanes.hash.sponge_mut()?,
        region,
        u64::from_le_bytes(domain),
    )
}
impl Circuit<Fp> for FirstCircuit {
    type Config = StageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StageConfig {
        let verifier = VerifierConfig::configure_serialized_foreign(meta, SOURCE_RANGE_BUSES)
            .expect("fixed ArchiveSent source profile");
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(69);
        meta.enable_equality(public);
        StageConfig {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(
        &self,
        config: StageConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "ArchiveSent own hard predecessor",
            |mut region| {
                let c = self.setup(&mut chip, &mut bytes, &mut region)?;
                let key = self.omega_key(&mut chip, &mut region)?;
                let proof = self.carrier(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    &self.source.predecessor.proof,
                )?;
                let predecessor = verify_predecessor(
                    &mut chip,
                    &mut region,
                    self.plan.context.operation(),
                    &key,
                    &c.pred,
                    &c.next,
                    &c.pp,
                    &c.pv,
                    &proof,
                )?;
                self.own_proof(&mut region, &c)?;
                let fold = self.carrier(&mut chip, &mut bytes, &mut region, &self.fold)?;
                close_first(
                    &mut chip,
                    &mut region,
                    &self.plan.context,
                    &c.context(),
                    Some(&predecessor),
                    &[],
                    &c.sigma,
                    Some(&fold),
                    &self.plan.pallas,
                )?
                .words(&mut chip, &mut region)
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Continuation {
    first: First,
    plan: SplitPlan,
    wrapper: Vec<u8>,
    vesta: AccumulatorT<Eq>,
    fold: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    carried: AccumulatorT<Ep>,
    history: Vec<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
}
#[derive(Clone)]
struct ContinuationCircuit {
    first: FirstCircuit,
    plan: SplitPlan,
    wrapper: Vec<u8>,
    vesta: Option<AccumulatorT<Eq>>,
    fold: Vec<u8>,
    carried: Option<FoldInput<Ep>>,
    history: Vec<(Option<FoldInput<Ep>>, Option<AccumulatorT<Eq>>)>,
}
impl Continuation {
    fn circuit(&self) -> ContinuationCircuit {
        ContinuationCircuit {
            first: self.first.circuit(),
            plan: self.plan.clone(),
            wrapper: self.wrapper.clone(),
            vesta: Some(self.vesta.clone()),
            fold: self.fold.clone(),
            carried: Some(self.carried.as_input()),
            history: self
                .history
                .iter()
                .map(|(p, v)| (Some(p.as_input()), Some(v.clone())))
                .collect(),
        }
    }
}
impl ContinuationCircuit {
    fn blank(first: FirstCircuit, plan: SplitPlan) -> Result<Self, Error> {
        if !(1..A_STAGE_COUNT).contains(&plan.stage()) {
            return Err(Error::Artifact);
        }
        Ok(Self {
            first,
            wrapper: vec![0; plan.wrap().verifier().proof_length()],
            vesta: None,
            fold: vec![0; 1120],
            carried: None,
            history: vec![(None, None); plan.stage() - 1],
            plan,
        })
    }
}
impl Circuit<Fp> for ContinuationCircuit {
    type Config = StageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            first: self.first.without_witnesses(),
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StageConfig {
        FirstCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: StageConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        let f = &self.first;
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "complete ArchiveSent source continuation",
            |mut region| {
                let c = f.setup(&mut chip, &mut bytes, &mut region)?;
                let cp = f.pallas(&mut chip, &mut region, self.carried.as_ref())?;
                let cv = f.vesta(&mut chip, &mut region, self.vesta.as_ref())?;
                let history = self
                    .history
                    .iter()
                    .map(|(p, v)| {
                        Ok(crate::a_relation::split::ContextLinkCells {
                            pallas: f.pallas(&mut chip, &mut region, p.as_ref())?,
                            vesta: f.vesta(&mut chip, &mut region, v.as_ref())?,
                        })
                    })
                    .collect::<Result<Vec<_>, LayoutError>>()?;
                let proof = f.carrier(&mut chip, &mut bytes, &mut region, &self.wrapper)?;
                let resumed = crate::a_relation::split::resume_context(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &c.context(),
                    &history,
                    &cp,
                    &cv,
                    &proof,
                    &c.sigma,
                )?;
                let mut verified = Vec::new();
                let mut own_signature = None;
                let mut incoming_signature = None;
                for index in f
                    .plan
                    .context
                    .q_partition(self.plan.stage())
                    .ok_or(LayoutError::Synthesis)?
                    .iter()
                    .copied()
                {
                    let proof = f.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &f.source.original.q[index].proof,
                    )?;
                    let q = if index == 0 {
                        verify_sigma(
                            &mut chip,
                            &mut region,
                            f.plan.context.operation(),
                            &c.q[0],
                            &proof,
                            &c.sigma,
                        )?
                        .0
                    } else {
                        crate::a_relation::verify_q(
                            &mut chip,
                            &mut region,
                            f.plan.context.operation(),
                            index,
                            &c.q[index],
                            &proof,
                        )?
                    };
                    if index == 1 || index == 2 {
                        let bundle = crate::a_relation::bind_signature_q(
                            &mut chip,
                            &mut region,
                            f.plan.context.operation(),
                            index,
                            &f.plan.signatures[index - 1],
                            &q,
                        )?;
                        if index == 1 {
                            own_signature = Some(bundle);
                        } else {
                            incoming_signature = Some(bundle);
                        }
                    }
                    verified.push(q);
                }
                let incoming = if self.plan.is_terminal() {
                    let bundle = incoming_signature.as_ref().ok_or(LayoutError::Synthesis)?;
                    let (soft, incoming) = f.evidence(&mut chip, &mut region, &c, bundle)?;
                    let valid = crate::a_relation::bind_modes(
                        &mut chip,
                        &mut region,
                        f.plan.context.operation(),
                        core::slice::from_ref(&soft),
                        &c.modes,
                    )?;
                    f.maps(&mut chip, &mut region, &c, &valid)?;
                    incoming
                } else {
                    if self.plan.stage() == 2 {
                        f.own_auth(
                            &mut chip,
                            &mut region,
                            &c,
                            own_signature.as_ref().ok_or(LayoutError::Synthesis)?,
                        )?;
                    }
                    None
                };
                let selected = incoming
                    .as_ref()
                    .map(|incoming| {
                        crate::a_relation::select_incoming(
                            &mut chip,
                            &mut region,
                            incoming,
                            &[c.modes[0].clone(), c.modes[1].clone()],
                            c.pc.as_slice()
                                .try_into()
                                .map_err(|_| LayoutError::Synthesis)?,
                        )
                    })
                    .transpose()?;
                let iv = if incoming.is_some() {
                    Some(IncomingVestaCells {
                        claim: c
                            .transport
                            .as_ref()
                            .ok_or(LayoutError::Synthesis)?
                            .vesta()
                            .clone(),
                        mode: c.modes[2].clone(),
                        corrected: c.vc[0].clone(),
                    })
                } else {
                    None
                };
                let fold = f.carrier(&mut chip, &mut bytes, &mut region, &self.fold)?;
                let closed = crate::a_relation::split::close_stage(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &resumed,
                    None,
                    selected.as_ref(),
                    iv.as_ref(),
                    &verified,
                    &fold,
                )?;
                if self.plan.is_terminal() {
                    closed.words(&mut chip, &mut region, &c.next)
                } else {
                    closed.continuation()?.words(&mut chip, &mut region)
                }
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}

#[derive(Clone)]
enum StageData {
    First(FirstCircuit),
    Continued(ContinuationCircuit),
}
/// Actual fixed source A circuit. Its private stage is selected by native checkpoint order.
#[derive(Clone)]
pub struct StageCircuit {
    inner: StageData,
}
impl Circuit<Fp> for StageCircuit {
    type Config = StageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            inner: match &self.inner {
                StageData::First(c) => StageData::First(c.without_witnesses()),
                StageData::Continued(c) => StageData::Continued(c.without_witnesses()),
            },
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StageConfig {
        FirstCircuit::configure(meta)
    }
    fn synthesize(
        &self,
        config: StageConfig,
        layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        match &self.inner {
            StageData::First(c) => c.synthesize(config, layouter),
            StageData::Continued(c) => c.synthesize(config, layouter),
        }
    }
}

/// Checked original source coupled to the fixed complete ArchiveSent plan.
#[derive(Clone)]
pub struct Prepared {
    plan: Plan,
    source: Arc<Sources>,
}
impl Prepared {
    fn first(&self, salt: Fp, config: &FoldConfig) -> Result<First, Error> {
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
        Ok(First {
            source: self.source.clone(),
            plan: self.plan.clone(),
            pallas,
            fold: fold.to_bytes().to_vec(),
            known: true,
        })
    }
    /// Build the actual A1 relation and exact public frame from checked originals.
    /// The salt randomizes an actual fold; it does not choose a profile or obligation list.
    /// # Errors
    /// A failed exact predecessor two-input fold or complete decide.
    pub fn first_circuit(
        &self,
        salt: Fp,
        config: &FoldConfig,
    ) -> Result<(StageCircuit, Vec<Fp>), Error> {
        self.verify_sources(config.kernel_budget)?;
        let first = self.first(salt, config)?;
        let public = first_public(&first)?;
        Ok((
            StageCircuit {
                inner: StageData::First(first.circuit()),
            },
            public,
        ))
    }
}

/// Borrowed original material for one independently installed A or W identity.
/// These bytes carry no scheme/catalog authority; the installation owner pins
/// the descriptor, VK and complete `Plan` before calling the importer.
#[derive(Clone, Copy, Debug)]
pub struct OriginalArtifact<'a> {
    /// Canonical V2 descriptor authenticated by the installation owner.
    pub descriptor: &'a [u8],
    /// Exact independently installed canonical verifying-key bytes.
    pub verifying_key: &'a [u8],
    /// Original `PIPAPK01` tables and key, checked against source and installed VK.
    pub proving_key: &'a [u8],
}

fn artifact_binding(
    original: OriginalArtifact<'_>,
    curve: CurveV1,
    lengths: &[u32],
    types: &[InstanceType],
    config: ReadConfig,
) -> Result<DescriptorBinding, Error> {
    if original.descriptor.is_empty()
        || original.descriptor.len() > DESCRIPTOR_MAX_BYTES
        || original.verifying_key.is_empty()
        || original.verifying_key.len() > VERIFYING_KEY_MAX_BYTES
        || original.proving_key.is_empty()
        || original.proving_key.len() > config.maximum_bytes
    {
        return Err(Error::Artifact);
    }
    let binding = DescriptorBinding::decode_v2(original.descriptor).map_err(|_| Error::Artifact)?;
    let d = binding.descriptor();
    if d.k != 16
        || d.curve != curve
        || d.transcript != TranscriptV2::KagemushaPoseidonRp57Base
        || d.instance_mode != InstanceModeV1::Direct
        || d.proof_suffix != ProofSuffixV1::FoldedGenerator
        || d.instance_lengths.as_slice() != lengths
        || d.instance_types.as_deref() != Some(types)
        || binding.n() > config.maximum_rows
    {
        return Err(Error::Artifact);
    }
    Ok(binding)
}

/// Original A1/A2/A3/A4 and W0/W1/W2 keys for the immutable archive plan.
/// Installation independently authenticates the catalog; source continuity grants no wallet-open authority.
#[derive(Clone)]
pub struct Prover {
    plan: Plan,
    a: [Arc<ProvingKey<Eq>>; A_STAGE_COUNT],
    w: [Arc<ProvingKey<Ep>>; W_STAGE_COUNT],
    wrappers: [WKey; W_STAGE_COUNT],
}
impl Prover {
    /// Import the fixed seven-stage source using only installed metadata.
    ///
    /// The native owner authenticates scheme/provider/root, predecessor and Q
    /// keys, hard signature slots, stage schedule and each descriptor/VK before
    /// this call. Existing PIPAPK01 source/copy/selector/commitment checks plus
    /// exact installed VK equality bind PK admissibility transitively; no new PK
    /// signature format, runtime key generation or alternate profile is used.
    /// Source construction uses unknown witnesses and creates no Prepared,
    /// verified opening or checkpoint. Bounds cover originals/domains, not total
    /// synthesis/prover memory. This component grants no Native wallet open.
    ///
    /// # Errors
    /// Invalid/bounded originals, nonuniform A/profile/curve/k, substituted VK,
    /// wrong source/stage/previous key, or source/copy/commitment mismatch.
    pub fn from_original_artifacts(
        plan: Plan,
        a: [OriginalArtifact<'_>; A_STAGE_COUNT],
        w: [OriginalArtifact<'_>; W_STAGE_COUNT],
        config: ReadConfig,
    ) -> Result<Self, Error> {
        let a_bindings = a
            .iter()
            .map(|&original| {
                artifact_binding(
                    original,
                    CurveV1::Vesta,
                    &[69],
                    &[InstanceType::Bounded],
                    config,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let w_bindings = w
            .iter()
            .map(|&original| {
                artifact_binding(
                    original,
                    CurveV1::Pallas,
                    &[1, 2, 16],
                    &crate::omega::OmegaPlan::instance_types(),
                    config,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (original, binding) in a.iter().zip(&a_bindings) {
            if binding != &a_bindings[0] {
                return Err(Error::Artifact);
            }
            VerifyingKey::<Eq>::read(original.verifying_key, binding)
                .map_err(|_| Error::Artifact)?;
        }
        for (original, binding) in w.iter().zip(&w_bindings) {
            VerifyingKey::<Ep>::read(original.verifying_key, binding)
                .map_err(|_| Error::Artifact)?;
        }
        let first = FirstCircuit::blank(&plan)?;
        let mut a_keys = Vec::new();
        let mut w_keys = Vec::new();
        let mut wrappers: Vec<WKey> = Vec::new();
        for stage in 0..A_STAGE_COUNT {
            let circuit = if stage == 0 {
                StageCircuit {
                    inner: StageData::First(first.clone()),
                }
            } else {
                let split = SplitPlan::new(
                    plan.context.clone(),
                    stage,
                    wrappers[stage - 1].clone(),
                    &plan.pallas,
                )
                .map_err(|_| Error::Artifact)?;
                StageCircuit {
                    inner: StageData::Continued(ContinuationCircuit::blank(first.clone(), split)?),
                }
            };
            let key = ProvingKey::from_artifact_v2(
                a[stage].proving_key,
                &a_bindings[stage],
                &plan.vesta,
                &circuit,
                config,
            )
            .map_err(|_| Error::Artifact)?;
            if key.vk().to_bytes() != a[stage].verifying_key {
                return Err(Error::Artifact);
            }
            if stage + 1 < A_STAGE_COUNT {
                let source = wrapper_source(&plan, stage, &a_bindings[stage], key.vk())?;
                let wrapper = ProvingKey::from_artifact_v2(
                    w[stage].proving_key,
                    &w_bindings[stage],
                    &plan.pallas,
                    &source,
                    config,
                )
                .map_err(|_| Error::Artifact)?;
                if wrapper.vk().to_bytes() != w[stage].verifying_key {
                    return Err(Error::Artifact);
                }
                wrappers.push(
                    WKey::from_artifact(
                        &plan.context,
                        stage,
                        w_bindings[stage].clone(),
                        plan.pallas.clone(),
                        wrapper.vk().clone(),
                    )
                    .map_err(|_| Error::Artifact)?,
                );
                w_keys.push(Arc::new(wrapper));
            }
            a_keys.push(Arc::new(key));
        }
        Ok(Self {
            plan,
            a: a_keys.try_into().map_err(|_| Error::Artifact)?,
            w: w_keys.try_into().map_err(|_| Error::Artifact)?,
            wrappers: wrappers.try_into().map_err(|_| Error::Artifact)?,
        })
    }
    /// Check all original proofs/tapes and create a session using only installed artifacts.
    /// # Errors
    /// Any predecessor/Q proof, transported decide or same-tape mismatch.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Session<'_>, Error> {
        Ok(Session {
            prover: self,
            prepared: self.plan.prepare(input, budget)?,
        })
    }
    /// Exact installed descriptors in alternating A/W checkpoint order, ending A4.
    #[must_use]
    pub fn descriptors(&self) -> Vec<&DescriptorBinding> {
        let mut all = Vec::with_capacity(A_STAGE_COUNT + W_STAGE_COUNT);
        for stage in 0..A_STAGE_COUNT {
            all.push(self.a[stage].binding());
            if stage < W_STAGE_COUNT {
                all.push(self.w[stage].binding());
            }
        }
        all
    }
}

fn wrapper_source(
    plan: &Plan,
    stage: usize,
    binding: &DescriptorBinding,
    key: &VerifyingKey<Eq>,
) -> Result<WCircuit, Error> {
    let allowed = key.kagemusha_digest(binding).map_err(|_| Error::Artifact)?;
    let length = iroha_plonk::Protocol::new(binding.descriptor())
        .map_err(|_| Error::Artifact)?
        .proof_length();
    WCircuit::new(
        &plan.context,
        stage,
        binding.clone(),
        plan.vesta.clone(),
        vec![allowed],
        OmegaWitness {
            key: key.clone(),
            instances: vec![Fp::ZERO; 69],
            proof: vec![0; length],
            length: u32::try_from(length).map_err(|_| Error::Artifact)?,
            fold: [0; 1120],
        },
    )
    .map(|source| source.without_witnesses())
    .map_err(|_| Error::Artifact)
}

/// Source-bound session over the fixed installed ArchiveSent artifacts.
pub struct Session<'a> {
    prover: &'a Prover,
    prepared: Prepared,
}
impl Session<'_> {
    fn require_source(&self, source: &ACheckpoint) -> Result<(), Error> {
        if source.stage >= A_STAGE_COUNT
            || !Arc::ptr_eq(&source.first.source, &self.prepared.source)
        {
            return Err(Error::Input);
        }
        Ok(())
    }
    fn verify_a(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<(), Error> {
        self.require_source(source)?;
        self.prepared.verify_sources(budget)?;
        let key = &self.prover.a[source.stage];
        verify_full(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            &[source.public.clone()],
            &source.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        source
            .pallas
            .decide(&self.prepared.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        source
            .part
            .decide(&self.prepared.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        source
            .opening
            .decide(&self.prepared.plan.vesta, budget)
            .map_err(|_| Error::Proof)
    }
    /// Prove A1 under its installed key, retaining the exact predecessor fold and opening.
    /// # Errors
    /// Fixed artifact/circuit mismatch, failed fold, proof or complete decide.
    pub fn first(
        &self,
        salt: Fp,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<ACheckpoint, Error> {
        self.prepared.verify_sources(fold.kernel_budget)?;
        let first = self.prepared.first(salt, fold)?;
        let public = first_public(&first)?;
        let circuit = StageCircuit {
            inner: StageData::First(first.circuit()),
        };
        let (proof, opening) = prove_a(
            &self.prepared.plan,
            &self.prover.a[0],
            &circuit,
            &public,
            randomness,
            config,
            fold.kernel_budget,
        )?;
        Ok(ACheckpoint {
            stage: 0,
            first: first.clone(),
            public,
            proof,
            pallas: first.pallas.clone(),
            part: first.source.part.clone(),
            history: vec![],
            opening,
        })
    }
    /// Restore A1 from original proof and exact544 carried Pallas bytes.
    /// Derive the expected frame from this session's original objects/Q/predecessor.
    /// # Errors
    /// Noncanonical/mismatching claim, changed source context, wrong proof or failed decide.
    pub fn restore_first(
        &self,
        proof: Vec<u8>,
        pallas: &[u8],
        budget: MemoryBudget,
    ) -> Result<ACheckpoint, Error> {
        self.prepared.verify_sources(budget)?;
        let pallas = AccumulatorT::<Ep>::from_bytes(pallas).map_err(|_| Error::Input)?;
        pallas
            .decide(&self.prepared.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        let first = First {
            source: self.prepared.source.clone(),
            plan: self.prepared.plan.clone(),
            pallas: pallas.clone(),
            fold: vec![],
            known: true,
        };
        let public = first_public(&first)?;
        let key = &self.prover.a[0];
        verify_full(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            &[public.clone()],
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_vesta(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            &[public.clone()],
            &proof,
            budget,
        )?;
        Ok(ACheckpoint {
            stage: 0,
            first,
            public,
            proof,
            pallas,
            part: self.prepared.source.part.clone(),
            history: vec![],
            opening,
        })
    }
    /// Prove the one fixed W continuation for a nonterminal A checkpoint.
    /// Its four Vesta slots are source part, actual A opening, and two explicit trivial16.
    /// # Errors
    /// Terminal/wrong-source checkpoint, artifact/circuit mismatch or any failed full proof.
    pub fn wrapper(
        &self,
        source: &ACheckpoint,
        salt: Fq,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<WCheckpoint, Error> {
        self.verify_a(source, fold.kernel_budget)?;
        if source.stage >= W_STAGE_COUNT {
            return Err(Error::Input);
        }
        let trivial = AccumulatorT::trivial(&self.prepared.plan.vesta, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let (vfold, vesta) = create_fold(
            &self.prepared.plan.vesta,
            &[
                source.part.clone(),
                source.opening.clone(),
                trivial.as_input(),
                trivial.as_input(),
            ],
            salt.to_repr(),
            fold,
        )
        .map_err(|_| Error::Proof)?;
        vesta
            .decide(&self.prepared.plan.vesta, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let a = &self.prover.a[source.stage];
        let allowed = a
            .vk()
            .kagemusha_digest(a.binding())
            .map_err(|_| Error::Artifact)?;
        let circuit = WCircuit::new(
            &self.prepared.plan.context,
            source.stage,
            a.binding().clone(),
            self.prepared.plan.vesta.clone(),
            vec![allowed],
            OmegaWitness {
                key: a.vk().clone(),
                instances: source.public.clone(),
                proof: source.proof.clone(),
                length: u32::try_from(source.proof.len()).map_err(|_| Error::Input)?,
                fold: vfold.to_bytes(),
            },
        )
        .map_err(|_| Error::Input)?;
        let public = omega_instances(source.public[0], &vesta)?;
        let w = &self.prover.w[source.stage];
        let witness = Witness::from_circuit(w, &circuit, &public).map_err(|_| Error::Prover)?;
        let output = create_proof_owned_with_claim(
            &self.prepared.plan.pallas,
            w,
            witness,
            randomness,
            config,
        )
        .map_err(|_| Error::Prover)?;
        self.restore_wrapper(source, output.proof, &vesta.to_bytes(), fold.kernel_budget)
    }
    /// Restore the exact W proof and canonical Vesta obligation for the source A checkpoint.
    /// # Errors
    /// Terminal/wrong source, wrong W public frame or any failed proof/full claim decide.
    pub fn restore_wrapper(
        &self,
        source: &ACheckpoint,
        proof: Vec<u8>,
        vesta: &[u8],
        budget: MemoryBudget,
    ) -> Result<WCheckpoint, Error> {
        self.verify_a(source, budget)?;
        if source.stage >= W_STAGE_COUNT {
            return Err(Error::Input);
        }
        let vesta = AccumulatorT::<Eq>::from_bytes(vesta).map_err(|_| Error::Input)?;
        vesta
            .decide(&self.prepared.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let public = omega_instances(source.public[0], &vesta)?;
        let w = &self.prover.w[source.stage];
        verify_full(
            &self.prepared.plan.pallas,
            w.binding(),
            w.vk(),
            &public,
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_pallas(
            &self.prepared.plan.pallas,
            w.binding(),
            w.vk(),
            &public,
            &proof,
            budget,
        )?;
        Ok(WCheckpoint {
            source: source.clone(),
            proof,
            vesta,
            opening,
        })
    }
    fn continuation(
        &self,
        wrapper: &WCheckpoint,
        pallas: AccumulatorT<Ep>,
        fold: Vec<u8>,
    ) -> Result<Continuation, Error> {
        self.require_source(&wrapper.source)?;
        let previous = wrapper.source.stage;
        if previous >= W_STAGE_COUNT {
            return Err(Error::Input);
        }
        let split = SplitPlan::new(
            self.prepared.plan.context.clone(),
            previous + 1,
            self.prover.wrappers[previous].clone(),
            &self.prepared.plan.pallas,
        )
        .map_err(|_| Error::Artifact)?;
        Ok(Continuation {
            first: wrapper.source.first.clone(),
            plan: split,
            wrapper: wrapper.proof.clone(),
            vesta: wrapper.vesta.clone(),
            fold,
            pallas,
            carried: wrapper.source.pallas.clone(),
            history: wrapper.source.history.clone(),
        })
    }
    /// Prove the next A stage using its installed key and all required Pallas slots.
    /// Ordered slots are previous carried P, actual W opening, then any fixed stage Q.
    /// # Errors
    /// Wrong source/stage, omitted obligation, circuit mismatch or any failed full proof.
    pub fn advance(
        &self,
        wrapper: &WCheckpoint,
        salt: Fp,
        fold: &FoldConfig,
        randomness: ProverRandomness<'_>,
        config: ProverConfig,
    ) -> Result<ACheckpoint, Error> {
        let restored = self.restore_wrapper(
            &wrapper.source,
            wrapper.proof.clone(),
            &wrapper.vesta.to_bytes(),
            fold.kernel_budget,
        )?;
        let next = restored.source.stage + 1;
        let indices = self
            .prepared
            .plan
            .context
            .q_partition(next)
            .ok_or(Error::Artifact)?;
        let mut claims = vec![restored.source.pallas.as_input(), restored.opening.clone()];
        if next + 1 == A_STAGE_COUNT && self.prepared.plan.incoming.is_some() {
            claims.extend(self.prepared.source.incoming.selected.iter().cloned());
        }
        claims.extend(
            indices
                .iter()
                .map(|i| self.prepared.source.q_openings[*i].clone()),
        );
        let (pfold, pallas) =
            create_fold(&self.prepared.plan.pallas, &claims, salt.to_repr(), fold)
                .map_err(|_| Error::Proof)?;
        pallas
            .decide(&self.prepared.plan.pallas, fold.kernel_budget)
            .map_err(|_| Error::Proof)?;
        let continuation =
            self.continuation(&restored, pallas.clone(), pfold.to_bytes().to_vec())?;
        let public = continuation_public(&continuation)?;
        let circuit = StageCircuit {
            inner: StageData::Continued(continuation.circuit()),
        };
        let (proof, opening) = prove_a(
            &self.prepared.plan,
            &self.prover.a[next],
            &circuit,
            &public,
            randomness,
            config,
            fold.kernel_budget,
        )?;
        let mut history = restored.source.history.clone();
        history.push((restored.source.pallas.clone(), restored.vesta.clone()));
        Ok(ACheckpoint {
            stage: next,
            first: restored.source.first,
            public,
            proof,
            pallas,
            part: restored.vesta.as_input(),
            history,
            opening,
        })
    }
    /// Restore each subsequent A from its verified source W and canonical new Pallas claim.
    /// Every history/context/public field is derived from the retained source chain.
    /// # Errors
    /// Canonical claim, source/history, installed key, proof or complete decide mismatch.
    pub fn restore_a(
        &self,
        wrapper: &WCheckpoint,
        proof: Vec<u8>,
        pallas: &[u8],
        budget: MemoryBudget,
    ) -> Result<ACheckpoint, Error> {
        let restored = self.restore_wrapper(
            &wrapper.source,
            wrapper.proof.clone(),
            &wrapper.vesta.to_bytes(),
            budget,
        )?;
        let pallas = AccumulatorT::<Ep>::from_bytes(pallas).map_err(|_| Error::Input)?;
        pallas
            .decide(&self.prepared.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        let continuation = self.continuation(&restored, pallas.clone(), vec![])?;
        let public = continuation_public(&continuation)?;
        let next = restored.source.stage + 1;
        let key = &self.prover.a[next];
        verify_full(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            &[public.clone()],
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_vesta(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            &[public.clone()],
            &proof,
            budget,
        )?;
        let mut history = restored.source.history.clone();
        history.push((restored.source.pallas.clone(), restored.vesta.clone()));
        Ok(ACheckpoint {
            stage: next,
            first: restored.source.first,
            public,
            proof,
            pallas,
            part: restored.vesta.as_input(),
            history,
            opening,
        })
    }
    /// Export A11 and all distinct final Omega obligations after another full native check.
    /// This grants neither monetary completion nor a final transported Omega.
    /// # Errors
    /// A nonterminal/wrong source checkpoint or any failed native proof/decide.
    pub fn terminal(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<Terminal, Error> {
        self.verify_a(source, budget)?;
        if source.stage + 1 != A_STAGE_COUNT {
            return Err(Error::Input);
        }
        if source.part.source_k() != 16 {
            return Err(Error::Input);
        }
        let vesta = AccumulatorT::<Eq>::new(*source.part.g(), *source.part.challenges())
            .map_err(|_| Error::Input)?;
        Ok(Terminal {
            proof: source.proof.clone(),
            instances: source.public.clone(),
            pallas: source.pallas.clone(),
            vesta_part: vesta,
            predecessor_vesta: self.prepared.source.predecessor.vesta.clone(),
            incoming_vesta: self.prepared.source.incoming.selected_vesta.clone(),
            opening: source.opening.clone(),
        })
    }
}

/// A fully verified source-bound A checkpoint; ordinal/history/claims are private.
#[derive(Clone)]
pub struct ACheckpoint {
    stage: usize,
    first: First,
    public: Vec<Fp>,
    proof: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    part: FoldInput<Eq>,
    history: Vec<(AccumulatorT<Ep>, AccumulatorT<Eq>)>,
    opening: FoldInput<Eq>,
}
impl ACheckpoint {
    /// Zero-based A ordinal, fixed by the native continuation chain.
    #[must_use]
    pub const fn stage(&self) -> usize {
        self.stage
    }
    /// Exact original proof for immutable custody.
    #[must_use]
    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
    /// Exact69-word native public frame.
    #[must_use]
    pub fn instances(&self) -> &[Fp] {
        &self.public
    }
    /// Exact canonical carried Pallas claim for restoration.
    #[must_use]
    pub fn pallas_bytes(&self) -> [u8; 544] {
        self.pallas.to_bytes()
    }
}
/// Fully verified internal W checkpoint bound to its exact source A/history.
#[derive(Clone)]
pub struct WCheckpoint {
    source: ACheckpoint,
    proof: Vec<u8>,
    vesta: AccumulatorT<Eq>,
    opening: FoldInput<Ep>,
}
impl WCheckpoint {
    /// Zero-based W ordinal in the fixed source schedule.
    #[must_use]
    pub const fn stage(&self) -> usize {
        self.source.stage
    }
    /// Exact original W proof for custody.
    #[must_use]
    pub fn proof(&self) -> &[u8] {
        &self.proof
    }
    /// Exact canonical deciding Vesta claim for restoration.
    #[must_use]
    pub fn vesta_bytes(&self) -> [u8; 544] {
        self.vesta.to_bytes()
    }
}
/// Actual terminal A proof and every distinct obligation consumed by the final Omega producer.
#[derive(Clone, Debug)]
pub struct Terminal {
    /// Original terminal A proof under its installed key.
    pub proof: Vec<u8>,
    /// Exact homogeneous69-word public frame.
    pub instances: Vec<Fp>,
    /// Full accumulated Pallas claim.
    pub pallas: AccumulatorT<Ep>,
    /// Full current Vesta part forwarded by the preceding W.
    pub vesta_part: AccumulatorT<Eq>,
    /// Full predecessor Vesta obligation, distinct from the current part and A opening.
    pub predecessor_vesta: AccumulatorT<Eq>,
    /// Selected actual incoming Vesta obligation, distinct from own/pred/opening.
    pub incoming_vesta: FoldInput<Eq>,
    /// Actual terminal A own opening, a separate final Omega slot.
    pub opening: FoldInput<Eq>,
}

fn signature_schemas(policy: OwnPolicy) -> Result<[QSignaturePlan; 2], Error> {
    Ok([
        QSignaturePlan::new(vec![
            SignatureSlot {
                mode: VerifyMode::Hard,
                key: SignatureKey::Variable,
            },
            SignatureSlot {
                mode: VerifyMode::Hard,
                key: SignatureKey::Variable,
            },
            SignatureSlot {
                mode: VerifyMode::Hard,
                key: SignatureKey::Fixed(policy.root),
            },
        ])
        .map_err(|_| Error::Artifact)?,
        QSignaturePlan::new(vec![SignatureSlot {
            mode: VerifyMode::Soft,
            key: SignatureKey::Variable,
        }])
        .map_err(|_| Error::Artifact)?,
    ])
}
fn operation_schedule() -> (Vec<Vec<usize>>, Vec<Vec<OperationTask>>) {
    (
        vec![vec![], vec![0], vec![1], vec![2]],
        vec![
            vec![OperationTask::ArchiveOwnProof],
            vec![],
            vec![OperationTask::ArchiveAuthorization],
            vec![OperationTask::ArchiveEvidence, OperationTask::ArchiveMaps],
        ],
    )
}
fn object_kind(index: usize) -> Option<ObjectKind> {
    match index {
        0 | 4 | 7 => Some(ObjectKind::Credential),
        1 => Some(ObjectKind::Certificate),
        2 | 5 | 8 => Some(ObjectKind::Receipt),
        3 => Some(ObjectKind::Request),
        _ => None,
    }
}
fn context_specs(
    variant: Variant,
    omega: usize,
    sigma: usize,
) -> Result<Vec<ContextObjectSpec>, Error> {
    let mut sizes = (0..10)
        .map(|i| object_kind(i).map_or(if i == 6 { 163 } else { 99 }, |k| k.body_len() + 64))
        .collect::<Vec<_>>();
    sizes.extend([omega, sigma]);
    if variant == Variant::ArchiveStatus {
        sizes.extend([omega, 162, 1125]);
    } else if variant == Variant::ArchiveReceive {
        sizes.push(sigma);
    } else {
        return Err(Error::Artifact);
    }
    sizes
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            Ok(ContextObjectSpec {
                tag: u32::try_from(i + 1).map_err(|_| Error::Artifact)?,
                capacity: u32::try_from(n).map_err(|_| Error::Artifact)?,
            })
        })
        .collect()
}
fn check_original_shapes(plan: &Plan, input: &Inputs) -> Result<(), Error> {
    for (i, raw) in input.objects.iter().enumerate() {
        let n = object_kind(i).map_or(if i == 6 { 163 } else { 99 }, |k| k.body_len() + 64);
        if raw.len() != n {
            return Err(Error::Input);
        }
    }
    let expected_omega = 320
        + plan
            .context
            .operation()
            .omega()
            .ok_or(Error::Artifact)?
            .proof_length()
        + 1088;
    if input.retained.omega.len() != expected_omega
        || input.retained.sigma.is_empty()
        || !input.retained.sigma.len().is_multiple_of(32)
    {
        return Err(Error::Input);
    }
    if input.retained.omega.len() > plan.omega_capacity
        || input.retained.sigma.len() > plan.sigma_capacity
        || input
            .retained
            .omega
            .len()
            .checked_add(input.retained.sigma.len())
            .is_none_or(|n| n > MAX_PAYMENT_ORIGINAL_BYTES)
    {
        return Err(Error::Input);
    }
    if plan.incoming.is_some() {
        if !input.incoming.sigma.is_empty()
            || input.incoming.omega.len() > plan.omega_capacity
            || input.incoming.status.len() != 162
            || input.incoming.opening.len() != 1125
        {
            return Err(Error::Input);
        }
    } else if !input.incoming.omega.is_empty()
        || !input.incoming.status.is_empty()
        || !input.incoming.opening.is_empty()
        || input.incoming.sigma.len() > plan.sigma_capacity
    {
        return Err(Error::Input);
    }
    Ok(())
}
fn frame(raw: &[u8]) -> Result<Vec<u8>, LayoutError> {
    let mut out = u32::try_from(raw.len())
        .map_err(|_| LayoutError::BoundsFailure)?
        .to_le_bytes()
        .to_vec();
    out.extend(raw);
    Ok(out)
}
fn field_bit(v: Fp) -> Result<bool, Error> {
    if v == Fp::ZERO {
        Ok(false)
    } else if v == Fp::ONE {
        Ok(true)
    } else {
        Err(Error::Input)
    }
}
fn embed(v: Fp) -> Result<Fq, Error> {
    Fq::from_repr(v.to_repr()).into_option().ok_or(Error::Input)
}
fn check_sigma_tape(plan: &Plan, input: &Inputs) -> Result<(), Error> {
    let columns = &input.q[0].instances;
    if columns.len() != 5
        || columns
            .iter()
            .zip(plan.context.operation().sigma.instance_lengths())
            .any(|(c, n)| c.len() != n)
        || columns[2][0] != Fq::from(12)
        || columns[3][0] != Fq::ONE
        || input.sigma.len()
            != plan
                .context
                .operation()
                .sigma
                .class(0)
                .ok_or(Error::Artifact)?
                .verifier()
                .proof_length()
    {
        return Err(Error::Input);
    }
    if plan.incoming.is_none() && columns[2][1] != Fq::from(10 + u64::from(plan.recorded_blacklist))
    {
        return Err(Error::Input);
    }
    let statements = if plan.incoming.is_none() {
        vec![input.state.statement, input.incoming.statement]
    } else {
        vec![input.state.statement]
    };
    for (slot, (statement, raw)) in statements
        .iter()
        .zip([&input.sigma, &input.incoming.sigma])
        .enumerate()
    {
        let length = plan
            .context
            .operation()
            .sigma
            .class(slot)
            .ok_or(Error::Artifact)?
            .verifier()
            .proof_length();
        let mut fixed = raw[..raw.len().min(length)].to_vec();
        fixed.resize(length, 0);
        let mut tape = u32::try_from(raw.len())
            .map_err(|_| Error::Input)?
            .to_le_bytes()
            .to_vec();
        tape.extend(fixed);
        let chunks = tape
            .chunks(31)
            .map(|c| le_value::<Fq>(c).ok_or(Error::Input))
            .collect::<Result<Vec<_>, _>>()?;
        let range = plan
            .context
            .operation()
            .sigma
            .chunk_range(slot)
            .ok_or(Error::Artifact)?;
        if columns[0][slot]
            != embed(hash_with_domain(
                iroha_plonk_gadgets::statement::STATEMENT_DOMAIN,
                statement,
            ))?
            || range.len() != chunks.len()
            || columns[0][range] != chunks
        {
            return Err(Error::Input);
        }
    }
    Ok(())
}
fn q_sigma_part(input: &QInput, source_k: u32) -> Result<FoldInput<Eq>, Error> {
    let [bounded, point, indices, verdicts, source] = input.instances.as_slice() else {
        return Err(Error::Input);
    };
    if point.len() != 2
        || indices.len() != if source_k == 16 { 2 } else { 1 }
        || verdicts.len() != if source_k == 16 { 5 } else { 1 }
        || indices.first() != Some(&Fq::from(12))
        || (source_k == 16
            && !matches!(indices[1], value if value==Fq::from(10)||value==Fq::from(11)))
        || verdicts[0] != Fq::ONE
        || source.as_slice() != [Fq::from(u64::from(source_k))]
        || bounded.len() < K
    {
        return Err(Error::Input);
    }
    let g = EqAffine::from_xy(point[0], point[1])
        .into_option()
        .ok_or(Error::Input)?;
    let u = bounded[bounded.len() - K..]
        .iter()
        .map(|v| Fp::from_repr(v.to_repr()).into_option().ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Input)?;
    FoldInput::from_normalized(g, source_k, u).map_err(|_| Error::Input)
}
fn pallas_fields(claim: &crate::a_relation::SelectedPallasCells) -> Vec<Word<Fp>> {
    let mut out = Vec::with_capacity(68);
    for p in [&claim.pallas, &claim.opening] {
        out.extend([p.g().x().clone(), p.g().y().clone()]);
        for u in p.challenges() {
            out.extend([u.lo().word().clone(), u.hi().word().clone()]);
        }
    }
    out
}
fn decode_foreign(lo: Fp, hi: Fp) -> Result<Fq, Error> {
    let l = lo.to_repr();
    let h = hi.to_repr();
    if l[16..].iter().any(|b| *b != 0) || h[16..].iter().any(|b| *b != 0) {
        return Err(Error::Input);
    }
    let mut raw = [0; 32];
    raw[..16].copy_from_slice(&l[..16]);
    raw[16..].copy_from_slice(&h[..16]);
    Fq::from_repr(raw).into_option().ok_or(Error::Input)
}
fn pallas_from_fields(f: &[Fp]) -> Result<FoldInput<Ep>, Error> {
    if f.len() != 34 {
        return Err(Error::Input);
    }
    let g = iroha_pasta::EpAffine::from_xy(f[0], f[1])
        .into_option()
        .ok_or(Error::Input)?;
    let u = f[2..]
        .chunks_exact(2)
        .map(|p| decode_foreign(p[0], p[1]))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| Error::Input)?;
    FoldInput::from_normalized(g, 16, u).map_err(|_| Error::Input)
}
fn vesta_words(c: &FoldInput<Eq>) -> Result<Vec<Fp>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(c.g().coordinates()).ok_or(Error::Input)?;
    let mut f = foreign_limbs(&x)
        .into_iter()
        .chain(foreign_limbs(&y))
        .map(Fp::from_u128)
        .collect::<Vec<_>>();
    f.extend(c.challenges());
    Ok(f)
}
fn incoming_from_fields(f: &[Fp]) -> Result<NativeIncoming, Error> {
    if f.len() != 106 {
        return Err(Error::Input);
    }
    let public = f[..18].try_into().map_err(|_| Error::Input)?;
    let pallas = pallas_from_fields(&f[18..52])?;
    let opening = pallas_from_fields(&f[52..86])?;
    let v = &f[86..];
    let g = EqAffine::from_xy(decode_foreign(v[0], v[1])?, decode_foreign(v[2], v[3])?)
        .into_option()
        .ok_or(Error::Input)?;
    let vesta = FoldInput::from_normalized(g, 16, v[4..].try_into().map_err(|_| Error::Input)?)
        .map_err(|_| Error::Input)?;
    Ok(NativeIncoming {
        public,
        pallas: pallas.clone(),
        opening: opening.clone(),
        vesta: vesta.clone(),
        selected: [pallas.clone(), opening],
        selected_vesta: vesta.clone(),
        modes: [[true, false, false]; 4],
        pallas_corrections: [Ep::from(*pallas.g()); 2],
        vesta_correction: Eq::from(*vesta.g()),
    })
}
fn proved_sigma_mode(input: &Inputs) -> Result<[bool; 3], Error> {
    let v = input.q[0].instances.get(3).ok_or(Error::Input)?;
    if v.len() != 5 {
        return Err(Error::Input);
    }
    let mut bits = [false; 3];
    for (i, v) in v[2..].iter().enumerate() {
        bits[i] = if *v == Fq::ZERO {
            false
        } else if *v == Fq::ONE {
            true
        } else {
            return Err(Error::Input);
        };
    }
    if bits.iter().filter(|b| **b).count() != 1 {
        return Err(Error::Input);
    }
    Ok(bits)
}
fn select_native_incoming(
    plan: &Plan,
    input: &Inputs,
    good: bool,
    incoming: &mut NativeIncoming,
    budget: MemoryBudget,
) -> Result<(), Error> {
    let tp = AccumulatorT::<Ep>::trivial(&plan.pallas, budget)
        .map_err(|_| Error::Proof)?
        .as_input();
    let tv = AccumulatorT::<Eq>::trivial(&plan.vesta, budget)
        .map_err(|_| Error::Proof)?
        .as_input();
    incoming.selected = [tp.clone(), tp.clone()];
    incoming.selected_vesta = tv.clone();
    incoming.modes = [[false, true, false]; 4];
    incoming.pallas_corrections = [Ep::from(*tp.g()); 2];
    incoming.vesta_correction = Eq::from(*tv.g());
    if plan.incoming.is_none() {
        let sigma = proved_sigma_mode(input)?;
        incoming.modes[3] = sigma;
        if (!good && sigma != [false, true, false]) || (good && sigma == [false, true, false]) {
            return Err(Error::Proof);
        }
        return Ok(());
    }
    let valid = [
        incoming.pallas.decide(&plan.pallas, budget).is_ok(),
        incoming.opening.decide(&plan.pallas, budget).is_ok(),
        incoming.vesta.decide(&plan.vesta, budget).is_ok(),
    ];
    if !good {
        return Ok(());
    }
    if let Some(failed) = valid.iter().position(|b| !*b) {
        incoming.modes[failed] = [false, false, true];
        if failed < 2 {
            let original = if failed == 0 {
                &incoming.pallas
            } else {
                &incoming.opening
            };
            let corrected = original
                .corrected(&plan.pallas, budget)
                .map_err(|_| Error::Proof)?;
            incoming.selected[failed] = corrected.replacement().clone();
            incoming.pallas_corrections[failed] = Ep::from(*corrected.replacement().g());
        } else {
            let corrected = incoming
                .vesta
                .corrected(&plan.vesta, budget)
                .map_err(|_| Error::Proof)?;
            incoming.selected_vesta = corrected.replacement().clone();
            incoming.vesta_correction = Eq::from(*corrected.replacement().g());
        }
    } else {
        incoming.modes[..3].fill([true, false, false]);
        incoming.selected = [incoming.pallas.clone(), incoming.opening.clone()];
        incoming.selected_vesta = incoming.vesta.clone();
    }
    Ok(())
}

// Witness synthesis evaluates the exact production gadgets. Its outputs are not admission
// flags: the installed terminal A must rederive every predicate and all proof obligations.
#[derive(Clone)]
struct Evaluation {
    first: FirstCircuit,
    evidence: bool,
}
impl Circuit<Fp> for Evaluation {
    type Config = StageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = usize;
    fn params(&self) -> usize {
        if self.evidence && self.first.plan.incoming.is_some() {
            107
        } else {
            1
        }
    }
    fn without_witnesses(&self) -> Self {
        Self {
            first: self.first.without_witnesses(),
            evidence: self.evidence,
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StageConfig {
        Self::configure_with_params(meta, 1)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, n: usize) -> StageConfig {
        let verifier = VerifierConfig::configure_serialized_foreign(meta, SOURCE_RANGE_BUSES)
            .expect("fixed Archive evaluation profile");
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let public = meta.instance_column(n);
        meta.enable_equality(public);
        StageConfig {
            verifier,
            bytes,
            public,
        }
    }
    fn synthesize(
        &self,
        config: StageConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        let f = &self.first;
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "Archive exact production witness evaluation",
            |mut region| {
                let c = f.setup(&mut chip, &mut bytes, &mut region)?;
                if self.evidence {
                    let proof = f.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &f.source.original.q[2].proof,
                    )?;
                    let q = crate::a_relation::verify_q(
                        &mut chip,
                        &mut region,
                        f.plan.context.operation(),
                        2,
                        &c.q[2],
                        &proof,
                    )?;
                    let bundle = crate::a_relation::bind_signature_q(
                        &mut chip,
                        &mut region,
                        f.plan.context.operation(),
                        2,
                        &f.plan.signatures[1],
                        &q,
                    )?;
                    let (valid, incoming) = f.evidence(&mut chip, &mut region, &c, &bundle)?;
                    let mut out = vec![valid.word().clone()];
                    if let Some(incoming) = &incoming {
                        let t = c.transport.as_ref().ok_or(LayoutError::Synthesis)?;
                        out.extend(t.public().fields().iter().cloned());
                        let words = chip.uint().glue().witnesses(
                            &mut region,
                            &[Fp::ONE, Fp::ZERO, Fp::ZERO].map(Value::known),
                        )?;
                        let accept = ModeCells::constrain(
                            chip.uint().glue(),
                            &mut region,
                            &words.try_into().map_err(|_| LayoutError::Synthesis)?,
                        )?;
                        let selected = crate::a_relation::select_incoming(
                            &mut chip,
                            &mut region,
                            incoming,
                            &[accept.clone(), accept],
                            c.pc.as_slice()
                                .try_into()
                                .map_err(|_| LayoutError::Synthesis)?,
                        )?;
                        out.extend(pallas_fields(&selected));
                        out.extend(t.vesta().words());
                    }
                    Ok(out)
                } else {
                    let p = f.pallas(&mut chip, &mut region, f.pallas.as_ref())?;
                    Ok(vec![f.plan.context.digest(
                        &mut chip,
                        &mut region,
                        &c.context(),
                        &p,
                    )?])
                }
            },
        )?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, row)?;
        }
        Ok(())
    }
}
fn assigned_outputs(circuit: &Evaluation) -> Result<Vec<Fp>, Error> {
    use iroha_plonk::cs::Any;
    let n = circuit.params();
    let compiled = iroha_plonk::frontend::synthesize(circuit, 16, Some(&[vec![Fp::ZERO; n]]))
        .map_err(|_| Error::Prover)?;
    let permutation = compiled.tables.permutation();
    let advice = compiled.tables.advice().ok_or(Error::Prover)?;
    let start = permutation
        .columns()
        .iter()
        .position(|c| *c.column_type() == Any::Instance)
        .ok_or(Error::Prover)?;
    let mut out = Vec::with_capacity(n);
    for row in 0..n {
        let mut cell = permutation.mapping(start, row).ok_or(Error::Prover)?;
        let mut value = None;
        let mut visited = 0;
        while cell != (start, row) {
            visited += 1;
            if visited > permutation.columns().len() * compiled.tables.n() {
                return Err(Error::Prover);
            }
            let column = permutation.columns()[cell.0];
            if matches!(column.column_type(), Any::Advice) {
                let actual = *advice
                    .get(column.index())
                    .and_then(|c| c.get(cell.1))
                    .ok_or(Error::Prover)?;
                if value.is_some_and(|previous| previous != actual) {
                    return Err(Error::Input);
                }
                value = Some(actual);
            }
            cell = permutation.mapping(cell.0, cell.1).ok_or(Error::Prover)?;
        }
        out.push(value.ok_or(Error::Prover)?);
    }
    Ok(out)
}
fn evaluate_evidence(prepared: &Prepared) -> Result<Vec<Fp>, Error> {
    assigned_outputs(&Evaluation {
        first: First {
            source: prepared.source.clone(),
            plan: prepared.plan.clone(),
            pallas: prepared.source.predecessor.pallas.clone(),
            fold: vec![],
            known: true,
        }
        .circuit(),
        evidence: true,
    })
}
fn context_digest(first: &First) -> Result<Fp, Error> {
    assigned_outputs(&Evaluation {
        first: first.circuit(),
        evidence: false,
    })?
    .first()
    .copied()
    .ok_or(Error::Input)
}
fn trivial_vesta_words() -> Result<Vec<Fp>, Error> {
    let g = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .map_err(|_| Error::Artifact)?;
    vesta_words(&FoldInput::from_normalized(g, 16, [Fp::ONE; K]).map_err(|_| Error::Artifact)?)
}
fn internal_public(digest: Fp, part: &FoldInput<Eq>) -> Result<Vec<Fp>, Error> {
    let mut out = vec![digest, Fp::from(u64::from(part.source_k()))];
    out.extend(vesta_words(part)?);
    let trivial = trivial_vesta_words()?;
    out.extend(&trivial);
    out.extend(&trivial);
    out.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    out.extend(&trivial[..4]);
    Ok(out)
}
fn first_public(first: &First) -> Result<Vec<Fp>, Error> {
    internal_public(context_digest(first)?, &first.source.part)
}
fn push_pallas(out: &mut Vec<Fp>, p: &FoldInput<Ep>) -> Result<(), Error> {
    let (x, y) = Option::<(Fp, Fp)>::from(p.g().coordinates()).ok_or(Error::Input)?;
    out.extend([x, y]);
    for u in p.challenges() {
        out.extend(foreign_limbs(u).map(Fp::from_u128));
    }
    Ok(())
}
fn continued_digest(
    previous: Fp,
    variant: Fp,
    stage: usize,
    p: &AccumulatorT<Ep>,
    v: &AccumulatorT<Eq>,
    next: &AccumulatorT<Ep>,
) -> Result<Fp, Error> {
    let mut out = vec![
        Fp::ONE,
        variant,
        Fp::from(u64::try_from(stage + 1).map_err(|_| Error::Input)?),
        previous,
    ];
    out.push(Fp::from(16));
    push_pallas(&mut out, &p.as_input())?;
    out.extend(vesta_words(&v.as_input())?);
    out.push(Fp::from(16));
    push_pallas(&mut out, &next.as_input())?;
    Ok(hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &out))
}
fn continuation_public(c: &Continuation) -> Result<Vec<Fp>, Error> {
    if c.history.len() + 1 != c.plan.stage() {
        return Err(Error::Input);
    }
    if c.plan.is_terminal() {
        let mut out = vec![
            terminal_digest(
                &c.first.source.original.state.after.lineage,
                &c.pallas.as_input(),
            )?,
            Fp::from(16),
        ];
        out.extend(vesta_words(&c.vesta.as_input())?);
        out.extend(vesta_words(&c.first.source.predecessor.vesta.as_input())?);
        if c.first.plan.incoming.is_some() {
            out.extend(vesta_words(&c.first.source.incoming.vesta)?);
            out.extend(c.first.source.incoming.modes[2].map(|b| Fp::from(u64::from(b))));
            let (x, y) = Option::<(Fq, Fq)>::from(
                c.first
                    .source
                    .incoming
                    .vesta_correction
                    .to_affine()
                    .coordinates(),
            )
            .ok_or(Error::Input)?;
            out.extend(foreign_limbs(&x).map(Fp::from_u128));
            out.extend(foreign_limbs(&y).map(Fp::from_u128));
        } else {
            let trivial = trivial_vesta_words()?;
            out.extend(&trivial);
            out.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
            out.extend(&trivial[..4]);
        }
        return Ok(out);
    }
    let mut digest = context_digest(&c.first)?;
    for (index, (p, v)) in c.history.iter().enumerate() {
        let next = c.history.get(index + 1).map_or(&c.carried, |p| &p.0);
        digest = continued_digest(
            digest,
            c.first.plan.context.schema()[1],
            index + 1,
            p,
            v,
            next,
        )?;
    }
    digest = continued_digest(
        digest,
        c.first.plan.context.schema()[1],
        c.plan.stage(),
        &c.carried,
        &c.vesta,
        &c.pallas,
    )?;
    internal_public(digest, &c.vesta.as_input())
}
fn terminal_digest(public: &[Fp; 18], pallas: &FoldInput<Ep>) -> Result<Fp, Error> {
    let mut out = public.to_vec();
    push_pallas(&mut out, pallas)?;
    if out.len() != 52 {
        return Err(Error::Input);
    }
    Ok(hash_with_domain(crate::a_relation::LINEAGE_DOMAIN, &out))
}
impl Prepared {
    fn verify_sources(&self, budget: MemoryBudget) -> Result<(), Error> {
        let fixed = self
            .plan
            .context
            .operation()
            .omega()
            .ok_or(Error::Artifact)?;
        let pred = &self.source.predecessor;
        pred.pallas
            .decide(&self.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        pred.vesta
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let public = omega_instances(
            terminal_digest(
                &self.source.original.state.before.lineage,
                &pred.pallas.as_input(),
            )?,
            &pred.vesta,
        )?;
        verify_full(
            &self.plan.pallas,
            fixed.binding(),
            &self.plan.predecessor_key,
            &public,
            &pred.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        pred.opening
            .decide(&self.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        let same_p = |a: &FoldInput<Ep>, b: &FoldInput<Ep>| {
            a.source_k() == b.source_k() && a.g() == b.g() && a.challenges() == b.challenges()
        };
        let actual = opening_pallas(
            &self.plan.pallas,
            fixed.binding(),
            &self.plan.predecessor_key,
            &public,
            &pred.proof,
            budget,
        )?;
        if !same_p(&actual, &pred.opening) {
            return Err(Error::Proof);
        }
        for (index, q) in self.source.original.q.iter().enumerate() {
            let fixed = self
                .plan
                .context
                .operation()
                .q(index)
                .ok_or(Error::Artifact)?;
            verify_full(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &q.instances,
                &q.proof,
                budget,
            )
            .map_err(|_| Error::Proof)?;
            self.source.q_openings[index]
                .decide(&self.plan.pallas, budget)
                .map_err(|_| Error::Proof)?;
            let actual = opening_pallas(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &q.instances,
                &q.proof,
                budget,
            )?;
            if !same_p(&actual, &self.source.q_openings[index]) {
                return Err(Error::Proof);
            }
        }
        self.source
            .part
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let actual = q_sigma_part(
            &self.source.original.q[0],
            self.plan.context.operation().frame().part_source_k(),
        )?;
        if actual.source_k() != self.source.part.source_k()
            || actual.g() != self.source.part.g()
            || actual.challenges() != self.source.part.challenges()
        {
            return Err(Error::Proof);
        }
        for p in &self.source.incoming.selected {
            p.decide(&self.plan.pallas, budget)
                .map_err(|_| Error::Proof)?;
        }
        self.source
            .incoming
            .selected_vesta
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        // Repeat the independent original P/opening/V decides and derive the
        // exact selection again on every restore. A carried mode/correction
        // cannot stand in for these source obligations.
        let incoming = &self.source.incoming;
        let mut reselected = incoming.clone();
        select_native_incoming(
            &self.plan,
            &self.source.original,
            self.source.evidence_valid,
            &mut reselected,
            budget,
        )?;
        if incoming.modes != reselected.modes
            || incoming.pallas_corrections != reselected.pallas_corrections
            || incoming.vesta_correction != reselected.vesta_correction
            || incoming
                .selected
                .iter()
                .zip(&reselected.selected)
                .any(|(a, b)| !same_p(a, b))
            || incoming.selected_vesta.source_k() != reselected.selected_vesta.source_k()
            || incoming.selected_vesta.g() != reselected.selected_vesta.g()
            || incoming.selected_vesta.challenges() != reselected.selected_vesta.challenges()
        {
            return Err(Error::Proof);
        }
        check_original_shapes(&self.plan, &self.source.original)?;
        check_sigma_tape(&self.plan, &self.source.original)?;
        let fresh = evaluate_evidence(self)?;
        if field_bit(fresh[0])? != self.source.evidence_valid {
            return Err(Error::Proof);
        }
        if self.plan.incoming.is_some() {
            let original = incoming_from_fields(&fresh[1..])?;
            if original.public != incoming.public
                || !same_p(&original.pallas, &incoming.pallas)
                || !same_p(&original.opening, &incoming.opening)
                || original.vesta.g() != incoming.vesta.g()
                || original.vesta.challenges() != incoming.vesta.challenges()
            {
                return Err(Error::Proof);
            }
            let v = AccumulatorT::new(*incoming.vesta.g(), *incoming.vesta.challenges())
                .map_err(|_| Error::Input)?;
            let public = omega_instances(terminal_digest(&incoming.public, &incoming.pallas)?, &v)?;
            let raw = &self.source.original.incoming.omega;
            let length = fixed.proof_length();
            let valid = raw.len() == 320 + length + 1088
                && verify_full(
                    &self.plan.pallas,
                    fixed.binding(),
                    &self.plan.predecessor_key,
                    &public,
                    &raw[320..320 + length],
                    budget,
                )
                .is_ok();
            if incoming.modes[0][0] && !valid {
                return Err(Error::Proof);
            }
        }
        Ok(())
    }
}

fn omega_instances(context: Fp, vesta: &AccumulatorT<Eq>) -> Result<Vec<Vec<Fq>>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(vesta.g().coordinates()).ok_or(Error::Input)?;
    let digest = Option::<Fq>::from(Fq::from_repr(context.to_repr())).ok_or(Error::Input)?;
    let challenges = vesta
        .challenges()
        .iter()
        .map(|v| Option::<Fq>::from(Fq::from_repr(v.to_repr())).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![vec![digest], vec![x, y], challenges])
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

fn prove_a(
    plan: &Plan,
    key: &ProvingKey<Eq>,
    circuit: &StageCircuit,
    public: &[Fp],
    randomness: ProverRandomness<'_>,
    config: ProverConfig,
    budget: MemoryBudget,
) -> Result<(Vec<u8>, FoldInput<Eq>), Error> {
    let public = [public.to_vec()];
    let witness = Witness::from_circuit(key, circuit, &public).map_err(|_| Error::Prover)?;
    let output = create_proof_owned_with_claim(&plan.vesta, key, witness, randomness, config)
        .map_err(|_| Error::Prover)?;
    verify_full(
        &plan.vesta,
        key.binding(),
        key.vk(),
        &public,
        &output.proof,
        budget,
    )
    .map_err(|_| Error::Proof)?;
    let opening = opening_vesta(
        &plan.vesta,
        key.binding(),
        key.vk(),
        &public,
        &output.proof,
        budget,
    )?;
    Ok((output.proof, opening))
}
