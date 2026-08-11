//! Bounded ownership corridor for server-owned singular-query execution.

use std::{cell::Cell, io::Write as _, marker::PhantomData};

use norito::core::{DecodeFlagsGuard, DeriveSmallBuf, Encoder, NoritoDeserialize, NoritoSerialize};

use super::Error;

/// Dynamic source/output ceilings for one singular query executed by a
/// server-owned memory lane.
///
/// The frame ceiling bounds the canonical transient used instead of an
/// unmetered deep clone. The allocation ceiling is installed while decoding
/// that frame into the owned query result. Both are deterministic limits
/// supplied by the embedding server's already-acquired memory reservation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SingularQueryOutputLimits {
    max_frame_bytes: u64,
    max_allocated_bytes: u64,
}

impl SingularQueryOutputLimits {
    /// Construct singular-query ownership limits.
    #[must_use]
    pub const fn new(max_frame_bytes: u64, max_allocated_bytes: u64) -> Self {
        Self {
            max_frame_bytes,
            max_allocated_bytes,
        }
    }

    /// Maximum complete canonical frame used to materialize one output.
    #[must_use]
    pub const fn max_frame_bytes(self) -> u64 {
        self.max_frame_bytes
    }

    /// Maximum cumulative allocation admitted while owning one output.
    #[must_use]
    pub const fn max_allocated_bytes(self) -> u64 {
        self.max_allocated_bytes
    }
}

thread_local! {
    static ACTIVE_LIMITS: Cell<Option<SingularQueryOutputLimits>> = const { Cell::new(None) };
}

struct SingularOutputLimitGuard {
    previous: Option<SingularQueryOutputLimits>,
}

impl SingularOutputLimitGuard {
    fn enter(limits: SingularQueryOutputLimits) -> Self {
        let previous = ACTIVE_LIMITS.replace(Some(limits));
        Self { previous }
    }
}

impl Drop for SingularOutputLimitGuard {
    fn drop(&mut self) {
        ACTIVE_LIMITS.set(self.previous);
    }
}

pub(super) fn execute_with_limits<T>(
    limits: Option<SingularQueryOutputLimits>,
    execute: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error>
where
    T: NoritoSerialize,
{
    let Some(limits) = limits else {
        return execute();
    };
    let _guard = SingularOutputLimitGuard::enter(limits);
    let output = execute()?;
    ensure_value_fits(&output, limits)?;
    Ok(output)
}

/// Own one borrowed producer value without invoking an unmetered deep clone.
///
/// Ordinary/IVM queries have no active guard and retain their existing clone
/// behavior. A server-owned singular lane instead canonicalizes into one
/// hard-capped frame and decodes under the admitted allocation ceiling.
pub(crate) fn own_singular_query_value<T>(value: &T) -> Result<T, Error>
where
    T: Clone + NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let Some(limits) = ACTIVE_LIMITS.get() else {
        return Ok(value.clone());
    };
    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    bounded_roundtrip::<T, T>(value, limits)
}

/// Own a sequence of borrowed producer values without first accumulating
/// clones in an intermediate `Vec`.
///
/// The borrowed wrapper advertises the exact `Vec<T>` schema and writes each
/// element directly. The only source-sized allocation is therefore the
/// admitted canonical frame, followed by the limit-checked decoded result.
pub(crate) fn own_singular_query_values<'a, T, I>(values: I) -> Result<Vec<T>, Error>
where
    T: Clone + NoritoSerialize + 'a,
    for<'de> T: NoritoDeserialize<'de>,
    I: Clone + Iterator<Item = &'a T>,
{
    let Some(limits) = ACTIVE_LIMITS.get() else {
        return Ok(values.cloned().collect());
    };
    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let borrowed = BorrowedSequence::<I, T> {
        values,
        marker: PhantomData,
    };
    bounded_roundtrip::<_, Vec<T>>(&borrowed, limits)
}

/// Materialize a struct directly from borrowed fields in declaration order.
///
/// This is the bounded counterpart for projections whose world-state storage
/// splits an entity into an identifier and a value. Outside the server-owned
/// lane the existing constructor is used unchanged. Inside it, the fields are
/// encoded under `T`'s schema without first constructing an owned clone.
pub(crate) fn own_singular_query_struct<T, const N: usize>(
    fields: [&dyn NoritoSerialize; N],
    fallback: impl FnOnce() -> T,
) -> Result<T, Error>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let Some(limits) = ACTIVE_LIMITS.get() else {
        return Ok(fallback());
    };
    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let borrowed = BorrowedStruct::<T, N> {
        fields,
        marker: PhantomData,
    };
    bounded_roundtrip::<_, T>(&borrowed, limits)
}

