//! Folded Receive A steps (the schedule in [`crate::fold_lineage`]).
//!
//! Every A sub-step of a Receive assigns the operation's step context as
//! today's split stages do and recomputes its digest with today's schema
//! ([`ContextPlan::digest`], `kgwctx_1`). The first sub-step writes that digest
//! into the A state; every later one must find the same digest there, so all
//! of them consume one context, as today's W continuations ensure. The
//! predecessor is the lineage the A state carries; inside a segment it has no
//! transport claims, so the context's predecessor claims are the trivial
//! ones, pinned as constants. The tasks run through today's
//! [`ReceiveStagePlan::constrain_stage`] with today's stage index, so every
//! result keeps its owner.

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    circuit::{
        FoldLanes,
        ivc::{fold_frame, frame_outputs},
    },
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, poseidon::hash_with_domain};
use iroha_plonk::{
    cs::{ConstraintSystem, InstanceType},
    frontend::{Circuit, Error as SynthesisError, Layouter, Region, SimpleFloorPlanner},
    transcript::{decode_point, generator_coordinates},
};
use iroha_plonk_gadgets::{GlueChip, UintChip, Word, bytes::tape::BytesChip, statement::foreign_limbs};
use iroha_plonk_recursion::{
    AccumulatorT, FoldInput, PALLAS_TRIVIAL_GENERATOR, VESTA_TRIVIAL_GENERATOR,
    accumulation_circuit::FoldInputCells,
    codec::{GeneratorPoint, ScalarCells},
    obligation::{ModeCells, ledger::Variant},
    verifier::VerifierChip,
};

use super::{AStateCells, AStepConfig, Assign, Error, pin_pallas, pin_vesta, push_pallas, vesta_words};
use crate::{
    a_relation::{
        IncomingLineageCells, LineagePublicCells, VestaClaimCells,
        context::{
            ContextIncoming, ContextIncomingProof, ContextInputs, ContextObjectCells, ContextPlan,
            ContextPredecessor, ContextState,
        },
        native::receive::{IncomingWitness, Transition},
        receive::{ReceiveStageInputs, ReceiveStagePlan, ReceiveStageWitness},
        results::ReceiveResultClaims,
        transition::TransitionInputs,
    },
    circuit::{PrefixMode, RelationShape},
    fold_lineage::{A_STATE, AKind, BASE},
    operation_relation::{
        incoming_statement::IncomingStatementCells,
        map_effects::{CONSUMED_DOMAIN, ReceiveMapWitness},
        state::StateCells,
        statement::StatementCells,
    },
    q_sigma::native::IncomingMode,
    witness::{NativeStep, SigmaRelation, StepWitness},
};

/// The Q partitions of today's Receive schedule: Q0 verified in stage A2, Q1
/// in stage A4. Folding binds Q0 in A3 and Q1 in A4.
const Q_PARTITIONS: [&[usize]; 5] = [&[], &[0], &[], &[1], &[]];

/// Today's stage whose tasks a folded Receive A kind runs.
#[must_use]
pub const fn stage_of(kind: AKind) -> usize {
    match kind {
        AKind::Receive1 => 0,
        AKind::Receive3 => 2,
        AKind::Receive4 => 3,
        AKind::Receive5 => 4,
    }
}

/// The program counter a folded Receive A kind requires (the number of the
/// Receive's A sub-steps done before it).
#[must_use]
pub const fn pc_before(kind: AKind) -> u64 {
    match kind {
        AKind::Receive1 => 0,
        AKind::Receive3 => 1,
        AKind::Receive4 => 2,
        AKind::Receive5 => 3,
    }
}

/// The immutable plan of the folded Receive A steps: today's Receive stage
/// plan (schedule, task owners, context schema and the source programs it
/// names) and the trivial transport claims of a predecessor inside a segment.
#[derive(Clone, Debug)]
pub struct ReceiveFoldPlan {
    stage: ReceiveStagePlan,
    trivial_pallas: FoldInput<Ep>,
    trivial_vesta: AccumulatorT<Eq>,
}

