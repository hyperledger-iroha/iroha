//! Parent-funded backing for local interpreter diagnostics.
//!
//! This reserves the step rows, memory-access rows, and optional initial image
//! as one checked demand before a diagnostic invocation. It does not cover the
//! ordinary VM, host scratch, or the access recorder's small shared control
//! allocation, and it is not a production private-witness custody boundary.

use mv::allocation::AllocationBudget;

use crate::{
    VMError,
    error::ExecutionDeferral,
    execution_memory::{ExecutionMemoryLease, ExecutionMemoryPlan},
    execution_memory_recorder::DiagnosticMemoryAccessRecorder,
    execution_step_recorder::DiagnosticStepRecorder,
};

/// Preallocated local step and memory diagnostics for one invocation.
pub struct DiagnosticExecutionRecorders {
    /// Register and control transitions for attempted instructions.
    pub steps: DiagnosticStepRecorder,
    /// Ordered checked memory accesses and optional pre-run image.
    pub memory_accesses: DiagnosticMemoryAccessRecorder,
}

impl DiagnosticExecutionRecorders {
    /// Calculate the exact sum of the three explicit backing capacities.
    ///
    /// `initial_image_bytes` must be the VM's physical memory length when the
    /// diagnostic run begins. A parent can include several such plans before
    /// reserving once for nested local diagnostic invocations.
    pub fn allocation_plan(
        step_capacity: usize,
        access_capacity: usize,
        initial_image_bytes: Option<usize>,
    ) -> Result<ExecutionMemoryPlan, VMError> {
        let mut plan = DiagnosticStepRecorder::allocation_plan(step_capacity)?;
        plan.include_child(DiagnosticMemoryAccessRecorder::allocation_plan(
            access_capacity,
            initial_image_bytes,
        )?)
        .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable))?;
        Ok(plan)
    }

    /// Admit all explicit backing before constructing either recorder.
    ///
    /// # Errors
    /// Returns a local execution deferral on capacity or allocation refusal.
    pub fn try_new(
        step_capacity: usize,
        access_capacity: usize,
        initial_image_bytes: Option<usize>,
        budget: &AllocationBudget,
    ) -> Result<Self, VMError> {
        let plan = Self::allocation_plan(step_capacity, access_capacity, initial_image_bytes)?;
        let mut root = ExecutionMemoryLease::reserve(budget, plan)
            .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::ActiveMemoryCapacity))?;
        Self::try_new_from_parent(
            step_capacity,
            access_capacity,
            initial_image_bytes,
            &mut root,
        )
    }

    /// Partition an already admitted parent's credit before a nested run.
    ///
    /// Refusal leaves the parent's remaining credit unchanged. Construction
    /// uses no second allocation-budget acquisition and never waits for a
    /// parent allocation to be released.
    ///
    /// # Errors
    /// Returns a local execution deferral on insufficient credit or allocation
    /// refusal, before any interpreter instruction runs.
    pub fn try_new_from_parent(
        step_capacity: usize,
        access_capacity: usize,
        initial_image_bytes: Option<usize>,
        parent: &mut ExecutionMemoryLease,
    ) -> Result<Self, VMError> {
        let plan = Self::allocation_plan(step_capacity, access_capacity, initial_image_bytes)?;
        let mut child = parent
            .partition(plan)
            .map_err(|_| VMError::ExecutionDeferred(ExecutionDeferral::ActiveMemoryCapacity))?;
        let steps = DiagnosticStepRecorder::try_new_from_lease(step_capacity, &mut child)?;
        let memory_accesses = DiagnosticMemoryAccessRecorder::try_new_from_lease(
            access_capacity,
            initial_image_bytes,
            &mut child,
        )?;
        Ok(Self {
            steps,
            memory_accesses,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IVM, encoding::wide, host::DefaultHost};

    #[test]
    fn parent_pre_funds_two_local_invocations_without_reacquiring_the_pool() {
        let one = DiagnosticExecutionRecorders::allocation_plan(1, 4, None).unwrap();
        let mut both = one;
        both.include_child(one).unwrap();
        let budget = AllocationBudget::new(both.requested_bytes());
        let mut parent = ExecutionMemoryLease::reserve(&budget, both).unwrap();
        assert!(budget.try_reserve_bytes(1).is_err());

        let outer = DiagnosticExecutionRecorders::try_new_from_parent(1, 4, None, &mut parent)
            .expect("outer diagnostic backing is prepaid");
        let child = DiagnosticExecutionRecorders::try_new_from_parent(1, 4, None, &mut parent)
            .expect("nested diagnostic backing uses original credit");
        assert_eq!(parent.remaining_bytes(), 0);
        drop(parent);
        assert_eq!(budget.reserved_bytes(), both.requested_bytes());
        drop(outer);
        assert_eq!(budget.reserved_bytes(), one.requested_bytes());
        drop(child);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn combined_backing_runs_a_real_diagnostic_and_frees_after_unwind() {
        let plan = DiagnosticExecutionRecorders::allocation_plan(1, 4, None).unwrap();
        let budget = AllocationBudget::new(plan.requested_bytes());
        let mut vm = IVM::new(100);
        vm.load_code(&wide::encode_halt().to_le_bytes()).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut recorders = DiagnosticExecutionRecorders::try_new(1, 4, None, &budget).unwrap();
            vm.run_with_host_diagnostic_steps_and_memory(
                &mut DefaultHost::default(),
                &mut recorders.steps,
                &recorders.memory_accesses,
            )
            .unwrap();
            assert_eq!(recorders.steps.records().len(), 1);
            assert_eq!(budget.reserved_bytes(), plan.requested_bytes());
            panic!("release both diagnostic backings on caller unwind");
        }));
        assert!(result.is_err());
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn combined_plan_funds_the_complete_initial_image_before_running() {
        let mut vm = IVM::new(100);
        let code = wide::encode_halt().to_le_bytes();
        vm.load_code(&code).unwrap();
        let image_bytes = usize::try_from(vm.memory.stack_top()).unwrap();
        let plan = DiagnosticExecutionRecorders::allocation_plan(1, 4, Some(image_bytes)).unwrap();
        let budget = AllocationBudget::new(plan.requested_bytes());
        let mut recorders =
            DiagnosticExecutionRecorders::try_new(1, 4, Some(image_bytes), &budget).unwrap();
        assert_eq!(budget.reserved_bytes(), plan.requested_bytes());
        vm.run_with_host_diagnostic_steps_and_memory(
            &mut DefaultHost::default(),
            &mut recorders.steps,
            &recorders.memory_accesses,
        )
        .unwrap();
        recorders.memory_accesses.with_initial_image(|image| {
            let (state, bytes) = image.expect("pre-run image is captured");
            assert_eq!(state.image_bytes, image_bytes);
            assert_eq!(&bytes[..4], &code);
        });
        drop(recorders);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn refused_child_or_image_geometry_preserves_parent_credit() {
        let plan = DiagnosticExecutionRecorders::allocation_plan(1, 1, None).unwrap();
        let budget = AllocationBudget::new(plan.requested_bytes());
        let mut parent = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
        assert!(matches!(
            DiagnosticExecutionRecorders::try_new_from_parent(2, 1, None, &mut parent),
            Err(VMError::ExecutionDeferred(
                ExecutionDeferral::ActiveMemoryCapacity
            ))
        ));
        assert_eq!(parent.remaining_bytes(), plan.requested_bytes());
        assert!(matches!(
            DiagnosticExecutionRecorders::try_new_from_parent(1, 1, Some(0), &mut parent),
            Err(VMError::MemoryOutOfBounds)
        ));
        assert_eq!(parent.remaining_bytes(), plan.requested_bytes());
        drop(parent);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