fn bounded_roundtrip<S, T>(source: &S, limits: SingularQueryOutputLimits) -> Result<T, Error>
where
    S: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let max_frame_bytes =
        usize::try_from(limits.max_frame_bytes).map_err(|_| Error::CapacityLimit)?;
    let bytes = norito::core::to_bytes_bounded(source, max_frame_bytes)
        .map_err(|_| Error::CapacityLimit)?;
    norito::decode_from_bytes_with_limits::<T>(
        &bytes,
        decode_limits(bytes.len(), limits.max_allocated_bytes)?,
    )
    .map_err(|_| Error::CapacityLimit)
}

struct BorrowedSequence<I, T> {
    values: I,
    marker: PhantomData<T>,
}

/// Borrowed wire-equivalent of `Option<T>` for a singular projection field.
pub(crate) struct BorrowedSingularOption<'a, T>(Option<&'a T>);

impl<'a, T> BorrowedSingularOption<'a, T> {
    /// Construct a borrowed optional field.
    #[must_use]
    pub(crate) const fn new(value: Option<&'a T>) -> Self {
        Self(value)
    }
}

impl<T: NoritoSerialize> NoritoSerialize for BorrowedSingularOption<'_, T> {
    fn schema_hash() -> [u8; 16] {
        Option::<T>::schema_hash()
    }

    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::core::Error> {
        match self.0 {
            Some(value) => {
                writer.write_all(&[1])?;
                let mut scratch = DeriveSmallBuf::new();
                norito::core::write_len_prefixed(writer, value, &mut scratch)
            }
            None => {
                writer.write_all(&[0])?;
                Ok(())
            }
        }
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        match self.0 {
            Some(value) => {
                let value_len = value.encoded_len_exact()?;
                1usize
                    .checked_add(norito::core::len_prefix_len(value_len))?
                    .checked_add(value_len)
            }
            None => Some(1),
        }
    }
}

struct BorrowedStruct<'a, T, const N: usize> {
    fields: [&'a dyn NoritoSerialize; N],
    marker: PhantomData<T>,
}

impl<T, const N: usize> NoritoSerialize for BorrowedStruct<'_, T, N>
where
    T: NoritoSerialize,
{
    fn schema_hash() -> [u8; 16] {
        T::schema_hash()
    }

    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::core::Error> {
        if norito::core::use_packed_struct() {
            return Err(norito::core::Error::UnsupportedFeature(
                "borrowed singular packed struct",
            ));
        }
        let mut scratch = DeriveSmallBuf::new();
        for value in self.fields.iter().copied() {
            norito::core::write_len_prefixed(writer, value, &mut scratch)?;
        }
        Ok(())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        if norito::core::use_packed_struct() {
            return None;
        }
        self.fields.iter().try_fold(0usize, |total, value| {
            let value_len = value.encoded_len_exact()?;
            total
                .checked_add(norito::core::len_prefix_len(value_len))?
                .checked_add(value_len)
        })
    }
}

impl<'a, I, T> NoritoSerialize for BorrowedSequence<I, T>
where
    T: NoritoSerialize + 'a,
    I: Clone + Iterator<Item = &'a T>,
{
    fn schema_hash() -> [u8; 16] {
        Vec::<T>::schema_hash()
    }

    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::core::Error> {
        let count = self.values.clone().count();
        norito::core::write_seq_len(
            writer,
            u64::try_from(count).map_err(|_| norito::core::Error::LengthMismatch)?,
        )?;
        let mut scratch = DeriveSmallBuf::new();
        for value in self.values.clone() {
            norito::core::write_len_prefixed(writer, value, &mut scratch)?;
        }
        Ok(())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.values.clone().try_fold(8usize, |total, value| {
            let value_len = value.encoded_len_exact()?;
            total
                .checked_add(norito::core::len_prefix_len(value_len))?
                .checked_add(value_len)
        })
    }
}

/// Return the currently admitted complete-frame ceiling, capped by a producer
/// protocol maximum.
pub(crate) fn singular_query_frame_limit(protocol_max: usize) -> usize {
    ACTIVE_LIMITS.get().map_or(protocol_max, |limits| {
        usize::try_from(limits.max_frame_bytes)
            .unwrap_or(usize::MAX)
            .min(protocol_max)
    })
}

/// Return whether server-owned singular limits are active on this worker.
pub(crate) fn singular_query_limits_active() -> bool {
    ACTIVE_LIMITS.get().is_some()
}

