//! Strict internal `Assignment` bridge for a single stored witness phase.
//!
//! This is a synthesis primitive, not circuit admission or a proof entry point. The owner must
//! derive the plan from its actual proving key and admit the concrete producer/configuration.
//! It must use the original floor planner, consume the synthesis result, and drop the final
//! producer before instance absorption or phase commitment. TODO: connect that consuming,
//! PK-bound owner and the complete stored proof suffix before exposing a production path.
//!
//! No transcript or proof RNG is available during synthesis. Every failure destroys all phase
//! writers, including errors ignored by the producer. Reference-return requests always return
//! unknown and latch refusal; no reference, fake value, or compatibility promotion is created.

use std::sync::Mutex;

use halo2curves::CurveAffine;

use super::{
    StoredAssignmentFieldV1, StoredPhaseAssignmentsV1, StoredPhaseErrorV1, StoredPhasePlanV1,
};
use crate::{
    circuit::Value,
    plonk::{
        Advice, Any, Assigned, Assignment, Challenge, Column, Error, Fixed, Instance, Selector,
    },
    poly::stored_advice::StoredAdviceWriterV1,
};

/// Bounded diagnostics for a refused single-phase synthesis; never includes witness material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredSynthesisErrorV1 {
    /// The phase or its authenticated storage refused an operation.
    Phase(StoredPhaseErrorV1),
    /// Instance shape, column, or row did not match the admitted plan and supplied values.
    Instances,
    /// A typed advice column or challenge was outside its admitted phase coordinate space.
    Coordinate,
    /// The producer attempted to advance the phase before its consuming owner returned.
    PhaseAdvance,
    /// Synthesis returned an error even though no earlier assignment refusal was latched.
    Synthesis,
}

struct State<'params, C: CurveAffine, W: StoredAdviceWriterV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    owner: Option<StoredPhaseAssignmentsV1<'params, C, W>>,
    error: Option<StoredSynthesisErrorV1>,
}

impl<C: CurveAffine, W: StoredAdviceWriterV1> State<'_, C, W>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    fn fail(&mut self, error: StoredSynthesisErrorV1) {
        // Record before destruction: a backend destructor unwinding cannot erase the refusal.
        if self.error.is_none() {
            self.error = Some(error);
        }
        self.owner.take();
    }

    fn active(&self) -> bool {
        self.error.is_none() && self.owner.is_some()
    }
}

/// Original floor-planner assignments with one failure-latched, move-only phase owner.
///
/// Mutex access is needed only for `Assignment` queries taking `&self`; advice writes use
/// exclusive `get_mut` access. Thread-safe floor planners still require the writer/backend to
/// satisfy their existing Send/Sync bounds; this type does not override them.
pub(crate) struct StoredSinglePhaseAssignmentV1<'params, 'instances, C, W>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    W: StoredAdviceWriterV1,
{
    state: Mutex<State<'params, C, W>>,
    instances: &'instances [&'instances [C::Scalar]],
    k: u32,
    usable_rows: usize,
    challenges: usize,
}

impl<'params, 'instances, C, W> StoredSinglePhaseAssignmentV1<'params, 'instances, C, W>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    W: StoredAdviceWriterV1,
{
    /// Admit exactly one configured phase and the complete supplied instance-column shape.
    /// Derivation of this internal plan from the owned proving key remains the caller's duty.
    pub(crate) fn new(
        plan: StoredPhasePlanV1<'params, C>,
        writers: Vec<W>,
        instances: &'instances [&'instances [C::Scalar]],
    ) -> Result<Self, StoredSynthesisErrorV1> {
        if plan.phases.len() != 1 {
            return Err(StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Admission));
        }
        if instances.len() != plan.instance_columns
            || instances
                .iter()
                .any(|values| values.len() > plan.usable_rows)
        {
            return Err(StoredSynthesisErrorV1::Instances);
        }
        let k = plan.k;
        let usable_rows = plan.usable_rows;
        let challenges = plan.challenges;
        let owner = StoredPhaseAssignmentsV1::begin(plan, writers)
            .map_err(StoredSynthesisErrorV1::Phase)?;
        Ok(Self {
            state: Mutex::new(State {
                owner: Some(owner),
                error: None,
            }),
            instances,
            k,
            usable_rows,
            challenges,
        })
    }

    fn state_mut(&mut self) -> &mut State<'params, C, W> {
        match self.state.get_mut() {
            Ok(state) => state,
            Err(poison) => {
                let state = poison.into_inner();
                state.fail(StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Poisoned));
                state
            }
        }
    }

    fn with_state<T>(&self, operation: impl FnOnce(&mut State<'params, C, W>) -> T) -> T {
        match self.state.lock() {
            Ok(mut state) => operation(&mut state),
            Err(poison) => {
                let mut state = poison.into_inner();
                state.fail(StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Poisoned));
                operation(&mut state)
            }
        }
    }

    /// Consume the actual floor-planner result and reject latched failures or adapter unwinds.
    /// An unwind outside this adapter is the consuming caller's responsibility: it must drop
    /// this owner, never catch the unwind and fabricate a successful synthesis result. The
    /// caller must end producer borrows and drop it before finishing this returned phase.
    pub(crate) fn into_assignments(
        self,
        result: Result<(), Error>,
    ) -> Result<StoredPhaseAssignmentsV1<'params, C, W>, StoredSynthesisErrorV1> {
        let mut state = match self.state.into_inner() {
            Ok(state) => state,
            Err(poison) => {
                let mut state = poison.into_inner();
                state.fail(StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Poisoned));
                state
            }
        };
        if result.is_err() {
            state.fail(StoredSynthesisErrorV1::Synthesis);
        }
        if let Some(error) = state.error {
            return Err(error);
        }
        state
            .owner
            .take()
            .ok_or(StoredSynthesisErrorV1::Phase(StoredPhaseErrorV1::Poisoned))
    }
}

