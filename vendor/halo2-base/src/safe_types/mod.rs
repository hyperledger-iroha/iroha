use std::{
    borrow::Borrow,
    cmp::{max, min},
};

use crate::{
    gates::{
        flex_gate::GateInstructions,
        range::{RangeChip, RangeInstructions},
    },
    utils::ScalarField,
    AssignedValue, Context,
    QuantumCell::Constant,
};

use itertools::Itertools;

mod bytes;
mod primitives;

pub use bytes::*;
pub use primitives::*;

/// Unit Tests
#[cfg(test)]
pub mod tests;

type RawAssignedValues<F> = Vec<AssignedValue<F>>;

const BITS_PER_BYTE: usize = 8;

/// [`SafeType`]'s goal is to avoid out-of-range undefined behavior.
/// When building circuits, it's common to use multiple [`AssignedValue<F>`]s to represent
/// a logical variable. For example, we might want to represent a hash with 32 [`AssignedValue<F>`]
/// where each [`AssignedValue`] represents 1 byte. However, the range of [`AssignedValue<F>`] is much
/// larger than 1 byte(0~255). If a circuit takes 32 [`AssignedValue<F>`] as inputs and some of them
/// are actually greater than 255, there could be some undefined behaviors.
/// [`SafeType`] gurantees the value range of its owned [`AssignedValue<F>`]. So circuits don't need to
/// do any extra value checking if they take SafeType as inputs.
/// - `TOTAL_BITS` is the number of total bits of this type.
/// - `BYTES_PER_ELE` is the number of bytes of each element.
#[derive(Clone, Debug)]
pub struct SafeType<F: ScalarField, const BYTES_PER_ELE: usize, const TOTAL_BITS: usize> {
    // value is stored in little-endian.
    value: RawAssignedValues<F>,
}

impl<F: ScalarField, const BYTES_PER_ELE: usize, const TOTAL_BITS: usize>
    SafeType<F, BYTES_PER_ELE, TOTAL_BITS>
{
    /// Number of bytes of each element.
    pub const BYTES_PER_ELE: usize = BYTES_PER_ELE;
    /// Total bits of this type.
    pub const TOTAL_BITS: usize = TOTAL_BITS;
    /// Number of elements of this type.
    pub const VALUE_LENGTH: usize = TOTAL_BITS.div_ceil(BYTES_PER_ELE * BITS_PER_BYTE);

    /// Number of bits of each element.
    pub fn bits_per_ele() -> usize {
        min(TOTAL_BITS, BYTES_PER_ELE * BITS_PER_BYTE)
    }

    // new is private so Safetype can only be constructed by this crate.
    fn new(raw_values: RawAssignedValues<F>) -> Self {
        assert!(raw_values.len() == Self::VALUE_LENGTH, "Invalid raw values length");
        Self { value: raw_values }
    }

    /// Return values in little-endian.
    pub fn value(&self) -> &[AssignedValue<F>] {
        &self.value
    }
}

impl<F: ScalarField, const BYTES_PER_ELE: usize, const TOTAL_BITS: usize> AsRef<[AssignedValue<F>]>
    for SafeType<F, BYTES_PER_ELE, TOTAL_BITS>
{
    fn as_ref(&self) -> &[AssignedValue<F>] {
        self.value()
    }
}

impl<F: ScalarField, const TOTAL_BITS: usize> TryFrom<Vec<SafeByte<F>>>
    for SafeType<F, 1, TOTAL_BITS>
{
    type Error = String;

    fn try_from(value: Vec<SafeByte<F>>) -> Result<Self, Self::Error> {
        if value.len() * 8 != TOTAL_BITS {
            return Err("Invalid length".to_owned());
        }
        Ok(Self::new(value.into_iter().map(|b| b.0).collect::<Vec<_>>()))
    }
}

/// SafeType for Address.
pub type SafeAddress<F> = SafeType<F, 1, 160>;
/// SafeType for bytes32.
pub type SafeBytes32<F> = SafeType<F, 1, 256>;

/// Chip for SafeType
pub struct SafeTypeChip<'a, F: ScalarField> {
    range_chip: &'a RangeChip<F>,
}

impl<'a, F: ScalarField> SafeTypeChip<'a, F> {
    /// Construct a SafeTypeChip.
    pub fn new(range_chip: &'a RangeChip<F>) -> Self {
        Self { range_chip }
    }

