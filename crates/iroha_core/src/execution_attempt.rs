//! Non-serializable local attempt outcomes and the instruction-layer retry owner.

use iroha_data_model::ValidationFail;
use ivm::error::{ExecutionDeferral, VMError};

/// An unfinished local execution, retaining the original capacity refusal owner.
///
/// This type has no wire codec. A capacity release observation survives cache
/// checkout, transaction rollback and output abandonment with the same pool
/// identity. It authorizes a fresh local attempt, never a transaction rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionDeferred {
    reason: ExecutionDeferral,
    allocation: Option<iroha_allocation::AllocationRefusal>,
}

impl ExecutionDeferred {
    /// The VM-facing local reason, without erasing this owner's retry evidence.
    pub const fn reason(&self) -> ExecutionDeferral {
        self.reason
    }

    /// Borrow the original allocation refusal and its pre-probe release observation.
    ///
    /// Only `Capacity` carries a release-driven retry source. An allocator
    /// refusal, arithmetic overflow, or demand exceeding the configured pool
    /// limit cannot be cured by waiting on an invented notification.
    pub fn allocation_refusal(&self) -> Option<&iroha_allocation::AllocationRefusal> {
        self.allocation.as_ref()
    }

    /// Preserve the complete local owner across a VM/host error boundary.
    pub fn from_vm_error(error: &VMError) -> Option<Self> {
        match error.as_unmetered() {
            VMError::AllocationDeferred(refusal) => Some(refusal.clone().into()),
            VMError::ExecutionDeferred(reason) => Some((*reason).into()),
            _ => None,
        }
    }

    /// Move this owner back through a VM host boundary without losing its release source.
    pub fn into_vm_error(self) -> VMError {
        match self.allocation {
            Some(refusal) => VMError::AllocationDeferred(refusal),
            None => VMError::ExecutionDeferred(self.reason),
        }
    }
}

impl From<ExecutionDeferral> for ExecutionDeferred {
    fn from(reason: ExecutionDeferral) -> Self {
        Self {
            reason,
            allocation: None,
        }
    }
}

impl From<iroha_allocation::AllocationRefusal> for ExecutionDeferred {
    fn from(refusal: iroha_allocation::AllocationRefusal) -> Self {
        Self {
            reason: ExecutionDeferral::ActiveMemoryCapacity,
            allocation: Some(refusal),
        }
    }
}

impl core::fmt::Display for ExecutionDeferred {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.allocation {
            Some(refusal) => refusal.fmt(formatter),
            None => self.reason.fmt(formatter),
        }
    }
}

impl std::error::Error for ExecutionDeferred {}

/// Require a completed outcome in deterministic execution regression tests.
#[cfg(test)]
pub(crate) fn expect_completed_rejection<E>(error: ExecutionAttemptError<E>) -> E {
    match error {
        ExecutionAttemptError::Rejected(error) => error,
        ExecutionAttemptError::Deferred(reason) => panic!("unexpected local deferral: {reason}"),
    }
}

/// Separate a deterministic rejection from an incomplete local attempt.
/// No conversion from this type to a wire rejection is provided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionAttemptError<E> {
    /// Execution completed with a deterministic rejection.
    Rejected(E),
    /// Execution did not complete and must be retried locally.
    Deferred(ExecutionDeferred),
}

impl<E> ExecutionAttemptError<E> {
    /// Transform a completed rejection while retaining the local retry carrier.
    pub fn map_rejection<T>(self, map: impl FnOnce(E) -> T) -> ExecutionAttemptError<T> {
        match self {
            Self::Rejected(error) => ExecutionAttemptError::Rejected(map(error)),
            Self::Deferred(reason) => ExecutionAttemptError::Deferred(reason),
        }
    }
}

impl<E> From<E> for ExecutionAttemptError<E> {
    fn from(error: E) -> Self {
        Self::Rejected(error)
    }
}

impl From<&str> for ExecutionAttemptError<String> {
    fn from(error: &str) -> Self {
        Self::Rejected(error.to_owned())
    }
}