impl ReceiveFoldPlan {
    /// Pins today's ordinary Receive schedule for folding.
    ///
    /// # Errors
    /// [`Error::Artifact`] for another variant or schedule.
    pub fn new(stage: ReceiveStagePlan) -> Result<Self, Error> {
        let context = stage.context();
        if context.operation().frame().variant() != Variant::Receive
            || context.stage_count() != Q_PARTITIONS.len()
            || Q_PARTITIONS.iter().enumerate().any(|(i, q)| context.q_partition(i) != Some(*q))
            || context.operation_tasks(1).is_none_or(|tasks| !tasks.is_empty())
        {
            return Err(Error::Artifact);
        }
        // The pinned trivial claims: every challenge one, the generator they fold.
        let pallas = decode_point::<Ep>(&PALLAS_TRIVIAL_GENERATOR).map_err(|_| Error::Artifact)?;
        let vesta = decode_point::<Eq>(&VESTA_TRIVIAL_GENERATOR).map_err(|_| Error::Artifact)?;
        let trivial_pallas = FoldInput::from_normalized(pallas, 16, [Fq::ONE; 16]).map_err(|_| Error::Artifact)?;
        let trivial_vesta = AccumulatorT::new(vesta, [Fp::ONE; 16]).map_err(|_| Error::Artifact)?;
        Ok(Self { stage, trivial_pallas, trivial_vesta })
    }

    /// Today's Receive stage plan.
    #[must_use]
    pub const fn stage(&self) -> &ReceiveStagePlan {
        &self.stage
    }

    fn context(&self) -> &ContextPlan {
        self.stage.context()
    }

    fn q_types(&self, index: usize) -> Result<&[InstanceType], Error> {
        self.context()
            .operation()
            .q(index)
            .ok_or(Error::Artifact)?
            .verifier()
            .binding()
            .descriptor()
            .instance_types
            .as_deref()
            .ok_or(Error::Artifact)
    }

    /// The step-context digest every A sub-step of `source` recomputes
    /// (today's `kgwctx_1` preimage, with the trivial predecessor claims).
    ///
    /// # Errors
    /// [`Error::Input`] for a non-canonical claim or a Q value outside its type.
    pub fn context_digest(&self, source: &ReceiveFoldSource) -> Result<Fp, Error> {
        let mut words = self.context().schema().to_vec();
        words.extend(source.transition.statement);
        words.extend(source.incoming_statement);
        let (before, after) = (&source.transition.before, &source.transition.after);
        words.extend(before.core);
        words.extend(before.rest);
        words.extend(before.lineage);
        push_pallas(&mut words, &self.trivial_pallas)?;
        words.extend(vesta_words(&self.trivial_vesta.as_input()));
        words.extend(after.core);
        words.extend(after.rest);
        words.extend(after.lineage);
        let incoming = &source.incoming;
        words.extend(incoming.public);
        words.push(Fp::from(u64::from(incoming.public_valid && lineage_fields_fit(&incoming.public))));
        push_pallas(&mut words, &incoming.pallas.as_input())?;
        words.extend(vesta_words(&incoming.vesta.as_input()));
        for (index, columns) in source.q.iter().enumerate() {
            let types = self.q_types(index)?;
            if types.len() != columns.len() {
                return Err(Error::Input);
            }
            for (column, ty) in columns.iter().zip(types) {
                for value in column {
                    if matches!(*ty, InstanceType::Bounded | InstanceType::Bits(0..=253)) {
                        words.push(Option::<Fp>::from(Fp::from_repr(value.to_repr())).ok_or(Error::Input)?);
                    } else {
                        words.extend(foreign_limbs(value).map(Fp::from_u128));
                    }
                }
            }
        }
        words.extend(source.commitments.iter().flatten().copied());
        words.extend(incoming.results.map(|v| Fp::from(u64::from(v))));
        push_pallas(&mut words, &incoming.opening)?;
        for mode in incoming.modes {
            words.extend(mode_words(mode));
        }
        for point in incoming.pallas_corrections {
            let (x, y) = generator_coordinates::<Ep>(&point);
            words.extend([x, y]);
        }
        let (x, y) = generator_coordinates::<Eq>(&incoming.vesta_correction);
        for v in [x, y] {
            words.extend(foreign_limbs(&v).map(Fp::from_u128));
        }
        Ok(hash_with_domain(u64::from_le_bytes(crate::a_relation::context::CONTEXT_DOMAIN), &words))
    }

    /// The A state after a sub-step of `kind` from `state`, natively.
    ///
    /// # Errors
    /// [`Error::Input`] for a state the sub-step refuses; [`Error::Artifact`]
    /// for a kind not folded yet.
    pub fn next_state(&self, kind: AKind, source: &ReceiveFoldSource, state: &[Fp]) -> Result<Vec<Fp>, Error> {
        match kind {
            AKind::Receive1 => {
                if state != source.initial_state() {
                    return Err(Error::Input);
                }
                let mut next = source.transition.before.lineage.to_vec();
                next.extend([self.context_digest(source)?, Fp::from(pc_before(kind) + 1)]);
                Ok(next)
            }
            AKind::Receive3 | AKind::Receive4 | AKind::Receive5 => Err(Error::Artifact),
        }
    }
}

