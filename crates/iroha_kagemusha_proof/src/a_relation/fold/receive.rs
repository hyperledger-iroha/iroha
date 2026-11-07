//! Folded Receive A steps (the schedule in [`crate::fold_lineage`]).
//!
//! Every A sub-step of a Receive assigns the operation's step context (today's
//! [`ContextInputs`]) and recomputes its digest ([`FOLD_CONTEXT_DOMAIN`]): the
//! digest of today's schema words (a circuit constant), statements, states,
//! incoming lineage and claims, object commitments, result claims, modes and
//! corrections. The first sub-step
//! writes that digest into the A state; every later one must find the same
//! digest there, so all of them consume one context, as today's W
//! continuations ensure.
//!
//! Two parts of today's context stay out of the folded one. The Q leaves'
//! columns: each is read only by the owners of the A sub-step that folds the
//! Q instance carrying them (Q0 by A3, Q1 by A4), straight from that instance
//! (`Frame::public`), which folding authenticates. The predecessor's transport
//! claims: inside a segment the predecessor is the lineage the A state
//! carries and has none; the owners never read them, and the struct holds the
//! pinned trivial claims. The tasks run through today's
//! [`ReceiveStagePlan::constrain_stage`] with today's stage index, so every
//! result keeps its owner.

use std::sync::Arc;

use ff::{Field, PrimeField};
use iroha_fold::{
    circuit::{
        FoldLanes, ScalarCell,
        ivc::{fold_frame, frame_outputs},
    },
    ivc::FrameWitness,
    relation::FoldShape,
};
use iroha_pasta::{Ep, Eq, Fp, Fq, poseidon::hash_with_domain};
use iroha_plonk::{
    VerifyingKey,
    cs::ConstraintSystem,
    frontend::{Circuit, Error as SynthesisError, Layouter, Region, SimpleFloorPlanner},
    transcript::{decode_point, generator_coordinates},
};
use iroha_plonk_gadgets::{GlueChip, UintChip, Word, bytes::tape::BytesChip, statement::foreign_limbs};
use iroha_plonk_recursion::{
    AccumulatorT, FoldInput, PALLAS_TRIVIAL_GENERATOR, VESTA_TRIVIAL_GENERATOR,
    accumulation_circuit::FoldInputCells,
    codec::{GeneratorPoint, ScalarCells},
    obligation::{ModeCells, ledger::Variant},
    verifier::{VerifierChip, VerifierKeyCells},
};

use super::{AStateCells, AStepConfig, Assign, Error, pin_pallas, pin_vesta, push_pallas, vesta_words};
use crate::{
    a_relation::{
        IncomingLineageCells, LineagePublicCells, VestaClaimCells,
        context::{
            ContextIncoming, ContextIncomingProof, ContextInputs, ContextObjectCells, ContextPlan,
            ContextPredecessor, ContextState,
        },
        SigmaBindingCells, VerifiedQCells, bind_signature_q, bounded_word,
        incoming_transport::IncomingTransportPlan,
        native::receive::{IncomingWitness, Transition},
        own::OwnPolicy,
        receive::{
            ReceiveObjectInputs, ReceiveObjectSources, ReceiveObjects, ReceiveProofDigest, ReceiveProofInputs,
            ReceiveProofSources, ReceiveSignedObjects, ReceiveStageInputs, ReceiveStagePlan, ReceiveStageWitness,
            authorization::{ReceiveAuthorizationObjects, ReceiveAuthorizationSources, ReceiveSignatureQProjection},
        },
        results::ReceiveResultClaims,
        transition::TransitionInputs,
    },
    circuit::{PrefixMode, RelationShape},
    fold_lineage::{A_STATE, AKind, BASE},
    operation_relation::{
        incoming_statement::{IncomingStatementCells, StatementView},
        map_effects::{CONSUMED_DOMAIN, ReceiveMapWitness},
        state::StateCells,
        statement::StatementCells,
    },
    q_sigma::{QSigmaPlan, native::IncomingMode},
    witness::{NativeStep, SigmaRelation, StepWitness},
};

