//! Exact bounded integer gates for the canonical RNS scale-down relation.
//!
//! Words borrow the parent's unique 64-bit wires; auxiliaries and residues are
//! caller-owned slices. No helper allocates. Schoolbook products use eight
//! 16-bit limbs and nine 18-bit carries, including constrained zero endpoints.
//! Each column equation has absolute value below `2^35`, so Goldilocks cannot
//! hide an integer discrepancy. Exact sums similarly constrain both carry
//! endpoints. Every polynomial has degree at most two, including activation.
//!
//! Concrete addition scratch clears on drop; other native arithmetic scratch
//! uses clearing owners. Borrowed openings, field-evaluation temporaries and
//! compiler/register copies are outside this custody claim. These are relation
//! numerators, not a committed proof or authenticated key/profile statement.

use core::{fmt, mem::size_of};

use zeroize::{Zeroize, Zeroizing};

use crate::gadgets::{
    arx64_air::{Add64Witness, BitWord64, add_residues},
    transfer_integer_air::IntegerAirField,
};

const WORD_BITS: usize = 64;
const LIMB_BITS: usize = 16;
const CARRY_BITS: usize = 18;
const PRODUCT_BITS: usize = 128;
const PRODUCT_AUX: usize = 290;

/// One public operation selected by the enclosing immutable plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GateKind {
    /// Check one word is below the modulus; no output wire is allocated.
    Canonical,
    /// Reduce an arbitrary complete `u64` word.
    Reduce,
    /// Add two canonical residues.
    AddMod,
    /// Subtract two canonical residues.
    SubMod,
    /// Multiply two canonical residues using complete integer products.
    MulMod,
}

/// Exact caller-owned cell and numerator counts for one gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GateLayout {
    /// Auxiliary cells, excluding the shared input/output word bits.
    pub auxiliary_columns: usize,
    /// Number of emitted polynomial numerators.
    pub constraint_count: usize,
}

impl GateKind {
    /// Fixed geometry; private values never select a layout.
    pub const fn layout(self) -> GateLayout {
        let (auxiliary_columns, constraint_count) = match self {
            Self::Canonical => (66, 198),
            Self::Reduce => (429, 903),
            Self::AddMod | Self::SubMod => (267, 991),
            Self::MulMod => (917, 1990),
        };
        GateLayout {
            auxiliary_columns,
            constraint_count,
        }
    }

    const fn binary(self) -> bool {
        matches!(self, Self::AddMod | Self::SubMod | Self::MulMod)
    }
}

/// Borrowed shared word wires, never a new witness owner.
#[derive(Clone, Copy)]
pub(super) struct WordViews<'a, F> {
    /// Sole input for canonical/reduction gates, or the first binary operand.
    pub left: &'a [F; WORD_BITS],
    /// Present exactly for binary gates.
    pub right: Option<&'a [F; WORD_BITS]>,
    /// Absent exactly for a canonical check.
    pub output: Option<&'a [F; WORD_BITS]>,
}

/// Public geometry/domain refusal or a noncanonical native input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IntegerError {
    /// Modulus is outside the helper's public interval `[2, 2^62)`.
    Modulus,
    /// Argument or slice geometry differs from the fixed gate layout.
    Layout,
    /// A native input to a canonical operation is outside `[0, q)`.
    NonCanonical,
}

impl fmt::Display for IntegerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Modulus => "RNS integer gate modulus outside [2, 2^62)",
            Self::Layout => "RNS integer gate layout mismatch",
            Self::NonCanonical => "RNS integer gate input is not canonical",
        })
    }
}

impl std::error::Error for IntegerError {}

fn check_geometry(
    kind: GateKind,
    modulus: u64,
    has_right: bool,
    auxiliary_columns: usize,
) -> Result<(), IntegerError> {
    if !(2..(1_u64 << 62)).contains(&modulus) {
        return Err(IntegerError::Modulus);
    }
    if has_right != kind.binary() || auxiliary_columns != kind.layout().auxiliary_columns {
        return Err(IntegerError::Layout);
    }
    Ok(())
}

/// Clearing guard for the existing concrete addition witness carrier.
struct ClearingAdd(Add64Witness<u64>);

/// Conservative payload of the named clearing temporaries on a native fill path.
///
/// Gates are filled sequentially, so the caller charges this once. The sum
/// deliberately includes temporaries whose scopes do not overlap. Scalar
/// arguments, registers/compiler copies, stack alignment/call frames, caller
/// buffers and generic polynomial-opening evaluation are separate scopes.
pub(super) const NATIVE_SCRATCH_BYTES: usize = size_of::<Zeroizing<[u64; 5]>>()
    + 2 * size_of::<Zeroizing<u128>>()
    + 2 * size_of::<Zeroizing<u64>>()
    + size_of::<ClearingAdd>();

