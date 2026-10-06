//! Installed native Refresh composition with exact signed originals and fixed owners.
//!
//! Each variant pins its source schedule, installed keys and every Q/map obligation.
//! Checkpoints retain original proof bytes and full accumulator claims. No operation
//! is monetarily complete before its admitted final Omega and durable wallet commit.
//! TODO: qualify native parity and final wrappers under the full terminal catalog;
//! the eight-stage Quota layout additionally requires actual recursive qualification.

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
    bytes::{
        element::le_message_segments,
        le_value, p_bytes_native,
        tape::{BytesChip, BytesConfig, SegmentSpec},
    },
    imt::{LeafCells, OpeningCells, PathCells},
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
    own::OwnPolicy,
    refresh::{QuotaCommitmentCells, RefreshObjects, RefreshStagePlan, RefreshStageWitness},
    schedule::OperationTask,
    split::{SplitPlan, WCircuit, WKey, close_first},
    verify_predecessor, verify_sigma,
};
use crate::{
    admin_sigma::{RefreshWitness, StateWitness},
    omega::OmegaWitness,
    operation_relation::{
        map_effects::InsertCells, objects::ObjectKind, quota_refresh::QuotaRebuildCells,
        state::StateCells, statement::StatementCells,
    },
    q_signature::QSignaturePlan,
    tree::IndexedInsert,
};

#[cfg(test)]
#[path = "refresh/tests.rs"]
mod tests;

/// Fixed native source profile; private inputs cannot choose another range-bus count.
pub const SOURCE_RANGE_BUSES: usize = 3;
/// Ordinary Refresh kinds have four source A stages and three W continuations.
pub const BASIC_A_STAGE_COUNT: usize = 4;
/// Quota adds three authenticated root owners and one exact matching owner.
pub const QUOTA_A_STAGE_COUNT: usize = 8;

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
        write!(f, "native refresh: {self:?}")
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

/// Exact fixed64 arrays carried across the mandatory quota root/merge owners.
#[derive(Clone, Debug)]
pub struct QuotaInput {
    /// Previous `(kind,start,end,used)` leaves in canonical slot order.
    pub old: [[Fp; 4]; 64],
    /// Signed replacement `(kind,start,end,limit)` windows in slot order.
    pub windows: [[Fp; 4]; 64],
    /// Successor usage aligned with the replacement windows.
    pub used: [Fp; 64],
    /// Original signed share issue time.
    pub issued: Fp,
    /// Original signed count of non-padding windows.
    pub window_count: Fp,
}
impl QuotaInput {
    fn commitments(&self) -> [Fp; 5] {
        let hash = |tag: u64, count: u64, fields: Vec<Fp>| {
            let mut words = vec![Fp::from(tag), Fp::from(count)];
            words.extend(fields);
            hash_with_domain(u64::from_le_bytes(*b"kgwciw_1"), &words)
        };
        [
            hash(7, 256, self.old.iter().flatten().copied().collect()),
            hash(8, 256, self.windows.iter().flatten().copied().collect()),
            hash(9, 64, self.used.to_vec()),
            self.issued,
            self.window_count,
        ]
    }
}

/// Post-Advance originals for the complete fixed Refresh relation.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// Exact predecessor/successor core33/rest8/public18 and own statement26.
    pub state: RefreshWitness,
    /// Original unframed sigma also exported by Q0.
    pub sigma: Vec<u8>,
    /// Update certificate/update/receipt/current certificate/current credential.
    pub objects: [Vec<u8>; 5],
    /// Exact history insertion, present only for Blacklist.
    pub blacklist: Option<IndexedInsert<Fp>>,
    /// Exact arrays and signed issue/count, present only for Quota.
    pub quota: Option<QuotaInput>,
    /// Own sigma Q, update/receipt/certificate Q, current credential/certificate Q.
    pub q: [QInput; 3],
    /// Actual predecessor proof and both original transported claims.
    pub predecessor: PredecessorInput,
}

