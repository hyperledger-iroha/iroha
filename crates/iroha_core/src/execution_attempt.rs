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
    /// Preserve the local retry owner while projecting only completed rejection into a VM error.
    pub(crate) fn into_vm_error(self, rejected: impl FnOnce(E) -> VMError) -> VMError {
        match self {
            Self::Rejected(error) => rejected(error),
            Self::Deferred(reason) => reason.into_vm_error(),
        }
    }

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

/// Preserve a local Norito refusal before a caller constructs a deterministic rejection.
///
/// Matching surviving field, element and allocation ceilings belong to the current attempt. An
/// allocator failure is local even without an enclosing scope. Global archive and inner format limits,
/// malformed input and recursive-depth rejection remain deterministic. Norito's cumulative
/// scope has no allocation-pool release owner, so this must not invent one.
pub(crate) fn norito_decode_attempt_error<E>(
    error: norito::Error,
    rejected: impl FnOnce(norito::Error) -> E,
) -> ExecutionAttemptError<E> {
    let local_limit = norito::core::decode_error_matches_active_limits(&error)
        || (cfg!(all(test, sumeragi_core_mutation = "HC33"))
            && norito::core::decode_limits_active()
            && matches!(
                &error,
                norito::Error::ArchiveLengthExceeded { .. }
                    | norito::Error::SequenceLengthExceeded { .. }
                    | norito::Error::FieldLengthExceeded { .. }
                    | norito::Error::TotalElementsExceeded { .. }
                    | norito::Error::TotalAllocationExceeded { .. }
            ));
    let reason = match &error {
        norito::Error::AllocationFailed { .. } => Some(ExecutionDeferral::AllocationUnavailable),
        _ if local_limit => Some(ExecutionDeferral::ActiveMemoryCapacity),
        _ => None,
    };
    if !cfg!(all(test, sumeragi_core_mutation = "HC32"))
        && let Some(reason) = reason
    {
        return ExecutionAttemptError::Deferred(reason.into());
    }
    ExecutionAttemptError::Rejected(rejected(error))
}

