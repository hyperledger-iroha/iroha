//! Genuine fixed native Receive eleven-A/ten-W composition from exact original tapes.
//!
//! All five results are derived by genuine typed owners, with hard own authorization,
//! every incoming original and all separate native obligations retained. Runtime
//! sessions use installed keys and no witness-selected profile. Native pre-Advance
//! G1 owns canonical original credentials/Request/Payment/policies/checks.
//! Original proving keys are imported against the same unknown compiled source used
//! by runtime proofs, with exact installed VK equality at every A/W stage. No
//! installation view can construct Prepared, an accepted opening or a checkpoint.
//! TODO: qualify this unmeasured schedule using genuine compact Omega artifacts
//! satisfying the mandatory 10,000-byte Payment cap, then mount the complete
//! producer catalog/final Omega before foreign wallet open.

#[path = "receive/checkpoint.rs"]
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
    verifier::{VerifierChip, VerifierConfig, VerifierPlan},
};

use super::super::{
    AProofPlan, IncomingVestaCells, LineagePublicCells, ProofMessageCells, SigmaBindingCells,
    VestaClaimCells,
    context::{ContextIncomingProof, ContextInputs, ContextPlan, ContextPredecessor, ContextState},
    incoming_transport::{IncomingTransportCells, IncomingTransportPlan},
    own::OwnPolicy,
    receive::{
        ReceiveObjectInputs, ReceiveObjectSources, ReceiveObjects, ReceiveProofInputs,
        ReceiveProofSources, ReceiveSignedObjects, ReceiveStageInputs, ReceiveStagePlan,
        ReceiveStageWitness,
        authorization::{
            ReceiveAuthorizationObjects, ReceiveAuthorizationSources, ReceiveSignatureInputs,
        },
        maps,
    },
    results::{ReceiveResultClaims, ReceiveResultTag},
    schedule::OperationTask,
    split::{SplitPlan, WCircuit, WKey, close_first},
    verify_predecessor, verify_sigma,
};
use crate::{
    admin_sigma::StateWitness,
    omega::OmegaWitness,
    operation_relation::{
        map_effects::{InsertCells, ReceiveMapWitness},
        objects::ObjectKind,
        state::StateCells,
        statement::StatementCells,
    },
    q_signature::QSignaturePlan,
    tree::{IndexedInsert, IndexedLeaf},
};

#[cfg(test)]
#[path = "receive/tests.rs"]
mod tests;

/// Fixed native source profile; private inputs cannot choose another range-bus count.
pub const SOURCE_RANGE_BUSES: usize = 4;
const DESCRIPTOR_MAX_BYTES: usize = 1 << 20;
const VERIFYING_KEY_MAX_BYTES: usize = 1 << 18;
/// Frozen initial eleven-A/ten-W schedule; actual synthesis/proof qualification remains required.
pub const A_STAGE_COUNT: usize = 11;
/// Exact internal wrapper count; terminal A cannot become another W.
pub const W_STAGE_COUNT: usize = A_STAGE_COUNT - 1;
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
        write!(f, "native Receive: {self:?}")
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

/// Exact canonical receiver state openings and Receive statement.
#[derive(Clone, Copy, Debug)]
pub struct ReceiveState {
    /// Own authenticated predecessor core/rest/public.
    pub before: StateWitness,
    /// Exact committed successor core/rest/public.
    pub after: StateWitness,
    /// Exact own statement26, identical to sigma's transcript.
    pub statement: [Fp; 26],
}
/// Original incoming Send package. No decoded proof verdict is an input.
#[derive(Clone, Debug)]
pub struct IncomingInput {
    /// Exact original unframed public320/proof/P/V lineage carrier.
    pub omega: Vec<u8>,
    /// Exact original unframed Send sigma carrier.
    pub sigma: Vec<u8>,
    /// Exact original incoming Send statement fields.
    pub statement: [Fp; 26],
}
/// One exact authenticated depth32 indexed-tree search route.
#[derive(Clone, Copy, Debug)]
pub struct Search {
    /// Opened authenticated indexed leaf.
    pub leaf: IndexedLeaf<Fp>,
    /// Exact leaf index, including sentinel slot zero.
    pub slot: u32,
    /// All32 leaf-to-root siblings.
    pub siblings: [Fp; 32],
}
/// Exact committed OQ3 consumed-map transition and permanent credit record.
#[derive(Clone, Copy, Debug)]
pub struct Effects {
    /// Actual insertion or fixed-layout no-op paths.
    pub consumed: IndexedInsert<Fp>,
    /// Actual structural inserted key, bound to credit on accept.
    pub inserted_key: Fp,
    /// Actual structural value, bound to descriptor on accept.
    pub inserted_value: Fp,
    /// Actual committed insertion/no-op branch, not an acceptance verdict.
    pub insert: bool,
    /// Preserve-first credit-digest record paths.
    pub credit: IndexedInsert<Fp>,
}
/// Post-Advance originals converted by the native canonical G1 owner.
/// G1 separately owns native recipient/payer credentials, original Request,
/// Payment, policy, recorded list/gap and complete checks before mutation.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Own source-bound state openings and statement.
    pub state: ReceiveState,
    /// Exact original own sigma, bound by Q0 slot0.
    pub sigma: Vec<u8>,
    /// Request, payer credential, Send receipt, Payment; then hard own current
    /// credential/Enrollment/receipt and original quoted receiver credential/Enrollment.
    /// The raw Omega and sigma occupy the separate fixed context slots4/5.
    pub objects: [Vec<u8>; 9],
    /// Actual incoming original proof package.
    pub incoming: IncomingInput,
    /// Unique credit search in predecessor consumed-credit map.
    pub nonmembership: Search,
    /// Mandatory exact Request-recorded history search. Zero or malformed pairs
    /// use the canonical relation's fixed (1,1) safe query, still authenticated.
    pub blacklist: Search,
    /// Actual committed and permanent map transitions.
    pub effects: Effects,
    /// Exact Q0(two sigma slots), Q1(hard own receipt/current/Enrollment), and
    /// Q2(soft Send receipt/Request, plus quoted credential/Enrollment when renewed).
    /// Every Q is verified fully and its original opening retained.
    pub q: [QInput; 3],
    /// Own hard predecessor proof and both transported claim originals.
    pub predecessor: PredecessorInput,
}