/// Immutable complete Refresh circuit metadata from the authenticated native artifact owner.
#[derive(Clone, Debug)]
pub struct Plan {
    context: ContextPlan,
    policy: OwnPolicy,
    signatures: [QSignaturePlan; 2],
    predecessor_key: VerifyingKey<Ep>,
    pallas: PinnedParams<Ep>,
    vesta: PinnedParams<Eq>,
}
impl Plan {
    /// Pin the exact four- or eight-stage schedule and both hard signature schemas.
    /// The variant is an installed key property, never chosen by private data.
    /// # Errors
    /// Wrong variant/k/schema, absent predecessor or foreign fixed artifact shape.
    pub fn new(
        operation: AProofPlan,
        policy: OwnPolicy,
        predecessor_key: VerifyingKey<Ep>,
        pallas: PinnedParams<Ep>,
        vesta: PinnedParams<Eq>,
    ) -> Result<Self, Error> {
        let variant = operation.frame().variant();
        let specs = RefreshObjects::context_specs(variant).map_err(|_| Error::Artifact)?;
        if operation.frame().part_source_k() != 12
            || operation.q_count() != 3
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
        let signatures =
            RefreshStagePlan::signature_schemas(policy).map_err(|_| Error::Artifact)?;
        let (partition, tasks) = schedule(variant)?;
        let context = ContextPlan::with_schedule(operation, partition, Some(0), specs)
            .and_then(|c| c.with_operation_tasks(tasks))
            .map_err(|_| Error::Artifact)?;
        RefreshStagePlan::new(context.clone(), policy).map_err(|_| Error::Artifact)?;
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
        for (kind, raw) in object_kinds(self.context.operation().frame().variant())?
            .into_iter()
            .zip(&input.objects)
        {
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
        let variant = self.context.operation().frame().variant();
        if input.blacklist.is_some() != (variant == Variant::RefreshBlacklist)
            || input.quota.is_some() != (variant == Variant::RefreshQuotaShare)
        {
            return Err(Error::Input);
        }
        check_sigma_tape(&input, variant)?;
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
        let part = q_sigma_part(&input.q[0], 12, variant)?;
        part.decide(&self.vesta, budget).map_err(|_| Error::Proof)?;
        let kinds = object_kinds(variant)?;
        let objects = core::array::from_fn(|i| SignedTape {
            kind: kinds[i],
            bytes: input.objects[i].clone(),
        });
        let maps = Maps {
            witness: input.state,
            blacklist: input.blacklist,
            quota: input.quota,
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
            objects,
            variant,
            sigma: input.sigma,
            q_instances: input.q.each_ref().map(|q| q.instances.clone()).to_vec(),
            q_proofs: input.q.each_ref().map(|q| q.proof.clone()).to_vec(),
            q_openings: openings,
            signature_schemas: self.signatures.clone(),
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
    objects: [SignedTape; 5],
    variant: Variant,
    sigma: Vec<u8>,
    q_instances: Vec<Vec<Vec<Fq>>>,
    q_proofs: Vec<Vec<u8>>,
    q_openings: Vec<FoldInput<Ep>>,
    signature_schemas: [QSignaturePlan; 2],
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
struct AssignedMaps {
    blacklist: Option<InsertCells>,
    quota: Option<QuotaRebuildCells>,
}
#[derive(Clone)]
struct Maps {
    witness: RefreshWitness,
    blacklist: Option<IndexedInsert<Fp>>,
    quota: Option<QuotaInput>,
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
    pub(crate) fn state(
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
    fn assign(
        &self,
        chip: &mut VerifierChip<Ep>,
        r: &mut Region<'_, Fp>,
        task: &[OperationTask],
    ) -> Result<AssignedMaps, LayoutError> {
        let blacklist = if task.contains(&OperationTask::RefreshBlacklist) {
            let proof = self.blacklist.ok_or(LayoutError::Synthesis)?;
            let fields = [proof.leaf.key, proof.leaf.value, proof.leaf.next_key];
            let leaf = LeafCells::from_words(
                chip.uint()
                    .glue()
                    .witnesses(r, &fields.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?,
            );
            let mut paths = Vec::new();
            for (index, siblings) in [
                (proof.leaf_slot, proof.leaf_siblings),
                (proof.slot, proof.slot_siblings),
            ] {
                let index = chip
                    .uint()
                    .glue()
                    .witness(r, self.value(Fp::from(u64::from(index))))?;
                let siblings = chip
                    .uint()
                    .glue()
                    .witnesses(r, &siblings.map(|v| self.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?;
                paths.push(PathCells::from_words(
                    &mut chip.uint(),
                    r,
                    &index,
                    siblings,
                )?);
            }
            let [low, slot] = paths.try_into().map_err(|_| LayoutError::Synthesis)?;
            Some(InsertCells {
                low: OpeningCells { leaf, path: low },
                slot,
            })
        } else {
            None
        };
        let quota = if task.iter().any(|task| {
            matches!(
                task,
                OperationTask::RefreshQuotaMerge
                    | OperationTask::RefreshQuotaPreviousRoot
                    | OperationTask::RefreshQuotaWindowRoot
                    | OperationTask::RefreshQuotaUsageRoot
            )
        }) {
            let q = self.quota.as_ref().ok_or(LayoutError::Synthesis)?;
            let values = q
                .old
                .iter()
                .flatten()
                .chain(q.windows.iter().flatten())
                .chain(&q.used)
                .chain([&q.issued, &q.window_count])
                .map(|v| self.value(*v))
                .collect::<Vec<_>>();
            let words = chip.uint().glue().witnesses(r, &values)?;
            Some(QuotaRebuildCells {
                old: ::core::array::from_fn(|i| {
                    ::core::array::from_fn(|j| words[4 * i + j].clone())
                }),
                windows: ::core::array::from_fn(|i| {
                    ::core::array::from_fn(|j| words[256 + 4 * i + j].clone())
                }),
                used: ::core::array::from_fn(|i| words[512 + i].clone()),
                issued: words[576].clone(),
                window_count: words[577].clone(),
            })
        } else {
            None
        };
        Ok(AssignedMaps { blacklist, quota })
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
/// Fixed Tagged3 verifier, exact byte tape and homogeneous public columns.
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
        let index = chip.uint().glue().constant(
            region,
            Fp::from(u64::from(
                super::super::schedule::sigma_selector(7, 0).ok_or(LayoutError::Synthesis)?,
            )),
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
        stage: usize,
    ) -> Result<RefreshObjects, LayoutError> {
        if RefreshObjects::quota_root_only(&self.plan, stage) {
            let quota = self
                .source
                .maps
                .quota
                .as_ref()
                .ok_or(LayoutError::Synthesis)?;
            let fields = chip
                .uint()
                .glue()
                .witnesses(region, &quota.commitments().map(|v| self.value(v)))?;
            let proposal = QuotaCommitmentCells {
                previous_usage: fields[0].clone(),
                windows: fields[1].clone(),
                successor_usage: fields[2].clone(),
                issued: fields[3].clone(),
                window_count: fields[4].clone(),
            };
            let claims = object_claims(&self.source)
                .map_err(|_| LayoutError::Synthesis)?
                .into_iter()
                .map(|triple| triple.map(|v| self.value(v)))
                .collect::<Vec<_>>();
            return RefreshObjects::quota_root_claims(
                chip, region, &self.plan, stage, &claims, &proposal,
            );
        }
        let sources = self
            .source
            .objects
            .each_ref()
            .map(|o| o.bytes.iter().map(|v| self.value(*v)).collect::<Vec<_>>());
        let mut objects = RefreshObjects::decode(
            chip,
            bytes,
            region,
            self.source.variant,
            sources.each_ref().map(Vec::as_slice),
        )?;
        if let Some(quota) = &self.source.maps.quota {
            let fields = chip
                .uint()
                .glue()
                .witnesses(region, &quota.commitments().map(|v| self.value(v)))?;
            let proposal = QuotaCommitmentCells {
                previous_usage: fields[0].clone(),
                windows: fields[1].clone(),
                successor_usage: fields[2].clone(),
                issued: fields[3].clone(),
                window_count: fields[4].clone(),
            };
            objects = objects.with_quota_commitment(chip, region, &proposal)?;
        }
        Ok(objects)
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
        let verifier =
            VerifierConfig::configure_serialized_foreign_tagged(meta, SOURCE_RANGE_BUSES)
                .expect("fixed native Refresh Tagged3 profile");
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
        layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        self.synthesize_stage(None, config, layouter)
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
        layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        self.first.synthesize_stage(Some(self), config, layouter)
    }
}
impl First {
    fn synthesize_stage(
        &self,
        resume: Option<&Continuation>,
        config: StageConfig,
        mut layouter: impl Layouter<Fp>,
    ) -> Result<(), LayoutError> {
        let mut chip = VerifierChip::new(config.verifier);
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(
            || "complete genuine Refresh stage",
            |mut r| {
                let o = self;
                let stage = resume.map_or(0, |c| c.plan.stage());
                let mut assigned = self.source.maps.clone();
                assigned.known = self.known;
                let source = &o.source;
                let w = &source.maps.witness;
                let (before, previous) = assigned.state(&mut chip, &mut r, &w.predecessor)?;
                let (after, next) = assigned.state(&mut chip, &mut r, &w.successor)?;
                let fields = chip
                    .uint()
                    .glue()
                    .witnesses(&mut r, &w.statement.map(|v| o.value(v)))?
                    .try_into()
                    .map_err(|_| LayoutError::Synthesis)?;
                let statement = StatementCells::constrain_with_verifier(
                    &mut chip,
                    &mut r,
                    source.variant,
                    &fields,
                )?;
                let sigma = o.sigma(&mut chip, &mut bytes, &mut r, &statement)?;
                let objects = self.objects(&mut chip, &mut bytes, &mut r, stage)?;
                let q_instances = source
                    .q_instances
                    .iter()
                    .map(|columns| {
                        columns
                            .iter()
                            .map(|column| {
                                column
                                    .iter()
                                    .map(|v| o.scalar(&mut chip, &mut r, *v))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect::<Result<Vec<Vec<Vec<_>>>, LayoutError>>()?;
                let pp = self.pallas(
                    &mut chip,
                    &mut r,
                    &self.source.predecessor.pallas.as_input(),
                )?;
                let pv = self.vesta(&mut chip, &mut r, &self.source.predecessor.vesta)?;
                let input = ContextInputs {
                    own_statement: &statement,
                    incoming_statement: None,
                    predecessor: Some(ContextPredecessor {
                        state: &before,
                        public: &previous,
                        pallas: &pp,
                        vesta: &pv,
                    }),
                    successor: ContextState {
                        state: &after,
                        public: &next,
                    },
                    incoming: None,
                    q_instances: &q_instances,
                    objects: objects.context(),
                    modes: &[],
                    pallas_corrections: &[],
                    vesta_corrections: &[],
                    receive_results: None,
                };
                let mut verified = Vec::new();
                let mut signatures = None;
                for &index in self.plan.q_partition(stage).ok_or(LayoutError::Synthesis)? {
                    let proof =
                        o.carrier(&mut chip, &mut bytes, &mut r, &source.q_proofs[index])?;
                    let q = if index == 0 {
                        verify_sigma(
                            &mut chip,
                            &mut r,
                            self.plan.operation(),
                            &q_instances[index],
                            &proof,
                            core::slice::from_ref(&sigma),
                        )?
                        .0
                    } else {
                        let q = super::super::verify_q(
                            &mut chip,
                            &mut r,
                            self.plan.operation(),
                            index,
                            &q_instances[index],
                            &proof,
                        )?;
                        signatures = Some(super::super::bind_signature_q(
                            &mut chip,
                            &mut r,
                            self.plan.operation(),
                            index,
                            &source.signature_schemas[index - 1],
                            &q,
                        )?);
                        q
                    };
                    verified.push(q);
                }
                let maps = assigned.assign(
                    &mut chip,
                    &mut r,
                    self.plan
                        .operation_tasks(stage)
                        .ok_or(LayoutError::Synthesis)?,
                )?;
                RefreshStagePlan::new(self.plan.clone(), self.source.policy)?.constrain_stage(
                    &mut chip,
                    &mut r,
                    stage,
                    &objects,
                    &input,
                    RefreshStageWitness {
                        sigma: self
                            .plan
                            .operation_tasks(stage)
                            .ok_or(LayoutError::Synthesis)?
                            .contains(&OperationTask::RefreshUpdateAuthorization)
                            .then_some(&sigma),
                        signatures: signatures.as_ref(),
                        blacklist: maps.blacklist.as_ref(),
                        quota: maps.quota.as_ref(),
                    },
                )?;
                let fold = o.carrier(
                    &mut chip,
                    &mut bytes,
                    &mut r,
                    resume.map_or(self.fold.as_slice(), |c| c.fold.as_slice()),
                )?;
                if let Some(resume) = resume {
                    let cp = self.pallas(&mut chip, &mut r, &resume.carried.as_input())?;
                    let cv = self.vesta(&mut chip, &mut r, &resume.vesta)?;
                    let history = resume
                        .history
                        .iter()
                        .map(|(p, v)| {
                            Ok(super::super::split::ContextLinkCells {
                                pallas: self.pallas(&mut chip, &mut r, &p.as_input())?,
                                vesta: self.vesta(&mut chip, &mut r, v)?,
                            })
                        })
                        .collect::<Result<Vec<_>, LayoutError>>()?;
                    let wrapper = o.carrier(&mut chip, &mut bytes, &mut r, &resume.wrapper)?;
                    let resumed = super::super::split::resume_context(
                        &mut chip,
                        &mut r,
                        &resume.plan,
                        &input,
                        &history,
                        &cp,
                        &cv,
                        &wrapper,
                        core::slice::from_ref(&sigma),
                    )?;
                    let closed = super::super::split::close_stage(
                        &mut chip,
                        &mut r,
                        &resume.plan,
                        &resumed,
                        None,
                        None,
                        None,
                        &verified,
                        &fold,
                    )?;
                    if resume.plan.is_terminal() {
                        closed.words(&mut chip, &mut r, &next)
                    } else {
                        closed.continuation()?.words(&mut chip, &mut r)
                    }
                } else {
                    let vk = &self.source.predecessor.key;
                    let iroha_plonk::transcript::TranscriptRepr::Base(repr) = *vk.transcript_repr()
                    else {
                        return Err(LayoutError::Synthesis);
                    };
                    let fixed = vk
                        .fixed_commitments()
                        .iter()
                        .map(|p| o.value(Ep::from(*p)))
                        .collect::<Vec<_>>();
                    let permutation = vk
                        .permutation_commitments()
                        .iter()
                        .map(|p| o.value(Ep::from(*p)))
                        .collect::<Vec<_>>();
                    let key = chip.witness_key(&mut r, o.value(repr), &fixed, &permutation)?;
                    let proof = o.carrier(
                        &mut chip,
                        &mut bytes,
                        &mut r,
                        &self.source.predecessor.proof,
                    )?;
                    let pred = verify_predecessor(
                        &mut chip,
                        &mut r,
                        self.plan.operation(),
                        &key,
                        &previous,
                        &next,
                        &pp,
                        &pv,
                        &proof,
                    )?;
                    let first = close_first(
                        &mut chip,
                        &mut r,
                        &self.plan,
                        &input,
                        Some(&pred),
                        &verified,
                        core::slice::from_ref(&sigma),
                        Some(&fold),
                        &self.source.params,
                    )?;
                    first.words(&mut chip, &mut r)
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
    First(Box<First>),
    Continued(Box<Continuation>),
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
                StageData::First(c) => StageData::First(Box::new(c.without_witnesses())),
                StageData::Continued(c) => StageData::Continued(Box::new(c.without_witnesses())),
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

/// Checked original source coupled to the fixed complete Refresh plan.
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
                inner: StageData::First(Box::new(first)),
            },
            public,
        ))
    }
}

/// Already installed source A and continuation W proving artifacts for this fixed profile.
/// Authentication and genuine PK import remain the native package owner's responsibility.
pub struct Prover {
    plan: Plan,
    a: Vec<Arc<ProvingKey<Eq>>>,
    w: Vec<Arc<ProvingKey<Ep>>>,
    wrappers: Vec<WKey>,
}
impl Prover {
    /// Import the complete fixed typed artifact set, never generating keys from a witness.
    /// # Errors
    /// Wrong fixed source descriptor or W stage/context identity.
    pub fn from_artifacts(
        plan: Plan,
        a: Vec<Arc<ProvingKey<Eq>>>,
        w: Vec<Arc<ProvingKey<Ep>>>,
    ) -> Result<Self, Error> {
        if a.len() != plan.context.stage_count() || w.len() + 1 != a.len() {
            return Err(Error::Artifact);
        }
        let expected =
            super::artifact::source_descriptor::<StageCircuit>(()).ok_or(Error::Artifact)?;
        for key in &a {
            if key.binding() != &expected {
                return Err(Error::Artifact);
            }
            VerifierPlan::new(key.binding().clone(), plan.vesta.clone())
                .map_err(|_| Error::Artifact)?;
        }
        let wrappers = (0..w.len())
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
            .collect::<Result<Vec<_>, _>>()?;
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
    /// Installed descriptors in exact alternating A/W checkpoint order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<&DescriptorBinding> {
        let mut out = Vec::with_capacity(self.a.len() + self.w.len());
        for (i, a) in self.a.iter().enumerate() {
            out.push(a.binding());
            if let Some(w) = self.w.get(i) {
                out.push(w.binding());
            }
        }
        out
    }
}

/// Source-bound session over the fixed installed Refresh artifacts.
pub struct Session<'a> {
    prover: &'a Prover,
    prepared: Prepared,
}
impl Session<'_> {
    fn require_source(&self, source: &ACheckpoint) -> Result<(), Error> {
        if source.stage >= self.prover.a.len()
            || !Arc::ptr_eq(&source.first.source, &self.prepared.source)
        {
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
            std::slice::from_ref(&source.public),
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
            inner: StageData::First(Box::new(first.clone())),
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
            std::slice::from_ref(&public),
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_vesta(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            std::slice::from_ref(&public),
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
        if source.stage + 1 >= self.prover.a.len() {
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
        if source.stage + 1 >= self.prover.a.len() {
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
        if previous + 1 >= self.prover.a.len() {
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
            inner: StageData::Continued(Box::new(continuation)),
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
    /// Restore any continued A stage from its verified source W and canonical new Pallas claim.
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
            std::slice::from_ref(&public),
            &proof,
            budget,
        )
        .map_err(|_| Error::Proof)?;
        let opening = opening_vesta(
            &self.prepared.plan.vesta,
            key.binding(),
            key.vk(),
            std::slice::from_ref(&public),
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
    /// Export terminal A and all distinct final Omega obligations after another full native check.
    /// This grants neither monetary completion nor a final transported Omega.
    /// # Errors
    /// A nonterminal/wrong source checkpoint or any failed native proof/decide.
    pub fn terminal(&self, source: &ACheckpoint, budget: MemoryBudget) -> Result<Terminal, Error> {
        self.verify_a(source, budget)?;
        if source.stage + 1 != self.prover.a.len() {
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
/// Actual terminal A proof and every distinct obligation consumed by the final Omega producer.
#[derive(Clone, Debug)]
pub struct Terminal {
    /// Original terminal A proof under its installed key.
    pub proof: Vec<u8>,
    /// Exact homogeneous69-word public frame.
    pub instances: Vec<Fp>,
    /// Full accumulated Pallas claim.
    pub pallas: AccumulatorT<Ep>,
    /// Full Vesta part forwarded by the last W.
    pub vesta_part: AccumulatorT<Eq>,
    /// Full predecessor Vesta obligation, distinct from the current part and A opening.
    pub predecessor_vesta: AccumulatorT<Eq>,
    /// Actual terminal A own opening, a separate final Omega slot.
    pub opening: FoldInput<Eq>,
}

type Schedule = (Vec<Vec<usize>>, Vec<Vec<OperationTask>>);
fn schedule(variant: Variant) -> Result<Schedule, Error> {
    object_kinds(variant)?;
    let mut tasks = vec![
        vec![OperationTask::RefreshEffects],
        vec![],
        vec![OperationTask::RefreshUpdateAuthorization],
        vec![OperationTask::RefreshCurrentAuthorization],
    ];
    let mut partition = vec![vec![], vec![0], vec![1], vec![2]];
    if variant == Variant::RefreshBlacklist {
        tasks[0].push(OperationTask::RefreshBlacklist);
    }
    if variant == Variant::RefreshQuotaShare {
        tasks.extend(
            [
                OperationTask::RefreshQuotaPreviousRoot,
                OperationTask::RefreshQuotaWindowRoot,
                OperationTask::RefreshQuotaUsageRoot,
                OperationTask::RefreshQuotaMerge,
            ]
            .map(|t| vec![t]),
        );
        partition.resize_with(tasks.len(), Vec::new);
    }
    Ok((partition, tasks))
}
fn object_kinds(variant: Variant) -> Result<[ObjectKind; 5], Error> {
    let update = match variant {
        Variant::RefreshCredential => ObjectKind::Credential,
        Variant::RefreshSchemePolicy => ObjectKind::SchemePolicy,
        Variant::RefreshBlacklist => ObjectKind::Blacklist,
        Variant::RefreshQuotaShare => ObjectKind::QuotaShare,
        Variant::RefreshTimeAnchor => ObjectKind::TimeAnchor,
        _ => return Err(Error::Artifact),
    };
    Ok([
        ObjectKind::Certificate,
        update,
        ObjectKind::Receipt,
        ObjectKind::Certificate,
        ObjectKind::Credential,
    ])
}

fn check_sigma_tape(input: &Inputs, variant: Variant) -> Result<(), Error> {
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
        || bounded[1..=chunks.len()] != chunks
        || input.q[0].instances.get(2).map(Vec::as_slice) != Some(&[sigma_selector(variant)?])
    {
        return Err(Error::Input);
    }
    Ok(())
}

fn q_sigma_part(input: &QInput, source_k: u32, variant: Variant) -> Result<FoldInput<Eq>, Error> {
    let [bounded, point, indices, verdicts, source] = input.instances.as_slice() else {
        return Err(Error::Input);
    };
    if point.len() != 2
        || indices.len() != 1
        || indices.as_slice() != [sigma_selector(variant)?]
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

fn object_claims(source: &Sources) -> Result<Vec<[Fp; 3]>, Error> {
    let mut words = Vec::new();
    for (object, spec) in source
        .objects
        .iter()
        .zip(RefreshObjects::context_specs(source.variant).map_err(|_| Error::Artifact)?)
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
        words.push([
            object.digest()?,
            Fp::from(u64::from(spec.capacity)),
            hash_with_domain(u64::from_le_bytes(*b"kgwctap1"), &tape),
        ]);
    }
    if let Some(quota) = &source.maps.quota {
        let mut framed = vec![Fp::from(6), Fp::from(5)];
        framed.extend(quota.commitments());
        let digest = hash_with_domain(u64::from_le_bytes(*b"kgwciw_1"), &framed);
        words.push([digest, Fp::from(160), digest]);
    }
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
    words.extend(object_claims(source)?.into_iter().flatten());
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

fn sigma_selector(variant: Variant) -> Result<Fq, Error> {
    object_kinds(variant)?;
    super::super::schedule::sigma_selector(7, 0)
        .map(|index| Fq::from(u64::from(index)))
        .ok_or(Error::Artifact)
}