#[cfg(test)]
thread_local! {
    static CLEAR_EVENTS: core::cell::Cell<(usize, usize)> = const { core::cell::Cell::new((0, 0)) };
}

impl Drop for ClearingAdd {
    fn drop(&mut self) {
        self.0.left.bits.zeroize();
        self.0.right.bits.zeroize();
        self.0.output.bits.zeroize();
        self.0.carry_32.zeroize();
        self.0.carry_64.zeroize();
        #[cfg(test)]
        CLEAR_EVENTS.with(|events| {
            let cleared = self
                .0
                .left
                .bits
                .iter()
                .chain(&self.0.right.bits)
                .chain(&self.0.output.bits)
                .all(|&bit| bit == 0)
                && self.0.carry_32 == 0
                && self.0.carry_64 == 0;
            let (seen, zeroed) = events.get();
            events.set((seen + 1, zeroed + usize::from(cleared)));
        });
    }
}

fn write_bits(value: u64, output: &mut [u64]) {
    for (bit, cell) in output.iter_mut().enumerate() {
        *cell = (value >> bit) & 1;
    }
}

fn fill_canonical(modulus: u64, value: u64, auxiliary: &mut [u64]) {
    let addition = ClearingAdd(Add64Witness::from_operands(value, modulus - 1 - value));
    auxiliary[..64].copy_from_slice(&addition.0.right.bits);
    auxiliary[64] = addition.0.carry_32;
    auxiliary[65] = addition.0.carry_64;
}

fn fill_add_carries(left: u64, right: u64, output: &mut [u64]) {
    let addition = ClearingAdd(Add64Witness::from_operands(left, right));
    output[0] = addition.0.carry_32;
    output[1] = addition.0.carry_64;
}

fn fill_product(left: u64, right: u64, auxiliary: &mut [u64]) {
    let product = Zeroizing::new(u128::from(left) * u128::from(right));
    for (bit, cell) in auxiliary[..PRODUCT_BITS].iter_mut().enumerate() {
        *cell = u64::try_from((*product >> bit) & 1).expect("one product bit");
    }
    let mut carry = Zeroizing::new(0_u64);
    for column in 0..8 {
        write_bits(
            *carry,
            &mut auxiliary
                [PRODUCT_BITS + column * CARRY_BITS..PRODUCT_BITS + (column + 1) * CARRY_BITS],
        );
        let mut total = Zeroizing::new(*carry);
        for left_limb in 0..4 {
            if column >= left_limb && column - left_limb < 4 {
                let right_limb = column - left_limb;
                *total += ((left >> (LIMB_BITS * left_limb)) & 0xffff)
                    * ((right >> (LIMB_BITS * right_limb)) & 0xffff);
            }
        }
        *carry = *total >> LIMB_BITS;
    }
    write_bits(*carry, &mut auxiliary[PRODUCT_BITS + 8 * CARRY_BITS..]);
}

fn fill_sum_carries(product: u128, remainder: u64, output: &mut [u64]) {
    let mut carry = Zeroizing::new(0_u64);
    for (column, cell) in output[..8].iter_mut().enumerate() {
        *cell = *carry;
        let mut total = Zeroizing::new(
            u64::try_from((product >> (LIMB_BITS * column)) & 0xffff).expect("one product limb")
                + *carry,
        );
        if column < 4 {
            *total += (remainder >> (LIMB_BITS * column)) & 0xffff;
        }
        *carry = *total >> LIMB_BITS;
    }
    output[8] = *carry;
}

