//! Production Retiring A1/W0/A2/W1/A3/W2/A4 composition from original signed tapes.
//!
//! A1 binds the complete hard predecessor Omega and own sigma to the receipt;
//! A2 proves the exact lifecycle transition and adjusted-field synchronization; A3 verifies Q_sigma;
//! A4 reauthenticates the current Credential, direct Enrollment certificate and
//! own Advance Receipt. Every stage commits identical state, object and Q tapes.
//! Installation imports original PKs against this same compiled source using
//! unknown optional claims, then requires exact installed VK equality. Installation
//! views cannot construct Prepared, accepted openings or custody checkpoints.
//! Every native source/generated proof and every carried Pasta claim is checked
//! in full. Installed keys and profiles are fixed, never generated from inputs.
//! The terminal A4 is an input to the final Omega producer, not a final lineage
//! or a ledger payout. Generic oversized Omega descriptors are rejected.

#[path = "retiring/checkpoint.rs"]
mod checkpoint;
pub use checkpoint::{CheckpointKind, CheckpointLayout};

use core::fmt;
use std::sync::Arc;

use ff::{Field, PrimeField};
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
    bytes::{
        element::le_message_segments,
        le_value, p_bytes_native,
        tape::{BytesChip, BytesConfig, SegmentSpec},
    },
    p256::VerifyMode,
    statement::foreign_limbs,
};
use iroha_plonk_recursion::{
    AccumulatorT, FoldConfig, FoldInput, K,
    accumulation_circuit::FoldInputCells,
    codec::ScalarCells,
    create_fold,
    obligation::ledger::Variant,
    verifier::{VerifierChip, VerifierConfig, VerifierPlan},
};

use super::super::{
    AProofPlan, LineagePublicCells, ProofMessageCells, SigmaBindingCells, VestaClaimCells,
    context::{ContextInputs, ContextPlan, ContextPredecessor, ContextState},
    own::{ConsumingProofCells, OwnPolicy},
    schedule::OperationTask,
    split::{SplitPlan, WCircuit, WKey, close_first},
    unload::{UnloadObjects, UnloadProofInputs, UnloadStagePlan, UnloadStageWitness},
    verify_predecessor, verify_sigma,
};
use crate::{
    admin_sigma::{ConsumingWitness, StateWitness},
    omega::OmegaWitness,
    operation_relation::{objects::ObjectKind, state::StateCells, statement::StatementCells},
    q_signature::{QSignaturePlan, SignatureKey},
};

#[cfg(test)]
#[path = "retiring/tests.rs"]
mod tests;

/// Fixed native source profile; private inputs cannot choose another range-bus count.
pub const SOURCE_RANGE_BUSES: usize = 4;
const DESCRIPTOR_MAX_BYTES: usize = 1 << 20;
const VERIFYING_KEY_MAX_BYTES: usize = 1 << 18;
/// Exact source schedule has four A stages and three W continuations.
pub const A_STAGE_COUNT: usize = 4;
/// Exact internal wrapper count, ending in final A4.
pub const W_STAGE_COUNT: usize = A_STAGE_COUNT - 1;
/// Uniform installed Omega transport bound after the largest accepted Send sigma.
/// The exact public lineage prefix is separate from this proof plus two-claim bound.
pub const OMEGA_TRANSPORT_CAP: usize = 10_000 - 1_723 - 3_456;

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
        write!(f, "native Retiring: {self:?}")
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

/// Post-Advance originals for complete native Retiring lineage composition.
/// The G1 owner authenticates canonical custody objects before producing these typed
/// field transcripts and exact signed-body transcript||signature tapes.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Exact predecessor/successor core33/rest8/public18 and own statement26.
    pub state: ConsumingWitness,
    /// Exact original unframed sigma bytes also exported by Q0.
    pub sigma: Vec<u8>,
    /// Current Credential, direct Enrollment certificate, own Advance Receipt.
    pub objects: [Vec<u8>; 3],
    /// Hard Q_sigma followed by hard receipt/current Credential/Enrollment Q.
    pub q: [QInput; 2],
    /// Exact canonical public320 || original predecessor proof || full accP || full accV.
    pub omega: Vec<u8>,
    /// Actual predecessor proof; its key/profile comes from the installed Plan.
    pub predecessor: PredecessorInput,
}