/// Whether incoming lineage fields have the version and widths
/// [`IncomingLineageCells`] requires for a valid view (version one, the 128-bit
/// fields below `2^128`, the packed field below `2^104`); the context carries
/// the proposed decoder bit and this rule together, as the circuit does.
fn lineage_fields_fit(fields: &[Fp; 18]) -> bool {
    let below = |value: &Fp, bits: u32| {
        let repr = value.to_repr();
        let bytes = (bits / 8) as usize;
        repr[bytes..].iter().all(|byte| *byte == 0)
    };
    fields[0] == Fp::ONE
        && [1, 2, 3, 4, 6, 7, 9, 10, 11, 12, 14].iter().all(|i| below(&fields[*i], 128))
        && below(&fields[13], 104)
}

/// Canonical one-hot Accept, Trivial and Corrected words.
fn mode_words(mode: IncomingMode) -> [Fp; 3] {
    match mode {
        IncomingMode::Accept => [Fp::ONE, Fp::ZERO, Fp::ZERO],
        IncomingMode::Trivial => [Fp::ZERO, Fp::ONE, Fp::ZERO],
        IncomingMode::Corrected => [Fp::ZERO, Fp::ZERO, Fp::ONE],
    }
}

/// The originals of one folded Receive, natively prepared.
#[derive(Clone, Debug)]
pub struct ReceiveFoldSource {
    /// The own state transition and its map routes.
    pub transition: Transition,
    /// The original incoming Send statement.
    pub incoming_statement: [Fp; 26],
    step: Option<(StepWitness<Fp>, NativeStep<Fp>)>,
    /// The nine context objects' commitments `(digest, length, tape digest)`.
    pub commitments: Vec<[Fp; 3]>,
    /// The public columns of Q0 (the `σ_send` soft verifier) and Q1 (the
    /// merged signature leaf), as the Q steps export them.
    pub q: [Vec<Vec<Fq>>; 2],
    /// The incoming decoder, result and obligation proposals.
    pub incoming: IncomingWitness,
}

impl ReceiveFoldSource {
    /// A source whose own step evaluates, rule for rule, to its statement.
    ///
    /// # Errors
    /// [`Error::Input`] for a step of another relation or statement, or
    /// originals of another shape.
    pub fn new(
        plan: &ReceiveFoldPlan,
        transition: Transition,
        incoming_statement: [Fp; 26],
        step: StepWitness<Fp>,
        commitments: Vec<[Fp; 3]>,
        q: [Vec<Vec<Fq>>; 2],
        incoming: IncomingWitness,
    ) -> Result<Self, Error> {
        if step.relation() != SigmaRelation::RECEIVE.step() || !step.canonical_digests() {
            return Err(Error::Input);
        }
        let native = step.evaluate(SigmaRelation::RECEIVE);
        if !native.violations.is_empty() || native.statement != transition.statement {
            return Err(Error::Input);
        }
        let source = Self { transition, incoming_statement, step: Some((step, native)), commitments, q, incoming };
        source.check_shape(plan)?;
        Ok(source)
    }

    /// The cells' shapes for key generation: zero values and trivial claims.
    #[must_use]
    pub fn blank(plan: &ReceiveFoldPlan) -> Self {
        let zero_state =
            crate::admin_sigma::StateWitness { core: [Fp::ZERO; 29], rest: [Fp::ZERO; 6], lineage: [Fp::ZERO; 18] };
        let insert = crate::tree::IndexedInsert {
            leaf: crate::tree::IndexedLeaf::default(),
            leaf_slot: 0,
            leaf_siblings: [Fp::ZERO; crate::tree::INDEXED_DEPTH],
            slot: 0,
            slot_siblings: [Fp::ZERO; crate::tree::INDEXED_DEPTH],
        };
        let q = core::array::from_fn(|index| {
            plan.context()
                .operation()
                .q(index)
                .map(|q| q.verifier().binding().descriptor().instance_lengths.iter().map(|n| vec![Fq::ZERO; *n as usize]).collect())
                .unwrap_or_default()
        });
        let g = *plan.trivial_pallas.g();
        Self {
            transition: Transition {
                before: zero_state,
                after: zero_state,
                statement: [Fp::ZERO; 26],
                consumed: insert,
                credit: insert,
                insert: false,
            },
            incoming_statement: [Fp::ZERO; 26],
            step: None,
            commitments: vec![[Fp::ZERO; 3]; plan.context().object_specs().len()],
            q,
            incoming: IncomingWitness {
                public: [Fp::ZERO; 18],
                public_valid: false,
                pallas: AccumulatorT::new(g, *plan.trivial_pallas.challenges()).expect("the trivial claim is canonical"),
                vesta: plan.trivial_vesta.clone(),
                opening: plan.trivial_pallas.clone(),
                results: [false; 4],
                modes: [IncomingMode::Trivial; 4],
                pallas_corrections: [g; 2],
                vesta_correction: *plan.trivial_vesta.g(),
            },
        }
    }