/// Domain of the folded Receive's step-context digest.
pub const FOLD_CONTEXT_DOMAIN: u64 = u64::from_le_bytes(*b"kgwfctx1");
/// Domain of the digest of today's context schema words (a circuit constant
/// the step-context digest absorbs in their place).
pub const FOLD_SCHEMA_DOMAIN: u64 = u64::from_le_bytes(*b"kgwfsch1");

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
    policy: OwnPolicy,
    omega_key: VerifyingKey<Ep>,
    schema_digest: Fp,
    trivial_pallas: FoldInput<Ep>,
    trivial_vesta: AccumulatorT<Eq>,
}

impl ReceiveFoldPlan {
    /// Pins today's ordinary Receive schedule for folding, with the own policy
    /// its stage plan was built under and the scheme's Omega key (the incoming
    /// Omega's soft verifier runs under it).
    ///
    /// # Errors
    /// [`Error::Artifact`] for another variant or schedule, or an Omega key of
    /// another descriptor.
    pub fn new(stage: ReceiveStagePlan, policy: OwnPolicy, omega_key: VerifyingKey<Ep>) -> Result<Self, Error> {
        let context = stage.context();
        let omega = context.operation().omega().ok_or(Error::Artifact)?;
        if omega_key.descriptor_digest() != omega.binding().digest()
            || !matches!(omega_key.transcript_repr(), iroha_plonk::transcript::TranscriptRepr::Base(_))
        {
            return Err(Error::Artifact);
        }
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
        let schema_digest = hash_with_domain(FOLD_SCHEMA_DOMAIN, stage.context().schema());
        Ok(Self { stage, policy, omega_key, schema_digest, trivial_pallas, trivial_vesta })
    }

    /// Today's Receive stage plan.
    #[must_use]
    pub const fn stage(&self) -> &ReceiveStagePlan {
        &self.stage
    }

    fn context(&self) -> &ContextPlan {
        self.stage.context()
    }

    /// The step-context digest every A sub-step of `source` recomputes
    /// ([`FOLD_CONTEXT_DOMAIN`]).
    ///
    /// # Errors
    /// [`Error::Input`] for a non-canonical claim.
    pub fn context_digest(&self, source: &ReceiveFoldSource) -> Result<Fp, Error> {
        let mut words = vec![self.schema_digest];
        words.extend(source.transition.statement);
        words.extend(source.incoming_statement);
        let (before, after) = (&source.transition.before, &source.transition.after);
        for state in [before, after] {
            words.extend(state.core);
            words.extend(state.rest);
            words.extend(state.lineage);
        }
        let incoming = &source.incoming;
        words.extend(incoming.public);
        words.push(Fp::from(u64::from(incoming.public_valid && lineage_fields_fit(&incoming.public))));
        push_pallas(&mut words, &incoming.pallas.as_input())?;
        words.extend(vesta_words(&incoming.vesta.as_input()));
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
        Ok(hash_with_domain(FOLD_CONTEXT_DOMAIN, &words))
    }

    /// The A state after a sub-step of `kind` from `state`, natively.
    ///
    /// # Errors
    /// [`Error::Input`] for a state the sub-step refuses; [`Error::Artifact`]
    /// for a kind not folded yet.
    pub fn next_state(&self, kind: AKind, source: &ReceiveFoldSource, state: &[Fp]) -> Result<Vec<Fp>, Error> {
        let digest = self.context_digest(source)?;
        if state != self.state_before(kind, source)? {
            return Err(Error::Input);
        }
        Ok(match kind {
            AKind::Receive1 | AKind::Receive3 | AKind::Receive4 => {
                let mut next = source.transition.before.lineage.to_vec();
                next.extend([digest, Fp::from(pc_before(kind) + 1)]);
                next
            }
            // The Receive is complete: the successor's lineage, no context.
            AKind::Receive5 => {
                let mut next = source.transition.after.lineage.to_vec();
                next.extend([Fp::ZERO, Fp::ZERO]);
                next
            }
        })
    }

