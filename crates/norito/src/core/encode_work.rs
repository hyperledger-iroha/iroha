//! Opt-in cumulative encoding work for the borrowed-payload frame corridor.
//!
//! Limits cover codec byte visits, codec operations and guarded value depth.
//! They do not bound arbitrary user code before it reaches a codec boundary,
//! nor fund serializer-internal allocations or the destination's storage.
use std::{
    cell::Cell,
    io::{self, Write},
    marker::PhantomData,
    rc::Rc,
};

/// Explicit finite limits. No default and no reset operation is provided.
#[derive(Debug, Clone, Copy)]
pub struct EncodeWorkLimits {
    /// Complete root header, original type padding and payload size ceiling.
    pub max_frame_bytes: usize,
    /// Cumulative bytes presented to encoders, checksum and output writers.
    /// A byte visited by several stages consumes allowance at every stage.
    pub max_byte_visits: usize,
    /// Cumulative codec boundary entries, including zero-byte writes.
    pub max_operations: usize,
    /// Simultaneously active guarded serializer/measurement boundaries.
    pub max_depth: usize,
}

/// Fixed, allocation-free reason for the first resource refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeWorkRefusal {
    ByteVisits,
    Operations,
    Depth,
    FrameBytes,
    NestedScope,
    /// An audited source adapter rejected its input before unsafe work.
    Source,
}

/// First sticky resource refusal, or the original codec/writer error.
#[derive(Debug)]
pub enum BoundedEncodeWorkError {
    Refused(EncodeWorkRefusal),
    Codec(super::Error),
}

/// Retained cumulative allowance shared by successive bounded encoding calls.
/// Moving/copying this counter does not itself reserve source or output memory.
#[derive(Clone, Copy, Debug)]
pub struct EncodeWorkBudget {
    byte_visits: usize,
    operations: usize,
}

impl EncodeWorkBudget {
    /// Start one root's finite, nonrefunded codec-work allowance.
    pub const fn new(byte_visits: usize, operations: usize) -> Self {
        Self {
            byte_visits,
            operations,
        }
    }

    /// Remaining cumulative byte visits.
    pub const fn remaining_byte_visits(&self) -> usize {
        self.byte_visits
    }
    /// Remaining cumulative codec operations.
    pub const fn remaining_operations(&self) -> usize {
        self.operations
    }
}

thread_local! {
    static CUMULATIVE: Cell<Option<EncodeWorkBudget>> = const { Cell::new(None) };
}

/// Debit every bounded call made by `encode` against one retained root budget.
/// Local per-frame/depth limits still apply. Neither completion nor unwind refunds work.
///
/// # Errors
/// Refuses nested cumulative scopes or entry from inside an encoding call.
#[doc(hidden)]
pub fn with_cumulative_encode_work<T>(
    budget: &mut EncodeWorkBudget,
    encode: impl FnOnce() -> T,
) -> Result<T, BoundedEncodeWorkError> {
    if CUMULATIVE.get().is_some() || ACTIVE.get().is_some() {
        return Err(BoundedEncodeWorkError::Refused(
            EncodeWorkRefusal::NestedScope,
        ));
    }
    struct Guard<'a>(&'a mut EncodeWorkBudget);
    impl Drop for Guard<'_> {
        fn drop(&mut self) {
            *self.0 = CUMULATIVE
                .replace(None)
                .expect("cumulative owner installed");
        }
    }
    CUMULATIVE.set(Some(*budget));
    let _guard = Guard(budget);
    Ok(encode())
}

#[derive(Clone, Copy)]
struct State {
    bytes: usize,
    operations: usize,
    starting_bytes: usize,
    starting_operations: usize,
    depth: usize,
    max_depth: usize,
    refusal: Option<EncodeWorkRefusal>,
}
thread_local! { static ACTIVE: Cell<Option<State>> = const { Cell::new(None) }; }