/// Allocate one producer vector fallibly and reject a capacity whose inline
/// storage alone exceeds the active decoded-output allowance.
pub(crate) fn singular_query_vec_with_capacity<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let inline_bytes = capacity
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(Error::CapacityLimit)?;
    if ACTIVE_LIMITS.get().is_some_and(|limits| {
        u64::try_from(inline_bytes).map_or(true, |bytes| bytes > limits.max_allocated_bytes)
    }) {
        return Err(Error::CapacityLimit);
    }
    if ACTIVE_LIMITS.get().is_some() {
        norito::core::reserve_decode_allocation(inline_bytes).map_err(|_| Error::CapacityLimit)?;
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| Error::CapacityLimit)?;
    Ok(values)
}

/// Append to a producer vector without allowing infallible capacity growth.
pub(crate) fn singular_query_vec_push<T>(values: &mut Vec<T>, value: T) -> Result<(), Error> {
    if values.len() == values.capacity() {
        let inline_bytes = core::mem::size_of::<T>();
        if ACTIVE_LIMITS.get().is_some() {
            norito::core::reserve_decode_allocation(inline_bytes)
                .map_err(|_| Error::CapacityLimit)?;
        }
        values
            .try_reserve_exact(1)
            .map_err(|_| Error::CapacityLimit)?;
    }
    values.push(value);
    Ok(())
}

/// Fallibly build one retained singular-query sequence under the active
/// resident-output allowance.
///
/// Each inserted value is first measured through the canonical codec. The
/// value's complete frame must fit `E`, and the final vector capacity plus the
/// conservative measured allocation charge for all retained elements must fit
/// `D`. The source value and its bounded frame are dropped before insertion,
/// so a producer retains only this builder between loop iterations.
pub(crate) struct SingularQueryVecBuilder<T> {
    values: Vec<T>,
    retained_nested_bytes: usize,
    encoded_frame_bytes: usize,
}

impl<T> SingularQueryVecBuilder<T>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    /// Construct a builder with a fallibly reserved element capacity.
    pub(crate) fn new(capacity: usize) -> Result<Self, Error> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(capacity)
            .map_err(|_| Error::CapacityLimit)?;

        let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let encoded_frame_bytes =
            norito::core::encoded_frame_len(&values).map_err(|_| Error::CapacityLimit)?;
        if let Some(limits) = ACTIVE_LIMITS.get() {
            let max_frame_bytes =
                usize::try_from(limits.max_frame_bytes).map_err(|_| Error::CapacityLimit)?;
            if encoded_frame_bytes > max_frame_bytes {
                return Err(Error::CapacityLimit);
            }
            ensure_retained_allocation_fits::<T>(values.capacity(), 0, limits)?;
        }

        Ok(Self {
            values,
            retained_nested_bytes: 0,
            encoded_frame_bytes,
        })
    }

    /// Canonicalize and admit one value before retaining it.
    pub(crate) fn try_push(&mut self, value: T) -> Result<(), Error> {
        let Some(limits) = ACTIVE_LIMITS.get() else {
            if self.values.len() == self.values.capacity() {
                self.values
                    .try_reserve_exact(1)
                    .map_err(|_| Error::CapacityLimit)?;
            }
            self.values.push(value);
            return Ok(());
        };

        let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let max_frame_bytes =
            usize::try_from(limits.max_frame_bytes).map_err(|_| Error::CapacityLimit)?;
        let item_payload_bytes =
            norito::core::encoded_payload_len(&value).map_err(|_| Error::CapacityLimit)?;
        let next_frame_bytes = self
            .encoded_frame_bytes
            .checked_add(norito::core::len_prefix_len(item_payload_bytes))
            .and_then(|bytes| bytes.checked_add(item_payload_bytes))
            .filter(|bytes| *bytes <= max_frame_bytes)
            .ok_or(Error::CapacityLimit)?;

        let bytes = norito::core::to_bytes_bounded(&value, max_frame_bytes)
            .map_err(|_| Error::CapacityLimit)?;
        let item_decode_limits = decode_limits(bytes.len(), limits.max_allocated_bytes)?;
        let (decoded, usage) =
            norito::core::with_decode_limits_measured(item_decode_limits, || {
                norito::decode_from_bytes_with_limits::<T>(&bytes, item_decode_limits)
            });
        let decoded = decoded.map_err(|_| Error::CapacityLimit)?;
        drop(bytes);
        drop(value);

        if self.values.len() == self.values.capacity() {
            self.values
                .try_reserve_exact(1)
                .map_err(|_| Error::CapacityLimit)?;
        }
        let retained_nested_bytes = self
            .retained_nested_bytes
            .checked_add(usage.total_allocated_bytes())
            .ok_or(Error::CapacityLimit)?;
        ensure_retained_allocation_fits::<T>(
            self.values.capacity(),
            retained_nested_bytes,
            limits,
        )?;

        self.values.push(decoded);
        self.retained_nested_bytes = retained_nested_bytes;
        self.encoded_frame_bytes = next_frame_bytes;
        Ok(())
    }

    /// Number of values currently retained by the builder.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether the builder currently retains no values.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Return the most recently retained value.
    #[must_use]
    pub(crate) fn last(&self) -> Option<&T> {
        self.values.last()
    }

    /// Finish the builder without copying its retained values.
    #[must_use]
    pub(crate) fn into_vec(self) -> Vec<T> {
        self.values
    }
}

