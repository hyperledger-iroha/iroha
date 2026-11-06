//! Opt-in transient validation under an original, prepaid allocation ceiling.
//!
//! The caller retains the reservation until every decoded value, serializer
//! scratch and failure diagnostic has been destroyed. The cumulative decoder
//! counts allocation requests before their construction; this scope makes those
//! counters a debit against already reserved physical capacity. Custom decoders
//! must use the allocation-admission primitives for every allocation.

use std::cell::Cell;

use iroha_allocation::AllocationReservation;

use super::{DecodeBudgetContext, Error};

thread_local! {
    static PREPAID_VALIDATION: Cell<usize> = const { Cell::new(0) };
}

/// Whether the current synchronous decoder is inside explicit prepaid validation.
/// This is an internal allocation-routing capability, never a protocol switch.
#[doc(hidden)]
pub fn prepaid_validation_active() -> bool {
    PREPAID_VALIDATION.get() != 0
}

/// Select a fixed diagnostic before any legacy formatting allocation begins.
/// Outside prepaid validation the original diagnostic is constructed unchanged.
#[doc(hidden)]
pub fn prepaid_validation_error(context: &'static str, legacy: impl FnOnce() -> Error) -> Error {
    if prepaid_validation_active() {
        Error::InvalidValue { context }
    } else {
        legacy()
    }
}

/// Run transient typed validation against an original prepaid allocation ceiling.
///
/// The reservation must cover the context's entire cumulative allocation limit.
/// No allocation funded by this reservation may escape the callback. A callback
/// must drop transient decoded values before returning; independently charged
/// retained frames may escape with their own original allocation owners. An error
/// carrying allocated diagnostics must remain accompanied by this reservation
/// until the error is disposed of. The
/// source bytes and context control have their own original owners. This scope
/// never adds a new allocation counter, resets work, or refunds capacity.
///
/// # Errors
/// Refuses an insufficient reservation before calling the decoder, then preserves
/// its original error and existing cumulative resource-scope provenance.
#[doc(hidden)]
pub fn with_prepaid_validation(
    reservation: &AllocationReservation,
    context: &DecodeBudgetContext,
    validate: impl FnOnce() -> Result<(), Error>,
) -> Result<(), Error> {
    if prepaid_validation_active() {
        return Err(Error::InvalidValue {
            context: "nested prepaid validation scope",
        });
    }
    let pool = reservation.allocation_budget();
    if !matches!(&context.layer.budget.counters, super::CounterOwner::Prepared(control)
        if control.belongs_to(&pool))
    {
        return Err(Error::InvalidValue {
            context: "prepaid validation control belongs to another pool",
        });
    }
    let required = context.layer.budget.limits.max_total_allocated_bytes();
    if reservation.remaining_bytes() < required {
        return Err(Error::AllocationFailed {
            bytes: super::limit_to_u64(required),
        });
    }
    struct Guard(usize);
    impl Drop for Guard {
        fn drop(&mut self) {
            PREPAID_VALIDATION.set(self.0);
        }
    }
    let previous = PREPAID_VALIDATION.get();
    let next = previous.checked_add(1).ok_or(Error::LengthMismatch)?;
    PREPAID_VALIDATION.set(next);
    let _guard = Guard(previous);
    context.with(validate)
}

/// Admit and fallibly construct one decoded typed box under the active context.
///
/// Unlike `Box::new`, physical refusal returns the original codec error. The
/// enclosing prepaid scope retains allocation credits through the box lifetime.
///
/// # Errors
/// Refuses the exact box layout before construction if counters or allocation fail.
#[doc(hidden)]
pub fn try_box_for_decode<T>(value: T) -> Result<Box<T>, Error> {
    // SAFETY: alloc_checked returns a non-null pointer correctly aligned for T,
    // including the allocation-free ZST case, after charging the exact layout.
    let (allocation, _) = unsafe { super::alloc_checked(std::alloc::Layout::new::<T>()) }?;
    let allocation = allocation.cast::<T>();
    // SAFETY: this is the sole uninitialized T allocation. Initialize it exactly
    // once and immediately transfer it to Box, which owns destruction/deallocation.
    unsafe {
        allocation.write(value);
        Ok(Box::from_raw(allocation))
    }
}