/// Immutable complete Retiring circuit metadata from the authenticated native artifact owner.
#[derive(Clone, Debug)]
pub struct Plan {
    context: ContextPlan,
    policy: OwnPolicy,
    signatures: QSignaturePlan,
    predecessor_key: VerifyingKey<Ep>,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin predecessor and the fixed [[],[],[Q0],[Q1]] schedule with every Retiring task.
    /// Signature slots are hard [receipt,current Credential,Enrollment certificate],
    /// with the certificate under the fixed scheme-root key.
    /// # Errors
    /// Wrong variant/k/schema, absent predecessor or substituted fixed signature/key policy.
    pub fn new(
        operation: AProofPlan,
        policy: OwnPolicy,
        signatures: QSignaturePlan,
        predecessor_key: VerifyingKey<Ep>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
    ) -> Result<Self, Error> {
        if operation.frame().variant() != Variant::Retiring
            || operation.frame().part_source_k() != 12
            || operation.q_count() != 2
            || !operation.frame().has_predecessor()
            || operation.sigma.slot_count() != 1
            || operation
                .sigma
                .class(0)
                .ok_or(Error::Artifact)?
                .verifier()
                .binding()
                .descriptor()
                .k
                != 12
        {
            return Err(Error::Artifact);
        }
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
        operation
            .sigma
            .class(0)
            .ok_or(Error::Artifact)?
            .selector_key_digest(15)
            .ok_or(Error::Artifact)?;
        check_omega_transport_length(predecessor.proof_length()).map_err(|_| Error::Artifact)?;
        let slots = signatures.slots();
        if slots.len() != 3
            || slots.iter().any(|slot| slot.mode != VerifyMode::Hard)
            || slots[0].key != SignatureKey::Variable
            || slots[1].key != SignatureKey::Variable
            || slots[2].key != SignatureKey::Fixed(policy.root)
        {
            return Err(Error::Artifact);
        }
        let d = operation
            .q(1)
            .ok_or(Error::Artifact)?
            .verifier()
            .binding()
            .descriptor();
        if d.instance_lengths != [signatures.instance_length() as u32]
            || d.instance_types.as_deref() != Some(&QSignaturePlan::instance_types())
        {
            return Err(Error::Artifact);
        }
        let context = ContextPlan::with_schedule(
            operation,
            vec![vec![], vec![], vec![0], vec![1]],
            Some(0),
            UnloadObjects::context_specs()
                .map_err(|_| Error::Artifact)?
                .to_vec(),
        )
        .and_then(|p| {
            p.with_operation_tasks(vec![
                vec![OperationTask::UnloadProof],
                vec![OperationTask::RetiringState],
                vec![],
                vec![OperationTask::UnloadAuthorization],
            ])
        })
        .map_err(|_| Error::Artifact)?;
        UnloadStagePlan::new(context.clone(), policy).map_err(|_| Error::Artifact)?;
        Ok(Self {
            context,
            policy,
            signatures,
            predecessor_key,
            pallas,
            vesta,
        })
    }
    /// Complete fixed source-bound context schema.
    #[must_use]
    pub const fn context(&self) -> &ContextPlan {
        &self.context
    }

    /// Verify all actual predecessor/Q proofs and both transported predecessor claims.
    /// Derive every Q opening and sigma Vesta claim from those checked originals.
    /// # Errors
    /// Wrong tape/length/schema or any native proof/full-accumulator failure.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Prepared, Error> {
        for (kind, raw) in object_kinds().into_iter().zip(&input.objects) {
            if raw.len() != kind.body_len() + 64 {
                return Err(Error::Input);
            }
        }
        let class = self
            .context
            .operation()
            .sigma
            .class(0)
            .ok_or(Error::Artifact)?;
        if input.sigma.len() != class.verifier().proof_length() {
            return Err(Error::Input);
        }
        check_sigma_tape(&input)?;
        let predecessor_length = self
            .context
            .operation()
            .omega()
            .ok_or(Error::Artifact)?
            .proof_length();
        if input.predecessor.proof.len() != predecessor_length
            || input.omega.len()
                != 320 + predecessor_length + 2 * iroha_plonk_recursion::ACCUMULATOR_BYTES
        {
            return Err(Error::Input);
        }
        check_omega_transport_length(predecessor_length)?;
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
        let program = self
            .context
            .operation()
            .omega()
            .ok_or(Error::Artifact)?
            .clone();
        let digest = self
            .predecessor_key
            .kagemusha_digest(program.binding())
            .map_err(|_| Error::Artifact)?;
        if input.state.predecessor.lineage[17] != digest {
            return Err(Error::Input);
        }
        let own = terminal_digest(&input.state.predecessor.lineage, &pallas.as_input())?;
        let public = omega_instances(own, &vesta)?;
        verify_full(
            &self.pallas,
            program.binding(),
            &self.predecessor_key,
            &public,
            &input.predecessor.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_pallas(
            &self.pallas,
            program.binding(),
            &self.predecessor_key,
            &public,
            &input.predecessor.proof,
            budget,
        )?;
        let mut openings = Vec::new();
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
            openings.push(opening_pallas(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &q.instances,
                &q.proof,
                budget,
            )?);
        }
        let part = q_sigma_part(&input.q[0], 12)?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        if input.omega
            != lineage_bytes(
                &input.state.predecessor.lineage,
                &input.predecessor.proof,
                &pallas,
                &vesta,
            )?
        {
            return Err(Error::Input);
        }
        let maps = Maps {
            witness: input.state,
            objects: core::array::from_fn(|i| SignedTape {
                kind: object_kinds()[i],
                bytes: input.objects[i].clone(),
            }),
            known: true,
        };
        let predecessor = Predecessor {
            key: self.predecessor_key.clone(),
            proof: input.predecessor.proof,
            pallas,
            opening,
            vesta,
        };
        let source = Arc::new(Sources {
            maps,
            omega: input.omega,
            sigma: input.sigma,
            q_instances: input.q.each_ref().map(|q| q.instances.clone()).to_vec(),
            q_proofs: input.q.each_ref().map(|q| q.proof.clone()).to_vec(),
            q_openings: openings,
            signature_schema: self.signatures.clone(),
            part,
            predecessor,
            params: self.pallas.clone(),
            policy: self.policy,
        });
        Ok(Prepared {
            plan: self.clone(),
            source,
        })
    }
}