fn ensure_retained_allocation_fits<T>(
    capacity: usize,
    retained_nested_bytes: usize,
    limits: SingularQueryOutputLimits,
) -> Result<(), Error> {
    let inline_bytes = capacity
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(Error::CapacityLimit)?;
    let retained_bytes = inline_bytes
        .checked_add(retained_nested_bytes)
        .ok_or(Error::CapacityLimit)?;
    let max_allocated_bytes =
        usize::try_from(limits.max_allocated_bytes).map_err(|_| Error::CapacityLimit)?;
    (retained_bytes <= max_allocated_bytes)
        .then_some(())
        .ok_or(Error::CapacityLimit)
}

/// Check a producer value against the active frame ceiling and a protocol cap.
pub(crate) fn singular_query_ensure_value_fits<T: NoritoSerialize>(
    value: &T,
    protocol_max: usize,
) -> Result<(), Error> {
    if !singular_query_limits_active() {
        return Ok(());
    }
    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let encoded = norito::core::encoded_frame_len(value).map_err(|_| Error::CapacityLimit)?;
    if encoded > singular_query_frame_limit(protocol_max) {
        return Err(Error::CapacityLimit);
    }
    Ok(())
}

/// Cap an existing producer decode policy by the active singular-query lane.
///
/// The protocol's tighter dimensions are preserved. Persisted source records
/// are not required to fit the eventual response frame, but no decoded field
/// or collection may allocate beyond the response's admitted decode budget.
pub(crate) fn singular_query_decode_limits(
    encoded_len: usize,
    protocol: norito::DecodeLimits,
) -> Result<norito::DecodeLimits, Error> {
    let Some(active) = ACTIVE_LIMITS.get() else {
        return Ok(protocol);
    };
    let allocated =
        usize::try_from(active.max_allocated_bytes).map_err(|_| Error::CapacityLimit)?;
    Ok(norito::DecodeLimits::new(
        protocol.max_sequence_elements().min(allocated),
        protocol.max_field_bytes().min(encoded_len).min(allocated),
        protocol.max_total_elements().min(allocated),
        protocol.max_total_allocated_bytes().min(allocated),
        protocol.max_nesting_depth(),
    ))
}

fn ensure_value_fits<T: NoritoSerialize>(
    value: &T,
    limits: SingularQueryOutputLimits,
) -> Result<(), Error> {
    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let max_frame_bytes =
        usize::try_from(limits.max_frame_bytes).map_err(|_| Error::CapacityLimit)?;
    let encoded = norito::core::encoded_frame_len(value).map_err(|_| Error::CapacityLimit)?;
    if encoded > max_frame_bytes {
        return Err(Error::CapacityLimit);
    }
    Ok(())
}

fn decode_limits(
    encoded_len: usize,
    max_allocated_bytes: u64,
) -> Result<norito::DecodeLimits, Error> {
    let max_allocated_bytes =
        usize::try_from(max_allocated_bytes).map_err(|_| Error::CapacityLimit)?;
    let elements = encoded_len.checked_mul(8).ok_or(Error::CapacityLimit)?;
    Ok(norito::DecodeLimits::new(
        elements,
        encoded_len,
        elements,
        max_allocated_bytes,
        64,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_limits_restore_the_previous_value() {
        let outer = SingularQueryOutputLimits::new(1024, 2048);
        let inner = SingularQueryOutputLimits::new(64, 128);
        let _outer = SingularOutputLimitGuard::enter(outer);
        assert_eq!(ACTIVE_LIMITS.get(), Some(outer));
        {
            let _inner = SingularOutputLimitGuard::enter(inner);
            assert_eq!(ACTIVE_LIMITS.get(), Some(inner));
        }
        assert_eq!(ACTIVE_LIMITS.get(), Some(outer));
    }
}