    /// The A state a sub-step of `kind` requires: the predecessor's lineage,
    /// then no context and pc 0 before A1, or the Receive's context digest and
    /// the sub-steps done.
    ///
    /// # Errors
    /// As [`Self::context_digest`].
    pub fn state_before(&self, kind: AKind, source: &ReceiveFoldSource) -> Result<Vec<Fp>, Error> {
        let mut state = source.transition.before.lineage.to_vec();
        match kind {
            AKind::Receive1 => state.extend([Fp::ZERO, Fp::ZERO]),
            _ => state.extend([self.context_digest(source)?, Fp::from(pc_before(kind))]),
        }
        Ok(state)
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

/// The context words of a full-k16 Pallas claim in circuit: `k`, the generator
/// coordinates, then each challenge's two limbs.
fn push_pallas_cells(words: &mut Vec<Word<Fp>>, claim: &FoldInputCells<Ep>) -> Result<(), SynthesisError> {
    if claim.source() != iroha_plonk_recursion::accumulation_circuit::FoldSource::Fixed(16) {
        return Err(SynthesisError::Synthesis);
    }
    words.extend([claim.source_k().clone(), claim.g().x().clone(), claim.g().y().clone()]);
    for value in claim.challenges() {
        words.extend([value.lo().word().clone(), value.hi().word().clone()]);
    }
    Ok(())
}

/// The columns of the Q leaf a step folded (the folded instance's public
/// columns after its frame column) as verifier scalar cells, limb for limb.
fn q_cells(
    chip: &mut VerifierChip<Ep>,
    region: &mut Region<'_, Fp>,
    folded: &[Vec<ScalarCell<Ep>>],
    lengths: &[u32],
) -> Result<Vec<Vec<ScalarCells<Ep>>>, SynthesisError> {
    let columns = folded.get(1..).ok_or(SynthesisError::Synthesis)?;
    if columns.len() != lengths.len() || columns.iter().zip(lengths).any(|(c, n)| c.len() != *n as usize) {
        return Err(SynthesisError::Synthesis);
    }
    columns
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| ScalarCells::from_limbs(&mut chip.uint(), region, value.s6.lo(), value.s6.hi()))
                .collect()
        })
        .collect()
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
    /// The nine original tapes: Request, payer Credential, Send Receipt,
    /// Payment, raw Omega, raw sigma, own Receipt, quoted Credential, quoted
    /// certificate.
    pub objects: [Vec<u8>; 9],
    /// The nine context objects' commitments `(digest, length, tape digest)`.
    pub commitments: Vec<[Fp; 3]>,
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
        objects: [Vec<u8>; 9],
        commitments: Vec<[Fp; 3]>,
        incoming: IncomingWitness,
    ) -> Result<Self, Error> {
        if step.relation() != SigmaRelation::RECEIVE.step() || !step.canonical_digests() {
            return Err(Error::Input);
        }
        let native = step.evaluate(SigmaRelation::RECEIVE);
        if !native.violations.is_empty() || native.statement != transition.statement {
            return Err(Error::Input);
        }
        let source = Self { transition, incoming_statement, step: Some((step, native)), objects, commitments, incoming };
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
            objects: core::array::from_fn(|i| {
                let spec = plan.context().object_specs()[i];
                if matches!(i, 4 | 5) { Vec::new() } else { vec![0; spec.capacity as usize] }
            }),
            commitments: vec![[Fp::ZERO; 3]; plan.context().object_specs().len()],
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
        let specs = plan.context().object_specs();
        if self.commitments.len() != specs.len()
            || specs.len() != self.objects.len()
            || self.objects.iter().zip(specs).enumerate().any(|(i, (raw, spec))| {
                let capacity = spec.capacity as usize;
                if matches!(i, 4 | 5) { raw.len() > capacity } else { raw.len() != capacity }
            })
        {
            return Err(Error::Input);
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
        // Q columns are not part of the folded context: an unread entry is one
        // shared constant zero, and the sub-step that folds a Q instance puts
        // that instance's columns in its place.
        let zero = {
            let low = chip.uint().constant::<128>(region, 0)?;
            let high = chip.uint().constant::<127>(region, 0)?;
            ScalarCells::from_limbs(&mut chip.uint(), region, &low, &high)?
        };
        let q_instances = (0..context.operation().q_count())
            .map(|index| {
                let lengths = &context.operation().q(index).ok_or(SynthesisError::Synthesis)?.verifier().binding().descriptor().instance_lengths;
                Ok(lengths.iter().map(|n| vec![zero.clone(); *n as usize]).collect())
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?;
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

    /// The folded step-context digest ([`FOLD_CONTEXT_DOMAIN`]), as
    /// [`ReceiveFoldPlan::context_digest`]; both state openings are bound to
    /// their lineage fields first.
    fn digest(
        &self,
        plan: &ReceiveFoldPlan,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<Word<Fp>, SynthesisError> {
        let context = plan.context();
        let mut words = vec![chip.uint().glue().constant(region, plan.schema_digest)?];
        words.extend(self.statement.fields().iter().cloned());
        words.extend(self.incoming_statement.fields().iter().cloned());
        for (state, public) in [(&self.before, &self.pred_public), (&self.after, &self.next_public)] {
            state.bind_lineage(&mut chip.uint(), region, public)?;
            words.extend(state.core().iter().cloned());
            words.extend(state.rest().iter().cloned());
            words.extend(public.fields().iter().cloned());
        }
        words.extend(self.incoming_public.fields().iter().cloned());
        words.push(self.incoming_public.valid().word().clone());
        push_pallas_cells(&mut words, &self.incoming_pallas)?;
        words.extend(self.incoming_vesta.words());
        for object in &self.objects {
            words.extend(object.commitment_words());
        }
        words.extend(self.results.context_words(context.receive_results().ok_or(SynthesisError::Synthesis)?)?);
        for mode in &self.modes {
            words.extend([mode.accept().word().clone(), mode.trivial().word().clone(), mode.corrected().word().clone()]);
        }
        for point in &self.corrections {
            words.extend([point.x().clone(), point.y().clone()]);
        }
        for coordinates in &self.vesta_corrections {
            for value in coordinates {
                words.extend([value.lo().word().clone(), value.hi().word().clone()]);
            }
        }
        chip.hash_words(region, FOLD_CONTEXT_DOMAIN, &words)
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
        Ok(Self { plan, kind, shape, witness })
    }

    fn layout(
        &self,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
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
        let mut context = ContextCells::assign(&self.plan, assign, chip, region, source)?;
        let digest = context.digest(&self.plan, chip, region)?;
        // The Q instance this step folded carries its Q leaf's columns.
        let operation = self.plan.context().operation();
        match self.kind {
            AKind::Receive3 | AKind::Receive4 => {
                let index = if self.kind == AKind::Receive3 { 0 } else { ReceiveStagePlan::SIGNATURE_Q };
                let lengths = &operation.q(index).ok_or(SynthesisError::Synthesis)?.verifier().binding().descriptor().instance_lengths;
                context.q_instances[index] = q_cells(chip, region, &frame.public, lengths)?;
            }
            AKind::Receive1 | AKind::Receive5 => {}
        }
        let input = context.inputs();
        // The program counter orders the sub-steps; the context digest binds
        // every sub-step to the one context the first wrote.
        GlueChip::assert_constant(region, &state.pc, Fp::from(pc_before(self.kind)))?;
        if self.kind == AKind::Receive1 {
            GlueChip::assert_constant(region, &state.context, Fp::ZERO)?;
            for (carried, opened) in state.lineage.iter().zip(context.pred_public.fields()) {
                GlueChip::assert_equal(region, carried, opened)?;
            }
        } else {
            GlueChip::assert_equal(region, &state.context, &digest)?;
        }
        match self.kind {
            AKind::Receive1 => self.receive1(assign, chip, region, &context, &input, source)?,
            AKind::Receive3 => self.receive3(assign, chip, bytes, region, &context, &input, source)?,
            AKind::Receive4 => self.receive4(assign, chip, bytes, region, &context, &input, source)?,
            AKind::Receive5 => self.receive5(assign, chip, bytes, region, &context, &input, source)?,
        }
        let next = if self.kind == AKind::Receive5 {
            // The Receive is complete: the successor's lineage, no context.
            let zero = chip.uint().glue().constant(region, Fp::ZERO)?;
            let mut next = context.next_public.fields().to_vec();
            next.extend([zero.clone(), zero]);
            next
        } else {
            let pc = chip.uint().glue().constant(region, Fp::from(pc_before(self.kind) + 1))?;
            let mut next = state.lineage.to_vec();
            next.extend([digest, pc]);
            next
        };
        let refs = chip.fold_lanes()?;
        let mut lanes = FoldLanes { glue: refs.glue, range: refs.range, ff: refs.ff, ecc: refs.ecc, duplex: refs.hash };
        frame_outputs(&mut lanes, region, &chain, &frame, &next, &frame.digests, &[])
    }

    /// Stage A3's tasks: the incoming Omega's soft verifier and decoders
    /// (Proofs) and the Payment, Request, payer credential and receipt
    /// semantics with the sigma binding (Objects), over Q0's columns. Folding
    /// authenticates those columns as A4 does Q1's: the Q Sigma instance this
    /// step folds carries exactly the context's Q0 columns, which replaces
    /// today's hard Q0 verifier (today's stage A2).
    #[allow(clippy::too_many_arguments)]
    fn receive3(
        &self,
        assign: Assign,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        context: &ContextCells,
        input: &ContextInputs<'_>,
        source: &ReceiveFoldSource,
    ) -> Result<(), SynthesisError> {
        let plan = self.plan.context();
        let operation = plan.operation();
        let specs = plan.object_specs();
        let sigma_plan = operation.sigma_plan()?;
        let class = sigma_plan.class(QSigmaPlan::INCOMING).ok_or(SynthesisError::Synthesis)?;
        let proof_length = class.verifier().proof_length();
        let transport_plan = IncomingTransportPlan::new(operation)?;
        let omega_raw = assign.active(
            chip,
            bytes,
            region,
            &source.objects[4],
            specs[4].capacity as usize,
            &transport_plan.active_segments()?,
        )?;
        let sigma_raw = assign.active(
            chip,
            bytes,
            region,
            &source.objects[5],
            specs[5].capacity as usize,
            &SigmaBindingCells::incoming_segments(proof_length)?,
        )?;
        let transport =
            transport_plan.decode_active(chip, region, &omega_raw, context.next_public.omega_key_digest())?;
        let row = sigma_plan.position(QSigmaPlan::INCOMING).ok_or(SynthesisError::Synthesis)?;
        let index = bounded_word(chip, region, &context.q_instances[0][2][row])?;
        let sigma = SigmaBindingCells::from_incoming_active(
            chip,
            region,
            &context.incoming_statement,
            index,
            &sigma_raw,
            proof_length,
        )?;
        let values = source.objects.each_ref().map(|raw| raw.iter().map(|byte| assign.value(*byte)).collect::<Vec<_>>());
        let objects = ReceiveObjects::decode(
            chip,
            bytes,
            region,
            self.plan.policy,
            ReceiveObjectSources { request: &values[0], payer: &values[1], receipt: &values[2], payment: &values[3] },
            ReceiveObjectInputs {
                own: &context.statement,
                receiver: &context.pred_public,
                incoming: &transport,
                sigma: &sigma,
                consuming_digest: context.objects[4].authenticated_digest(),
            },
        )?;
        let sources = ReceiveProofSources::from_active_omega(&transport, &sigma)?;
        let omega_key = self.omega_key(assign, chip, region)?;
        let stage = u32::try_from(stage_of(self.kind)).map_err(|_| SynthesisError::BoundsFailure)?;
        self.plan.stage.constrain_stage(
            chip,
            region,
            stage,
            ReceiveStageInputs {
                context: input,
                proof_digest: None,
                objects: Some(&objects),
                signed: None,
                authorization: None,
            },
            ReceiveStageWitness {
                proofs: Some(ReceiveProofInputs { sources: &sources, omega_key: &omega_key }),
                ..ReceiveStageWitness::default()
            },
        )?;
        Ok(())
    }

    /// The scheme's Omega key as verifier key cells (witnessed, as today's
    /// stages do; the Proofs owner binds its digest to the lineage's).
    fn omega_key(
        &self,
        assign: Assign,
        chip: &mut VerifierChip<Ep>,
        region: &mut Region<'_, Fp>,
    ) -> Result<VerifierKeyCells<Ep>, SynthesisError> {
        let key = &self.plan.omega_key;
        let iroha_plonk::transcript::TranscriptRepr::Base(representation) = *key.transcript_repr() else {
            return Err(SynthesisError::Synthesis);
        };
        let points = |points: &[iroha_pasta::EpAffine]| points.iter().map(|p| assign.value(Ep::from(*p))).collect::<Vec<_>>();
        chip.witness_key(
            region,
            assign.value(representation),
            &points(key.fixed_commitments()),
            &points(key.permutation_commitments()),
        )
    }

    /// Stage A4's tasks: the hard own receipt (Authorization) and the soft
    /// Send receipt and Request signatures (Signatures), over Q1's columns.
    /// Folding authenticates those columns: the Q Signature instance this step
    /// folds carries exactly the context's Q1 columns, and the decider of its
    /// kind decides that today's Q1 leaf holds for them, which replaces today's
    /// hard Q1 verifier (its key digest is the plan's constant; no Q1 proof
    /// opening exists, so the opening slot is the pinned trivial claim).
    #[allow(clippy::too_many_arguments)]
    fn receive4(
        &self,
        assign: Assign,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        context: &ContextCells,
        input: &ContextInputs<'_>,
        source: &ReceiveFoldSource,
    ) -> Result<(), SynthesisError> {
        let index = ReceiveStagePlan::SIGNATURE_Q;
        let values = source.objects.each_ref().map(|raw| raw.iter().map(|byte| assign.value(*byte)).collect::<Vec<_>>());
        let signed = ReceiveSignedObjects::decode(chip, bytes, region, [&values[0], &values[1], &values[2]])?;
        let authorization = ReceiveAuthorizationObjects::decode(
            chip,
            bytes,
            region,
            Variant::Receive,
            ReceiveAuthorizationSources { receipt: &values[6], quoted: [&values[7], &values[8]] },
        )?;
        let operation = self.plan.context().operation();
        let fixed = operation.q(index).ok_or(SynthesisError::Synthesis)?;
        let key_digest = fixed.key.kagemusha_digest(fixed.verifier().binding()).map_err(|_| SynthesisError::Synthesis)?;
        let opening = assign.pallas(chip, region, &self.plan.trivial_pallas)?;
        pin_pallas(region, &opening, &self.plan.trivial_pallas)?;
        let verified = VerifiedQCells {
            index,
            opening,
            instances: context.q_instances[index].clone(),
            key_digest: chip.uint().glue().constant(region, key_digest)?,
        };
        let schema = self.plan.stage.signature_schema(index).ok_or(SynthesisError::Synthesis)?;
        let own = bind_signature_q(chip, region, operation, index, schema, &verified)?;
        let stage = u32::try_from(stage_of(self.kind)).map_err(|_| SynthesisError::BoundsFailure)?;
        let incoming =
            ReceiveSignatureQProjection::from_context(chip, region, self.plan.context(), stage, self.plan.policy, input)?;
        self.plan.stage.constrain_stage(
            chip,
            region,
            stage,
            ReceiveStageInputs {
                context: input,
                proof_digest: None,
                objects: None,
                signed: Some(&signed),
                authorization: Some(&authorization),
            },
            ReceiveStageWitness {
                own_signatures: Some(&own),
                incoming_signatures: Some(&incoming),
                ..ReceiveStageWitness::default()
            },
        )?;
        Ok(())
    }

    /// Stage A5's tasks: the consumed-credit nonmembership, the terminal burn
    /// and preserve effects, and the combined digest of the original Omega and
    /// sigma tapes.
    #[allow(clippy::too_many_arguments)]
    fn receive5(
        &self,
        assign: Assign,
        chip: &mut VerifierChip<Ep>,
        bytes: &mut BytesChip<Fp>,
        region: &mut Region<'_, Fp>,
        context: &ContextCells,
        input: &ContextInputs<'_>,
        source: &ReceiveFoldSource,
    ) -> Result<(), SynthesisError> {
        let specs = self.plan.context().object_specs();
        let omega = assign.active(chip, bytes, region, &source.objects[4], specs[4].capacity as usize, &[])?;
        let sigma = assign.active(chip, bytes, region, &source.objects[5], specs[5].capacity as usize, &[])?;
        let proof_digest =
            ReceiveProofDigest::from_active(chip, region, &omega, &sigma, context.objects[5].authenticated_digest())?;
        let nonmembership = assign.insertion(&mut chip.uint(), region, &source.transition.consumed)?.low;
        let stage = u32::try_from(stage_of(self.kind)).map_err(|_| SynthesisError::BoundsFailure)?;
        self.plan.stage.constrain_stage(
            chip,
            region,
            stage,
            ReceiveStageInputs {
                context: input,
                proof_digest: Some(&proof_digest),
                objects: None,
                signed: None,
                authorization: None,
            },
            ReceiveStageWitness { nonmembership: Some(&nonmembership), ..ReceiveStageWitness::default() },
        )?;
        Ok(())
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
        let mut bytes = BytesChip::new(config.bytes);
        chip.load_tables(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let out =
            layouter.assign_region(|| "folded Receive A step", |mut region| self.layout(&mut chip, &mut bytes, &mut region))?;
        for (row, word) in out.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.frame, row)?;
        }
        Ok(())
    }
}