/// Immutable genuine eleven-A/ten-W Receive operation metadata.
/// This initial fixed schedule requires actual profile qualification before
/// mounting. Neither witnesses nor foreign inputs select another profile.
#[derive(Clone, Debug)]
pub struct Plan {
    context: ContextPlan,
    relation: ReceiveStagePlan,
    recorded_blacklist: bool,
    policy: OwnPolicy,
    signatures: [QSignaturePlan; 2],
    predecessor_key: VerifyingKey<Ep>,
    incoming: IncomingTransportPlan,
    omega_capacity: usize,
    sigma_capacity: usize,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin every actual Q/task/own/incoming obligation to the installed keys.
    /// # Errors
    /// Wrong fixed variant/catalog/schema/root or descriptor/parameter profile.
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
        if !matches!(
            operation.frame().variant(),
            Variant::Receive | Variant::ReceiveRenewed
        ) || operation.frame().part_source_k() != 16
            || operation.q_count() != 3
            || !operation.frame().has_predecessor()
            || !operation.frame().has_incoming()
            || operation.sigma.slot_count() != 2
        {
            return Err(Error::Artifact);
        }
        operation
            .sigma
            .class(0)
            .ok_or(Error::Artifact)?
            .selector_key_digest(10 + u8::from(recorded_blacklist))
            .ok_or(Error::Artifact)?;
        pallas.require_k(16).map_err(|_| Error::Artifact)?;
        vesta.require_k(16).map_err(|_| Error::Artifact)?;
        let fixed = operation.omega().ok_or(Error::Artifact)?;
        if predecessor_key.descriptor_digest() != fixed.binding().digest() {
            return Err(Error::Artifact);
        }
        predecessor_key
            .kagemusha_digest(fixed.binding())
            .map_err(|_| Error::Artifact)?;
        let incoming = IncomingTransportPlan::new(&operation).map_err(|_| Error::Artifact)?;
        if omega_capacity < incoming.payload_length().map_err(|_| Error::Artifact)?
            || sigma_capacity
                < operation
                    .sigma
                    .class(1)
                    .ok_or(Error::Artifact)?
                    .verifier()
                    .proof_length()
            || omega_capacity > 10_000
            || sigma_capacity > 10_000
        {
            return Err(Error::Artifact);
        }
        let operation_variant = operation.frame().variant();
        let expected_signatures = ReceiveStagePlan::signature_schemas(operation_variant, policy)
            .map_err(|_| Error::Artifact)?;
        if signatures
            .iter()
            .zip(&expected_signatures)
            .any(|(actual, expected)| actual.slots() != expected.slots())
        {
            return Err(Error::Artifact);
        }
        let (partitions, tasks) = operation_schedule();
        let context = ContextPlan::with_schedule(
            operation,
            partitions,
            Some(0),
            ReceiveStagePlan::context_specs(operation_variant, omega_capacity, sigma_capacity)
                .map_err(|_| Error::Artifact)?,
        )
        .and_then(|context| context.with_operation_tasks(tasks))
        .map_err(|_| Error::Artifact)?;
        let relation =
            ReceiveStagePlan::new(context.clone(), policy).map_err(|_| Error::Artifact)?;
        Ok(Self {
            context,
            relation,
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
    /// Complete frozen Q/task/object inventory.
    pub const fn context(&self) -> &ContextPlan {
        &self.context
    }
    /// Verify own predecessor/allQ proofs, preserve all original incoming tapes,
    /// derive result witnesses by genuine source synthesis, then select incoming
    /// claims using their actual complete native decides and the proved Q0 mode.
    /// No result or native verdict bit can be supplied by the caller.
    /// # Errors
    /// Wrong originals/schema, failed hard proof/claim, unmatched global mode,
    /// impossible correction or current fixed schedule synthesis bounds.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Prepared, Error> {
        check_payment_original_sizes(input.incoming.omega.len(), input.incoming.sigma.len())?;
        if input.objects[3].len() != 163
            || input.incoming.omega.len() > self.omega_capacity
            || input.incoming.sigma.len() > self.sigma_capacity
            || input
                .incoming
                .omega
                .len()
                .checked_add(input.incoming.sigma.len())
                .is_none_or(|n| n > MAX_PAYMENT_ORIGINAL_BYTES)
        {
            return Err(Error::Input);
        }
        for (i, kind) in signed_kinds().into_iter().enumerate() {
            let at = if i < 3 { i } else { i + 1 };
            if input.objects[at].len() != kind.body_len() + 64 {
                return Err(Error::Input);
            }
        }
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
        let key_digest = self
            .predecessor_key
            .kagemusha_digest(fixed.binding())
            .map_err(|_| Error::Artifact)?;
        if input.state.before.lineage[17] != key_digest
            || input.state.after.lineage[17] != key_digest
        {
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
        let part = q_sigma_part(&input.q[0], 16)?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        let trivial_p = AccumulatorT::<Ep>::trivial(&self.pallas, budget)
            .map_err(|_| Error::Proof)?
            .as_input();
        let trivial_v = AccumulatorT::<Eq>::trivial(&self.vesta, budget)
            .map_err(|_| Error::Proof)?
            .as_input();
        let source = Sources {
            original: input,
            q_openings,
            part,
            predecessor: Predecessor {
                proof: Vec::new(),
                pallas,
                opening,
                vesta,
            },
            results: [false; 5],
            incoming: NativeIncoming {
                public: [Fp::ZERO; 18],
                pallas: trivial_p.clone(),
                opening: trivial_p.clone(),
                vesta: trivial_v.clone(),
                selected: [trivial_p.clone(), trivial_p.clone()],
                selected_vesta: trivial_v.clone(),
                modes: [[false, true, false]; 4],
                pallas_corrections: [Ep::from(*trivial_p.g()); 2],
                vesta_correction: Eq::from(*trivial_v.g()),
            },
        };
        let mut prepared = Prepared {
            plan: self.clone(),
            source: Arc::new(source),
        };
        let original_predecessor = prepared.source.original.predecessor.proof.clone();
        Arc::get_mut(&mut prepared.source)
            .ok_or(Error::Input)?
            .predecessor
            .proof = original_predecessor;
        let mut results = [false; 5];
        let mut incoming = None;
        for tag in ReceiveResultTag::ALL {
            let values = evaluate_result(&prepared, tag)?;
            results[tag as usize - 1] = field_bit(values[0])?;
            if tag == ReceiveResultTag::Proofs {
                incoming = Some(incoming_from_fields(&values[1..])?);
            }
        }
        let mut incoming = incoming.ok_or(Error::Input)?;
        select_native_incoming(
            self,
            &prepared.source.original,
            &results,
            &mut incoming,
            budget,
        )?;
        let source = Arc::get_mut(&mut prepared.source).ok_or(Error::Input)?;
        source.results = results;
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
    results: [bool; 5],
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
// Circuit-only views never manufacture an accepted native claim or Prepared.
#[derive(Clone)]
struct CircuitPredecessor {
    proof: Vec<u8>,
    pallas: Option<FoldInput<Ep>>,
    vesta: Option<AccumulatorT<Eq>>,
}
#[derive(Clone)]
struct CircuitIncoming {
    opening: Option<FoldInput<Ep>>,
    modes: [[bool; 3]; 4],
    pallas_corrections: [Option<Ep>; 2],
    vesta_correction: Option<Eq>,
}
#[derive(Clone)]
struct CircuitSources {
    original: Inputs,
    predecessor: CircuitPredecessor,
    results: [bool; 5],
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
        let s = &self.source;
        FirstCircuit {
            source: Arc::new(CircuitSources {
                original: s.original.clone(),
                predecessor: CircuitPredecessor {
                    proof: s.predecessor.proof.clone(),
                    pallas: Some(s.predecessor.pallas.as_input()),
                    vesta: Some(s.predecessor.vesta.clone()),
                },
                results: s.results,
                incoming: CircuitIncoming {
                    opening: Some(s.incoming.opening.clone()),
                    modes: s.incoming.modes,
                    pallas_corrections: s.incoming.pallas_corrections.map(Some),
                    vesta_correction: Some(s.incoming.vesta_correction),
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
        let incoming = operation.sigma.class(1).ok_or(Error::Artifact)?;
        let pred = operation.omega().ok_or(Error::Artifact)?;
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
        let insertion = IndexedInsert {
            leaf: search.leaf,
            leaf_slot: 0,
            leaf_siblings: search.siblings,
            slot: 1,
            slot_siblings: [Fp::ZERO; 32],
        };
        // Fixed original object lengths come from the same immutable context schema.
        let objects = core::array::from_fn(|i| vec![0; operation_object_length(i)]);
        let original = Inputs {
            state: ReceiveState {
                before: state,
                after: state,
                statement: [Fp::ZERO; 26],
            },
            sigma: vec![0; own.verifier().proof_length()],
            objects,
            incoming: IncomingInput {
                omega: vec![
                    0;
                    plan.incoming
                        .payload_length()
                        .map_err(|_| Error::Artifact)?
                ],
                sigma: vec![0; incoming.verifier().proof_length()],
                statement: [Fp::ZERO; 26],
            },
            nonmembership: search,
            blacklist: search,
            effects: Effects {
                consumed: insertion,
                inserted_key: Fp::ZERO,
                inserted_value: Fp::ZERO,
                insert: false,
                credit: insertion,
            },
            q,
            predecessor: PredecessorInput {
                proof: vec![0; pred.proof_length()],
                pallas: [0; 544],
                vesta: [0; 544],
            },
        };
        Ok(Self {
            source: Arc::new(CircuitSources {
                original,
                predecessor: CircuitPredecessor {
                    proof: vec![0; pred.proof_length()],
                    pallas: None,
                    vesta: None,
                },
                results: [false; 5],
                incoming: CircuitIncoming {
                    opening: None,
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
    match index {
        0 => ObjectKind::Request.body_len() + 64,
        1 | 4 | 7 => ObjectKind::Credential.body_len() + 64,
        2 | 6 => ObjectKind::Receipt.body_len() + 64,
        3 => 163,
        5 | 8 => ObjectKind::Certificate.body_len() + 64,
        _ => unreachable!("fixed nine-object source array"),
    }
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
    incoming_statement: crate::operation_relation::incoming_statement::IncomingStatementCells,
    transport: IncomingTransportCells,
    sigma: [SigmaBindingCells; 2],
    objects: ReceiveObjects,
    auth: ReceiveAuthorizationObjects,
    signed: ReceiveSignedObjects,
    proofs: ReceiveProofSources,
    context_objects: Vec<crate::a_relation::context::ContextObjectCells>,
    q: Vec<Vec<Vec<ScalarCells<Ep>>>>,
    pp: FoldInputCells<Ep>,
    pv: VestaClaimCells,
    modes: [ModeCells<Fp>; 4],
    pc: [iroha_plonk_gadgets::ecc::NonIdentityPoint<Fp>; 2],
    vc: [[ScalarCells<Ep>; 2]; 1],
    results: ReceiveResultClaims,
}
impl Cells {
    fn context(&self) -> ContextInputs<'_> {
        ContextInputs {
            own_statement: &self.statement,
            incoming_statement: Some(&self.incoming_statement),
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
            incoming: Some(crate::a_relation::context::ContextIncoming {
                public: self.transport.public(),
                pallas: self.transport.pallas(),
                vesta: self.transport.vesta(),
                proof: ContextIncomingProof::ReceiveActive,
            }),
            q_instances: &self.q,
            objects: &self.context_objects,
            modes: &self.modes,
            pallas_corrections: &self.pc,
            vesta_corrections: &self.vc,
            receive_results: Some(&self.results),
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
        let fields = chip
            .uint()
            .glue()
            .witnesses(region, &w.lineage.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        Ok((
            state,
            LineagePublicCells::constrain(&mut chip.uint(), region, &fields)?,
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
    fn insertion(
        &self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        w: &IndexedInsert<Fp>,
    ) -> Result<InsertCells, LayoutError> {
        let low = self.search(
            uint,
            region,
            &Search {
                leaf: w.leaf,
                slot: w.leaf_slot,
                siblings: w.leaf_siblings,
            },
        )?;
        let index = uint
            .glue()
            .witness(region, self.value(Fp::from(u64::from(w.slot))))?;
        let siblings = uint
            .glue()
            .witnesses(region, &w.slot_siblings.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        Ok(InsertCells {
            low,
            slot: PathCells::from_words(uint, region, &index, siblings)?,
        })
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
        let own = chip
            .uint()
            .glue()
            .witnesses(region, &source.state.statement.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let statement = StatementCells::constrain_with_verifier(
            chip,
            region,
            self.plan.context.operation().frame().variant(),
            &own,
        )?;
        let fields = chip
            .uint()
            .glue()
            .witnesses(region, &source.incoming.statement.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let lanes = chip.operation_lanes()?;
        let incoming_statement =
            crate::operation_relation::incoming_statement::IncomingStatementCells::constrain(
                &mut UintChip::new(lanes.glue, lanes.range),
                lanes.hash,
                region,
                Variant::Send,
                &fields,
            )?;
        let omega = ActiveBytes::assign(
            &mut chip.uint(),
            bytes,
            region,
            self.plan.omega_capacity,
            &if self.known {
                Value::known(source.incoming.omega.clone())
            } else {
                Value::unknown()
            },
            &self.plan.incoming.active_segments()?,
        )?;
        let transport =
            self.plan
                .incoming
                .decode_active(chip, region, &omega, next.omega_key_digest())?;
        let own_raw = frame(&source.sigma)?;
        let run = bytes.run(
            region,
            &own_raw.iter().map(|b| self.value(*b)).collect::<Vec<_>>(),
            &iroha_plonk_gadgets::bytes::chunk_segments(0, own_raw.len()),
            &[SegmentSpec::little(0, 4)],
        )?;
        let own_index = chip.uint().glue().constant(
            region,
            Fp::from(10 + u64::from(self.plan.recorded_blacklist)),
        )?;
        let own = SigmaBindingCells::from_run(chip, region, &statement, own_index, &run)?;
        let proof_bytes = self
            .plan
            .context
            .operation()
            .sigma
            .class(1)
            .ok_or(LayoutError::Synthesis)?
            .verifier()
            .proof_length();
        let raw = ActiveBytes::assign(
            &mut chip.uint(),
            bytes,
            region,
            self.plan.sigma_capacity,
            &if self.known {
                Value::known(source.incoming.sigma.clone())
            } else {
                Value::unknown()
            },
            &SigmaBindingCells::incoming_segments(proof_bytes)?,
        )?;
        let actual = source.q[0]
            .instances
            .get(2)
            .and_then(|c| c.get(1))
            .ok_or(LayoutError::Synthesis)?;
        let index = Fp::from_repr(actual.to_repr())
            .into_option()
            .ok_or(LayoutError::Synthesis)?;
        let incoming_index = chip.uint().glue().witness(region, self.value(index))?;
        let incoming = SigmaBindingCells::from_incoming_active(
            chip,
            region,
            &incoming_statement,
            incoming_index,
            &raw,
            proof_bytes,
        )?;
        let sigma = [own, incoming];
        let original = source
            .objects
            .each_ref()
            .map(|raw| raw.iter().map(|b| self.value(*b)).collect::<Vec<_>>());
        let objects = ReceiveObjects::decode(
            chip,
            bytes,
            region,
            self.plan.policy,
            ReceiveObjectSources {
                request: &original[0],
                payer: &original[1],
                receipt: &original[2],
                payment: &original[3],
            },
            ReceiveObjectInputs {
                own: &statement,
                receiver: &pred,
                incoming: &transport,
                sigma: &sigma[1],
            },
        )?;
        let signed = objects.signed_sources().clone();
        let proofs = ReceiveProofSources::from_active(&transport, &sigma[1])?;
        let auth = ReceiveAuthorizationObjects::decode(
            chip,
            bytes,
            region,
            self.plan.context.operation().frame().variant(),
            ReceiveAuthorizationSources {
                current: &original[4],
                certificate: &original[5],
                receipt: &original[6],
                quoted: [&original[7], &original[8]],
            },
        )?;
        let mut context_objects = objects.context().to_vec();
        context_objects.extend_from_slice(auth.context());
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
        let modes = self
            .source
            .incoming
            .modes
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
            .collect::<Result<Vec<_>, LayoutError>>()?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let pc = self
            .source
            .incoming
            .pallas_corrections
            .iter()
            .map(|g| chip.witness_point(region, g.map_or(Value::unknown(), |v| self.value(v))))
            .collect::<Result<Vec<_>, LayoutError>>()?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let coordinates = self
            .source
            .incoming
            .vesta_correction
            .map(|g| {
                Option::<(Fq, Fq)>::from(g.to_affine().coordinates()).ok_or(LayoutError::Synthesis)
            })
            .transpose()?;
        let vc = [[
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
        ]];
        let incoming_opening = self.pallas(chip, region, self.source.incoming.opening.as_ref())?;
        let results = ReceiveResultClaims::assign(
            chip.uint().glue(),
            region,
            self.plan
                .context
                .receive_results()
                .ok_or(LayoutError::Synthesis)?,
            self.source.results.map(|b| self.value(b)),
        )?
        .with_opening(&incoming_opening)?;
        Ok(Cells {
            old,
            new,
            pred,
            next,
            statement,
            incoming_statement,
            transport,
            sigma,
            objects,
            auth,
            signed,
            proofs,
            context_objects,
            q,
            pp,
            pv,
            modes,
            pc,
            vc,
            results,
        })
    }
    fn task(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        stage: usize,
        c: &Cells,
        own_signature: Option<&crate::a_relation::SignatureQCells>,
        incoming_signature: Option<&crate::a_relation::SignatureQCells>,
    ) -> Result<(), LayoutError> {
        let tasks = self
            .plan
            .context
            .operation_tasks(stage)
            .ok_or(LayoutError::Synthesis)?;
        let absent = if tasks.contains(&OperationTask::ReceiveNonmembership) {
            Some(self.search(
                &mut chip.uint(),
                region,
                &self.source.original.nonmembership,
            )?)
        } else {
            None
        };
        let blacklist = if tasks.contains(&OperationTask::ReceiveBlacklist) {
            Some(self.search(&mut chip.uint(), region, &self.source.original.blacklist)?)
        } else {
            None
        };
        let effects = if tasks.contains(&OperationTask::ReceiveEffects) {
            let e = &self.source.original.effects;
            Some(ReceiveMapWitness {
                consumed: self.insertion(&mut chip.uint(), region, &e.consumed)?,
                inserted_key: chip
                    .uint()
                    .glue()
                    .witness(region, self.value(e.inserted_key))?,
                inserted_value: chip
                    .uint()
                    .glue()
                    .witness(region, self.value(e.inserted_value))?,
                insert: chip.uint().glue().boolean(region, self.value(e.insert))?,
                credit: self.insertion(&mut chip.uint(), region, &e.credit)?,
                payment_digest: c.objects.payment().payment().digest().clone(),
            })
        } else {
            None
        };
        let omega_key = if tasks.contains(&OperationTask::ReceiveProofs) {
            let fixed = self
                .plan
                .context
                .operation()
                .omega()
                .ok_or(LayoutError::Synthesis)?;
            Some(chip.constant_key(region, fixed, &self.plan.predecessor_key)?)
        } else {
            None
        };
        let context = c.context();
        self.plan.relation.constrain_stage(
            chip,
            region,
            u32::try_from(stage).map_err(|_| LayoutError::BoundsFailure)?,
            ReceiveStageInputs {
                context: &context,
                objects: tasks
                    .contains(&OperationTask::ReceiveObjects)
                    .then_some(&c.objects),
                signed: tasks
                    .iter()
                    .any(|task| {
                        matches!(
                            task,
                            OperationTask::ReceiveSignatures | OperationTask::ReceiveBlacklist
                        )
                    })
                    .then_some(&c.signed),
                authorization: tasks
                    .iter()
                    .any(|task| {
                        matches!(
                            task,
                            OperationTask::ReceiveAuthorization
                                | OperationTask::ReceiveSignatures
                                | OperationTask::ReceiveOwnProof
                        )
                    })
                    .then_some(&c.auth),
                own_sigma: tasks
                    .contains(&OperationTask::ReceiveOwnProof)
                    .then_some(&c.sigma[0]),
            },
            ReceiveStageWitness {
                proofs: omega_key.as_ref().map(|omega_key| ReceiveProofInputs {
                    sources: &c.proofs,
                    omega_key,
                    own_sigma: &c.sigma[0],
                }),
                own_signatures: own_signature,
                incoming_signatures: incoming_signature,
                nonmembership: absent.as_ref(),
                blacklist: blacklist.as_ref(),
                effects: effects.as_ref(),
            },
        )?;
        Ok(())
    }
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
            .expect("fixed Receive source profile");
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
            || "Receive own hard predecessor",
            |mut region| {
                let c = self.setup(&mut chip, &mut bytes, &mut region)?;
                let fixed = self
                    .plan
                    .context
                    .operation()
                    .omega()
                    .ok_or(LayoutError::Synthesis)?;
                let key = chip.constant_key(&mut region, fixed, &self.plan.predecessor_key)?;
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
                self.task(&mut chip, &mut region, 0, &c, None, None)?;
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
            || "complete Receive source continuation",
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
                    let fixed = f
                        .plan
                        .context
                        .operation()
                        .omega()
                        .ok_or(LayoutError::Synthesis)?;
                    let key = chip.constant_key(&mut region, fixed, &f.plan.predecessor_key)?;
                    Some(c.transport.verify(
                        &mut chip,
                        &mut region,
                        f.plan.context.operation(),
                        &key,
                    )?)
                } else {
                    None
                };
                f.task(
                    &mut chip,
                    &mut region,
                    self.plan.stage(),
                    &c,
                    own_signature.as_ref(),
                    incoming_signature.as_ref(),
                )?;
                if let Some(incoming) = incoming.as_ref() {
                    // Even on a false verifier result, terminal folding consumes the exact
                    // full opening exported by the earlier same-source Proofs owner.
                    crate::a_relation::split::bind_claim(
                        &mut region,
                        c.results.opening()?,
                        &incoming.opening,
                    )?;
                }
                let selected = incoming
                    .as_ref()
                    .map(|incoming| {
                        crate::a_relation::select_incoming(
                            &mut chip,
                            &mut region,
                            incoming,
                            &[c.modes[0].clone(), c.modes[1].clone()],
                            &c.pc,
                        )
                    })
                    .transpose()?;
                let iv = incoming.as_ref().map(|_| IncomingVestaCells {
                    claim: c.transport.vesta().clone(),
                    mode: c.modes[2].clone(),
                    corrected: c.vc[0].clone(),
                });
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

/// Checked original source coupled to the fixed complete Receive plan.
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

/// Already installed eleven A and ten W proving artifacts for this exact fixed profile.
/// Authentication and genuine PK import remain the native package owner's responsibility.
#[derive(Clone)]
pub struct Prover {
    plan: Plan,
    a: [Arc<ProvingKey<Eq>>; A_STAGE_COUNT],
    w: [Arc<ProvingKey<Ep>>; W_STAGE_COUNT],
    wrappers: [WKey; W_STAGE_COUNT],
}
impl Prover {
    /// Import the fixed source A/W stages using only installed metadata.
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
        a: [OriginalArtifact<'_>; 11],
        w: [OriginalArtifact<'_>; 10],
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

    /// Import the complete fixed typed artifact set, never generating keys from a witness.
    /// # Errors
    /// Nonuniform A descriptors, wrong k/public schema, or wrong W stage/context identity.
    pub fn from_artifacts(
        plan: Plan,
        a: [Arc<ProvingKey<Eq>>; A_STAGE_COUNT],
        w: [Arc<ProvingKey<Ep>>; W_STAGE_COUNT],
    ) -> Result<Self, Error> {
        for key in &a {
            let d = key.binding().descriptor();
            if d.k != 16
                || d.instance_lengths != [69]
                || d.instance_types.as_deref() != Some(&[InstanceType::Bounded])
                || key.binding() != a[0].binding()
            {
                return Err(Error::Artifact);
            }
            VerifierPlan::new(key.binding().clone(), plan.vesta.clone())
                .map_err(|_| Error::Artifact)?;
        }
        let wrappers = (0..W_STAGE_COUNT)
            .map(|stage| {
                WKey::from_artifact(
                    &plan.context,
                    stage,
                    w[stage].binding().clone(),
                    plan.pallas.clone(),
                    w[stage].vk().clone(),
                )
                .map_err(|_| Error::Artifact)
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| Error::Artifact)?;
        Ok(Self {
            plan,
            a,
            w,
            wrappers,
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
    /// Exact installed descriptors in alternating A/W checkpoint order, ending A11.
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

/// Source-bound session over the fixed installed Receive artifacts.
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
        if next + 1 == A_STAGE_COUNT {
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

/// Enforce the exact shared wire Payment limit before producer work.
/// This is a size check only, never proof or monetary admission.
/// # Errors
/// Addition overflow or originals exceeding the fixed10,000-byte record.
pub fn check_payment_original_sizes(omega: usize, sigma: usize) -> Result<(), Error> {
    if omega
        .checked_add(sigma)
        .is_none_or(|length| length > MAX_PAYMENT_ORIGINAL_BYTES)
    {
        Err(Error::Input)
    } else {
        Ok(())
    }
}

fn operation_schedule() -> (Vec<Vec<usize>>, Vec<Vec<OperationTask>>) {
    // Q1 and Q2 must be verified in their respective hard/soft signature owners.
    // Proofs exports its original opening before terminal Effects consumes the iff rule.
    (
        vec![
            vec![],
            vec![],
            vec![],
            vec![],
            vec![0],
            vec![1],
            vec![2],
            vec![],
            vec![],
            vec![],
            vec![],
        ],
        vec![
            vec![OperationTask::ReceiveOwnProof],
            vec![OperationTask::ReceiveObjects],
            vec![OperationTask::ReceiveNonmembership],
            vec![OperationTask::ReceiveBlacklist],
            vec![],
            vec![OperationTask::ReceiveAuthorization],
            vec![OperationTask::ReceiveSignatures],
            vec![],
            vec![],
            vec![OperationTask::ReceiveProofs],
            vec![OperationTask::ReceiveEffects],
        ],
    )
}
fn recorded_selector(request: &[u8]) -> Result<usize, Error> {
    if request.len() != ObjectKind::Request.body_len() + 64 {
        return Err(Error::Input);
    }
    // Exact LE64 field15 determines the fixed sigma selector. Malformed roots and
    // version/root pairs remain soft inputs of the canonical Blacklist owner.
    let version = u64::from_le_bytes(request[354..362].try_into().map_err(|_| Error::Input)?);
    Ok(usize::from(version != 0))
}
fn signed_kinds() -> [ObjectKind; 8] {
    [
        ObjectKind::Request,
        ObjectKind::Credential,
        ObjectKind::Receipt,
        ObjectKind::Credential,
        ObjectKind::Certificate,
        ObjectKind::Receipt,
        ObjectKind::Credential,
        ObjectKind::Certificate,
    ]
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
        || columns[2][0] != Fq::from(10 + u64::from(plan.recorded_blacklist))
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
    let statements = [input.state.statement, input.incoming.statement];
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
        || indices.len() != 2
        || verdicts.len() != 5
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
    results: &[bool; 5],
    incoming: &mut NativeIncoming,
    budget: MemoryBudget,
) -> Result<(), Error> {
    let sigma = proved_sigma_mode(input)?;
    let good = results.iter().all(|b| *b);
    let valid = [
        incoming.pallas.decide(&plan.pallas, budget).is_ok(),
        incoming.opening.decide(&plan.pallas, budget).is_ok(),
        incoming.vesta.decide(&plan.vesta, budget).is_ok(),
    ];
    let trivial_p = AccumulatorT::<Ep>::trivial(&plan.pallas, budget)
        .map_err(|_| Error::Proof)?
        .as_input();
    let trivial_v = AccumulatorT::<Eq>::trivial(&plan.vesta, budget)
        .map_err(|_| Error::Proof)?
        .as_input();
    incoming.selected = [trivial_p.clone(), trivial_p.clone()];
    incoming.selected_vesta = trivial_v.clone();
    incoming.pallas_corrections = [Ep::from(*trivial_p.g()); 2];
    incoming.vesta_correction = Eq::from(*trivial_v.g());
    incoming.modes = [[false, true, false]; 4];
    incoming.modes[3] = sigma;
    if !good {
        if sigma != [false, true, false] {
            return Err(Error::Proof);
        }
    } else if let Some(failed) = valid.iter().position(|b| !*b) {
        if sigma != [false, true, false] {
            return Err(Error::Proof);
        }
        incoming.modes[failed] = [false, false, true];
        if failed < 2 {
            let source = if failed == 0 {
                &incoming.pallas
            } else {
                &incoming.opening
            };
            let corrected = source
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
    } else if sigma == [true, false, false] {
        incoming.modes = [[true, false, false]; 4];
        incoming.selected = [incoming.pallas.clone(), incoming.opening.clone()];
        incoming.selected_vesta = incoming.vesta.clone();
    } else if sigma != [false, false, true] {
        return Err(Error::Proof);
    }
    Ok(())
}

// These internal synthesis passes evaluate the exact production gadget code and
// extract its assigned outputs. They produce witness data, never an acceptance
// verdict, keys or an alternate profile. Actual installed A proofs must bind all
// five values; native hard Q/pred verification precedes these evaluations.
#[derive(Clone)]
struct Evaluation {
    first: FirstCircuit,
    tag: Option<ReceiveResultTag>,
}
impl Circuit<Fp> for Evaluation {
    type Config = StageConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = Option<ReceiveResultTag>;
    fn params(&self) -> Self::Params {
        self.tag
    }
    fn without_witnesses(&self) -> Self {
        Self {
            first: self.first.without_witnesses(),
            tag: self.tag,
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> StageConfig {
        Self::configure_with_params(meta, None)
    }
    fn configure_with_params(meta: &mut ConstraintSystem<Fp>, tag: Self::Params) -> StageConfig {
        let verifier = VerifierConfig::configure_serialized_foreign(meta, SOURCE_RANGE_BUSES)
            .expect("fixed Receive evaluation profile");
        let a = meta.advice_column();
        let b = meta.advice_column();
        let bytes = BytesConfig::configure(meta, a, b);
        let n = if tag == Some(ReceiveResultTag::Proofs) {
            107
        } else {
            1
        };
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
            || "genuine Receive witness evaluation",
            |mut region| {
                let c = f.setup(&mut chip, &mut bytes, &mut region)?;
                if let Some(tag) = self.tag {
                    let mut incoming = None;
                    let context = c.context();
                    let stage = f
                        .plan
                        .context
                        .receive_results()
                        .ok_or(LayoutError::Synthesis)?
                        .owner(tag);
                    let valid = match tag {
                        ReceiveResultTag::Proofs => {
                            let fixed = f
                                .plan
                                .context
                                .operation()
                                .omega()
                                .ok_or(LayoutError::Synthesis)?;
                            let key =
                                chip.constant_key(&mut region, fixed, &f.plan.predecessor_key)?;
                            let (valid, omega) = c.proofs.derive_proofs(
                                &mut chip,
                                &mut region,
                                &f.plan.context,
                                stage,
                                &context,
                                ReceiveProofInputs {
                                    sources: &c.proofs,
                                    omega_key: &key,
                                    own_sigma: &c.sigma[0],
                                },
                            )?;
                            incoming = Some(omega);
                            valid
                        }
                        ReceiveResultTag::Objects => c.objects.derive_objects(
                            &mut chip,
                            &mut region,
                            &f.plan.context,
                            &context,
                        )?,
                        ReceiveResultTag::Nonmembership => {
                            let search = f.search(
                                &mut chip.uint(),
                                &mut region,
                                &f.source.original.nonmembership,
                            )?;
                            maps::derive_nonmembership(
                                &mut chip,
                                &mut region,
                                &f.plan.context,
                                stage,
                                &context,
                                &search,
                            )?
                        }
                        ReceiveResultTag::Blacklist => {
                            let search = f.search(
                                &mut chip.uint(),
                                &mut region,
                                &f.source.original.blacklist,
                            )?;
                            c.signed.derive_blacklist(
                                &mut chip,
                                &mut region,
                                &f.plan.context,
                                stage,
                                &context,
                                &search,
                            )?
                        }
                        ReceiveResultTag::Signatures => {
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
                            c.auth.derive_signatures(
                                &mut chip,
                                &mut region,
                                &f.plan.context,
                                stage,
                                &context,
                                ReceiveSignatureInputs {
                                    policy: f.plan.policy,
                                    objects: &c.signed,
                                    incoming: &bundle,
                                },
                            )?
                        }
                    };
                    let mut out = vec![valid.word().clone()];
                    if let Some(incoming) = incoming.as_ref() {
                        out.extend(c.transport.public().fields().iter().cloned());
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
                            &c.pc,
                        )?;
                        out.extend(pallas_fields(&selected));
                        out.extend(c.transport.vesta().words());
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
    let n = if circuit.tag == Some(ReceiveResultTag::Proofs) {
        107
    } else {
        1
    };
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
fn evaluate_result(prepared: &Prepared, tag: ReceiveResultTag) -> Result<Vec<Fp>, Error> {
    let first = First {
        source: prepared.source.clone(),
        plan: prepared.plan.clone(),
        pallas: prepared.source.predecessor.pallas.clone(),
        fold: Vec::new(),
        known: true,
    };
    assigned_outputs(&Evaluation {
        first: first.circuit(),
        tag: Some(tag),
    })
}
fn context_digest(first: &First) -> Result<Fp, Error> {
    assigned_outputs(&Evaluation {
        first: first.circuit(),
        tag: None,
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
        }
        self.source
            .part
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
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
            &self.source.results,
            &mut reselected,
            budget,
        )?;
        let same_p = |a: &FoldInput<Ep>, b: &FoldInput<Ep>| {
            a.source_k() == b.source_k() && a.g() == b.g() && a.challenges() == b.challenges()
        };
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
        // Always attempt the actual original Omega proof using its independent
        // frozen key and exact D_A/V public columns. A false soft source is
        // retained and may burn only through the genuine complete A relation.
        let v = AccumulatorT::new(*incoming.vesta.g(), *incoming.vesta.challenges())
            .map_err(|_| Error::Input)?;
        let public = omega_instances(terminal_digest(&incoming.public, &incoming.pallas)?, &v)?;
        let length = fixed.proof_length();
        let raw = &self.source.original.incoming.omega;
        let incoming_proof_valid = raw.len() == 320 + length + 1088
            && verify_full(
                &self.plan.pallas,
                fixed.binding(),
                &self.plan.predecessor_key,
                &public,
                &raw[320..320 + length],
                budget,
            )
            .is_ok();
        if incoming.modes[0][0] && !incoming_proof_valid {
            return Err(Error::Proof);
        }
        Ok(())
    }
}

/// Both exact Request-recorded blacklist profiles for one installed variant.
/// Duplicate selector10 profiles cannot create a complete producer catalog.
#[derive(Clone)]
pub struct Catalog {
    entries: [Prover; 2],
}
impl Catalog {
    /// Mount both frozen selectors; they share variant/scheme/provider/root.
    /// # Errors
    /// Missing/reordered/duplicate selector or inconsistent frozen scope.
    pub fn new(entries: [Prover; 2]) -> Result<Self, Error> {
        for (i, p) in entries.iter().enumerate() {
            let first = &entries[0].plan;
            if p.plan.recorded_blacklist != (i == 1)
                || p.plan.context.operation().frame().variant()
                    != first.context.operation().frame().variant()
                || p.plan.policy.scheme != first.policy.scheme
                || p.plan.policy.provider != first.policy.provider
                || p.plan.policy.root != first.policy.root
                || p.plan
                    .context
                    .operation()
                    .omega()
                    .ok_or(Error::Artifact)?
                    .binding()
                    != first
                        .context
                        .operation()
                        .omega()
                        .ok_or(Error::Artifact)?
                        .binding()
                || p.plan
                    .predecessor_key
                    .kagemusha_digest(
                        p.plan
                            .context
                            .operation()
                            .omega()
                            .ok_or(Error::Artifact)?
                            .binding(),
                    )
                    .map_err(|_| Error::Artifact)?
                    != first
                        .predecessor_key
                        .kagemusha_digest(
                            first
                                .context
                                .operation()
                                .omega()
                                .ok_or(Error::Artifact)?
                                .binding(),
                        )
                        .map_err(|_| Error::Artifact)?
            {
                return Err(Error::Artifact);
            }
        }
        Ok(Self { entries })
    }
    /// Select solely from the signed original Request's recorded version/root,
    /// independent of the current receiver enabled-controls mask.
    /// # Errors
    /// Wrong Request shape, unpaired version/root, or any original/proof failure.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Session<'_>, Error> {
        let selector = recorded_selector(&input.objects[0])?;
        self.entries[selector].prepare(input, budget)
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