#[derive(Clone)]
struct Predecessor {
    key: VerifyingKey<Ep>,
    proof: Vec<u8>,
    pallas: AccumulatorT<Ep>,
    opening: FoldInput<Ep>,
    vesta: AccumulatorT<Eq>,
}
#[derive(Clone)]
struct Sources {
    maps: Maps,
    omega: Vec<u8>,
    sigma: Vec<u8>,
    q_instances: Vec<Vec<Vec<Fq>>>,
    q_proofs: Vec<Vec<u8>>,
    q_openings: Vec<FoldInput<Ep>>,
    signature_schema: QSignaturePlan,
    part: FoldInput<Eq>,
    predecessor: Predecessor,
    params: PinnedParams<Ep>,
    policy: OwnPolicy,
}
#[derive(Clone)]
struct SignedTape {
    kind: ObjectKind,
    bytes: Vec<u8>,
}
impl SignedTape {
    fn digest(&self) -> Result<Fp, Error> {
        object_digest(self.kind, &self.bytes)
    }
}
#[derive(Clone)]
struct Maps {
    witness: ConsumingWitness,
    objects: [SignedTape; 3],
    known: bool,
}
impl Maps {
    fn value<T: Copy>(&self, value: T) -> Value<T> {
        if self.known {
            Value::known(value)
        } else {
            Value::unknown()
        }
    }
    fn state(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        witness: &StateWitness,
    ) -> Result<(StateCells, LineagePublicCells), LayoutError> {
        let core = chip
            .uint()
            .glue()
            .witnesses(region, &witness.core.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let rest = chip
            .uint()
            .glue()
            .witnesses(region, &witness.rest.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let state = StateCells::constrain_with_verifier(chip, region, &core, &rest)?;
        let public = chip
            .uint()
            .glue()
            .witnesses(region, &witness.lineage.map(|v| self.value(v)))?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let public = LineagePublicCells::constrain(&mut chip.uint(), region, &public)?;
        Ok((state, public))
    }
}
#[derive(Clone)]
struct First {
    source: Arc<Sources>,
    plan: ContextPlan,
    pallas: AccumulatorT<Ep>,
    fold: Vec<u8>,
    known: bool,
}
// Circuit-only views contain no verified native claim or Prepared authority.
#[derive(Clone)]
struct CircuitPredecessor {
    key: VerifyingKey<Ep>,
    proof: Vec<u8>,
    pallas: Option<FoldInput<Ep>>,
    vesta: Option<AccumulatorT<Eq>>,
}
#[derive(Clone)]
struct CircuitSources {
    maps: Maps,
    omega: Vec<u8>,
    sigma: Vec<u8>,
    q_instances: Vec<Vec<Vec<Fq>>>,
    q_proofs: Vec<Vec<u8>>,
    signature_schema: QSignaturePlan,
    predecessor: CircuitPredecessor,
    params: PinnedParams<Ep>,
    policy: OwnPolicy,
}
#[derive(Clone)]
struct FirstCircuit {
    source: Arc<CircuitSources>,
    plan: ContextPlan,
    fold: Vec<u8>,
    known: bool,
}
impl First {
    fn circuit(&self) -> FirstCircuit {
        let source = &self.source;
        FirstCircuit {
            source: Arc::new(CircuitSources {
                maps: source.maps.clone(),
                omega: source.omega.clone(),
                sigma: source.sigma.clone(),
                q_instances: source.q_instances.clone(),
                q_proofs: source.q_proofs.clone(),
                signature_schema: source.signature_schema.clone(),
                predecessor: CircuitPredecessor {
                    key: source.predecessor.key.clone(),
                    proof: source.predecessor.proof.clone(),
                    pallas: Some(source.predecessor.pallas.as_input()),
                    vesta: Some(source.predecessor.vesta.clone()),
                },
                params: source.params.clone(),
                policy: source.policy,
            }),
            plan: self.plan.clone(),
            fold: self.fold.clone(),
            known: self.known,
        }
    }
}
impl FirstCircuit {
    fn blank(plan: &Plan) -> Result<Self, Error> {
        let operation = plan.context.operation();
        let class = operation.sigma.class(0).ok_or(Error::Artifact)?;
        let predecessor = operation.omega().ok_or(Error::Artifact)?;
        let state = StateWitness {
            core: [Fp::ZERO; 33],
            rest: [Fp::ZERO; 8],
            lineage: [Fp::ZERO; 18],
        };
        let mut q_instances = Vec::new();
        let mut q_proofs = Vec::new();
        for index in 0..2 {
            let q = operation.q(index).ok_or(Error::Artifact)?;
            q_instances.push(
                q.verifier()
                    .binding()
                    .descriptor()
                    .instance_lengths
                    .iter()
                    .map(|length| vec![Fq::ZERO; *length as usize])
                    .collect(),
            );
            q_proofs.push(vec![0; q.verifier().proof_length()]);
        }
        let objects = object_kinds().map(|kind| SignedTape {
            kind,
            bytes: vec![0; kind.body_len() + 64],
        });
        let source = CircuitSources {
            maps: Maps {
                witness: ConsumingWitness {
                    predecessor: state,
                    successor: state,
                    statement: [Fp::ZERO; 26],
                },
                objects,
                known: false,
            },
            omega: vec![
                0;
                predecessor
                    .proof_length()
                    .checked_add(320 + 2 * 544)
                    .ok_or(Error::Artifact)?
            ],
            sigma: vec![0; class.verifier().proof_length()],
            q_instances,
            q_proofs,
            signature_schema: plan.signatures.clone(),
            predecessor: CircuitPredecessor {
                key: plan.predecessor_key.clone(),
                proof: vec![0; predecessor.proof_length()],
                pallas: None,
                vesta: None,
            },
            params: plan.pallas.clone(),
            policy: plan.policy,
        };
        Ok(Self {
            source: Arc::new(source),
            plan: plan.context.clone(),
            fold: vec![0; 1120],
            known: false,
        })
    }
}
#[derive(Clone, Debug)]
/// Fixed source-stage columns and range buses; all fields are native metadata.
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
    fn sigma(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        statement: &StatementCells,
    ) -> Result<SigmaBindingCells, LayoutError> {
        let length =
            u32::try_from(self.source.sigma.len()).map_err(|_| LayoutError::BoundsFailure)?;
        let source = length
            .to_le_bytes()
            .into_iter()
            .chain(self.source.sigma.iter().copied())
            .map(|v| self.value(v))
            .collect::<Vec<_>>();
        let run = bytes.run(
            region,
            &source,
            &source
                .chunks(31)
                .map(<[Value<u8>]>::len)
                .collect::<Vec<_>>(),
            &[SegmentSpec::little(0, 4)],
        )?;
        let index = chip.uint().glue().constant(
            region,
            Fp::from(u64::from(
                crate::a_relation::schedule::sigma_selector(6, 0).unwrap(),
            )),
        )?;
        SigmaBindingCells::from_run(chip, region, statement, index, &run)
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
            .expect("fixed four-bus native Retiring profile");
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
            || "genuine Retiring predecessor-first relation",
            |mut region| {
                let mut maps = self.source.maps.clone();
                maps.known = self.known;
                let (old, pred_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.predecessor)?;
                let (new, next_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.successor)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Retiring,
                    &fields,
                )?;
                let sigma = self.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let objects = maps
                    .objects
                    .each_ref()
                    .map(|o| o.bytes.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
                let objects = UnloadObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    objects.each_ref().map(Vec::as_slice),
                )?;
                let q_instances = self
                    .source
                    .q_instances
                    .iter()
                    .map(|cols| {
                        cols.iter()
                            .map(|column| {
                                column
                                    .iter()
                                    .map(|v| self.scalar(&mut chip, &mut region, *v))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<Vec<_>>>, _>>()?;
                let pp = self.pallas(
                    &mut chip,
                    &mut region,
                    self.source.predecessor.pallas.as_ref(),
                )?;
                let pv = self.vesta(
                    &mut chip,
                    &mut region,
                    self.source.predecessor.vesta.as_ref(),
                )?;
                let input = ContextInputs {
                    own_statement: &statement,
                    incoming_statement: None,
                    predecessor: Some(ContextPredecessor {
                        state: &old,
                        public: &pred_public,
                        pallas: &pp,
                        vesta: &pv,
                    }),
                    successor: ContextState {
                        state: &new,
                        public: &next_public,
                    },
                    incoming: None,
                    q_instances: &q_instances,
                    objects: objects.context(),
                    modes: &[],
                    pallas_corrections: &[],
                    vesta_corrections: &[],
                    receive_results: None,
                };
                let vk = &self.source.predecessor.key;
                let iroha_plonk::transcript::TranscriptRepr::Base(repr) = *vk.transcript_repr()
                else {
                    return Err(LayoutError::Synthesis);
                };
                let fixed = vk
                    .fixed_commitments()
                    .iter()
                    .map(|p| self.value(Ep::from(*p)))
                    .collect::<Vec<_>>();
                let permutation = vk
                    .permutation_commitments()
                    .iter()
                    .map(|p| self.value(Ep::from(*p)))
                    .collect::<Vec<_>>();
                let key = chip.witness_key(&mut region, self.value(repr), &fixed, &permutation)?;
                let proof = self.carrier(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    &self.source.predecessor.proof,
                )?;
                let pred = verify_predecessor(
                    &mut chip,
                    &mut region,
                    self.plan.operation(),
                    &key,
                    &pred_public,
                    &next_public,
                    &pp,
                    &pv,
                    &proof,
                )?;
                let omega = frame(&self.source.omega)?;
                let omega_run = bytes.run(
                    &mut region,
                    &omega.iter().map(|v| self.value(*v)).collect::<Vec<_>>(),
                    &iroha_plonk_gadgets::bytes::chunk_segments(0, omega.len()),
                    &ConsumingProofCells::omega_segments(self.source.predecessor.proof.len())?,
                )?;
                let sigma_tape = frame(&self.source.sigma)?;
                let sigma_run = bytes.run(
                    &mut region,
                    &sigma_tape
                        .iter()
                        .map(|v| self.value(*v))
                        .collect::<Vec<_>>(),
                    &iroha_plonk_gadgets::bytes::chunk_segments(0, sigma_tape.len()),
                    &[SegmentSpec::little(0, 4)],
                )?;
                let consuming = ConsumingProofCells::from_runs(
                    &mut chip,
                    &mut region,
                    &pred,
                    &sigma,
                    &omega_run,
                    &sigma_run,
                )?;
                UnloadStagePlan::new(self.plan.clone(), self.source.policy)?.constrain_stage(
                    &mut chip,
                    &mut region,
                    0,
                    &objects,
                    &input,
                    UnloadStageWitness {
                        proof: Some(UnloadProofInputs {
                            proof: &consuming,
                            sigma: &sigma,
                        }),
                        ..UnloadStageWitness::default()
                    },
                )?;
                let mut verified = Vec::new();
                for (index, columns) in q_instances
                    .iter()
                    .enumerate()
                    .take(self.plan.first_q_count())
                {
                    let proof = self.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &self.source.q_proofs[index],
                    )?;
                    let (q, _) = verify_sigma(
                        &mut chip,
                        &mut region,
                        self.plan.operation(),
                        columns,
                        &proof,
                        core::slice::from_ref(&sigma),
                    )?;
                    verified.push(q);
                }
                let fold = self.carrier(&mut chip, &mut bytes, &mut region, &self.fold)?;
                let first = close_first(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &input,
                    Some(&pred),
                    &verified,
                    core::slice::from_ref(&sigma),
                    Some(&fold),
                    &self.source.params,
                )?;
                first.words(&mut chip, &mut region)
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
    plan: crate::a_relation::split::SplitPlan,
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
        let first = &self.first;
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "Retiring deferred Q continuation",
            |mut region| {
                let mut maps = first.source.maps.clone();
                maps.known = first.known;
                let (old, pred_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.predecessor)?;
                let (new, next_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.successor)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| first.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Retiring,
                    &fields,
                )?;
                let sigma = first.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let sources = maps
                    .objects
                    .each_ref()
                    .map(|o| o.bytes.iter().map(|v| first.value(*v)).collect::<Vec<_>>());
                let objects = UnloadObjects::decode(
                    &mut chip,
                    &mut bytes,
                    &mut region,
                    sources.each_ref().map(Vec::as_slice),
                )?;
                let q_instances = first
                    .source
                    .q_instances
                    .iter()
                    .map(|cols| {
                        cols.iter()
                            .map(|column| {
                                column
                                    .iter()
                                    .map(|v| first.scalar(&mut chip, &mut region, *v))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<Vec<_>>>, _>>()?;
                let pp = first.pallas(
                    &mut chip,
                    &mut region,
                    first.source.predecessor.pallas.as_ref(),
                )?;
                let pv = first.vesta(
                    &mut chip,
                    &mut region,
                    first.source.predecessor.vesta.as_ref(),
                )?;
                let input = ContextInputs {
                    own_statement: &statement,
                    incoming_statement: None,
                    predecessor: Some(ContextPredecessor {
                        state: &old,
                        public: &pred_public,
                        pallas: &pp,
                        vesta: &pv,
                    }),
                    successor: ContextState {
                        state: &new,
                        public: &next_public,
                    },
                    incoming: None,
                    q_instances: &q_instances,
                    objects: objects.context(),
                    modes: &[],
                    pallas_corrections: &[],
                    vesta_corrections: &[],
                    receive_results: None,
                };
                let cp = first.pallas(&mut chip, &mut region, self.carried.as_ref())?;
                let history = self
                    .history
                    .iter()
                    .map(|(p, v)| {
                        Ok(crate::a_relation::split::ContextLinkCells {
                            pallas: first.pallas(&mut chip, &mut region, p.as_ref())?,
                            vesta: first.vesta(&mut chip, &mut region, v.as_ref())?,
                        })
                    })
                    .collect::<Result<Vec<_>, LayoutError>>()?;
                let cv = first.vesta(&mut chip, &mut region, self.vesta.as_ref())?;
                let proof = first.carrier(&mut chip, &mut bytes, &mut region, &self.wrapper)?;
                let resumed = crate::a_relation::split::resume_context(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &input,
                    &history,
                    &cp,
                    &cv,
                    &proof,
                    core::slice::from_ref(&sigma),
                )?;
                let mut verified = Vec::new();
                let mut authorization = None;
                for index in first
                    .plan
                    .q_partition(self.plan.stage())
                    .ok_or(LayoutError::Synthesis)?
                    .iter()
                    .copied()
                {
                    let columns = &q_instances[index];
                    let proof = first.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut region,
                        &first.source.q_proofs[index],
                    )?;
                    let q = if index == 0 {
                        verify_sigma(
                            &mut chip,
                            &mut region,
                            first.plan.operation(),
                            columns,
                            &proof,
                            core::slice::from_ref(&sigma),
                        )?
                        .0
                    } else {
                        let q = crate::a_relation::verify_q(
                            &mut chip,
                            &mut region,
                            first.plan.operation(),
                            index,
                            columns,
                            &proof,
                        )?;
                        let slots = crate::a_relation::bind_signature_q(
                            &mut chip,
                            &mut region,
                            first.plan.operation(),
                            index,
                            &first.source.signature_schema,
                            &q,
                        )?;
                        let verified = slots.verified().clone();
                        authorization = Some(slots);
                        verified
                    };
                    verified.push(q);
                }
                UnloadStagePlan::new(first.plan.clone(), first.source.policy)?.constrain_stage(
                    &mut chip,
                    &mut region,
                    self.plan.stage(),
                    &objects,
                    &input,
                    UnloadStageWitness {
                        recovery: None,
                        signatures: authorization.as_ref(),
                        proof: None,
                    },
                )?;
                let fold = first.carrier(&mut chip, &mut bytes, &mut region, &self.fold)?;
                let closed = crate::a_relation::split::close_stage(
                    &mut chip,
                    &mut region,
                    &self.plan,
                    &resumed,
                    None,
                    None,
                    None,
                    &verified,
                    &fold,
                )?;
                if self.plan.is_terminal() {
                    closed.words(&mut chip, &mut region, &next_public)
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

/// Checked original source coupled to the fixed complete Retiring plan.
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
            plan: self.plan.context.clone(),
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

/// Already installed A1/A2/A3/A4 and W0/W1/W2 proving artifacts for this fixed profile.
/// Authentication and genuine PK import remain the native package owner's responsibility.
pub struct Prover {
    plan: Plan,
    a: [Arc<ProvingKey<Eq>>; 4],
    w: [Arc<ProvingKey<Ep>>; 3],
    wrappers: [WKey; 3],
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
        a: [OriginalArtifact<'_>; 3],
        w: [OriginalArtifact<'_>; 3],
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
        a: [Arc<ProvingKey<Eq>>; 4],
        w: [Arc<ProvingKey<Ep>>; 3],
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
        let wrappers = (0..3)
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
    /// Exact installed descriptors in actual A1/W0/A2/W1/A3/W2/A4 checkpoint order.
    #[must_use]
    pub fn descriptors(&self) -> [&DescriptorBinding; 7] {
        [
            self.a[0].binding(),
            self.w[0].binding(),
            self.a[1].binding(),
            self.w[1].binding(),
            self.a[2].binding(),
            self.w[2].binding(),
            self.a[3].binding(),
        ]
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

/// Source-bound session over the fixed installed Retiring artifacts.
pub struct Session<'a> {
    prover: &'a Prover,
    prepared: Prepared,
}
impl Session<'_> {
    fn require_source(&self, source: &ACheckpoint) -> Result<(), Error> {
        if source.stage >= 4 || !Arc::ptr_eq(&source.first.source, &self.prepared.source) {
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
            plan: self.prepared.plan.context.clone(),
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
        if source.stage >= 3 {
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
        if source.stage >= 3 {
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
        if previous >= 3 {
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
    /// Ordered slots are previous carried P, actual W opening, and that stage's actual Q.
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
    /// Restore A2/A3/A4 from its verified source W and canonical new Pallas claim.
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
    /// Export A4 and all distinct final Omega obligations after another full native check.
    /// This grants neither monetary completion nor a final transported Omega.
    /// # Errors
    /// A nonterminal/wrong source checkpoint or any failed native proof/decide.
    pub fn terminal(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<Terminal, Error> {
        self.verify_a(source, budget)?;
        if source.stage != 3 {
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
/// Actual A4 proof and every distinct obligation consumed by the final Omega producer.
#[derive(Clone, Debug)]
pub struct Terminal {
    /// Original terminal A4 proof under its installed key.
    pub proof: Vec<u8>,
    /// Exact homogeneous69-word public frame.
    pub instances: Vec<Fp>,
    /// Full accumulated Pallas claim.
    pub pallas: AccumulatorT<Ep>,
    /// Full Vesta part forwarded by W2.
    pub vesta_part: AccumulatorT<Eq>,
    /// Full predecessor Vesta obligation, distinct from the current part and A opening.
    pub predecessor_vesta: AccumulatorT<Eq>,
    /// Actual terminal A4 own opening, a separate final Omega slot.
    pub opening: FoldInput<Eq>,
}

fn object_kinds() -> [ObjectKind; 3] {
    [
        ObjectKind::Credential,
        ObjectKind::Certificate,
        ObjectKind::Receipt,
    ]
}

fn check_sigma_tape(input: &Inputs) -> Result<(), Error> {
    let bounded = input.q[0].instances.first().ok_or(Error::Input)?;
    let mut raw = u32::try_from(input.sigma.len())
        .map_err(|_| Error::Input)?
        .to_le_bytes()
        .to_vec();
    raw.extend(&input.sigma);
    let chunks = raw
        .chunks(31)
        .map(|chunk| le_value::<Fq>(chunk).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    let statement = hash_with_domain(
        iroha_plonk_gadgets::statement::STATEMENT_DOMAIN,
        &input.state.statement,
    );
    let statement = Option::<Fq>::from(Fq::from_repr(statement.to_repr())).ok_or(Error::Input)?;
    if bounded.len() != 1 + chunks.len() + K
        || bounded[0] != statement
        || bounded[1..1 + chunks.len()] != chunks
        || input.q[0].instances.get(2).map(Vec::as_slice) != Some(&[Fq::from(15)])
    {
        return Err(Error::Input);
    }
    Ok(())
}

fn q_sigma_part(input: &QInput, source_k: u32) -> Result<FoldInput<Eq>, Error> {
    let [bounded, point, indices, verdicts, source] = input.instances.as_slice() else {
        return Err(Error::Input);
    };
    if point.len() != 2
        || indices.len() != 1
        || indices.as_slice() != [Fq::from(15)]
        || verdicts.as_slice() != [Fq::ONE]
        || source.as_slice() != [Fq::from(u64::from(source_k))]
        || bounded.len() < K
    {
        return Err(Error::Input);
    }
    let g = Option::<EqAffine>::from(EqAffine::from_xy(point[0], point[1])).ok_or(Error::Input)?;
    let challenges = bounded[bounded.len() - K..]
        .iter()
        .map(|v| Option::<Fp>::from(Fp::from_repr(v.to_repr())).ok_or(Error::Input))
        .collect::<Result<Vec<_>, _>>()?;
    FoldInput::from_normalized(
        g,
        source_k,
        challenges.try_into().map_err(|_| Error::Input)?,
    )
    .map_err(|_| Error::Input)
}

fn object_digest(kind: ObjectKind, bytes: &[u8]) -> Result<Fp, Error> {
    let end = kind.body_len();
    if bytes.len() != end + 64 {
        return Err(Error::Input);
    }
    let mut words = vec![p_bytes_native(kind.signing_domain(), &bytes[..end])];
    for offset in [16, 0, 48, 32] {
        words.push(Fp::from_u128(u128::from_be_bytes(
            bytes[end + offset..end + offset + 16]
                .try_into()
                .map_err(|_| Error::Input)?,
        )));
    }
    Ok(hash_with_domain(kind.object_domain(), &words))
}

fn push_pallas(words: &mut Vec<Fp>, claim: &FoldInput<Ep>) -> Result<(), Error> {
    if claim.source_k() != 16 {
        return Err(Error::Input);
    }
    let (x, y) = Option::<(Fp, Fp)>::from(claim.g().coordinates()).ok_or(Error::Input)?;
    words.extend([Fp::from(16), x, y]);
    for value in claim.challenges() {
        words.extend(foreign_limbs(value).map(Fp::from_u128));
    }
    Ok(())
}
fn vesta_words(claim: &FoldInput<Eq>) -> Result<Vec<Fp>, Error> {
    let (x, y) = Option::<(Fq, Fq)>::from(claim.g().coordinates()).ok_or(Error::Input)?;
    let mut words = Vec::with_capacity(20);
    for value in [x, y] {
        words.extend(foreign_limbs(&value).map(Fp::from_u128));
    }
    words.extend(claim.challenges());
    Ok(words)
}

fn context_digest(first: &First) -> Result<Fp, Error> {
    let source = &first.source;
    let mut words = first.plan.schema().to_vec();
    words.extend(source.maps.witness.statement);
    let old = &source.maps.witness.predecessor;
    let new = &source.maps.witness.successor;
    words.extend(old.core);
    words.extend(old.rest);
    words.extend(old.lineage);
    push_pallas(&mut words, &source.predecessor.pallas.as_input())?;
    words.extend(vesta_words(&source.predecessor.vesta.as_input())?);
    words.extend(new.core);
    words.extend(new.rest);
    words.extend(new.lineage);
    for value in source.q_instances.iter().flatten().flatten() {
        words.extend(foreign_limbs(value).map(Fp::from_u128));
    }
    for (object, spec) in source
        .maps
        .objects
        .iter()
        .zip(UnloadObjects::context_specs().map_err(|_| Error::Artifact)?)
    {
        if object.bytes.len() != usize::try_from(spec.capacity).map_err(|_| Error::Input)? {
            return Err(Error::Input);
        }
        let mut bytes = spec.capacity.to_le_bytes().to_vec();
        bytes.extend(&object.bytes);
        let mut tape = vec![
            Fp::from(u64::from(spec.tag)),
            Fp::from(u64::from(spec.capacity)),
        ];
        for chunk in bytes.chunks(31) {
            tape.push(le_value::<Fp>(chunk).ok_or(Error::Input)?);
        }
        words.extend([
            object.digest()?,
            Fp::from(u64::from(spec.capacity)),
            hash_with_domain(u64::from_le_bytes(*b"kgwctap1"), &tape),
        ]);
    }
    push_pallas(&mut words, &first.pallas.as_input())?;
    Ok(hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words))
}

fn trivial_vesta_words() -> Result<Vec<Fp>, Error> {
    let g = iroha_plonk::transcript::decode_point::<Eq>(
        &iroha_plonk_recursion::VESTA_TRIVIAL_GENERATOR,
    )
    .map_err(|_| Error::Artifact)?;
    let claim =
        FoldInput::<Eq>::from_normalized(g, 16, [Fp::ONE; K]).map_err(|_| Error::Artifact)?;
    vesta_words(&claim)
}
fn internal_public(digest: Fp, part: &FoldInput<Eq>) -> Result<Vec<Fp>, Error> {
    let mut words = vec![digest, Fp::from(u64::from(part.source_k()))];
    words.extend(vesta_words(part)?);
    let trivial = trivial_vesta_words()?;
    words.extend(&trivial);
    words.extend(&trivial);
    words.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    words.extend(&trivial[..4]);
    if words.len() != 69 {
        return Err(Error::Input);
    }
    Ok(words)
}
fn first_public(first: &First) -> Result<Vec<Fp>, Error> {
    internal_public(context_digest(first)?, &first.source.part)
}
fn continued_digest(
    plan: &ContextPlan,
    stage: usize,
    previous: Fp,
    pallas: &AccumulatorT<Ep>,
    vesta: &AccumulatorT<Eq>,
    current: &AccumulatorT<Ep>,
) -> Result<Fp, Error> {
    let mut words = vec![
        Fp::ONE,
        plan.schema()[1],
        Fp::from(u64::try_from(stage + 1).map_err(|_| Error::Input)?),
        previous,
    ];
    push_pallas(&mut words, &pallas.as_input())?;
    words.extend(vesta_words(&vesta.as_input())?);
    push_pallas(&mut words, &current.as_input())?;
    Ok(hash_with_domain(u64::from_le_bytes(*b"kgwctx_1"), &words))
}
fn continuation_public(c: &Continuation) -> Result<Vec<Fp>, Error> {
    if c.history.len() + 1 != c.plan.stage() {
        return Err(Error::Input);
    }
    if !c.plan.is_terminal() {
        let mut digest = context_digest(&c.first)?;
        for (i, (p, v)) in c.history.iter().enumerate() {
            let next = c.history.get(i + 1).map_or(&c.carried, |(p, _)| p);
            digest = continued_digest(&c.first.plan, i + 1, digest, p, v, next)?;
        }
        digest = continued_digest(
            &c.first.plan,
            c.plan.stage(),
            digest,
            &c.carried,
            &c.vesta,
            &c.pallas,
        )?;
        return internal_public(digest, &c.vesta.as_input());
    }
    let mut words = vec![
        terminal_digest(
            &c.first.source.maps.witness.successor.lineage,
            &c.pallas.as_input(),
        )?,
        Fp::from(16),
    ];
    words.extend(vesta_words(&c.vesta.as_input())?);
    words.extend(vesta_words(&c.first.source.predecessor.vesta.as_input())?);
    let trivial = trivial_vesta_words()?;
    words.extend(&trivial);
    words.extend([Fp::ZERO, Fp::ONE, Fp::ZERO]);
    words.extend(&trivial[..4]);
    if words.len() != 69 {
        return Err(Error::Input);
    }
    Ok(words)
}

fn terminal_digest(public: &[Fp; 18], pallas: &FoldInput<Ep>) -> Result<Fp, Error> {
    let mut words = public.to_vec();
    let mut claim = Vec::new();
    push_pallas(&mut claim, pallas)?;
    words.extend(&claim[1..]);
    if words.len() != 52 {
        return Err(Error::Input);
    }
    Ok(hash_with_domain(super::super::LINEAGE_DOMAIN, &words))
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

fn frame(bytes: &[u8]) -> Result<Vec<u8>, LayoutError> {
    let mut out = u32::try_from(bytes.len())
        .map_err(|_| LayoutError::BoundsFailure)?
        .to_le_bytes()
        .to_vec();
    out.extend(bytes);
    Ok(out)
}
fn lineage_bytes(
    fields: &[Fp; 18],
    proof: &[u8],
    pallas: &AccumulatorT<Ep>,
    vesta: &AccumulatorT<Eq>,
) -> Result<Vec<u8>, Error> {
    if fields[0] != Fp::ONE {
        return Err(Error::Input);
    }
    let mut out = 1_u16.to_le_bytes().to_vec();
    let mut push128 = |word: Fp| -> Result<(), Error> {
        let bytes = word.to_repr();
        if bytes[16..] != [0; 16] {
            return Err(Error::Input);
        }
        out.extend(&bytes[..16]);
        Ok(())
    };
    for i in [1, 2, 3, 4] {
        push128(fields[i])?;
    }
    out.extend(fields[5].to_repr());
    for i in [6, 7] {
        let bytes = fields[i].to_repr();
        if bytes[16..] != [0; 16] {
            return Err(Error::Input);
        }
        out.extend(&bytes[..16]);
    }
    out.extend(fields[8].to_repr());
    out.push(4);
    for i in [10, 9, 12, 11] {
        let bytes = fields[i].to_repr();
        if bytes[16..] != [0; 16] {
            return Err(Error::Input);
        }
        out.extend(bytes[..16].iter().rev());
    }
    let policy = fields[13].to_repr();
    if policy[13..] != [0; 19] {
        return Err(Error::Input);
    }
    out.extend(&policy[..13]);
    let burned = fields[14].to_repr();
    if burned[16..] != [0; 16] {
        return Err(Error::Input);
    }
    out.extend(&burned[..16]);
    out.extend(fields[15].to_repr());
    out.extend(fields[16].to_repr());
    if out.len() != 320 {
        return Err(Error::Input);
    }
    out.extend(proof);
    out.extend(pallas.to_bytes());
    out.extend(vesta.to_bytes());
    Ok(out)
}

fn check_omega_transport_length(proof: usize) -> Result<(), Error> {
    if proof == 0
        || !proof.is_multiple_of(32)
        || proof
            .checked_add(2 * iroha_plonk_recursion::ACCUMULATOR_BYTES)
            .is_none_or(|transport| transport > OMEGA_TRANSPORT_CAP)
    {
        return Err(Error::Input);
    }
    Ok(())
}

impl Prepared {
    fn verify_sources(&self, budget: MemoryBudget) -> Result<(), Error> {
        let source = &self.source;
        let fixed = self
            .plan
            .context
            .operation()
            .omega()
            .ok_or(Error::Artifact)?;
        source
            .predecessor
            .pallas
            .decide(&self.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        source
            .predecessor
            .vesta
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        source
            .predecessor
            .opening
            .decide(&self.plan.pallas, budget)
            .map_err(|_| Error::Proof)?;
        let public = omega_instances(
            terminal_digest(
                &source.maps.witness.predecessor.lineage,
                &source.predecessor.pallas.as_input(),
            )?,
            &source.predecessor.vesta,
        )?;
        verify_full(
            &self.plan.pallas,
            fixed.binding(),
            &source.predecessor.key,
            &public,
            &source.predecessor.proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let actual = opening_pallas(
            &self.plan.pallas,
            fixed.binding(),
            &source.predecessor.key,
            &public,
            &source.predecessor.proof,
            budget,
        )?;
        let same = |a: &FoldInput<Ep>, b: &FoldInput<Ep>| {
            a.source_k() == b.source_k() && a.g() == b.g() && a.challenges() == b.challenges()
        };
        if !same(&actual, &source.predecessor.opening) {
            return Err(Error::Proof);
        }
        if source.q_instances.len() != self.plan.context.operation().q_count()
            || source.q_proofs.len() != source.q_instances.len()
            || source.q_openings.len() != source.q_instances.len()
        {
            return Err(Error::Input);
        }
        for index in 0..source.q_instances.len() {
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
                &source.q_instances[index],
                &source.q_proofs[index],
                budget,
            )
            .map_err(|_| Error::Proof)?;
            let actual = opening_pallas(
                fixed.verifier().params(),
                fixed.verifier().binding(),
                &fixed.key,
                &source.q_instances[index],
                &source.q_proofs[index],
                budget,
            )?;
            if !same(&actual, &source.q_openings[index]) {
                return Err(Error::Proof);
            }
        }
        source
            .part
            .decide(&self.plan.vesta, budget)
            .map_err(|_| Error::Proof)?;
        let q = QInput {
            proof: source.q_proofs[0].clone(),
            instances: source.q_instances[0].clone(),
        };
        let actual = q_sigma_part(
            &q,
            self.plan.context.operation().frame().part_source_k() as u32,
        )?;
        if actual.source_k() != source.part.source_k()
            || actual.g() != source.part.g()
            || actual.challenges() != source.part.challenges()
        {
            return Err(Error::Proof);
        }
        Ok(())
    }
}