impl<E: core::fmt::Display> core::fmt::Display for ExecutionAttemptError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rejected(error) => error.fmt(f),
            Self::Deferred(reason) => write!(f, "execution deferred: {reason}"),
        }
    }
}

impl<E: core::fmt::Display + core::fmt::Debug + 'static> std::error::Error
    for ExecutionAttemptError<E>
{
}

/// Classify VM failure before any diagnostic conversion can discard local retry identity.
pub(crate) fn vm_attempt_error(
    error: VMError,
    deterministic: impl FnOnce(VMError) -> ValidationFail,
) -> ExecutionAttemptError<ValidationFail> {
    match ExecutionDeferred::from_vm_error(&error) {
        Some(reason) => ExecutionAttemptError::Deferred(reason),
        None => ExecutionAttemptError::Rejected(deterministic(error)),
    }
}

impl crate::state::StateTransaction<'_, '_> {
    /// Record the first local refusal before bridging a model-owned ISI signature.
    /// The enclosing attempt must extract this owner before settling any output.
    pub(crate) fn defer_execution(
        &mut self,
        reason: impl Into<ExecutionDeferred>,
    ) -> ValidationFail {
        self.execution_deferral.get_or_insert_with(|| reason.into());
        ValidationFail::InternalError("local execution attempt did not complete".into())
    }

    /// Borrow the sticky local retry reason without clearing its publication guard.
    pub(crate) fn execution_deferral(&self) -> Option<ExecutionDeferred> {
        self.execution_deferral.clone()
    }

    /// Bridge a model-owned instruction result while retaining the retry owner.
    pub(crate) fn attempt_error_to_validation_fail(
        &mut self,
        error: ExecutionAttemptError<ValidationFail>,
    ) -> ValidationFail {
        match error {
            ExecutionAttemptError::Rejected(error) => error,
            ExecutionAttemptError::Deferred(reason) => self.defer_execution(reason),
        }
    }

