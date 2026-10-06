//! Genuine native Send five-A/four-W composition from exact original tapes.
//!
//! Every hard predecessor, own sigma, own authorization, depth32 map and carried
//! opening is retained. Runtime sessions use installed keys and no witness-selected
//! profile. The native pre-Advance owner verifies the recipient Credential and
//! Request signature separately; Send A binds the exact signed Request digest.
//! TODO: mount complete native G1 preparation and qualify all eight control masks,
//! producer catalog and final Omega before foreign wallet open.

use core::fmt;
use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_pasta::{
    Ep, Eq, EqAffine, Fp, Fq, PastaAffine, msm::MemoryBudget, poseidon::hash_with_domain,
};
use iroha_plonk::{
    DescriptorBinding, ProverConfig, ProverRandomness, ProvingKey, VerifyingKey, Witness,
    create_proof_owned_with_claim,
    cs::{Column, ConstraintSystem, Instance, InstanceType},
    frontend::{Circuit, Error as LayoutError, Layouter, Region, SimpleFloorPlanner, Value},
    pcs::ipa::PinnedParams,
    verifier::{accumulate_generator, verify_full},
};
use iroha_plonk_gadgets::{
    UintChip,
    bytes::{
        element::le_message_segments,
        le_value, p_bytes_native,
        tape::{BytesChip, BytesConfig, SegmentSpec},
    },
    imt::{LeafCells, OpeningCells, PathCells},
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
    own::ConsumingProofCells,
    own::OwnPolicy,
    schedule::OperationTask,
    send::{
        SendAuthorization, SendAuthorizationObjects, SendInputs, SendObjects, SendProofBinding,
        SendStagePlan, SendStageWitness,
    },
    split::{SplitPlan, WCircuit, WKey, close_first},
    verify_predecessor, verify_sigma,
};
use crate::{
    admin_sigma::StateWitness,
    omega::OmegaWitness,
    operation_relation::{
        map_effects::{InsertCells, MapState},
        objects::ObjectKind,
        state::StateCells,
        statement::StatementCells,
    },
    q_signature::{QSignaturePlan, SignatureKey},
    tree::IndexedInsert,
};

#[cfg(test)]
#[path = "send/tests.rs"]
mod tests;

/// Fixed native source profile; private inputs cannot choose another range-bus count.
pub const SOURCE_RANGE_BUSES: usize = 4;
/// Exact genuine source schedule has five A stages and four W continuations.
pub const A_STAGE_COUNT: usize = 5;

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
        write!(f, "native Send: {self:?}")
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

/// Exact canonical source state openings and Send statement.
#[derive(Clone, Copy, Debug)]
pub struct SendState {
    /// Hard predecessor core33/rest8/public18.
    pub before: StateWitness,
    /// Exact successor core33/rest8/public18.
    pub after: StateWitness,
    /// Original statement26.
    pub statement: [Fp; 26],
}
/// Original post-Advance inputs converted by native canonical G1 preparation.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Same state openings and statement used by sigma and every A stage.
    pub state: SendState,
    /// Descriptor-sized original sigma with every selected control proved.
    pub sigma: Vec<u8>,
    /// Original320-byte public predecessor transcript followed by proof/P/V originals.
    pub omega: Vec<u8>,
    /// Current payer Credential, signed Request, fee slot, Enrollment certificate, Receipt.
    pub objects: [Vec<u8>; 5],
    /// Actual depth32 pending insertion witness.
    pub pending: IndexedInsert<Fp>,
    /// Actual depth32 fee insertion or explicit zero-fee no-op witness.
    pub fee: IndexedInsert<Fp>,
    /// Q_sigma and hard receipt/currentCredential/Enrollment Q.
    pub q: [QInput; 2],
    /// Actual hard predecessor proof and transported claims.
    pub predecessor: PredecessorInput,
}