pub(super) struct Guard {
    _thread: PhantomData<Rc<()>>,
}
impl Guard {
    pub(super) fn enter(limits: EncodeWorkLimits) -> Result<Self, BoundedEncodeWorkError> {
        if ACTIVE.get().is_some() {
            refuse(EncodeWorkRefusal::NestedScope);
            return Err(BoundedEncodeWorkError::Refused(
                first_refusal().expect("active refusal"),
            ));
        }
        let bytes = CUMULATIVE.get().map_or(limits.max_byte_visits, |budget| {
            budget.byte_visits.min(limits.max_byte_visits)
        });
        let operations = CUMULATIVE.get().map_or(limits.max_operations, |budget| {
            budget.operations.min(limits.max_operations)
        });
        ACTIVE.set(Some(State {
            bytes,
            operations,
            starting_bytes: bytes,
            starting_operations: operations,
            depth: 0,
            max_depth: limits.max_depth.min(super::MAX_VALUE_NESTING_DEPTH),
            refusal: None,
        }));
        Ok(Self {
            _thread: PhantomData,
        })
    }
    pub(super) fn check(&self) -> Result<(), super::Error> {
        if first_refusal().is_some() {
            Err(super::Error::LengthMismatch)
        } else {
            Ok(())
        }
    }
    pub(super) fn finish(
        &self,
        result: Result<(), super::Error>,
    ) -> Result<(), BoundedEncodeWorkError> {
        match first_refusal() {
            Some(reason) => Err(BoundedEncodeWorkError::Refused(reason)),
            None => result.map_err(BoundedEncodeWorkError::Codec),
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(state) = ACTIVE.replace(None)
            && let Some(mut budget) = CUMULATIVE.get()
        {
            budget.byte_visits -= state.starting_bytes - state.bytes;
            budget.operations -= state.starting_operations - state.operations;
            CUMULATIVE.set(Some(budget));
        }
    }
}

fn first_refusal() -> Option<EncodeWorkRefusal> {
    ACTIVE.get().and_then(|s| s.refusal)
}
fn refuse(reason: EncodeWorkRefusal) {
    if let Some(mut state) = ACTIVE.get() {
        state.refusal.get_or_insert(reason);
        ACTIVE.set(Some(state));
    }
}

/// Charge an audited serializer's prewrite byte inspection in the current scope.
/// The caller must include every visit and fixed setup in its conservative cost.
/// This is not permission to allocate scratch or execute arbitrary callbacks.
pub fn charge_encode_prewrite(bytes: usize) -> Result<bool, super::Error> {
    if ACTIVE.get().is_none() {
        return Ok(false);
    }
    charge(bytes)?;
    Ok(true)
}

/// Retain a fixed source refusal even when an Option hint or serializer drops it.
/// Outside a bounded scope this returns the fixed codec error without state.
pub fn refuse_encode_source() -> super::Error {
    refuse(EncodeWorkRefusal::Source);
    super::Error::NonCanonicalEncoding
}
/// Refuse before forwarding bytes or touching an output/checksum destination.
pub(super) fn charge(bytes: usize) -> io::Result<()> {
    let Some(mut state) = ACTIVE.get() else {
        return Ok(());
    };
    let reason = state.refusal.or({
        if state.operations == 0 {
            Some(EncodeWorkRefusal::Operations)
        } else if bytes > state.bytes {
            Some(EncodeWorkRefusal::ByteVisits)
        } else {
            None
        }
    });
    if let Some(reason) = reason {
        state.refusal = Some(reason);
        ACTIVE.set(Some(state));
        return Err(io::ErrorKind::InvalidData.into());
    }
    state.operations -= 1;
    state.bytes -= bytes;
    ACTIVE.set(Some(state));
    Ok(())
}
/// Guards codec-driven recursion, including derived field-free values.
pub(super) struct ValueGuard {
    active: bool,
    _thread: PhantomData<Rc<()>>,
}
impl ValueGuard {
    pub(super) fn enter() -> Result<Self, super::Error> {
        charge(0)?;
        let Some(mut state) = ACTIVE.get() else {
            return Ok(Self {
                active: false,
                _thread: PhantomData,
            });
        };
        if state.depth >= state.max_depth {
            refuse(EncodeWorkRefusal::Depth);
            return Err(super::Error::LengthMismatch);
        }
        state.depth += 1;
        ACTIVE.set(Some(state));
        Ok(Self {
            active: true,
            _thread: PhantomData,
        })
    }
}
impl Drop for ValueGuard {
    fn drop(&mut self) {
        if self.active {
            let mut state = ACTIVE.get().expect("value guard inside encoding scope");
            state.depth -= 1;
            ACTIVE.set(Some(state));
        }
    }
}
pub(super) fn admit_frame(length: usize, maximum: usize) -> Result<(), super::Error> {
    charge(0)?;
    if length > maximum {
        refuse(EncodeWorkRefusal::FrameBytes);
        return Err(super::Error::LengthMismatch);
    }
    Ok(())
}
/// Charges header, padding and actual output bytes before reaching the owner.
pub(super) struct Output<'a, W: ?Sized> {
    pub(super) inner: &'a mut W,
}
impl<W: Write + ?Sized> Write for Output<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        charge(bytes.len())?;
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        charge(0)?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod cumulative_tests {
    use super::*;

    fn limits() -> EncodeWorkLimits {
        EncodeWorkLimits {
            max_frame_bytes: 100,
            max_byte_visits: 100,
            max_operations: 100,
            max_depth: 8,
        }
    }

    #[test]
    fn cumulative_encode_work_crosses_calls_without_refunding() {
        let mut budget = EncodeWorkBudget::new(5, 3);
        with_cumulative_encode_work(&mut budget, || {
            super::super::with_bounded_encode_work(limits(), || {
                charge_encode_prewrite(3)?;
                Ok(())
            })
            .unwrap();
            super::super::with_bounded_encode_work(limits(), || {
                charge_encode_prewrite(2)?;
                Ok(())
            })
            .unwrap();
            assert!(matches!(
                super::super::with_bounded_encode_work(limits(), || {
                    charge_encode_prewrite(1)?;
                    Ok(())
                }),
                Err(BoundedEncodeWorkError::Refused(
                    EncodeWorkRefusal::ByteVisits
                ))
            ));
        })
        .unwrap();
        assert_eq!(budget.remaining_byte_visits(), 0);
        assert_eq!(budget.remaining_operations(), 1);
        with_cumulative_encode_work(&mut budget, || {
            super::super::with_bounded_encode_work(limits(), || {
                charge_encode_prewrite(0)?;
                Ok(())
            })
            .unwrap();
        })
        .unwrap();
        assert_eq!(budget.remaining_operations(), 0);
    }

    #[test]
    fn cumulative_encode_work_restores_scope_and_debits_on_unwind() {
        let mut budget = EncodeWorkBudget::new(20, 20);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_cumulative_encode_work(&mut budget, || {
                let _: Result<(), _> = super::super::with_bounded_encode_work(limits(), || {
                    charge_encode_prewrite(7)?;
                    panic!("test unwind");
                });
            });
        }));
        assert!(result.is_err());
        assert_eq!(budget.remaining_byte_visits(), 13);
        assert_eq!(budget.remaining_operations(), 19);
        assert!(ACTIVE.get().is_none());
        assert!(CUMULATIVE.get().is_none());
        with_cumulative_encode_work(&mut budget, || {
            let mut nested = EncodeWorkBudget::new(10, 10);
            assert!(with_cumulative_encode_work(&mut nested, || ()).is_err());
        })
        .unwrap();
    }
}