    fn check_shape(&self, plan: &ReceiveFoldPlan) -> Result<(), Error> {
        if self.commitments.len() != plan.context().object_specs().len() {
            return Err(Error::Input);
        }
        for (index, columns) in self.q.iter().enumerate() {
            let lengths = &plan.context().operation().q(index).ok_or(Error::Artifact)?.verifier().binding().descriptor().instance_lengths;
            if columns.len() != lengths.len() || columns.iter().zip(lengths).any(|(c, n)| c.len() != *n as usize) {
                return Err(Error::Input);
            }
        }
        Ok(())
    }

    /// The A state before the Receive: the predecessor's lineage, no context,
    /// no sub-step done.
    #[must_use]
    pub fn initial_state(&self) -> Vec<Fp> {
        let mut state = self.transition.before.lineage.to_vec();
        state.extend([Fp::ZERO, Fp::ZERO]);
        state
    }
}

/// The step context's cells, owned together (today's [`ContextInputs`] borrows
/// them).
struct ContextCells {
    statement: StatementCells,
    incoming_statement: IncomingStatementCells,
    before: StateCells,
    pred_public: LineagePublicCells,
    pred_pallas: FoldInputCells<Ep>,
    pred_vesta: VestaClaimCells,
    after: StateCells,
    next_public: LineagePublicCells,
    incoming_public: IncomingLineageCells,
    incoming_pallas: FoldInputCells<Ep>,
    incoming_vesta: VestaClaimCells,
    q_instances: Vec<Vec<Vec<ScalarCells<Ep>>>>,
    objects: Vec<ContextObjectCells>,
    modes: Vec<ModeCells<Fp>>,
    corrections: Vec<GeneratorPoint<Fp>>,
    vesta_corrections: Vec<[ScalarCells<Ep>; 2]>,
    results: ReceiveResultClaims,
}