/// Immutable complete Send circuit metadata from the authenticated native artifact owner.
#[derive(Clone, Debug)]
pub struct Plan {
    context: ContextPlan,
    mask: u8,
    policy: OwnPolicy,
    signatures: QSignaturePlan,
    predecessor_key: VerifyingKey<Ep>,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin the fixed five-stage [[],[],[],[Q0],[Q1]] genuine source schedule.
    /// Signature slots are hard [receipt,current Credential,Enrollment certificate],
    /// with the certificate under the fixed installed scheme root.
    /// # Errors
    /// Wrong variant/k/schema, absent predecessor or substituted fixed signature/key policy.
    pub fn new(
        operation: AProofPlan,
        mask: u8,
        policy: OwnPolicy,
        signatures: QSignaturePlan,
        predecessor_key: VerifyingKey<Ep>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
    ) -> Result<Self, Error> {
        if mask > 7
            || operation.frame().variant() != Variant::Send
            || !matches!(operation.frame().part_source_k(), 12 | 14)
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
                != operation.frame().part_source_k() as u8
        {
            return Err(Error::Artifact);
        }
        operation
            .sigma
            .class(0)
            .ok_or(Error::Artifact)?
            .selector_key_digest(2 + mask)
            .ok_or(Error::Artifact)?;
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
        let slots = signatures.slots();
        if slots.len() != 3
            || slots.iter().any(|s| s.mode != VerifyMode::Hard)
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
            vec![vec![], vec![], vec![], vec![0], vec![1]],
            Some(0),
            SendStagePlan::context_specs()
                .map_err(|_| Error::Artifact)?
                .to_vec(),
        )
        .and_then(|p| {
            p.with_operation_tasks(vec![
                vec![OperationTask::SendObjects, OperationTask::SendProof],
                vec![OperationTask::SendPending],
                vec![OperationTask::SendFeeAndCarry],
                vec![],
                vec![OperationTask::SendAuthorization],
            ])
        })
        .map_err(|_| Error::Artifact)?;
        SendStagePlan::new(context.clone()).map_err(|_| Error::Artifact)?;
        Ok(Self {
            context,
            mask,
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
        check_sigma_tape(&input, self.mask)?;
        if input.state.statement[11] != Fp::from(u64::from(self.mask))
            || input.state.before.core[21] != Fp::from(u64::from(self.mask))
        {
            return Err(Error::Input);
        }
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
        if input.state.before.lineage[17] != digest {
            return Err(Error::Input);
        }
        let own = terminal_digest(&input.state.before.lineage, &pallas.as_input())?;
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
        let part = q_sigma_part(
            &input.q[0],
            self.context.operation().frame().part_source_k(),
        )?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        if input.omega
            != lineage_bytes(
                &input.state.before.lineage,
                &input.predecessor.proof,
                &pallas,
                &vesta,
            )?
        {
            return Err(Error::Input);
        }
        let maps = Maps {
            witness: MapsWitness {
                before: input.state.before,
                after: input.state.after,
                statement: input.state.statement,
                pending: input.pending,
                fee: input.fee,
                objects: core::array::from_fn(|i| input.objects[i].clone()),
            },
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
            own_objects: core::array::from_fn(|i| SignedTape {
                bytes: input.objects[i + 3].clone(),
            }),
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
    own_objects: [SignedTape; 2],
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
    bytes: Vec<u8>,
}
#[derive(Clone)]
struct MapsWitness {
    before: StateWitness,
    after: StateWitness,
    statement: [Fp; 26],
    pending: IndexedInsert<Fp>,
    fee: IndexedInsert<Fp>,
    objects: [Vec<u8>; 3],
}
#[derive(Clone)]
struct Maps {
    witness: MapsWitness,
    known: bool,
}
impl Maps {
    fn value<T: Copy>(&self, v: T) -> Value<T> {
        if self.known {
            Value::known(v)
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
    fn insertion(
        &self,
        uint: &mut UintChip<'_, Fp>,
        region: &mut Region<'_, Fp>,
        witness: &IndexedInsert<Fp>,
    ) -> Result<InsertCells, LayoutError> {
        let leaf = witness.leaf;
        let words = uint
            .glue()
            .witnesses(
                region,
                &[leaf.key, leaf.value, leaf.next_key].map(|v| self.value(v)),
            )?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        let leaf = LeafCells::from_words(words);
        let mut paths = Vec::new();
        for (index, siblings) in [
            (witness.leaf_slot, witness.leaf_siblings),
            (witness.slot, witness.slot_siblings),
        ] {
            let index = uint
                .glue()
                .witness(region, self.value(Fp::from(u64::from(index))))?;
            let siblings = uint
                .glue()
                .witnesses(region, &siblings.map(|v| self.value(v)))?
                .try_into()
                .map_err(|_| LayoutError::Synthesis)?;
            paths.push(PathCells::from_words(uint, region, &index, siblings)?);
        }
        let [low, slot] = paths.try_into().map_err(|_| LayoutError::Synthesis)?;
        Ok(InsertCells {
            low: OpeningCells { leaf, path: low },
            slot,
        })
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
#[derive(Clone, Debug)]
/// Fixed source-stage columns and range buses.
pub struct StageConfig {
    verifier: VerifierConfig<Ep>,
    bytes: BytesConfig,
    public: Column<Instance>,
}
impl First {
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
        let [lo, hi] = foreign_limbs(&v);
        let lo = chip.uint().assign::<128>(region, self.value(lo))?;
        let hi = chip.uint().assign::<127>(region, self.value(hi))?;
        ScalarCells::from_limbs(&mut chip.uint(), region, &lo, &hi)
    }
    fn pallas(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: &FoldInput<Ep>,
    ) -> Result<FoldInputCells<Ep>, LayoutError> {
        let g = chip.witness_point(region, self.value(Ep::from(*input.g())))?;
        let challenges = input
            .challenges()
            .iter()
            .map(|v| self.scalar(chip, region, *v))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| LayoutError::Synthesis)?;
        FoldInputCells::from_normalized(chip, region, 16, g, challenges)
    }
    fn vesta(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        input: &AccumulatorT<Eq>,
    ) -> Result<VestaClaimCells, LayoutError> {
        let (x, y) = Option::from(input.g().coordinates()).ok_or(LayoutError::Synthesis)?;
        let coordinates = [self.scalar(chip, region, x)?, self.scalar(chip, region, y)?];
        let challenges = chip
            .uint()
            .glue()
            .witnesses(region, &input.challenges().map(|v| self.value(v)))?
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
        let index = crate::a_relation::schedule::constrain_sigma_selector(
            &mut chip.uint(),
            region,
            3,
            &statement.fields()[11],
        )?;
        SigmaBindingCells::from_run(chip, region, statement, index, &run)
    }
}
impl First {
    fn objects(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
    ) -> Result<
        (
            SendObjects,
            SendAuthorizationObjects,
            Vec<crate::a_relation::context::ContextObjectCells>,
        ),
        LayoutError,
    > {
        let sources = self
            .source
            .maps
            .witness
            .objects
            .each_ref()
            .map(|o| o.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
        let objects =
            SendObjects::decode(chip, bytes, region, sources.each_ref().map(Vec::as_slice))?;
        let sources = self
            .source
            .own_objects
            .each_ref()
            .map(|o| o.bytes.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
        let own = SendAuthorizationObjects::decode(
            chip,
            bytes,
            region,
            sources.each_ref().map(Vec::as_slice),
        )?;
        let mut context = objects.context().to_vec();
        context.extend_from_slice(own.context());
        Ok((objects, own, context))
    }
}

impl Circuit<Fp> for First {
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
            .expect("fixed four-bus native Send profile");
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
            || "genuine Send predecessor-first relation",
            |mut region| {
                let mut maps = self.source.maps.clone();
                maps.known = self.known;
                let (old, pred_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.before)?;
                let (new, next_public) = maps.state(&mut chip, &mut region, &maps.witness.after)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Send,
                    &fields,
                )?;
                let sigma = self.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let (objects, auth_objects, context_objects) =
                    self.objects(&mut chip, &mut bytes, &mut region)?;
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
                    &self.source.predecessor.pallas.as_input(),
                )?;
                let pv = self.vesta(&mut chip, &mut region, &self.source.predecessor.vesta)?;
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
                    objects: &context_objects,
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
                SendStagePlan::new(self.plan.clone())?.constrain_stage(
                    &mut chip,
                    &mut region,
                    0,
                    &objects,
                    SendInputs {
                        predecessor: MapState {
                            state: &old,
                            lineage: &pred_public,
                        },
                        successor: MapState {
                            state: &new,
                            lineage: &next_public,
                        },
                        sigma: &sigma,
                    },
                    SendStageWitness {
                        proof: Some(SendProofBinding {
                            objects: &auth_objects,
                            proof: &consuming,
                        }),
                        ..SendStageWitness::default()
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
impl Circuit<Fp> for Continuation {
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
        First::configure(meta)
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
            || "Send deferred Q continuation",
            |mut region| {
                let mut maps = first.source.maps.clone();
                maps.known = first.known;
                let (old, pred_public) =
                    maps.state(&mut chip, &mut region, &maps.witness.before)?;
                let (new, next_public) = maps.state(&mut chip, &mut region, &maps.witness.after)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut region, &maps.witness.statement.map(|v| first.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut region,
                    Variant::Send,
                    &fields,
                )?;
                let sigma = first.sigma(&mut chip, &mut bytes, &mut region, &statement)?;
                let (objects, auth_objects, context_objects) =
                    first.objects(&mut chip, &mut bytes, &mut region)?;
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
                    &first.source.predecessor.pallas.as_input(),
                )?;
                let pv = first.vesta(&mut chip, &mut region, &first.source.predecessor.vesta)?;
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
                    objects: &context_objects,
                    modes: &[],
                    pallas_corrections: &[],
                    vesta_corrections: &[],
                    receive_results: None,
                };
                let cp = first.pallas(&mut chip, &mut region, &self.carried.as_input())?;
                let history = self
                    .history
                    .iter()
                    .map(|(p, v)| {
                        Ok(crate::a_relation::split::ContextLinkCells {
                            pallas: first.pallas(&mut chip, &mut region, &p.as_input())?,
                            vesta: first.vesta(&mut chip, &mut region, v)?,
                        })
                    })
                    .collect::<Result<Vec<_>, LayoutError>>()?;
                let cv = first.vesta(&mut chip, &mut region, &self.vesta)?;
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
                        SendStagePlan::new(first.plan.clone())?.constrain_stage(
                            &mut chip,
                            &mut region,
                            self.plan.stage(),
                            &objects,
                            SendInputs {
                                predecessor: MapState {
                                    state: &old,
                                    lineage: &pred_public,
                                },
                                successor: MapState {
                                    state: &new,
                                    lineage: &next_public,
                                },
                                sigma: &sigma,
                            },
                            SendStageWitness {
                                authorization: Some(SendAuthorization {
                                    objects: &auth_objects,
                                    policy: first.source.policy,
                                    signature: crate::a_relation::SignatureQContext {
                                        bundle: &slots,
                                        instances: columns,
                                    },
                                }),
                                ..SendStageWitness::default()
                            },
                        )?;
                        slots.verified().clone()
                    };
                    verified.push(q);
                }
                if !first
                    .plan
                    .q_partition(self.plan.stage())
                    .ok_or(LayoutError::Synthesis)?
                    .contains(&1)
                {
                    let tasks = first
                        .plan
                        .operation_tasks(self.plan.stage())
                        .ok_or(LayoutError::Synthesis)?;
                    let pending = if tasks.contains(&OperationTask::SendPending) {
                        Some(maps.insertion(
                            &mut chip.uint(),
                            &mut region,
                            &maps.witness.pending,
                        )?)
                    } else {
                        None
                    };
                    let fee = if tasks.contains(&OperationTask::SendFeeAndCarry) {
                        Some(maps.insertion(&mut chip.uint(), &mut region, &maps.witness.fee)?)
                    } else {
                        None
                    };
                    SendStagePlan::new(first.plan.clone())?.constrain_stage(
                        &mut chip,
                        &mut region,
                        self.plan.stage(),
                        &objects,
                        SendInputs {
                            predecessor: MapState {
                                state: &old,
                                lineage: &pred_public,
                            },
                            successor: MapState {
                                state: &new,
                                lineage: &next_public,
                            },
                            sigma: &sigma,
                        },
                        SendStageWitness {
                            pending: pending.as_ref(),
                            fee: fee.as_ref(),
                            ..SendStageWitness::default()
                        },
                    )?;
                }
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
    First(First),
    Continued(Continuation),
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
        First::configure(meta)
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

/// Checked original source coupled to the fixed complete Send plan.
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
                inner: StageData::First(first),
            },
            public,
        ))
    }
}

/// Already installed A1/A2/A3/A4/A5 and W0/W1/W2/W3 proving artifacts for this fixed profile.
/// Authentication and genuine PK import remain the native package owner's responsibility.
#[derive(Clone)]
pub struct Prover {
    plan: Plan,
    a: [Arc<ProvingKey<Eq>>; 5],
    w: [Arc<ProvingKey<Ep>>; 4],
    wrappers: [WKey; 4],
}
impl Prover {
    /// Import the complete fixed typed artifact set, never generating keys from a witness.
    /// # Errors
    /// Nonuniform A descriptors, wrong k/public schema, or wrong W stage/context identity.
    pub fn from_artifacts(
        plan: Plan,
        a: [Arc<ProvingKey<Eq>>; 5],
        w: [Arc<ProvingKey<Ep>>; 4],
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
        let wrappers = (0..4)
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
    /// Exact installed descriptors in actual A1/W0/A2/W1/A3/W2/A4/W3/A5 checkpoint order.
    #[must_use]
    pub fn descriptors(&self) -> [&DescriptorBinding; 9] {
        [
            self.a[0].binding(),
            self.w[0].binding(),
            self.a[1].binding(),
            self.w[1].binding(),
            self.a[2].binding(),
            self.w[2].binding(),
            self.a[3].binding(),
            self.w[3].binding(),
            self.a[4].binding(),
        ]
    }
}

/// Source-bound session over the fixed installed Send artifacts.
pub struct Session<'a> {
    prover: &'a Prover,
    prepared: Prepared,
}
impl Session<'_> {
    fn require_source(&self, source: &ACheckpoint) -> Result<(), Error> {
        if source.stage >= 5 || !Arc::ptr_eq(&source.first.source, &self.prepared.source) {
            return Err(Error::Input);
        }
        Ok(())
    }
    fn verify_a(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<(), Error> {
        self.require_source(source)?;
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
        let first = self.prepared.first(salt, fold)?;
        let public = first_public(&first)?;
        let circuit = StageCircuit {
            inner: StageData::First(first.clone()),
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
        if source.stage >= 4 {
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
        if source.stage >= 4 {
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
        if previous >= 4 {
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
            inner: StageData::Continued(continuation),
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
    /// Restore A2/A3/A4/A5 from its verified source W and canonical new Pallas claim.
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
    /// Export A5 and all distinct final Omega obligations after another full native check.
    /// This grants neither monetary completion nor a final transported Omega.
    /// # Errors
    /// A nonterminal/wrong source checkpoint or any failed native proof/decide.
    pub fn terminal(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<Terminal, Error> {
        self.verify_a(source, budget)?;
        if source.stage != 4 {
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
/// Actual A5 proof and every distinct obligation consumed by the final Omega producer.
#[derive(Clone, Debug)]
pub struct Terminal {
    /// Original terminal A5 proof under its installed key.
    pub proof: Vec<u8>,
    /// Exact homogeneous69-word public frame.
    pub instances: Vec<Fp>,
    /// Full accumulated Pallas claim.
    pub pallas: AccumulatorT<Ep>,
    /// Full Vesta part forwarded by W2.
    pub vesta_part: AccumulatorT<Eq>,
    /// Full predecessor Vesta obligation, distinct from the current part and A opening.
    pub predecessor_vesta: AccumulatorT<Eq>,
    /// Actual terminal A5 own opening, a separate final Omega slot.
    pub opening: FoldInput<Eq>,
}

fn object_kinds() -> [ObjectKind; 5] {
    [
        ObjectKind::Credential,
        ObjectKind::Request,
        ObjectKind::FeeSchedule,
        ObjectKind::Certificate,
        ObjectKind::Receipt,
    ]
}

fn check_sigma_tape(input: &Inputs, mask: u8) -> Result<(), Error> {
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
        || input.q[0].instances.get(2).map(Vec::as_slice) != Some(&[Fq::from(2 + u64::from(mask))])
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
    let old = &source.maps.witness.before;
    let new = &source.maps.witness.after;
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
    let originals = source
        .maps
        .witness
        .objects
        .iter()
        .cloned()
        .chain(source.own_objects.iter().map(|o| o.bytes.clone()));
    for ((raw, kind), spec) in originals
        .zip(object_kinds())
        .zip(SendStagePlan::context_specs().map_err(|_| Error::Artifact)?)
    {
        if raw.len() != usize::try_from(spec.capacity).map_err(|_| Error::Input)? {
            return Err(Error::Input);
        }
        let mut bytes = spec.capacity.to_le_bytes().to_vec();
        bytes.extend(&raw);
        let mut tape = vec![
            Fp::from(u64::from(spec.tag)),
            Fp::from(u64::from(spec.capacity)),
        ];
        for chunk in bytes.chunks(31) {
            tape.push(le_value::<Fp>(chunk).ok_or(Error::Input)?);
        }
        words.extend([
            object_digest(kind, &raw)?,
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
            &c.first.source.maps.witness.after.lineage,
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

/// Complete immutable eight-mask Send source catalog. Construction is not release qualification.
pub struct Catalog {
    masks: [Prover; 8],
}
impl Catalog {
    /// Require exactly one installed source pipeline for each global Send selector2..9.
    /// # Errors
    /// A missing/reordered mask or another scheme/provider/root policy.
    pub fn new(masks: [Prover; 8]) -> Result<Self, Error> {
        let first = masks[0].plan.policy;
        for (mask, prover) in masks.iter().enumerate() {
            if usize::from(prover.plan.mask) != mask
                || prover.plan.policy.scheme != first.scheme
                || prover.plan.policy.provider != first.provider
                || prover.plan.policy.root != first.root
            {
                return Err(Error::Artifact);
            }
        }
        Ok(Self { masks })
    }
    /// Select from the exact original statement mask, then verify its predecessor/core and proofs.
    /// # Errors
    /// Noncanonical mask, differing opened core, source/tape or native proof/decide failure.
    pub fn prepare(&self, input: Inputs, budget: MemoryBudget) -> Result<Session<'_>, Error> {
        let repr = input.state.statement[11].to_repr();
        if repr[0] > 7 || repr[1..] != [0; 31] {
            return Err(Error::Input);
        }
        self.masks[usize::from(repr[0])].prepare(input, budget)
    }
}
