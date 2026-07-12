#![allow(clippy::len_without_is_empty)]
use crate::{
    gates::GateInstructions,
    utils::bit_length,
    AssignedValue, Context,
    QuantumCell::{Constant, Existing},
};

use super::{SafeByte, ScalarField};

use getset::Getters;
use itertools::Itertools;

/// Represents a variable length byte array in circuit.
///
/// Each element is guaranteed to be a byte, given by type [`SafeByte`].
/// To represent a variable length array, we must know the maximum possible length `MAX_LEN` the array could be -- this is some additional context the user must provide.
/// Then we right pad the array with 0s to the maximum length (we do **not** constrain that these paddings must be 0s).
#[derive(Debug, Clone, Getters)]
pub struct VarLenBytes<F: ScalarField, const MAX_LEN: usize> {
    /// The byte array, right padded
    #[getset(get = "pub")]
    bytes: [SafeByte<F>; MAX_LEN],
    /// Witness representing the actual length of the byte array. Upon construction, this is range checked to be at most `MAX_LEN`
    #[getset(get = "pub")]
    len: AssignedValue<F>,
}

impl<F: ScalarField, const MAX_LEN: usize> VarLenBytes<F, MAX_LEN> {
    /// Slightly unsafe constructor: it is not constrained that `len <= MAX_LEN`.
    pub fn new(bytes: [SafeByte<F>; MAX_LEN], len: AssignedValue<F>) -> Self {
        assert!(
            len.value().le(&F::from(MAX_LEN as u64)),
            "Invalid length which exceeds MAX_LEN {MAX_LEN}",
        );
        Self { bytes, len }
    }

    /// Returns the maximum length of the byte array.
    pub fn max_len(&self) -> usize {
        MAX_LEN
    }

    /// Left pads the variable length byte array with 0s to the `MAX_LEN`.
    /// Takes a fixed length array `self.bytes` and returns a length `MAX_LEN` array equal to
    /// `[[0; MAX_LEN - len], self.bytes[..len]].concat()`, i.e., we take `self.bytes[..len]` and
    /// zero pad it on the left, where `len = self.len`
    ///
    /// Assumes `0 < self.len <= MAX_LEN`.
    ///
    /// ## Panics
    /// If `self.len` is not in the range `(0, MAX_LEN]`.
    pub fn left_pad_to_fixed(
        &self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
    ) -> FixLenBytes<F, MAX_LEN> {
        let padded = left_pad_var_array_to_fixed(ctx, gate, &self.bytes, self.len, MAX_LEN);
        FixLenBytes::new(
            padded.into_iter().map(|b| SafeByte(b)).collect::<Vec<_>>().try_into().unwrap(),
        )
    }

    /// Return a copy of the byte array with 0 padding ensured.
    pub fn ensure_0_padding(&self, ctx: &mut Context<F>, gate: &impl GateInstructions<F>) -> Self {
        let bytes = ensure_0_padding(ctx, gate, &self.bytes, self.len);
        Self::new(bytes.try_into().unwrap(), self.len)
    }
}

/// Represents a variable length byte array in circuit. Not encouraged to use because `MAX_LEN` cannot be verified at compile time.
///
/// Each element is guaranteed to be a byte, given by type [`SafeByte`].
/// To represent a variable length array, we must know the maximum possible length `MAX_LEN` the array could be -- this is provided when constructing and `bytes.len()` == `MAX_LEN` is enforced.
/// Then we right pad the array with 0s to the maximum length (we do **not** constrain that these paddings must be 0s).
#[derive(Debug, Clone, Getters)]
pub struct VarLenBytesVec<F: ScalarField> {
    /// The byte array, right padded
    #[getset(get = "pub")]
    bytes: Vec<SafeByte<F>>,
    /// Witness representing the actual length of the byte array. Upon construction, this is range checked to be at most `MAX_LEN`
    #[getset(get = "pub")]
    len: AssignedValue<F>,
}

impl<F: ScalarField> VarLenBytesVec<F> {
    /// Slightly unsafe constructor: it is not constrained that `len <= max_len`.
    pub fn new(bytes: Vec<SafeByte<F>>, len: AssignedValue<F>, max_len: usize) -> Self {
        assert!(
            len.value().le(&F::from(max_len as u64)),
            "Invalid length which exceeds MAX_LEN {}",
            max_len
        );
        assert_eq!(bytes.len(), max_len, "bytes is not padded correctly");
        Self { bytes, len }
    }

    /// Returns the maximum length of the byte array.
    pub fn max_len(&self) -> usize {
        self.bytes.len()
    }

    /// Left pads the variable length byte array with 0s to the MAX_LEN
    pub fn left_pad_to_fixed(
        &self,
        ctx: &mut Context<F>,
        gate: &impl GateInstructions<F>,
    ) -> FixLenBytesVec<F> {
        let padded = left_pad_var_array_to_fixed(ctx, gate, &self.bytes, self.len, self.max_len());
        FixLenBytesVec::new(padded.into_iter().map(|b| SafeByte(b)).collect_vec(), self.max_len())
    }