    /// Preserve a local VM refusal through an instruction API owned by the model.
    pub(crate) fn vm_error_to_validation_fail(
        &mut self,
        error: VMError,
        deterministic: impl FnOnce(VMError) -> ValidationFail,
    ) -> ValidationFail {
        match ExecutionDeferred::from_vm_error(&error) {
            Some(reason) => self.defer_execution(reason),
            None => deterministic(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExecutionAttemptError, ExecutionDeferral, ExecutionDeferred};

    #[test]
    fn original_capacity_refusal_survives_owner_clone_and_budget_handle_drop() {
        use std::{
            future::Future,
            pin::Pin,
            sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            },
            task::{Context, Poll, Wake, Waker},
        };
        #[derive(Default)]
        struct Wakes(AtomicUsize);
        impl Wake for Wakes {
            fn wake(self: Arc<Self>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        let budget = iroha_allocation::AllocationBudget::new(8);
        let occupied = budget.try_reserve_bytes(8).expect("initial reservation");
        let refusal = budget.try_reserve_bytes(1).expect_err("pool is occupied");
        let owner = ExecutionDeferred::from(refusal.clone());
        assert_eq!(owner.reason(), ExecutionDeferral::ActiveMemoryCapacity);
        assert_eq!(owner.allocation_refusal(), Some(&refusal));
        let cloned = owner.clone();
        drop(owner);
        drop(budget);
        let Some(iroha_allocation::AllocationRefusal::Capacity { release, .. }) =
            cloned.allocation_refusal()
        else {
            panic!("original capacity release evidence must survive");
        };
        let mut release = release.clone().wait_for_release();
        let wakes = Arc::new(Wakes::default());
        let waker = Waker::from(Arc::clone(&wakes));
        let mut context = Context::from_waker(&waker);
        assert_eq!(Pin::new(&mut release).poll(&mut context), Poll::Pending);
        drop(occupied);
        assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
        assert_eq!(Pin::new(&mut release).poll(&mut context), Poll::Ready(()));
    }

    #[test]
    fn non_capacity_deferral_never_fabricates_a_release_source() {
        let allocator = ExecutionDeferred::from(ExecutionDeferral::AllocationUnavailable);
        assert_eq!(allocator.reason(), ExecutionDeferral::AllocationUnavailable);
        assert!(allocator.allocation_refusal().is_none());
        let overflow = ExecutionDeferred::from(iroha_allocation::AllocationRefusal::DemandOverflow);
        assert!(matches!(
            overflow.allocation_refusal(),
            Some(iroha_allocation::AllocationRefusal::DemandOverflow)
        ));
        let budget = iroha_allocation::AllocationBudget::new(0);
        let impossible = ExecutionDeferred::from(budget.try_reserve_bytes(1).unwrap_err());
        assert!(matches!(
            impossible.allocation_refusal(),
            Some(iroha_allocation::AllocationRefusal::ExceedsLimit { .. })
        ));
        assert_eq!(
            ExecutionDeferred::from_vm_error(&allocator.clone().into_vm_error()),
            Some(allocator)
        );
        assert_eq!(
            ExecutionDeferred::from_vm_error(&impossible.clone().into_vm_error()),
            Some(impossible)
        );
        assert_eq!(
            ExecutionDeferred::from_vm_error(&ivm::VMError::OutOfMemory),
            None
        );
    }

    #[test]
    fn transaction_bridge_is_sticky_and_does_not_account_gas() {
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut block = state.block(iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::MIN,
            None,
            None,
            0,
            0,
        ));
        let mut transaction = block.transaction();
        assert_eq!(transaction.execution_deferral(), None);
        let refusal = ivm::VMError::ExecutionDeferred(ExecutionDeferral::ActiveMemoryCapacity);
        transaction.vm_error_to_validation_fail(refusal, |_| {
            panic!("local refusal cannot enter deterministic mapper")
        });
        transaction.defer_execution(ExecutionDeferral::AllocationUnavailable);
        assert_eq!(
            transaction.execution_deferral(),
            Some(ExecutionDeferral::ActiveMemoryCapacity.into())
        );
        assert_eq!(transaction.last_tx_gas_used, 0);
        let failure = transaction.vm_error_to_validation_fail(ivm::VMError::OutOfMemory, |error| {
            iroha_data_model::ValidationFail::NotPermitted(error.to_string())
        });
        assert!(matches!(
            failure,
            iroha_data_model::ValidationFail::NotPermitted(_)
        ));
        assert_eq!(
            transaction.execution_deferral(),
            Some(ExecutionDeferral::ActiveMemoryCapacity.into())
        );
    }

    #[test]
    fn sticky_deferral_alone_prevents_regular_transaction_publication() {
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };
        use iroha_data_model::{
            Registrable,
            prelude::{Account, Domain},
        };
        use iroha_model_base::domain::DomainId;
        use mv::storage::StorageReadOnly as _;
        let owner = iroha_test_samples::ALICE_ID.clone();
        let state = State::new_for_testing(
            World::with([], [Account::new(owner.clone()).build(&owner)], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let mut block = state.block(iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::MIN,
            None,
            None,
            0,
            0,
        ));
        let domain_id = DomainId::try_new("deferred", "universal").unwrap();
        let mut transaction = block.transaction();
        transaction.world.domains.insert(
            domain_id.clone(),
            Domain::new(domain_id.clone()).build(&owner),
        );
        transaction.defer_execution(ExecutionDeferral::AllocationUnavailable);
        transaction.apply();
        assert!(
            block.world.domains.get(&domain_id).is_none(),
            "a bare local deferral must abandon staged semantic writes"
        );
    }

    #[test]
    fn mapping_rejections_cannot_erase_a_local_deferral() {
        let refusal =
            ExecutionAttemptError::<u8>::Deferred(ExecutionDeferral::ActiveMemoryCapacity.into());
        let mapped = refusal.map_rejection(|_| panic!("local refusal is not a rejection"));
        assert_eq!(
            mapped,
            ExecutionAttemptError::<()>::Deferred(ExecutionDeferral::ActiveMemoryCapacity.into())
        );
        assert_eq!(
            ExecutionAttemptError::Rejected(2).map_rejection(|n| n + 3),
            ExecutionAttemptError::Rejected(5)
        );
    }
}