    /// Convert a vector of AssignedValue (treated as little-endian) to a SafeType.
    /// The number of bytes of inputs must equal to the number of bytes of outputs.
    /// This function also add contraints that a AssignedValue in inputs must be in the range of a byte.
    pub fn raw_bytes_to<const BYTES_PER_ELE: usize, const TOTAL_BITS: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: RawAssignedValues<F>,
    ) -> SafeType<F, BYTES_PER_ELE, TOTAL_BITS> {
        let element_bits = SafeType::<F, BYTES_PER_ELE, TOTAL_BITS>::bits_per_ele();
        let bits = TOTAL_BITS;
        assert!(
            inputs.len() * BITS_PER_BYTE == max(bits, BITS_PER_BYTE),
            "number of bits doesn't match"
        );
        self.add_bytes_constraints(ctx, &inputs, bits);
        // inputs is a bool or uint8.
        if bits == 1 || element_bits == BITS_PER_BYTE {
            return SafeType::<F, BYTES_PER_ELE, TOTAL_BITS>::new(inputs);
        };
        assert!(
            element_bits <= F::CAPACITY as usize,
            "packed SafeType element exceeds native field capacity"
        );

        let byte_base = (0..BYTES_PER_ELE)
            .map(|i| Constant(self.range_chip.gate.pow_of_two[i * BITS_PER_BYTE]))
            .collect::<Vec<_>>();
        let value = inputs
            .chunks(BYTES_PER_ELE)
            .map(|chunk| {
                self.range_chip.gate.inner_product(
                    ctx,
                    chunk.to_vec(),
                    byte_base[..chunk.len()].to_vec(),
                )
            })
            .collect::<Vec<_>>();
        SafeType::<F, BYTES_PER_ELE, TOTAL_BITS>::new(value)
    }

    /// Unsafe method that directly converts `input` to [`SafeType`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeType`].
    pub fn unsafe_to_safe_type<const BYTES_PER_ELE: usize, const TOTAL_BITS: usize>(
        inputs: RawAssignedValues<F>,
    ) -> SafeType<F, BYTES_PER_ELE, TOTAL_BITS> {
        assert_eq!(inputs.len(), SafeType::<F, BYTES_PER_ELE, TOTAL_BITS>::VALUE_LENGTH);
        SafeType::<F, BYTES_PER_ELE, TOTAL_BITS>::new(inputs)
    }

    /// Constrains that the `input` is a boolean value (either 0 or 1) and wraps it in [`SafeBool`].
    pub fn assert_bool(&self, ctx: &mut Context<F>, input: AssignedValue<F>) -> SafeBool<F> {
        self.range_chip.gate().assert_bit(ctx, input);
        SafeBool(input)
    }

    /// Load a boolean value as witness and constrain it is either 0 or 1.
    pub fn load_bool(&self, ctx: &mut Context<F>, input: bool) -> SafeBool<F> {
        let input = ctx.load_witness(F::from(input));
        self.assert_bool(ctx, input)
    }

    /// Unsafe method that directly converts `input` to [`SafeBool`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeBool`].
    pub fn unsafe_to_bool(input: AssignedValue<F>) -> SafeBool<F> {
        SafeBool(input)
    }

    /// Constrains that the `input` is a byte value and wraps it in [`SafeByte`].
    pub fn assert_byte(&self, ctx: &mut Context<F>, input: AssignedValue<F>) -> SafeByte<F> {
        self.range_chip.range_check(ctx, input, BITS_PER_BYTE);
        SafeByte(input)
    }

    /// Load a boolean value as witness and constrain it is either 0 or 1.
    pub fn load_byte(&self, ctx: &mut Context<F>, input: u8) -> SafeByte<F> {
        let input = ctx.load_witness(F::from(input as u64));
        self.assert_byte(ctx, input)
    }

    /// Unsafe method that directly converts `input` to [`SafeByte`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeByte`].
    pub fn unsafe_to_byte(input: AssignedValue<F>) -> SafeByte<F> {
        SafeByte(input)
    }

    /// Unsafe method that directly converts `inputs` to [`VarLenBytes`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeByte`].
    pub fn unsafe_to_var_len_bytes<const MAX_LEN: usize>(
        inputs: [AssignedValue<F>; MAX_LEN],
        len: AssignedValue<F>,
    ) -> VarLenBytes<F, MAX_LEN> {
        VarLenBytes::<F, MAX_LEN>::new(inputs.map(|input| Self::unsafe_to_byte(input)), len)
    }

    /// Unsafe method that directly converts `inputs` to [`VarLenBytesVec`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeByte`].
    pub fn unsafe_to_var_len_bytes_vec(
        inputs: RawAssignedValues<F>,
        len: AssignedValue<F>,
        max_len: usize,
    ) -> VarLenBytesVec<F> {
        VarLenBytesVec::<F>::new(
            inputs.iter().map(|input| Self::unsafe_to_byte(*input)).collect_vec(),
            len,
            max_len,
        )
    }

    /// Unsafe method that directly converts `inputs` to [`FixLenBytes`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeByte`].
    pub fn unsafe_to_fix_len_bytes<const MAX_LEN: usize>(
        inputs: [AssignedValue<F>; MAX_LEN],
    ) -> FixLenBytes<F, MAX_LEN> {
        FixLenBytes::<F, MAX_LEN>::new(inputs.map(|input| Self::unsafe_to_byte(input)))
    }

    /// Unsafe method that directly converts `inputs` to [`FixLenBytesVec`] **without any checks**.
    /// This should **only** be used if an external library needs to convert their types to [`SafeByte`].
    pub fn unsafe_to_fix_len_bytes_vec(
        inputs: RawAssignedValues<F>,
        len: usize,
    ) -> FixLenBytesVec<F> {
        FixLenBytesVec::<F>::new(
            inputs.into_iter().map(|input| Self::unsafe_to_byte(input)).collect_vec(),
            len,
        )
    }

    /// Converts a slice of AssignedValue(treated as little-endian) to VarLenBytes.
    ///
    /// * inputs: Slice representing the byte array.
    /// * len: [`AssignedValue<F>`] witness representing the variable length of the byte array. Constrained to be `<= MAX_LEN`.
    /// * MAX_LEN: [usize] representing the maximum length of the byte array and the number of elements it must contain.
    ///
    /// ## Assumptions
    /// * `MAX_LEN < u64::MAX` to prevent overflow (but you should never make an array this large)
    /// * `ceil((MAX_LEN + 1).bits() / lookup_bits) * lookup_bits < F::CAPACITY` where `lookup_bits = self.range_chip.lookup_bits`
    pub fn raw_to_var_len_bytes<const MAX_LEN: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: [AssignedValue<F>; MAX_LEN],
        len: AssignedValue<F>,
    ) -> VarLenBytes<F, MAX_LEN> {
        self.range_chip.check_less_than_safe(ctx, len, MAX_LEN as u64 + 1);
        VarLenBytes::<F, MAX_LEN>::new(inputs.map(|input| self.assert_byte(ctx, input)), len)
    }

    /// Converts a vector of AssignedValue to [VarLenBytesVec]. Not encouraged to use because `MAX_LEN` cannot be verified at compile time.
    ///
    /// * inputs: Vector representing the byte array, right padded to `max_len`. See [VarLenBytesVec] for details about padding.
    /// * len: [`AssignedValue<F>`] witness representing the variable length of the byte array. Constrained to be `<= max_len`.
    /// * max_len: [usize] representing the maximum length of the byte array and the number of elements it must contain. We enforce this to be provided explictly to make sure length of `inputs` is determinstic.
    ///
    /// ## Assumptions
    /// * `max_len < u64::MAX` to prevent overflow (but you should never make an array this large)
    /// * `ceil((max_len + 1).bits() / lookup_bits) * lookup_bits < F::CAPACITY` where `lookup_bits = self.range_chip.lookup_bits`
    pub fn raw_to_var_len_bytes_vec(
        &self,
        ctx: &mut Context<F>,
        inputs: RawAssignedValues<F>,
        len: AssignedValue<F>,
        max_len: usize,
    ) -> VarLenBytesVec<F> {
        self.range_chip.check_less_than_safe(ctx, len, max_len as u64 + 1);
        VarLenBytesVec::<F>::new(
            inputs.iter().map(|input| self.assert_byte(ctx, *input)).collect_vec(),
            len,
            max_len,
        )
    }

    /// Converts a slice of AssignedValue(treated as little-endian) to FixLenBytes.
    ///
    /// * inputs: Slice representing the byte array.
    /// * LEN: length of the byte array.
    pub fn raw_to_fix_len_bytes<const LEN: usize>(
        &self,
        ctx: &mut Context<F>,
        inputs: [AssignedValue<F>; LEN],
    ) -> FixLenBytes<F, LEN> {
        FixLenBytes::<F, LEN>::new(inputs.map(|input| self.assert_byte(ctx, input)))
    }

    /// Converts a slice of AssignedValue(treated as little-endian) to FixLenBytesVec.
    ///
    /// * inputs: Slice representing the byte array.
    /// * len: length of the byte array. We enforce this to be provided explictly to make sure length of `inputs` is determinstic.
    pub fn raw_to_fix_len_bytes_vec(
        &self,
        ctx: &mut Context<F>,
        inputs: RawAssignedValues<F>,
        len: usize,
    ) -> FixLenBytesVec<F> {
        FixLenBytesVec::<F>::new(
            inputs.into_iter().map(|input| self.assert_byte(ctx, input)).collect_vec(),
            len,
        )
    }

    /// Assumes that `bits <= inputs.len() * 8`.
    fn add_bytes_constraints(
        &self,
        ctx: &mut Context<F>,
        inputs: &RawAssignedValues<F>,
        bits: usize,
    ) {
        let mut bits_left = bits;
        for input in inputs {
            let num_bit = min(bits_left, BITS_PER_BYTE);
            self.range_chip.range_check(ctx, *input, num_bit);
            bits_left -= num_bit;
        }
    }

    // TODO: Add comparison. e.g. is_less_than(SafeUint8, SafeUint8) -> SafeBool
    // TODO: Add type castings. e.g. uint256 -> bytes32/uint32 -> uint64
}