/// Fill every auxiliary cell of an active gate, returning its output word.
///
/// Public geometry and canonical inputs are checked before any write. The
/// parent writes the returned word to its unique output-bit wire and owns
/// clearing of the complete buffer on error/unwind. Padding is filled by the
/// parent with zeros. Primality and authoritative parameter admission belong
/// to the enclosing shared FHE plan, not this arithmetic helper.
pub(super) fn fill(
    kind: GateKind,
    modulus: u64,
    left: u64,
    right: Option<u64>,
    auxiliary: &mut [u64],
) -> Result<u64, IntegerError> {
    check_geometry(kind, modulus, right.is_some(), auxiliary.len())?;
    if kind != GateKind::Reduce && (left >= modulus || right.is_some_and(|v| v >= modulus)) {
        return Err(IntegerError::NonCanonical);
    }
    // Explicit native scratch owner: left, right, result, quotient and sum.
    let mut values = Zeroizing::new([left, right.unwrap_or(0), 0, 0, 0]);
    match kind {
        GateKind::Canonical => {
            fill_canonical(modulus, values[0], auxiliary);
            Ok(values[0])
        }
        GateKind::Reduce => {
            values[2] = values[0] % modulus;
            values[3] = values[0] / modulus;
            write_bits(values[3], &mut auxiliary[..64]);
            fill_canonical(modulus, values[2], &mut auxiliary[64..130]);
            fill_product(modulus, values[3], &mut auxiliary[130..420]);
            let product = Zeroizing::new(u128::from(modulus) * u128::from(values[3]));
            fill_sum_carries(*product, values[2], &mut auxiliary[420..429]);
            Ok(values[2])
        }
        GateKind::MulMod => {
            let product = Zeroizing::new(u128::from(values[0]) * u128::from(values[1]));
            values[2] = u64::try_from(*product % u128::from(modulus)).expect("canonical residue");
            values[3] = u64::try_from(*product / u128::from(modulus)).expect("quotient below q");
            write_bits(values[3], &mut auxiliary[..64]);
            for (index, &value) in values[..4].iter().enumerate() {
                fill_canonical(
                    modulus,
                    value,
                    &mut auxiliary[64 + index * 66..64 + (index + 1) * 66],
                );
            }
            fill_product(values[0], values[1], &mut auxiliary[328..618]);
            fill_product(modulus, values[3], &mut auxiliary[618..908]);
            let quotient_product = Zeroizing::new(u128::from(modulus) * u128::from(values[3]));
            fill_sum_carries(*quotient_product, values[2], &mut auxiliary[908..917]);
            Ok(values[2])
        }
        GateKind::AddMod | GateKind::SubMod => {
            if kind == GateKind::AddMod {
                values[4] = values[0] + values[1];
                values[3] = u64::from(values[4] >= modulus);
                values[2] = values[4] - values[3] * modulus;
            } else {
                values[3] = u64::from(values[0] < values[1]);
                values[4] = values[0] + values[3] * modulus;
                values[2] = values[4] - values[1];
            }
            write_bits(values[4], &mut auxiliary[..64]);
            auxiliary[64] = values[3];
            for (index, &value) in values[..3].iter().enumerate() {
                fill_canonical(
                    modulus,
                    value,
                    &mut auxiliary[65 + index * 66..65 + (index + 1) * 66],
                );
            }
            if kind == GateKind::AddMod {
                fill_add_carries(values[0], values[1], &mut auxiliary[263..265]);
                fill_add_carries(modulus * values[3], values[2], &mut auxiliary[265..267]);
            } else {
                fill_add_carries(values[0], modulus * values[3], &mut auxiliary[263..265]);
                fill_add_carries(values[1], values[2], &mut auxiliary[265..267]);
            }
            Ok(values[2])
        }
    }
}

struct Residues<'a, F> {
    output: &'a mut [F],
    next: usize,
}

impl<F: IntegerAirField> Residues<'_, F> {
    fn push(&mut self, value: F) {
        self.output[self.next] = value;
        self.next += 1;
    }

    fn bits(&mut self, active: F, bits: &[F]) {
        for &bit in bits {
            self.push(bit.mul(bit.sub(active)));
        }
    }
}

fn word<F>(bits: &[F]) -> &[F; WORD_BITS] {
    bits.try_into().expect("checked fixed word geometry")
}

fn pack<F: IntegerAirField>(bits: &[F]) -> F {
    bits.iter()
        .rev()
        .fold(F::ZERO, |value, &bit| value.add(value).add(bit))
}

fn public_word<F: IntegerAirField>(value: u64, selector: F) -> [F; WORD_BITS] {
    core::array::from_fn(|bit| {
        if (value >> bit) & 1 == 0 {
            F::ZERO
        } else {
            selector
        }
    })
}

fn emit_add<F: IntegerAirField>(
    active: F,
    left: &[F; WORD_BITS],
    right: &[F; WORD_BITS],
    output: &[F; WORD_BITS],
    carries: &[F],
    sink: &mut Residues<'_, F>,
) {
    let witness = Add64Witness {
        left: BitWord64 { bits: *left },
        right: BitWord64 { bits: *right },
        output: BitWord64 { bits: *output },
        carry_32: carries[0],
        carry_64: carries[1],
    };
    for residue in add_residues(active, &witness) {
        sink.push(residue);
    }
    sink.push(carries[1]);
}