/// Run a synchronous serializer/hash operation under one cumulative work allowance.
///
/// The returned value may be a fixed digest; source and destination allocations
/// remain owned by the caller. Nested encoding scopes are refused rather than
/// granting a fresh allowance. This does not change ordinary encoding behavior.
///
/// # Errors
/// Preserves the first original codec error or sticky work refusal.
#[doc(hidden)]
pub fn with_bounded_encode_work<T>(
    limits: super::EncodeWorkLimits,
    encode: impl FnOnce() -> Result<T, Error>,
) -> Result<T, super::BoundedEncodeWorkError> {
    let guard = super::encode_work::Guard::enter(limits)?;
    match encode() {
        Ok(value) => {
            guard.finish(Ok(()))?;
            Ok(value)
        }
        Err(error) => {
            guard.finish(Err(error))?;
            unreachable!("an error cannot finish successfully")
        }
    }
}

/// Compare a complete typed payload with its original bytes under finite encoder work.
///
/// The writer retains no replacement output. Any custom serializer allocation
/// remains subject to the caller's prepaid validation context.
///
/// # Errors
/// Refuses noncanonical bytes, encoder work exhaustion and original codec errors.
#[doc(hidden)]
pub fn verify_exact_payload_bounded<T: super::SerializePayload>(
    value: &T,
    bytes: &[u8],
    limits: super::EncodeWorkLimits,
) -> Result<(), super::BoundedEncodeWorkError> {
    let guard = super::encode_work::Guard::enter(limits)?;
    let result = crate::codec::verify_exact_payload(value, bytes);
    guard.finish(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{DecodeLimits, reserve_decode_allocation};
    use iroha_allocation::AllocationBudget;

    #[derive(Debug, crate::SerializePayload, crate::DeserializePayload, crate::NoritoSchema)]
    #[norito_schema(name = "norito.test.prepaid_validation.ScopedEnum")]
    enum ScopedEnum {
        First,
    }

    #[test]
    fn prepaid_diagnostics_skip_legacy_allocation_and_preserve_ordinary_errors() {
        assert!(
            matches!(prepaid_validation_error("fixed", || Error::Message("legacy".into())),
            Error::Message(message) if message == "legacy")
        );
        let pool = AllocationBudget::new(4096);
        let context =
            DecodeBudgetContext::try_new_owned(DecodeLimits::new(32, 256, 32, 128, 16), &pool)
                .unwrap();
        let reservation = pool.try_reserve_bytes(128).unwrap();
        with_prepaid_validation(&reservation, &context, || {
            assert!(matches!(
                prepaid_validation_error("fixed", || panic!("legacy formatter ran")),
                Error::InvalidValue { context: "fixed" }
            ));
            assert!(matches!(
                super::super::decode_field_canonical::<ScopedEnum>(&99_u32.to_le_bytes()),
                Err(Error::InvalidValue {
                    context: "invalid enum discriminant"
                })
            ));
            assert!(matches!(
                <char as super::super::DecodeFromSlice>::decode_from_slice(&u32::MAX.to_le_bytes()),
                Err(Error::InvalidValue { .. })
            ));
            assert!(matches!(
                <Option<u8> as super::super::DecodeFromSlice>::decode_from_slice(&[7]),
                Err(Error::InvalidValue { .. })
            ));
            Ok(())
        })
        .unwrap();
        assert!(!prepaid_validation_active());
    }

    #[test]
    fn enclosing_depth_keeps_original_counters_and_refuses_at_the_real_boundary() {
        let pool = AllocationBudget::new(4096);
        let context =
            DecodeBudgetContext::try_new_owned(DecodeLimits::new(32, 256, 32, 128, 8), &pool)
                .unwrap();
        let reservation = pool.try_reserve_bytes(128).unwrap();
        let nested = context.with_enclosing_depth(8, 8).unwrap();
        with_prepaid_validation(&reservation, &nested, || {
            reserve_decode_allocation(7)?;
            assert!(super::super::decode_field_canonical::<u64>(&7_u64.to_le_bytes()).is_err());
            Ok(())
        })
        .unwrap();
        assert_eq!(context.consumed_allocated_bytes(), 7);
        assert!(context.with_enclosing_depth(9, 8).is_err());
        // A fresh control is not fabricated by adding the outer prefix.
        assert_eq!(
            nested.consumed_allocated_bytes(),
            context.consumed_allocated_bytes()
        );
    }

    #[test]
    fn explicit_literal_identity_preserves_generic_bytes_alignment_and_bounded_refusal() {
        use super::super::{write_canonical_payload_with_identity, write_canonical_to_writer};
        let value = Some(7_u128);
        let identity = "core::option::Option<u128>";
        assert_eq!(
            <Option<u128> as crate::NoritoSchema>::frame_name(),
            identity
        );
        let original = crate::encode_canonical(&value).unwrap();
        let limits = super::super::EncodeWorkLimits {
            max_frame_bytes: 4096,
            max_byte_visits: 4096,
            max_operations: 4096,
            max_depth: 16,
        };
        let mut output = Vec::new();
        assert!(
            write_canonical_payload_with_identity::<Option<u128>, _, _>(
                &value,
                &mut output,
                identity
            )
            .is_err()
        );
        assert!(output.is_empty());
        assert!(matches!(
            with_bounded_encode_work(limits, || write_canonical_to_writer(&value, &mut output)),
            Err(super::super::BoundedEncodeWorkError::Refused(
                super::super::EncodeWorkRefusal::Source
            ))
        ));
        assert!(output.is_empty());
        with_bounded_encode_work(limits, || {
            write_canonical_payload_with_identity::<Option<u128>, _, _>(
                &value,
                &mut output,
                identity,
            )
        })
        .unwrap();
        assert_eq!(output, original);
        output.clear();
        assert!(
            with_bounded_encode_work(limits, || write_canonical_payload_with_identity::<
                ScopedEnum,
                _,
                _,
            >(
                &ScopedEnum::First, &mut output, "incorrect"
            ))
            .is_err()
        );
        assert!(output.is_empty());
        let short = super::super::EncodeWorkLimits {
            max_byte_visits: 1,
            ..limits
        };
        assert!(
            with_bounded_encode_work(short, || write_canonical_payload_with_identity::<
                Option<u128>,
                _,
                _,
            >(&value, &mut output, identity))
            .is_err()
        );
        assert!(output.is_empty());
    }

    #[test]
    fn prepaid_validation_requires_full_capacity_and_preserves_cumulative_refusal() {
        let pool = AllocationBudget::new(4096);
        let context =
            DecodeBudgetContext::try_new_owned(DecodeLimits::new(8, 64, 8, 64, 8), &pool).unwrap();
        let short = pool.try_reserve_bytes(63).unwrap();
        assert!(with_prepaid_validation(&short, &context, || panic!("not admitted")).is_err());
        drop(short);
        let original = pool.try_reserve_bytes(64).unwrap();
        with_prepaid_validation(&original, &context, || {
            assert!(prepaid_validation_active());
            reserve_decode_allocation(64)
        })
        .unwrap();
        assert!(!prepaid_validation_active());
        assert!(
            with_prepaid_validation(&original, &context, || reserve_decode_allocation(1)).is_err()
        );
        assert!(!prepaid_validation_active());
    }

    #[test]
    fn typed_box_admission_precedes_construction_and_owns_its_value() {
        let pool = AllocationBudget::new(4096);
        let context =
            DecodeBudgetContext::try_new_owned(DecodeLimits::new(8, 64, 8, 8, 8), &pool).unwrap();
        let reservation = pool.try_reserve_bytes(8).unwrap();
        with_prepaid_validation(&reservation, &context, || {
            let value = try_box_for_decode(7_u64)?;
            assert_eq!(*value, 7);
            drop(value);
            assert!(try_box_for_decode(8_u64).is_err());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn prepaid_validation_refuses_foreign_controls_and_nested_scope() {
        let first = AllocationBudget::new(4096);
        let second = AllocationBudget::new(4096);
        let context =
            DecodeBudgetContext::try_new_owned(DecodeLimits::new(8, 64, 8, 64, 8), &first).unwrap();
        let foreign = second.try_reserve_bytes(64).unwrap();
        assert!(with_prepaid_validation(&foreign, &context, || panic!("foreign pool")).is_err());
        let original = first.try_reserve_bytes(64).unwrap();
        with_prepaid_validation(&original, &context, || {
            assert!(
                with_prepaid_validation(&original, &context, || panic!("nested reset")).is_err()
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn exact_payload_comparison_is_bounded_without_a_second_output() {
        let value = vec![1_u8, 2, 3];
        let bytes = crate::codec::Encode::encode(&value);
        let limits = super::super::EncodeWorkLimits {
            max_frame_bytes: 1024,
            max_byte_visits: 1024,
            max_operations: 1024,
            max_depth: 16,
        };
        verify_exact_payload_bounded(&value, &bytes, limits).unwrap();
        assert_eq!(
            with_bounded_encode_work(limits, || {
                crate::codec::verify_exact_payload(&value, &bytes)?;
                Ok(7_u32)
            })
            .unwrap(),
            7
        );
        let mut short = limits;
        short.max_byte_visits = 0;
        assert!(verify_exact_payload_bounded(&value, &bytes, short).is_err());
    }
}