/// Classify original JSON decoding before a diagnostic can discard local retry identity.
///
/// JSON's resource-limit error is emitted by the active decoder budget. Malformed input,
/// intrinsic parser bounds and recursive depth remain completed errors.
pub(crate) fn json_decode_attempt_error<E>(
    error: norito::json::Error,
    rejected: impl FnOnce(norito::json::Error) -> E,
) -> ExecutionAttemptError<E> {
    match error {
        norito::json::Error::DecodeResourceLimit => {
            ExecutionAttemptError::Deferred(ExecutionDeferral::ActiveMemoryCapacity.into())
        }
        norito::json::Error::AllocationFailed => {
            ExecutionAttemptError::Deferred(ExecutionDeferral::AllocationUnavailable.into())
        }
        error => ExecutionAttemptError::Rejected(rejected(error)),
    }
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
    fn norito_global_archive_cap_is_terminal_inside_an_outer_decode_scope() {
        let mut bytes = norito::to_bytes(&vec![7_u64]).unwrap();
        let limit = norito::core::max_archive_len();
        let length_offset = 4 + 1 + 1 + 16 + 1;
        bytes[length_offset..length_offset + 8].copy_from_slice(&(limit + 1).to_le_bytes());
        norito::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 32),
            || {
                let error = norito::decode_from_bytes::<Vec<u64>>(&bytes).unwrap_err();
                assert!(
                    matches!(&error, norito::Error::ArchiveLengthExceeded { length, limit: actual } if *length == limit + 1 && *actual == limit)
                );
                assert!(
                    matches!(
                        super::norito_decode_attempt_error(error, std::convert::identity),
                        ExecutionAttemptError::Rejected(
                            norito::Error::ArchiveLengthExceeded { .. }
                        )
                    ),
                    "global archive format cap was mistaken for an inherited local refusal"
                );
            },
        );
    }

    #[test]
    fn norito_inner_format_limits_are_terminal_under_a_wider_outer_scope() {
        let sequence = norito::to_bytes(&vec![7_u64, 11, 13]).unwrap();
        let text = norito::to_bytes(&String::from("bounded field")).unwrap();
        for dimension in 0..4 {
            let mut limits = [usize::MAX; 4];
            limits[dimension] = 0;
            norito::with_decode_limits_scope(
                norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 32),
                || {
                    let inner =
                        norito::DecodeLimits::new(limits[0], limits[1], limits[2], limits[3], 32);
                    let error = if dimension == 1 {
                        norito::decode_from_bytes_with_limits::<String>(&text, inner).unwrap_err()
                    } else {
                        norito::decode_from_bytes_with_limits::<Vec<u64>>(&sequence, inner)
                            .unwrap_err()
                    };
                    assert!(matches!(
                        (dimension, &error),
                        (0, norito::Error::SequenceLengthExceeded { limit: 0, .. })
                            | (1, norito::Error::FieldLengthExceeded { limit: 0, .. })
                            | (2, norito::Error::TotalElementsExceeded { limit: 0, .. })
                            | (3, norito::Error::TotalAllocationExceeded { limit: 0, .. })
                    ));
                    let original = error.decode_resource_error();
                    let ExecutionAttemptError::Rejected(error) =
                        super::norito_decode_attempt_error(error, std::convert::identity)
                    else {
                        panic!(
                            "inner format dimension {dimension} was mistaken for an outer local refusal"
                        );
                    };
                    assert_eq!(error.decode_resource_error(), original);
                    assert_eq!(
                        norito::decode_from_bytes::<Vec<u64>>(&sequence).unwrap(),
                        vec![7_u64, 11, 13]
                    );
                    assert_eq!(
                        norito::decode_from_bytes::<String>(&text).unwrap(),
                        "bounded field"
                    );
                },
            );
        }
    }

    #[test]
    fn norito_outer_allocation_and_element_refusals_retry_original_bytes() {
        let expected = vec![7_u64, 11, 13];
        let bytes = norito::to_bytes(&expected).unwrap();
        for allocation in [false, true] {
            let limits = norito::DecodeLimits::new(
                usize::MAX,
                usize::MAX,
                if allocation { usize::MAX } else { 0 },
                if allocation { 0 } else { usize::MAX },
                usize::MAX,
            );
            let refused = norito::with_decode_limits_scope(limits, || {
                let error = norito::decode_from_bytes::<Vec<u64>>(&bytes).unwrap_err();
                if allocation {
                    assert!(matches!(
                        error,
                        norito::Error::TotalAllocationExceeded { limit: 0, .. }
                    ));
                } else {
                    assert!(matches!(
                        error,
                        norito::Error::TotalElementsExceeded { limit: 0, .. }
                    ));
                }
                super::norito_decode_attempt_error::<()>(error, |_| {
                    panic!("a local decode refusal cannot enter the deterministic mapper")
                })
            });
            let ExecutionAttemptError::Deferred(reason) = refused else {
                panic!("the original local scope must remain retryable")
            };
            assert_eq!(reason.reason(), ExecutionDeferral::ActiveMemoryCapacity);
            assert!(reason.allocation_refusal().is_none());
            let retried = norito::decode_from_bytes::<Vec<u64>>(&bytes).unwrap();
            assert_eq!(retried, expected);
            norito::verify_exact_frame(&retried, &bytes).unwrap();
        }
    }

    #[test]
    fn norito_intrinsic_limits_and_malformed_or_deep_values_remain_rejections() {
        let intrinsic = [
            norito::Error::ArchiveLengthExceeded {
                length: 2,
                limit: 1,
            },
            norito::Error::SequenceLengthExceeded {
                length: 2,
                limit: 1,
            },
            norito::Error::FieldLengthExceeded {
                length: 2,
                limit: 1,
            },
            norito::Error::TotalElementsExceeded {
                attempted: 2,
                limit: 1,
            },
            norito::Error::TotalAllocationExceeded {
                attempted: 2,
                limit: 1,
            },
        ];
        assert!(!norito::core::decode_limits_active());
        for error in intrinsic {
            let expected = error.decode_resource_error();
            let ExecutionAttemptError::Rejected(error) =
                super::norito_decode_attempt_error(error, std::convert::identity)
            else {
                panic!("an intrinsic format bound cannot imply an inherited local refusal")
            };
            assert_eq!(error.decode_resource_error(), expected);
        }
        let bytes = norito::to_bytes(&vec![vec![7_u64]]).unwrap();
        norito::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
            || {
                let error = norito::decode_from_bytes::<Vec<Vec<u64>>>(&bytes).unwrap_err();
                assert!(matches!(error, norito::Error::NestingDepthExceeded { .. }));
                assert!(matches!(
                    super::norito_decode_attempt_error(error, std::convert::identity),
                    ExecutionAttemptError::Rejected(norito::Error::NestingDepthExceeded { .. })
                ));
                let malformed = norito::decode_from_bytes::<Vec<u64>>(&[]).unwrap_err();
                assert!(malformed.decode_resource_error().is_none());
                assert!(matches!(
                    super::norito_decode_attempt_error(malformed, std::convert::identity),
                    ExecutionAttemptError::Rejected(_)
                ));
            },
        );
    }

    #[test]
    fn norito_physical_refusal_is_local_without_fabricating_a_pool_waiter() {
        assert!(!norito::core::decode_limits_active());
        let refusal = super::norito_decode_attempt_error::<()>(
            norito::Error::AllocationFailed { bytes: 128 },
            |_| panic!("an allocator refusal must not produce a wire rejection"),
        );
        let ExecutionAttemptError::Deferred(reason) = refusal else {
            panic!("physical allocator refusal is local even without an inherited scope")
        };
        assert_eq!(reason.reason(), ExecutionDeferral::AllocationUnavailable);
        assert!(reason.allocation_refusal().is_none());
    }

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
    #[test]
    fn vm_projection_preserves_original_refusal_and_does_not_invoke_rejection_mapper() {
        let budget = iroha_allocation::AllocationBudget::new(8);
        let occupied = budget.try_reserve_bytes(8).unwrap();
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        let error = ExecutionAttemptError::<u8>::Deferred(refusal.clone().into())
            .into_vm_error(|_| panic!("local refusal cannot enter deterministic mapper"));
        assert_eq!(
            ExecutionDeferred::from_vm_error(&error),
            Some(refusal.into())
        );
        drop(occupied);
        assert_eq!(
            ExecutionAttemptError::Rejected(3_u8).into_vm_error(|value| {
                assert_eq!(value, 3);
                ivm::VMError::PermissionDenied
            }),
            ivm::VMError::PermissionDenied,
        );
    }
}