impl ContextCells {
    fn assign(
        plan: &ReceiveFoldPlan,
        assign: Assign,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        source: &ReceiveFoldSource,
    ) -> Result<Self, SynthesisError> {
        let context = plan.context();
        let transition = &source.transition;
        let (before, pred_public) = assign.state(chip, region, &transition.before)?;
        let (after, next_public) = assign.state(chip, region, &transition.after)?;
        let fields = assign.words(chip, region, &transition.statement)?;
        let statement = StatementCells::constrain_with_verifier(chip, region, Variant::Receive, &fields)?;
        let fields = assign.words(chip, region, &source.incoming_statement)?;
        let lanes = chip.operation_lanes()?;
        let incoming_statement = IncomingStatementCells::constrain(
            &mut UintChip::new(lanes.glue, lanes.range),
            lanes.hash,
            region,
            Variant::Send,
            &fields,
        )?;
        let mut q_instances = Vec::with_capacity(source.q.len());
        for (index, columns) in source.q.iter().enumerate() {
            let types = plan.q_types(index).map_err(|_| SynthesisError::Synthesis)?;
            if types.len() != columns.len() {
                return Err(SynthesisError::Synthesis);
            }
            let mut cells = Vec::with_capacity(columns.len());
            for (column, ty) in columns.iter().zip(types) {
                cells.push(
                    column
                        .iter()
                        .map(|value| assign.q_instance(chip, region, *value, *ty))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            q_instances.push(cells);
        }
        // Inside a segment the predecessor carries no transport claims: the
        // context holds the trivial ones, as constants.
        let pred_pallas = assign.pallas(chip, region, &plan.trivial_pallas)?;
        pin_pallas(region, &pred_pallas, &plan.trivial_pallas)?;
        let pred_vesta = assign.vesta(chip, region, &plan.trivial_vesta)?;
        pin_vesta(region, &pred_vesta, &plan.trivial_vesta)?;
        let incoming = &source.incoming;
        let incoming_pallas = assign.pallas(chip, region, &incoming.pallas.as_input())?;
        let incoming_vesta = assign.vesta(chip, region, &incoming.vesta)?;
        let original = assign.words(chip, region, &incoming.public)?;
        let public_valid = chip.uint().glue().boolean(region, assign.value(incoming.public_valid))?;
        let incoming_public = IncomingLineageCells::constrain(&mut chip.uint(), region, &original, &public_valid)?;
        let modes = incoming
            .modes
            .iter()
            .map(|mode| {
                let bits = assign.words(chip, region, &mode_words(*mode))?;
                ModeCells::constrain(chip.uint().glue(), region, &bits)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let corrections = incoming
            .pallas_corrections
            .iter()
            .map(|point| chip.witness_generator(region, assign.value(Ep::from(*point))))
            .collect::<Result<Vec<_>, _>>()?;
        let (x, y) = generator_coordinates::<Eq>(&incoming.vesta_correction);
        let vesta_corrections = vec![[assign.scalar(chip, region, x)?, assign.scalar(chip, region, y)?]];
        let opening = assign.pallas(chip, region, &incoming.opening)?;
        let results = ReceiveResultClaims::assign(
            chip.uint().glue(),
            region,
            context.receive_results().ok_or(SynthesisError::Synthesis)?,
            incoming.results.map(|v| assign.value(v)),
        )?
        .with_opening(&opening)?;
        let commitments = source.commitments.iter().map(|v| v.map(|v| assign.value(v))).collect::<Vec<_>>();
        let objects = context.assign_receive_object_claims(chip, region, &commitments)?;
        Ok(Self {
            statement,
            incoming_statement,
            before,
            pred_public,
            pred_pallas,
            pred_vesta,
            after,
            next_public,
            incoming_public,
            incoming_pallas,
            incoming_vesta,
            q_instances,
            objects,
            modes,
            corrections,
            vesta_corrections,
            results,
        })
    }

    fn inputs(&self) -> ContextInputs<'_> {
        ContextInputs {
            own_statement: &self.statement,
            incoming_statement: Some(&self.incoming_statement),
            predecessor: Some(ContextPredecessor {
                state: &self.before,
                public: &self.pred_public,
                pallas: &self.pred_pallas,
                vesta: &self.pred_vesta,
            }),
            successor: ContextState { state: &self.after, public: &self.next_public },
            incoming: Some(ContextIncoming {
                public: &self.incoming_public,
                pallas: &self.incoming_pallas,
                vesta: &self.incoming_vesta,
                proof: ContextIncomingProof::ReceiveActive,
            }),
            q_instances: &self.q_instances,
            objects: &self.objects,
            modes: &self.modes,
            pallas_corrections: &self.corrections,
            vesta_corrections: &self.vesta_corrections,
            receive_results: Some(&self.results),
        }
    }
}

/// The witness of a folded Receive A step.
#[derive(Clone, Debug)]
pub struct ReceiveStepWitness {
    /// The Receive's originals.
    pub source: Arc<ReceiveFoldSource>,
    /// The frame: the preceding Q instance, the accumulator it folds into and
    /// the A state before this sub-step.
    pub frame: FrameWitness<Ep>,
}

/// A folded Receive A step of one kind.
#[derive(Clone, Debug)]
pub struct ReceiveFoldStep {
    plan: Arc<ReceiveFoldPlan>,
    kind: AKind,
    shape: FoldShape,
    witness: Option<ReceiveStepWitness>,
}

impl ReceiveFoldStep {
    /// The step of `kind` folding instances of the Q relation of `shape`
    /// (`kind.folds()`).
    ///
    /// # Errors
    /// [`Error::Artifact`] for a kind not folded yet.
    pub fn new(
        plan: Arc<ReceiveFoldPlan>,
        kind: AKind,
        shape: FoldShape,
        witness: Option<ReceiveStepWitness>,
    ) -> Result<Self, Error> {
        if kind != AKind::Receive1 {
            return Err(Error::Artifact);
        }
        Ok(Self { plan, kind, shape, witness })
    }

    fn layout(
        &self,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Vec<Word<Fp>>, SynthesisError> {
        let assign = Assign { known: self.witness.is_some() };
        let blank;
        let source = match &self.witness {
            Some(witness) => witness.source.as_ref(),
            None => {
                blank = ReceiveFoldSource::blank(&self.plan);
                &blank
            }
        };
        let chain = self.kind.chain();
        let frame = {
            let refs = chip.fold_lanes()?;
            let mut lanes =
                FoldLanes { glue: refs.glue, range: refs.range, ff: refs.ff, ecc: refs.ecc, duplex: refs.hash };
            fold_frame(
                &mut lanes,
                region,
                &chain,
                &self.shape,
                Fp::from(BASE),
                Fp::ZERO,
                A_STATE,
                self.witness.as_ref().map(|w| &w.frame),
            )?
        };
        let state = AStateCells::from_frame(&frame.state)?;
        let context = ContextCells::assign(&self.plan, assign, chip, region, source)?;
        let input = context.inputs();
        let digest = self.plan.context().digest(chip, region, &input)?;
        // The program counter orders the sub-steps; the context digest binds
        // every sub-step to the one context the first wrote.
        GlueChip::assert_constant(region, &state.pc, Fp::from(pc_before(self.kind)))?;
        let next = match self.kind {
            AKind::Receive1 => {
                GlueChip::assert_constant(region, &state.context, Fp::ZERO)?;
                for (carried, opened) in state.lineage.iter().zip(context.pred_public.fields()) {
                    GlueChip::assert_equal(region, carried, opened)?;
                }
                self.receive1(assign, chip, region, &context, &input, source)?;
                let pc = chip.uint().glue().constant(region, Fp::from(pc_before(self.kind) + 1))?;
                let mut next = state.lineage.to_vec();
                next.extend([digest, pc]);
                next
            }
            AKind::Receive3 | AKind::Receive4 | AKind::Receive5 => return Err(SynthesisError::Synthesis),
        };
        let refs = chip.fold_lanes()?;
        let mut lanes = FoldLanes { glue: refs.glue, range: refs.range, ff: refs.ff, ecc: refs.ecc, duplex: refs.hash };
        frame_outputs(&mut lanes, region, &chain, &frame, &next, &frame.digests, &[])
    }

    /// Stage A1's tasks: the consumed-credit and credit-record effects and the
    /// own step relation.
    fn receive1(
        &self,
        assign: Assign,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
        context: &ContextCells,
        input: &ContextInputs<'_>,
        source: &ReceiveFoldSource,
    ) -> Result<(), SynthesisError> {
        let transition = &source.transition;
        let consumed = assign.insertion(&mut chip.uint(), region, &transition.consumed)?;
        let credit = assign.insertion(&mut chip.uint(), region, &transition.credit)?;
        let fields = context.statement.fields();
        let inserted_key = fields[17].clone();
        let inserted_value =
            chip.hash_words(region, CONSUMED_DOMAIN, &[inserted_key.clone(), fields[20].clone(), fields[9].clone()])?;
        let insert = chip.uint().glue().boolean(region, assign.value(transition.insert))?;
        let effects = ReceiveMapWitness {
            consumed,
            credit,
            inserted_key,
            inserted_value,
            insert,
            payment_digest: context.objects[3].authenticated_digest().clone(),
        };
        let stage = u32::try_from(stage_of(self.kind)).map_err(|_| SynthesisError::BoundsFailure)?;
        let shape = RelationShape::new(SigmaRelation::RECEIVE, PrefixMode::Absorbed);
        self.plan.stage.constrain_stage(
            chip,
            region,
            stage,
            ReceiveStageInputs { context: input, proof_digest: None, objects: None, signed: None, authorization: None },
            ReceiveStageWitness {
                effects: Some(&effects),
                transition: Some(TransitionInputs {
                    shape,
                    witness: source.step.as_ref().filter(|_| assign.known).map(|(step, native)| (step, native)),
                }),
                ..ReceiveStageWitness::default()
            },
        )?;
        Ok(())
    }
}

impl Circuit<Fp> for ReceiveFoldStep {
    type Config = AStepConfig;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();

    fn without_witnesses(&self) -> Self {
        Self { witness: None, ..self.clone() }
    }

    fn configure(meta: &mut ConstraintSystem<Fp>) -> AStepConfig {
        AStepConfig::configure(meta)
    }

    fn synthesize(&self, config: AStepConfig, mut layouter: impl Layouter<Fp>) -> Result<(), SynthesisError> {
        let mut chip = VerifierChip::new(config.verifier);
        let bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out = layouter.assign_region(|| "folded Receive A step", |mut region| self.layout(&mut chip, &mut region))?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        Ok(())
    }
}