fn emit_canonical<F: IntegerAirField>(
    active: F,
    modulus: u64,
    value: &[F; WORD_BITS],
    auxiliary: &[F],
    sink: &mut Residues<'_, F>,
) {
    let limit = public_word(modulus - 1, active);
    emit_add(
        active,
        value,
        word(&auxiliary[..64]),
        &limit,
        &auxiliary[64..],
        sink,
    );
}

fn emit_product<F: IntegerAirField>(
    active: F,
    left: &[F; WORD_BITS],
    right: &[F; WORD_BITS],
    auxiliary: &[F],
    sink: &mut Residues<'_, F>,
) {
    let product = &auxiliary[..PRODUCT_BITS];
    let carries = &auxiliary[PRODUCT_BITS..PRODUCT_AUX];
    sink.bits(active, left);
    sink.bits(active, right);
    sink.bits(active, product);
    sink.bits(active, carries);
    sink.push(active.mul(active.sub(F::ONE)));
    sink.push(pack(&carries[..CARRY_BITS]));
    sink.push(pack(&carries[8 * CARRY_BITS..]));
    let radix = F::from_u32(1 << LIMB_BITS);
    for column in 0..8 {
        let mut total = pack(&carries[column * CARRY_BITS..(column + 1) * CARRY_BITS]);
        for left_limb in 0..4 {
            if column >= left_limb && column - left_limb < 4 {
                let right_limb = column - left_limb;
                total = total.add(
                    pack(&left[left_limb * LIMB_BITS..(left_limb + 1) * LIMB_BITS]).mul(pack(
                        &right[right_limb * LIMB_BITS..(right_limb + 1) * LIMB_BITS],
                    )),
                );
            }
        }
        sink.push(
            total
                .sub(pack(&product[column * LIMB_BITS..(column + 1) * LIMB_BITS]))
                .sub(radix.mul(pack(
                    &carries[(column + 1) * CARRY_BITS..(column + 2) * CARRY_BITS],
                ))),
        );
    }
}

fn emit_sum<F: IntegerAirField>(
    active: F,
    product: &[F],
    remainder: &[F; WORD_BITS],
    output: &[F],
    carries: &[F],
    sink: &mut Residues<'_, F>,
) {
    sink.bits(active, product);
    sink.bits(active, remainder);
    sink.bits(active, output);
    sink.bits(active, carries);
    sink.push(active.mul(active.sub(F::ONE)));
    sink.push(carries[0]);
    sink.push(carries[8]);
    let radix = F::from_u32(1 << LIMB_BITS);
    for column in 0..8 {
        let start = column * LIMB_BITS;
        let end = start + LIMB_BITS;
        let addend = if column < 4 {
            pack(&remainder[start..end])
        } else {
            F::ZERO
        };
        let result = if end <= output.len() {
            pack(&output[start..end])
        } else {
            F::ZERO
        };
        sink.push(
            pack(&product[start..end])
                .add(addend)
                .add(carries[column])
                .sub(result)
                .sub(radix.mul(carries[column + 1])),
        );
    }
}