impl<C, W> Assignment<C::Scalar> for StoredSinglePhaseAssignmentV1<'_, '_, C, W>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    W: StoredAdviceWriterV1,
{
    fn enter_region<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn exit_region(&mut self) {}
    fn annotate_column<A: FnOnce() -> AR, AR: Into<String>>(&mut self, _: A, _: Column<Any>) {}
    fn push_namespace<NR: Into<String>, N: FnOnce() -> NR>(&mut self, _: N) {}
    fn pop_namespace(&mut self, _: Option<String>) {}

    fn enable_selector<A: FnOnce() -> AR, AR: Into<String>>(
        &mut self,
        _: A,
        _: &Selector,
        _: usize,
    ) -> Result<(), Error> {
        // As in the ordinary prover, selector activations are already frozen in the PK.
        // Direct selector conversion erases num_selectors, so it cannot validate the
        // original producer's selector indexes. Exact producer/config admission is separate.
        if self.state_mut().active() {
            Ok(())
        } else {
            Err(Error::Synthesis)
        }
    }

    fn query_instance(
        &self,
        column: Column<Instance>,
        row: usize,
    ) -> Result<Value<C::Scalar>, Error> {
        self.with_state(|state| {
            if !state.active() {
                return Err(Error::Synthesis);
            }
            if row >= self.usable_rows {
                state.fail(StoredSynthesisErrorV1::Instances);
                return Err(Error::not_enough_rows_available(self.k));
            }
            if let Some(value) = self
                .instances
                .get(column.index())
                .and_then(|values| values.get(row))
            {
                return Ok(Value::known(*value));
            }
            state.fail(StoredSynthesisErrorV1::Instances);
            Err(Error::BoundsFailure)
        })
    }

    fn assign_advice<'v>(
        &mut self,
        _: Column<Advice>,
        _: usize,
        _: Value<Assigned<C::Scalar>>,
    ) -> Value<&'v Assigned<C::Scalar>> {
        self.state_mut().fail(StoredSynthesisErrorV1::Phase(
            StoredPhaseErrorV1::ReferenceReturn,
        ));
        Value::unknown()
    }

    fn assign_advice_discarding_value(
        &mut self,
        column: Column<Advice>,
        row: usize,
        to: Value<Assigned<C::Scalar>>,
    ) {
        if column.column_type().phase() != 0 {
            self.state_mut().fail(StoredSynthesisErrorV1::Coordinate);
            return;
        }
        let state = self.state_mut();
        if !state.active() {
            return;
        }
        let Ok(value) = to.assign() else {
            state.fail(StoredSynthesisErrorV1::Phase(
                StoredPhaseErrorV1::UnknownValue,
            ));
            return;
        };
        // Take the entire owner before a backend call: caught panics cannot leave it reusable.
        let Some(mut owner) = state.owner.take() else {
            return;
        };
        match owner.assign_discarding_value(column.index(), row, value) {
            Ok(()) => state.owner = Some(owner),
            Err(error) => state.fail(StoredSynthesisErrorV1::Phase(error)),
        }
    }

    fn assign_fixed(&mut self, _: Column<Fixed>, _: usize, _: Assigned<C::Scalar>) {
        // Fixed assignments are supplied by the proving key, matching the ordinary prover.
    }

    fn copy(&mut self, _: Column<Any>, _: usize, _: Column<Any>, _: usize) {
        // Copy constraints are supplied by the proving key, matching the ordinary prover.
    }

    fn fill_from_row(
        &mut self,
        _: Column<Fixed>,
        _: usize,
        _: Value<Assigned<C::Scalar>>,
    ) -> Result<(), Error> {
        if self.state_mut().active() {
            Ok(())
        } else {
            Err(Error::Synthesis)
        }
    }

    fn get_challenge(&self, challenge: Challenge) -> Value<C::Scalar> {
        self.with_state(|state| {
            if state.active() && (challenge.index() >= self.challenges || challenge.phase() != 0) {
                state.fail(StoredSynthesisErrorV1::Coordinate);
            }
            // Valid phase-zero challenges are unavailable until the owner commits after return.
            Value::unknown()
        })
    }

    fn next_phase(&mut self) {
        self.state_mut().fail(StoredSynthesisErrorV1::PhaseAdvance);
    }
}