    /// Return a copy of the byte array with 0 padding ensured.
    pub fn ensure_0_padding(&self, ctx: &mut Context<F>, gate: &impl GateInstructions<F>) -> Self {
        let bytes = ensure_0_padding(ctx, gate, &self.bytes, self.len);
        Self::new(bytes, self.len, self.max_len())
    }
}

/// Represents a fixed length byte array in circuit.
#[derive(Debug, Clone, Getters)]
pub struct FixLenBytes<F: ScalarField, const LEN: usize> {
    /// The byte array
    #[getset(get = "pub")]
    bytes: [SafeByte<F>; LEN],
}

impl<F: ScalarField, const LEN: usize> FixLenBytes<F, LEN> {
    /// Constructor
    pub fn new(bytes: [SafeByte<F>; LEN]) -> Self {
        Self { bytes }
    }

    /// Returns the length of the byte array.
    pub fn len(&self) -> usize {
        LEN
    }

    /// Returns inner array of [SafeByte]s.
    pub fn into_bytes(self) -> [SafeByte<F>; LEN] {
        self.bytes
    }
}

/// Represents a fixed length byte array in circuit. Not encouraged to use because `MAX_LEN` cannot be verified at compile time.
#[derive(Debug, Clone, Getters)]
pub struct FixLenBytesVec<F: ScalarField> {
    /// The byte array
    #[getset(get = "pub")]
    bytes: Vec<SafeByte<F>>,
}

impl<F: ScalarField> FixLenBytesVec<F> {
    /// Constructor
    pub fn new(bytes: Vec<SafeByte<F>>, len: usize) -> Self {
        assert_eq!(bytes.len(), len, "bytes length doesn't match");
        Self { bytes }
    }

    /// Returns the length of the byte array.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns inner array of [SafeByte]s.
    pub fn into_bytes(self) -> Vec<SafeByte<F>> {
        self.bytes
    }
}

// Represents a fixed length byte array in circuit as a vector, where length must be fixed.
// Not encouraged to use because `LEN` cannot be verified at compile time.
// pub type FixLenBytesVec<F> = Vec<SafeByte<F>>;

/// Takes a fixed length array `arr` and returns a length `out_len` array equal to
/// `[[0; out_len - len], arr[..len]].concat()`, i.e., we take `arr[..len]` and
/// zero pad it on the left.
///
/// Assumes `0 < len <= max_len <= out_len`.
pub fn left_pad_var_array_to_fixed<F: ScalarField>(
    ctx: &mut Context<F>,
    gate: &impl GateInstructions<F>,
    arr: &[impl AsRef<AssignedValue<F>>],
    len: AssignedValue<F>,
    out_len: usize,
) -> Vec<AssignedValue<F>> {
    debug_assert!(arr.len() <= out_len);
    debug_assert!(bit_length(out_len as u64) < F::CAPACITY as usize);

    let mut padded = arr.iter().map(|b| *b.as_ref()).collect_vec();
    padded.resize(out_len, padded[0]);
    // We use a barrel shifter to shift `arr` to the right by `out_len - len` bits.
    let shift = gate.sub(ctx, Constant(F::from(out_len as u64)), len);
    let shift_bits = gate.num_to_bits(ctx, shift, bit_length(out_len as u64));
    for (i, shift_bit) in shift_bits.into_iter().enumerate() {
        let shifted = (0..out_len)
            .map(|j| if j >= (1 << i) { Existing(padded[j - (1 << i)]) } else { Constant(F::ZERO) })
            .collect_vec();
        padded = padded
            .into_iter()
            .zip(shifted)
            .map(|(noshift, shift)| gate.select(ctx, shift, noshift, shift_bit))
            .collect_vec();
    }
    padded
}

fn ensure_0_padding<F: ScalarField>(
    ctx: &mut Context<F>,
    gate: &impl GateInstructions<F>,
    bytes: &[SafeByte<F>],
    len: AssignedValue<F>,
) -> Vec<SafeByte<F>> {
    let max_len = bytes.len();
    // Generate a mask array where a[i] = i < len for i = 0..max_len.
    let idx = gate.dec(ctx, len);
    let len_indicator = gate.idx_to_indicator(ctx, idx, max_len);
    // inputs_mask[i] = sum(len_indicator[i..])
    let mut mask = gate.partial_sums(ctx, len_indicator.clone().into_iter().rev()).collect_vec();
    mask.reverse();

    bytes
        .iter()
        .zip(mask.iter())
        .map(|(byte, mask)| SafeByte(gate.mul(ctx, byte.0, *mask)))
        .collect_vec()
}