/// Emit the same quadratic numerators at base- or extension-field openings.
///
/// Only public modulus, gate kind and slice geometry are inspected. Every
/// private cell is used algebraically. Canonical checks read `left` only;
/// reduction requires `output`, and binary gates require `right` and `output`.
/// Each active-zero referenced word, auxiliary bit and carry is forced to zero.
pub(super) fn evaluate<F: IntegerAirField>(
    kind: GateKind,
    modulus: u64,
    active: F,
    views: WordViews<'_, F>,
    auxiliary: &[F],
    residues: &mut [F],
) -> Result<(), IntegerError> {
    check_geometry(kind, modulus, views.right.is_some(), auxiliary.len())?;
    if views.output.is_some() != (kind != GateKind::Canonical)
        || residues.len() != kind.layout().constraint_count
    {
        return Err(IntegerError::Layout);
    }
    let mut sink = Residues {
        output: residues,
        next: 0,
    };
    match kind {
        GateKind::Canonical => emit_canonical(active, modulus, views.left, auxiliary, &mut sink),
        GateKind::Reduce => {
            let output = views.output.expect("checked output geometry");
            let quotient = word(&auxiliary[..64]);
            let modulus_word = public_word(modulus, active);
            emit_canonical(active, modulus, output, &auxiliary[64..130], &mut sink);
            emit_product(
                active,
                &modulus_word,
                quotient,
                &auxiliary[130..420],
                &mut sink,
            );
            emit_sum(
                active,
                &auxiliary[130..258],
                output,
                views.left,
                &auxiliary[420..429],
                &mut sink,
            );
        }
        GateKind::MulMod => {
            let right = views.right.expect("checked right geometry");
            let output = views.output.expect("checked output geometry");
            let quotient = word(&auxiliary[..64]);
            for (index, value) in [views.left, right, output, quotient]
                .into_iter()
                .enumerate()
            {
                emit_canonical(
                    active,
                    modulus,
                    value,
                    &auxiliary[64 + index * 66..64 + (index + 1) * 66],
                    &mut sink,
                );
            }
            let modulus_word = public_word(modulus, active);
            emit_product(active, views.left, right, &auxiliary[328..618], &mut sink);
            emit_product(
                active,
                &modulus_word,
                quotient,
                &auxiliary[618..908],
                &mut sink,
            );
            emit_sum(
                active,
                &auxiliary[618..746],
                output,
                &auxiliary[328..456],
                &auxiliary[908..917],
                &mut sink,
            );
        }
        GateKind::AddMod | GateKind::SubMod => {
            let right = views.right.expect("checked right geometry");
            let output = views.output.expect("checked output geometry");
            let sum = word(&auxiliary[..64]);
            let quotient = auxiliary[64];
            let scaled_modulus = public_word(modulus, quotient);
            for (index, value) in [views.left, right, output].into_iter().enumerate() {
                emit_canonical(
                    active,
                    modulus,
                    value,
                    &auxiliary[65 + index * 66..65 + (index + 1) * 66],
                    &mut sink,
                );
            }
            if kind == GateKind::AddMod {
                emit_add(
                    active,
                    views.left,
                    right,
                    sum,
                    &auxiliary[263..265],
                    &mut sink,
                );
                emit_add(
                    active,
                    &scaled_modulus,
                    output,
                    sum,
                    &auxiliary[265..267],
                    &mut sink,
                );
            } else {
                emit_add(
                    active,
                    views.left,
                    &scaled_modulus,
                    sum,
                    &auxiliary[263..265],
                    &mut sink,
                );
                emit_add(active, right, output, sum, &auxiliary[265..267], &mut sink);
            }
            sink.push(quotient.mul(quotient.sub(active)));
        }
    }
    if sink.next != sink.output.len() {
        return Err(IntegerError::Layout);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GoldilocksFp4V1, field};

    const MODES: [GateKind; 5] = [
        GateKind::Canonical,
        GateKind::Reduce,
        GateKind::AddMod,
        GateKind::SubMod,
        GateKind::MulMod,
    ];
    const GOLDILOCKS: u64 = 0xffff_ffff_0000_0001;

    struct Case {
        kind: GateKind,
        modulus: u64,
        left: [u64; 64],
        right: Option<[u64; 64]>,
        output: Option<[u64; 64]>,
        auxiliary: Vec<u64>,
        result: u64,
    }

    impl Case {
        fn new(kind: GateKind, modulus: u64, left: u64, right: Option<u64>) -> Self {
            let mut auxiliary = vec![0; kind.layout().auxiliary_columns];
            let result = fill(kind, modulus, left, right, &mut auxiliary).unwrap();
            Self {
                kind,
                modulus,
                left: BitWord64::from_integer(left).bits,
                right: right.map(|value| BitWord64::from_integer(value).bits),
                output: (kind != GateKind::Canonical).then(|| BitWord64::from_integer(result).bits),
                auxiliary,
                result,
            }
        }

        fn residues(&self, active: u64) -> Vec<u64> {
            let mut residues = vec![0; self.kind.layout().constraint_count];
            evaluate(
                self.kind,
                self.modulus,
                active,
                WordViews {
                    left: &self.left,
                    right: self.right.as_ref(),
                    output: self.output.as_ref(),
                },
                &self.auxiliary,
                &mut residues,
            )
            .unwrap();
            residues
        }
    }

    fn all_zero(values: &[u64]) -> bool {
        values.iter().all(|&value| value == 0)
    }

    fn decode(bits: &[u64]) -> u64 {
        bits.iter()
            .enumerate()
            .fold(0, |word, (bit, &value)| word | (value << bit))
    }

    #[test]
    fn exact_geometry_and_exhaustive_small_modular_arithmetic() {
        for (kind, expected) in
            MODES
                .into_iter()
                .zip([(66, 198), (429, 903), (267, 991), (267, 991), (917, 1990)])
        {
            assert_eq!(
                (
                    kind.layout().auxiliary_columns,
                    kind.layout().constraint_count
                ),
                expected
            );
        }
        for modulus in [3_u64, 5, 17] {
            for left in 0..modulus {
                let canonical = Case::new(GateKind::Canonical, modulus, left, None);
                assert_eq!(canonical.result, left);
                assert!(all_zero(&canonical.residues(1)));
                for right in 0..modulus {
                    for kind in [GateKind::AddMod, GateKind::SubMod, GateKind::MulMod] {
                        let case = Case::new(kind, modulus, left, Some(right));
                        let expected = match kind {
                            GateKind::AddMod => (left + right) % modulus,
                            GateKind::SubMod => (left + modulus - right) % modulus,
                            GateKind::MulMod => left * right % modulus,
                            _ => unreachable!("binary test modes"),
                        };
                        assert_eq!(case.result, expected);
                        assert!(
                            all_zero(&case.residues(1)),
                            "{kind:?} q={modulus} a={left} b={right}"
                        );
                    }
                }
            }
            for input in 0..=(4 * modulus + 3) {
                let case = Case::new(GateKind::Reduce, modulus, input, None);
                assert_eq!(case.result, input % modulus);
                assert!(all_zero(&case.residues(1)));
            }
        }
    }

    #[test]
    fn full_u64_reduction_and_sixty_two_bit_products_are_exact() {
        for modulus in [3_u64, 97, (1_u64 << 61) - 1, (1_u64 << 62) - 57] {
            for input in [0, modulus - 1, modulus, GOLDILOCKS, u64::MAX] {
                let case = Case::new(GateKind::Reduce, modulus, input, None);
                assert_eq!(case.result, input % modulus);
                assert_eq!(decode(&case.auxiliary[..64]), input / modulus);
                assert!(all_zero(&case.residues(1)));
            }
            for (left, right) in [
                (0, modulus - 1),
                (modulus - 1, modulus - 1),
                (modulus - 2, modulus - 3),
            ] {
                for kind in [GateKind::AddMod, GateKind::SubMod, GateKind::MulMod] {
                    let case = Case::new(kind, modulus, left, Some(right));
                    assert!(all_zero(&case.residues(1)));
                    let expected = match kind {
                        GateKind::AddMod => {
                            (u128::from(left) + u128::from(right)) % u128::from(modulus)
                        }
                        GateKind::SubMod => {
                            (u128::from(left) + u128::from(modulus) - u128::from(right))
                                % u128::from(modulus)
                        }
                        GateKind::MulMod => {
                            u128::from(left) * u128::from(right) % u128::from(modulus)
                        }
                        _ => unreachable!("binary test modes"),
                    };
                    assert_eq!(u128::from(case.result), expected);
                }
            }
        }
        let case = Case::new(GateKind::Reduce, 3, u64::MAX, None);
        assert_eq!(decode(&case.auxiliary[..64]), 6_148_914_691_236_517_205);
        assert!(decode(&case.auxiliary[..64]) > 3);
        // Cross-modulus digits need reduction, not a false canonical assumption.
        assert_eq!(Case::new(GateKind::Reduce, 3, 96, None).result, 0);
    }

    #[test]
    fn every_auxiliary_word_bit_and_carry_is_polynomially_constrained() {
        for kind in MODES {
            let left = if kind == GateKind::Reduce {
                u64::MAX
            } else {
                89
            };
            let mut case = Case::new(kind, 97, left, kind.binary().then_some(91));
            assert!(all_zero(&case.residues(1)));
            for column in 0..case.auxiliary.len() {
                case.auxiliary[column] ^= 1;
                assert!(!all_zero(&case.residues(1)), "{kind:?} auxiliary {column}");
                case.auxiliary[column] ^= 1;
                let saved = case.auxiliary[column];
                case.auxiliary[column] = 2;
                assert!(
                    !all_zero(&case.residues(1)),
                    "{kind:?} nonboolean auxiliary {column}"
                );
                case.auxiliary[column] = saved;
            }
            for bit in 0..64 {
                case.left[bit] ^= 1;
                assert!(!all_zero(&case.residues(1)), "{kind:?} left {bit}");
                case.left[bit] ^= 1;
                if let Some(word) = &mut case.right {
                    word[bit] ^= 1;
                    assert!(!all_zero(&case.residues(1)), "{kind:?} right {bit}");
                    if let Some(word) = &mut case.right {
                        word[bit] ^= 1;
                    }
                }
                if let Some(word) = &mut case.output {
                    word[bit] ^= 1;
                    assert!(!all_zero(&case.residues(1)), "{kind:?} output {bit}");
                    if let Some(word) = &mut case.output {
                        word[bit] ^= 1;
                    }
                }
            }
            assert!(!all_zero(&case.residues(2)), "active must be Boolean");
        }
    }

    #[test]
    fn inactive_rows_force_all_owned_and_referenced_bits_to_zero() {
        for kind in MODES {
            let mut case = Case::new(kind, 97, 0, kind.binary().then_some(0));
            case.auxiliary.fill(0);
            assert!(all_zero(&case.residues(0)));
            for column in 0..case.auxiliary.len() {
                case.auxiliary[column] = 1;
                assert!(
                    !all_zero(&case.residues(0)),
                    "{kind:?} inactive auxiliary {column}"
                );
                case.auxiliary[column] = 0;
            }
            for bit in 0..64 {
                case.left[bit] = 1;
                assert!(!all_zero(&case.residues(0)));
                case.left[bit] = 0;
                if let Some(word) = &mut case.right {
                    word[bit] = 1;
                    assert!(!all_zero(&case.residues(0)));
                    if let Some(word) = &mut case.right {
                        word[bit] = 0;
                    }
                }
                if let Some(word) = &mut case.output {
                    word[bit] = 1;
                    assert!(!all_zero(&case.residues(0)));
                    if let Some(word) = &mut case.output {
                        word[bit] = 0;
                    }
                }
            }
        }
    }

    #[test]
    fn goldilocks_alias_and_noncanonical_residue_alias_are_rejected() {
        let modulus = (1_u64 << 61) - 1;
        let left = modulus - 2;
        let right = modulus - 3;
        let mut case = Case::new(GateKind::MulMod, modulus, left, Some(right));
        let product = u128::from(left) * u128::from(right);
        let forged = product + u128::from(GOLDILOCKS);
        assert_eq!(
            product % u128::from(GOLDILOCKS),
            forged % u128::from(GOLDILOCKS)
        );
        for (bit, value) in case.auxiliary[328..456].iter_mut().enumerate() {
            *value = u64::try_from((forged >> bit) & 1).unwrap();
        }
        assert!(!all_zero(&case.residues(1)));
        let mut alias = Case::new(GateKind::Canonical, 97, 1, None);
        alias.left = BitWord64::from_integer(98).bits;
        assert!(!all_zero(&alias.residues(1)));
    }

    #[test]
    fn explicit_top_carry_overflow_quotient_and_wrapping_slack_forgeries_fail() {
        let mut product = Case::new(GateKind::MulMod, 97, 89, Some(91));
        // Both product top carries and the final exact-sum carry must be zero.
        for column in [328 + 128 + 8 * 18, 618 + 128 + 8 * 18, 916] {
            product.auxiliary[column] = 1;
            assert!(!all_zero(&product.residues(1)));
            product.auxiliary[column] = 0;
        }
        // Encoding 2 in bit17 attempts an integer carry of2^18, outside its range.
        let start = 328 + 128 + 18;
        let saved = product.auxiliary[start..start + 18].to_vec();
        product.auxiliary[start..start + 18].fill(0);
        product.auxiliary[start + 17] = 2;
        assert_eq!(decode(&product.auxiliary[start..start + 18]), 1 << 18);
        assert!(!all_zero(&product.residues(1)));
        product.auxiliary[start..start + 18].copy_from_slice(&saved);
        write_bits(97, &mut product.auxiliary[..64]);
        assert!(
            !all_zero(&product.residues(1)),
            "modular product quotient must be below q"
        );

        // This forged canonical check satisfies EVERY wrapping Add64 equation:
        // 98 + (2^64-2) = 96 mod2^64. Only the extra final-carry-zero relation
        // rules out the noncanonical input98 for modulus97.
        let mut canonical = Case::new(GateKind::Canonical, 97, 1, None);
        canonical.left = BitWord64::from_integer(98).bits;
        write_bits(u64::MAX - 1, &mut canonical.auxiliary[..64]);
        fill_add_carries(98, u64::MAX - 1, &mut canonical.auxiliary[64..66]);
        let residues = canonical.residues(1);
        assert!(all_zero(&residues[..197]));
        assert_eq!(residues[197], 1);
    }

    #[test]
    fn base_extension_agreement_and_quadratic_degree_include_activation() {
        for kind in MODES {
            let case = Case::new(kind, 97, 91, kind.binary().then_some(89));
            let mut values = Vec::new();
            for parameter in 0..4_u64 {
                let shift = |value: u64| field::add_base(value, parameter);
                let left = case.left.map(shift);
                let right = case.right.map(|bits| bits.map(shift));
                let output = case.output.map(|bits| bits.map(shift));
                let auxiliary: Vec<_> = case.auxiliary.iter().copied().map(shift).collect();
                let mut residues = vec![0; kind.layout().constraint_count];
                evaluate(
                    kind,
                    97,
                    parameter,
                    WordViews {
                        left: &left,
                        right: right.as_ref(),
                        output: output.as_ref(),
                    },
                    &auxiliary,
                    &mut residues,
                )
                .unwrap();
                let embed = |value| GoldilocksFp4V1::from_base(value).unwrap();
                let extension_left = left.map(embed);
                let extension_right = right.map(|bits| bits.map(embed));
                let extension_output = output.map(|bits| bits.map(embed));
                let extension_auxiliary: Vec<_> = auxiliary.iter().copied().map(embed).collect();
                let mut extension_residues =
                    vec![GoldilocksFp4V1::ZERO; kind.layout().constraint_count];
                evaluate(
                    kind,
                    97,
                    embed(parameter),
                    WordViews {
                        left: &extension_left,
                        right: extension_right.as_ref(),
                        output: extension_output.as_ref(),
                    },
                    &extension_auxiliary,
                    &mut extension_residues,
                )
                .unwrap();
                assert_eq!(
                    residues.iter().copied().map(embed).collect::<Vec<_>>(),
                    extension_residues
                );
                values.push(residues);
            }
            for (column, &zero_value) in values[0][..kind.layout().constraint_count]
                .iter()
                .enumerate()
            {
                let third_difference = field::sub_base(
                    field::add_base(values[3][column], field::mul_base(3, values[1][column])),
                    field::add_base(field::mul_base(3, values[2][column]), zero_value),
                );
                assert_eq!(third_difference, 0, "{kind:?} cubic numerator {column}");
            }
        }
    }

    #[test]
    fn public_geometry_and_native_domain_refuse_before_writes() {
        for kind in MODES {
            let mut auxiliary = vec![7; kind.layout().auxiliary_columns];
            for modulus in [0, 1, 1_u64 << 62, u64::MAX] {
                assert_eq!(
                    fill(kind, modulus, 0, kind.binary().then_some(0), &mut auxiliary),
                    Err(IntegerError::Modulus)
                );
                assert!(auxiliary.iter().all(|&value| value == 7));
            }
            assert_eq!(
                fill(kind, 97, 0, (!kind.binary()).then_some(0), &mut auxiliary),
                Err(IntegerError::Layout)
            );
            assert!(auxiliary.iter().all(|&value| value == 7));
            if kind != GateKind::Reduce {
                assert_eq!(
                    fill(kind, 97, 97, kind.binary().then_some(0), &mut auxiliary),
                    Err(IntegerError::NonCanonical)
                );
                assert!(auxiliary.iter().all(|&value| value == 7));
            }
            auxiliary.pop();
            assert_eq!(
                fill(kind, 97, 0, kind.binary().then_some(0), &mut auxiliary),
                Err(IntegerError::Layout)
            );
            let bits = [0_u64; 64];
            let mut residues = vec![7; kind.layout().constraint_count];
            assert_eq!(
                evaluate(
                    kind,
                    97,
                    1,
                    WordViews {
                        left: &bits,
                        right: kind.binary().then_some(&bits),
                        output: (kind != GateKind::Canonical).then_some(&bits)
                    },
                    &auxiliary,
                    &mut residues
                ),
                Err(IntegerError::Layout)
            );
            assert!(residues.iter().all(|&value| value == 7));
        }
        assert_eq!(
            IntegerError::Modulus.to_string(),
            "RNS integer gate modulus outside [2, 2^62)"
        );
        assert_eq!(
            IntegerError::Layout.to_string(),
            "RNS integer gate layout mismatch"
        );
        assert_eq!(
            IntegerError::NonCanonical.to_string(),
            "RNS integer gate input is not canonical"
        );
    }

    fn error_after_addition_scratch() -> Result<(), ()> {
        let _guard = ClearingAdd(Add64Witness::from_operands(u64::MAX, 17));
        Err(())
    }

    #[test]
    fn concrete_addition_scratch_clears_on_success_error_and_unwind() {
        assert_eq!(NATIVE_SCRATCH_BYTES, 1_640);
        let before = CLEAR_EVENTS.with(core::cell::Cell::get);
        {
            let _guard = ClearingAdd(Add64Witness::from_operands(u64::MAX, 1));
        }
        assert!(error_after_addition_scratch().is_err());
        assert!(
            std::panic::catch_unwind(|| {
                let _guard = ClearingAdd(Add64Witness::from_operands(u64::MAX, u64::MAX));
                panic!("test unwind after concrete addition scratch");
            })
            .is_err()
        );
        let after = CLEAR_EVENTS.with(core::cell::Cell::get);
        assert_eq!(after, (before.0 + 3, before.1 + 3));
    }
}
