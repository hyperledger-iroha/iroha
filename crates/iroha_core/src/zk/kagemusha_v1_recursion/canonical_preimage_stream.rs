//! Fixed-topology concatenation of constrained, zero-padded byte prefixes.
//!
//! Capacity is a circuit-construction parameter, never a witness-derived shape or a new
//! protocol limit. Callers must select it from the existing envelope budget. This primitive
//! proves byte order, exact length and zero padding only: canonical codec structure and
//! authenticated semantic fields must still be established by the enclosing relation.
//!
//! TODO: integrate with the canonical variable-field context/statement/envelope assemblers;
//! the byte-stream relation alone grants no MintFold or recipient authority.

use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};

use crate::zk::{kagemusha_v1_poseidon::KagemushaPoseidonFieldV1, pasta_sha256::PastaSha256ByteV1};

/// An exact active byte prefix with a circuit-constrained length and zero tail.
///
/// Byte provenance and capacity must be identical during key generation and proving. In
/// particular a variable-length input must assign its entire fixed-capacity buffer; replacing
/// its tail with constants according to the witness length would change the circuit shape.
/// This type does not attest canonical encoding or authenticate the bytes it carries.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaBoundedByteStreamV1<F: KagemushaPoseidonFieldV1> {
    bytes: Vec<PastaSha256ByteV1<F>>,
    actual_len: AssignedValue<F>,
}

impl<F: KagemushaPoseidonFieldV1> KagemushaBoundedByteStreamV1<F> {
    /// Constrain `actual_len` to the buffer capacity and every byte outside its prefix to zero.
    pub(crate) fn constrain(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        bytes: Vec<PastaSha256ByteV1<F>>,
        actual_len: AssignedValue<F>,
    ) -> Result<Self, String> {
        if bytes
            .iter()
            .any(|byte| byte.assigned().is_some_and(|value| value.cell.is_none()))
        {
            return Err("bounded byte-stream source has no virtual-cell identity".to_owned());
        }
        let length_bits = constrain_stream_length_v1(ctx, range, bytes.len(), actual_len)?;
        let gate = range.gate();
        for (index, byte) in bytes.iter().enumerate() {
            let active = range.is_less_than(
                ctx,
                QuantumCell::Constant(F::from(index as u64)),
                actual_len,
                length_bits,
            );
            let tail = gate.mul_not(ctx, active, byte.quantum_cell());
            gate.assert_is_const(ctx, &tail, &F::ZERO);
        }
        Ok(Self { bytes, actual_len })
    }

    /// Borrow the fixed-capacity buffer, including its proven zero tail.
    pub(crate) fn bytes(&self) -> &[PastaSha256ByteV1<F>] {
        &self.bytes
    }

    /// Return the exact constrained active length for framing, CRC or bounded hashing.
    pub(crate) fn actual_len(&self) -> AssignedValue<F> {
        self.actual_len
    }

    /// Concatenate two active prefixes without using their witness lengths as host indices.
    ///
    /// A zero-fill barrel shifter moves `other` right by the proven length of `self`. Each
    /// stage uses one Boolean length bit and a fixed power-of-two offset. Adding the two
    /// disjoint prefixes then gives the exact concatenation. The total length is bounded
    /// before any truncation, so bytes discarded beyond `output_capacity` are provably zero.
    /// The returned tail is zero by construction, not by a host assumption.
    ///
    /// The number of routing gates is `O(output_capacity * log(self.capacity + 1))` and
    /// depends only on fixed capacities and byte provenance, never on lengths or contents.
    /// No digest, field encoding, length prefix or transport limit is changed here.
    pub(crate) fn concat(
        &self,
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        other: &Self,
        output_capacity: usize,
    ) -> Result<Self, String> {
        let gate = range.gate();
        let actual_len = gate.add(ctx, self.actual_len, other.actual_len);
        constrain_stream_length_v1(ctx, range, output_capacity, actual_len)?;

        let left_capacity = u64::try_from(self.bytes.len())
            .map_err(|_| "bounded byte-stream capacity exceeds u64".to_owned())?;
        let length_bits = (u64::BITS - left_capacity.leading_zeros()) as usize;
        let offset_bits = if length_bits == 0 {
            Vec::new()
        } else {
            gate.num_to_bits(ctx, self.actual_len, length_bits)
        };
        let zero = QuantumCell::Constant(F::ZERO);
        let mut shifted = (0..output_capacity)
            .map(|index| {
                other
                    .bytes
                    .get(index)
                    .map_or(zero, |byte| byte.quantum_cell())
            })
            .collect::<Vec<_>>();
        for (bit_index, selected) in offset_bits.into_iter().enumerate() {
            // Every bit index is smaller than usize::BITS: the source capacity is a usize.
            let offset = 1_usize << bit_index;
            shifted = (0..output_capacity)
                .map(|index| {
                    let moved = index
                        .checked_sub(offset)
                        .map_or(zero, |source| shifted[source]);
                    QuantumCell::Existing(gate.select(ctx, moved, shifted[index], selected))
                })
                .collect();
        }
        let bytes = shifted
            .into_iter()
            .enumerate()
            .map(|(index, right)| {
                let left = self
                    .bytes
                    .get(index)
                    .map_or(zero, |byte| byte.quantum_cell());
                let byte = gate.add(ctx, left, right);
                PastaSha256ByteV1::range_checked(ctx, range, byte)
            })
            .collect();
        Ok(Self { bytes, actual_len })
    }
}

/// Prove an ordinary integer length in `0..=capacity` without a field-wraparound escape.
fn constrain_stream_length_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    capacity: usize,
    actual_len: AssignedValue<F>,
) -> Result<usize, String> {
    if actual_len.cell.is_none() {
        return Err("bounded byte-stream length has no virtual-cell identity".to_owned());
    }
    let capacity = u64::try_from(capacity)
        .map_err(|_| "bounded byte-stream capacity exceeds u64".to_owned())?;
    let gate = range.gate();
    if capacity == 0 {
        gate.assert_is_const(ctx, &actual_len, &F::ZERO);
        return Ok(0);
    }
    let length_bits = (u64::BITS - capacity.leading_zeros()) as usize;
    // Each nonnegative summand has at most 64 bits. Neither it nor their sum can wrap a
    // Pasta field, so the two range checks prove the exact inclusive upper bound.
    range.range_check(ctx, actual_len, length_bits);
    let padding_len = gate.sub(ctx, QuantumCell::Constant(F::from(capacity)), actual_len);
    range.range_check(ctx, padding_len, length_bits);
    Ok(length_bits)
}

#[cfg(test)]
#[path = "canonical_preimage_stream_tests.rs"]
mod tests;
