//! Exact fixed-layout Norito preimages, including their constrained CRC64-XZ.
//!
//! A framing template comes from the authoritative model, never witness bytes. Every variable
//! payload byte must be supplied by an already constrained semantic field. In particular fresh
//! commitments cannot borrow the authenticated-ID exception used by historical credential
//! openings: their checksum is computed here, not accepted as an unconstrained witness.

use core::ops::Range;

use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
    utils::fe_to_biguint,
};
use iroha_data_model::kagemusha::KagemushaCanonicalFramePrefixV1;

use crate::zk::{kagemusha_v1_poseidon::KagemushaPoseidonFieldV1, pasta_sha256::PastaSha256ByteV1};

#[path = "canonical_preimage_stream.rs"]
pub(super) mod stream;

use self::stream::KagemushaBoundedByteStreamV1;

const CHECKSUM_RANGE: Range<usize> = 31..39;
const PAYLOAD_LENGTH_RANGE: Range<usize> = 23..31;
const HEADER_BYTES: usize = 40;
// Bit-reflected ECMA polynomial used by Norito's CRC64-XZ implementation.
const REFLECTED_CRC64_POLYNOMIAL: u64 = 0xC96C_5795_D787_0F42;

/// Assemble the exact model frame from semantic bytes and compute its canonical checksum.
///
/// The layout must pin header, padding and field prefixes. Each remaining byte must be covered
/// exactly once by a semantic field or by the checksum; holes, overlap and width drift fail closed.
pub(super) fn assemble_canonical_preimage_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    layout: &[Option<u8>],
    field_ranges: &[Range<usize>],
    fields: &[&[PastaSha256ByteV1<F>]],
) -> Result<Vec<PastaSha256ByteV1<F>>, String> {
    if layout.len() < HEADER_BYTES || field_ranges.len() != fields.len() {
        return Err("canonical preimage frame/field inventory mismatch".to_owned());
    }
    let length_bytes = layout[PAYLOAD_LENGTH_RANGE]
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "canonical preimage payload length is not fixed".to_owned())?;
    let payload_len = usize::try_from(u64::from_le_bytes(
        length_bytes.try_into().expect("eight-byte payload length"),
    ))
    .map_err(|_| "canonical preimage payload length exceeds usize".to_owned())?;
    let payload_start = layout
        .len()
        .checked_sub(payload_len)
        .filter(|start| *start >= HEADER_BYTES)
        .ok_or_else(|| "canonical preimage payload length exceeds frame".to_owned())?;
    if layout[CHECKSUM_RANGE].iter().any(Option::is_some) {
        return Err("canonical preimage checksum must be computed".to_owned());
    }
    let mut bytes = layout
        .iter()
        .map(|value| value.map(PastaSha256ByteV1::constant))
        .collect::<Vec<_>>();
    for (field_range, field) in field_ranges.iter().zip(fields) {
        if field_range.start > field_range.end
            || field_range.start < payload_start
            || field_range.end > bytes.len()
            || field_range.len() != field.len()
        {
            return Err("canonical preimage semantic field width/offset mismatch".to_owned());
        }
        for (slot, value) in bytes[field_range.clone()].iter_mut().zip(*field) {
            if slot.replace(*value).is_some() {
                return Err("canonical preimage semantic fields overlap framing".to_owned());
            }
        }
    }
    if bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| byte.is_none() && !CHECKSUM_RANGE.contains(&index))
    {
        return Err("canonical preimage has an unbound byte".to_owned());
    }
    let payload = bytes[payload_start..]
        .iter()
        .map(|byte| byte.expect("checked complete payload"))
        .collect::<Vec<_>>();
    let checksum = crc64_xz_bytes_v1(ctx, range, &payload)?;
    for (slot, value) in bytes[CHECKSUM_RANGE].iter_mut().zip(checksum) {
        *slot = Some(value);
    }
    Ok(bytes
        .into_iter()
        .map(|byte| byte.expect("checked complete canonical frame"))
        .collect())
}

/// Assemble one bounded canonical Norito frame around an active payload prefix.
///
/// `framing` is the model-owned fixed header followed by any type-specific alignment padding. Its
/// only holes are the header's eight-byte payload length and eight-byte checksum.
/// The constrained header length excludes alignment padding, exactly like Norito's encoder, and
/// the checksum covers only the active payload. The returned stream has capacity
/// `framing_template.len() + payload.capacity()` and an active length equal to the fixed framing
/// prefix plus the constrained payload length.
///
/// Template size and payload capacity are circuit-construction parameters. No host branch or
/// index depends on the witnessed payload length.
pub(super) fn assemble_bounded_canonical_frame_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    framing: &KagemushaCanonicalFramePrefixV1,
    payload: &KagemushaBoundedByteStreamV1<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    assemble_bounded_canonical_frame_template_v1(ctx, range, framing.bytes(), payload)
}

/// Raw-template implementation kept private so monetary callers cannot bypass the model-owned
/// framing descriptor.
fn assemble_bounded_canonical_frame_template_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    framing_template: &[Option<u8>],
    payload: &KagemushaBoundedByteStreamV1<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if framing_template.len() < HEADER_BYTES {
        return Err("bounded canonical frame prefix is shorter than the Norito header".to_owned());
    }
    for (index, byte) in framing_template.iter().enumerate() {
        let is_derived_header_byte =
            PAYLOAD_LENGTH_RANGE.contains(&index) || CHECKSUM_RANGE.contains(&index);
        if byte.is_none() != is_derived_header_byte {
            return Err(
                "bounded canonical frame template has a missing or prefilled derived byte"
                    .to_owned(),
            );
        }
    }
    let required_header_bytes = [
        (0, norito::core::MAGIC[0]),
        (1, norito::core::MAGIC[1]),
        (2, norito::core::MAGIC[2]),
        (3, norito::core::MAGIC[3]),
        (4, norito::core::VERSION_MAJOR),
        (5, norito::core::VERSION_MINOR),
        (22, norito::core::Compression::None as u8),
    ];
    if required_header_bytes
        .into_iter()
        .any(|(index, expected)| framing_template[index] != Some(expected))
    {
        return Err("bounded canonical frame template has a noncanonical header".to_owned());
    }
    let flags = framing_template[HEADER_BYTES - 1].expect("validated fixed flags byte");
    norito::core::validate_header_flags(flags)
        .map_err(|_| "bounded canonical frame template has unsupported flags".to_owned())?;
    if framing_template[HEADER_BYTES..]
        .iter()
        .any(|byte| *byte != Some(0))
    {
        return Err("bounded canonical frame alignment prefix is not zero".to_owned());
    }
    let prefix_len = u64::try_from(framing_template.len())
        .map_err(|_| "bounded canonical frame prefix exceeds u64".to_owned())?;
    let output_capacity = framing_template
        .len()
        .checked_add(payload.bytes().len())
        .ok_or_else(|| "bounded canonical frame capacity overflows usize".to_owned())?;
    u64::try_from(output_capacity)
        .map_err(|_| "bounded canonical frame capacity exceeds u64".to_owned())?;

    let gate = range.gate();
    // The bounded stream has already proved this cell is an integer in 0..=capacity. A full
    // 64-bit decomposition derives the wire's fixed-width little-endian header without relying
    // on the host representation of the witness.
    let payload_length_bits = gate.num_to_bits(ctx, payload.actual_len(), 64);
    let payload_length_bytes: [PastaSha256ByteV1<F>; 8] = core::array::from_fn(|byte| {
        let value = gate.inner_product(
            ctx,
            payload_length_bits[byte * 8..byte * 8 + 8].iter().copied(),
            (0..8).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
        );
        PastaSha256ByteV1::range_checked(ctx, range, value)
    });
    let checksum = crc64_xz_prefix_bytes_v1(ctx, range, payload.bytes(), payload.actual_len())?;
    let prefix_bytes = framing_template
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if PAYLOAD_LENGTH_RANGE.contains(&index) {
                payload_length_bytes[index - PAYLOAD_LENGTH_RANGE.start]
            } else if CHECKSUM_RANGE.contains(&index) {
                checksum[index - CHECKSUM_RANGE.start]
            } else {
                PastaSha256ByteV1::constant(byte.expect("validated fixed framing byte"))
            }
        })
        .collect();
    let prefix_actual_len = ctx.load_constant(F::from(prefix_len));
    let prefix =
        KagemushaBoundedByteStreamV1::constrain(ctx, range, prefix_bytes, prefix_actual_len)?;
    prefix.concat(ctx, range, payload, output_capacity)
}

/// Encode a V1 canonical compact length in a fixed two-byte stream.
///
/// This helper covers the complete one- and two-byte range `0..=16_383`; that range is an
/// encoding primitive bound, not a new protocol or account limit. Values below 128 produce one
/// active byte and a proven-zero tail. Larger values produce both bytes. The active width is
/// derived in-circuit, so values 127 and 128 retain identical topology while using their unique
/// minimal encodings. The caller must equality-bind `length` to the exact byte stream or field
/// whose prefix this represents; this primitive proves the encoding, not that external binding.
pub(super) fn canonical_compact_length_u14_stream_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    length: AssignedValue<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    if length.cell.is_none() {
        return Err("canonical compact length has no virtual-cell identity".to_owned());
    }
    let gate = range.gate();
    // Reconstruction from fourteen Boolean cells proves the exact non-wrapping u14 range.
    let bits = gate.num_to_bits(ctx, length, 14);
    let low = gate.inner_product(
        ctx,
        bits[..7].iter().copied(),
        (0..7).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
    );
    let high = gate.inner_product(
        ctx,
        bits[7..].iter().copied(),
        (0..7).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
    );
    let high_is_zero = gate.is_zero(ctx, high);
    let has_second_byte = gate.not(ctx, high_is_zero);
    let continuation = gate.mul(ctx, has_second_byte, QuantumCell::Constant(F::from(0x80)));
    let first = gate.add(ctx, low, continuation);
    let actual_len = gate.add(ctx, QuantumCell::Constant(F::ONE), has_second_byte);
    let bytes = vec![
        PastaSha256ByteV1::range_checked(ctx, range, first),
        PastaSha256ByteV1::range_checked(ctx, range, high),
    ];
    KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, actual_len)
}

/// Compute CRC64-XZ as its affine map over Boolean-proven input bits.
///
/// Instead of a serial XOR gate for every polynomial tap, each output bit is the parity of a
/// bounded integer sum. Boolean decomposition proves that parity without field wraparound. The
/// matrix depends only on length and byte provenance, not values. Norito's accelerated entry
/// point (with its portable fallback) supplies the affine constant: fixed bytes, all-ones
/// initialization and final complement. No platform-specific instruction changes constraints;
/// tests compare this path against the portable reference.
fn crc64_xz_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    payload: &[PastaSha256ByteV1<F>],
) -> Result<[PastaSha256ByteV1<F>; 8], String> {
    let gate = range.gate();
    let mut affine_payload = Vec::with_capacity(payload.len());
    let mut input_bits = Vec::with_capacity(payload.len());
    for byte in payload {
        if let Some(assigned) = byte.assigned() {
            affine_payload.push(0);
            input_bits.push(Some(gate.num_to_bits(ctx, assigned, 8)));
        } else if let QuantumCell::Constant(value) = byte.quantum_cell() {
            affine_payload.push(
                u8::try_from(fe_to_biguint(&value))
                    .map_err(|_| "canonical preimage constant is not a byte".to_owned())?,
            );
            input_bits.push(None);
        } else {
            return Err("canonical preimage byte has unknown provenance".to_owned());
        }
    }
    let affine = norito::hardware_crc64(&affine_payload);
    let mut terms: [Vec<AssignedValue<F>>; 64] = core::array::from_fn(|_| Vec::new());
    let mut effect = REFLECTED_CRC64_POLYNOMIAL;
    for byte in input_bits.iter().rev() {
        for bit in (0..8).rev() {
            if let Some(bits) = byte {
                for (output_bit, column) in terms.iter_mut().enumerate() {
                    if (effect >> output_bit) & 1 != 0 {
                        column.push(bits[bit]);
                    }
                }
            }
            effect = (effect >> 1) ^ (REFLECTED_CRC64_POLYNOMIAL.wrapping_mul(effect & 1));
        }
    }
    let output_bits: [AssignedValue<F>; 64] = core::array::from_fn(|bit| {
        let affine_bit = (affine >> bit) & 1;
        if terms[bit].is_empty() {
            return ctx.load_constant(F::from(affine_bit));
        }
        // A Vec cannot contain remotely enough Boolean inputs to wrap a Pasta field. The exact
        // sum fits this usize-width bit decomposition, independently of witness values.
        let bits = usize::BITS as usize - terms[bit].len().leading_zeros() as usize;
        let sum = gate.sum(ctx, terms[bit].iter().copied());
        let parity = gate.num_to_bits(ctx, sum, bits)[0];
        if affine_bit == 0 {
            parity
        } else {
            gate.sub(ctx, QuantumCell::Constant(F::ONE), parity)
        }
    });
    Ok(core::array::from_fn(|byte| {
        let value = gate.inner_product(
            ctx,
            output_bits[byte * 8..byte * 8 + 8].iter().copied(),
            (0..8).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
        );
        PastaSha256ByteV1::range_checked(ctx, range, value)
    }))
}

/// Compute the exact active-prefix CRC64-XZ from a fixed-capacity, zero-padded payload.
///
/// `actual_len` is constrained to `0..=payload.len()`, and every byte at or after that length
/// is constrained to zero. Capacity, byte provenance, and all constant bytes must be pinned
/// between key generation and proving; neither the active length nor assigned byte values
/// affect the circuit topology. The eight returned typed bytes are the little-endian checksum,
/// not a supplied checksum witness. The caller must bind `actual_len` to its authenticated length.
///
/// Let `Z` be the linear CRC transition for one zero byte. Before the final complement, the
/// padded state is `Z^(N - L)` applied to the active-prefix state. We compute the fixed-length
/// checksum, undo its complement, apply constant binary powers of `Z^-1` selected by proven
/// padding-length bits, then restore the complement. All-ones initialization is already present
/// in the fixed-length checksum, including when the active prefix is empty.
pub(super) fn crc64_xz_prefix_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    payload: &[PastaSha256ByteV1<F>],
    actual_len: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 8], String> {
    let capacity = u64::try_from(payload.len())
        .map_err(|_| "canonical prefix CRC capacity exceeds u64".to_owned())?;
    let gate = range.gate();
    if capacity == 0 {
        gate.assert_is_const(ctx, &actual_len, &F::ZERO);
        return crc64_xz_bytes_v1(ctx, range, payload);
    }
    let length_bits = (u64::BITS - capacity.leading_zeros()) as usize;
    // Both nonnegative integers fit at most 64 bits, so their sum cannot wrap either Pasta
    // field. Constraining L and N - L to this width therefore proves 0 <= L <= N exactly.
    range.range_check(ctx, actual_len, length_bits);
    let padding_len = gate.sub(ctx, QuantumCell::Constant(F::from(capacity)), actual_len);
    let padding_bits = gate.num_to_bits(ctx, padding_len, length_bits);
    for (index, byte) in payload.iter().enumerate() {
        // Capacity fits u64, so every fixed index does too. The comparison inputs have the
        // proven width above; no host-side indexing or branching depends on the length value.
        let active = range.is_less_than(
            ctx,
            QuantumCell::Constant(F::from(index as u64)),
            actual_len,
            length_bits,
        );
        let tail_byte = gate.mul_not(ctx, active, byte.quantum_cell());
        gate.assert_is_const(ctx, &tail_byte, &F::ZERO);
    }

    let checksum = crc64_xz_bytes_v1(ctx, range, payload)?;
    let mut state_bits = Vec::with_capacity(64);
    for byte in checksum {
        let assigned = byte
            .assigned()
            .expect("the fixed CRC always returns constrained checksum bytes");
        for bit in gate.num_to_bits(ctx, assigned, 8) {
            state_bits.push(gate.sub(ctx, QuantumCell::Constant(F::ONE), bit));
        }
    }
    let mut state: [AssignedValue<F>; 64] = state_bits
        .try_into()
        .expect("eight little-endian checksum bytes contain 64 bits");
    let mut inverse_power = crc64_inverse_zero_byte_matrix_v1();
    for selected in padding_bits {
        let transformed = crc64_apply_matrix_assigned_v1(ctx, range, &inverse_power, &state);
        state =
            core::array::from_fn(|bit| gate.select(ctx, transformed[bit], state[bit], selected));
        inverse_power = crc64_square_matrix_v1(&inverse_power);
    }
    let output_bits = state.map(|bit| gate.sub(ctx, QuantumCell::Constant(F::ONE), bit));
    Ok(core::array::from_fn(|byte| {
        let value = gate.inner_product(
            ctx,
            output_bits[byte * 8..byte * 8 + 8].iter().copied(),
            (0..8).map(|bit| QuantumCell::Constant(F::from(1_u64 << bit))),
        );
        PastaSha256ByteV1::range_checked(ctx, range, value)
    }))
}

/// Columns of a constant GF(2) linear map, indexed by the input bit in little-endian order.
type Crc64MatrixV1 = [u64; 64];

/// Derive, rather than witness, the inverse transition for a single zero byte.
fn crc64_inverse_zero_byte_matrix_v1() -> Crc64MatrixV1 {
    core::array::from_fn(|bit| {
        let mut state = 1_u64 << bit;
        for _ in 0..8 {
            // For y = (x >> 1) XOR (P * (x & 1)), P's high bit is one and x >> 1 has
            // high bit zero. Hence y's high bit is exactly x's discarded low bit b,
            // and x = ((y XOR P*b) << 1) | b. Eight inverse bit steps invert Z.
            let discarded_bit = state >> 63;
            state = ((state ^ REFLECTED_CRC64_POLYNOMIAL.wrapping_mul(discarded_bit)) << 1)
                | discarded_bit;
        }
        state
    })
}

/// Apply a constant matrix natively only to derive other constant matrices.
fn crc64_apply_matrix_native_v1(matrix: &Crc64MatrixV1, state: u64) -> u64 {
    matrix.iter().enumerate().fold(0, |output, (bit, column)| {
        output ^ (column & 0_u64.wrapping_sub((state >> bit) & 1))
    })
}

/// Square the constant map to obtain the next inverse binary power.
fn crc64_square_matrix_v1(matrix: &Crc64MatrixV1) -> Crc64MatrixV1 {
    core::array::from_fn(|bit| crc64_apply_matrix_native_v1(matrix, matrix[bit]))
}

/// Constrain a constant GF(2) map using parity of bounded sums of Boolean inputs.
fn crc64_apply_matrix_assigned_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    matrix: &Crc64MatrixV1,
    input: &[AssignedValue<F>; 64],
) -> [AssignedValue<F>; 64] {
    let gate = range.gate();
    core::array::from_fn(|output_bit| {
        let terms = matrix
            .iter()
            .zip(input)
            .filter_map(|(column, input_bit)| {
                ((column >> output_bit) & 1 != 0).then_some(*input_bit)
            })
            .collect::<Vec<_>>();
        if terms.is_empty() {
            return ctx.load_constant(F::ZERO);
        }
        // At most 64 proven bits enter this integer sum. Its parity is exactly the low bit
        // of this bounded decomposition; the field characteristic cannot alter that parity.
        let sum_bits = (usize::BITS - terms.len().leading_zeros()) as usize;
        let sum = gate.sum(ctx, terms);
        gate.num_to_bits(ctx, sum, sum_bits)[0]
    })
}

#[cfg(test)]
#[path = "canonical_preimage_prefix_crc_tests.rs"]
mod prefix_crc_tests;

#[cfg(test)]
#[path = "canonical_preimage_bounded_frame_tests.rs"]
mod bounded_frame_tests;

#[cfg(test)]
mod tests {
    use ff::Field as _;
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    use super::*;
    use crate::zk::kagemusha_v1_recursion::guard_bundle::assign_bytes;

    fn crc_case<F: KagemushaPoseidonFieldV1>(payload: &[u8], corrupt_output: bool) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::default()
            .use_k(12)
            .use_lookup_bits(11)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let bytes = assign_bytes(ctx, &range, payload);
        let output = crc64_xz_bytes_v1(ctx, &range, &bytes).expect("CRC relation");
        let mut expected = norito::crc64_fallback(payload).to_le_bytes();
        if corrupt_output {
            expected[3] ^= 0x80;
        }
        let instances = expected
            .into_iter()
            .map(|byte| F::from(u64::from(byte)))
            .collect::<Vec<_>>();
        builder.assigned_instances = vec![
            output
                .map(|byte| byte.assigned().expect("CRC byte"))
                .to_vec(),
        ];
        builder.calculate_params(Some(9));
        MockProver::run(12, &builder, vec![instances])
            .expect("CRC mock prover")
            .verify()
            .is_ok()
    }

    #[test]
    fn crc64_matches_norito_in_both_pasta_fields() {
        for payload in [
            vec![],
            b"123456789".to_vec(),
            (0..64).map(|i| i * 3).collect(),
        ] {
            assert!(crc_case::<Fp>(&payload, false));
            assert!(crc_case::<Fq>(&payload, false));
        }
    }

    #[test]
    fn crc64_rejects_substituted_checksum() {
        assert!(!crc_case::<Fp>(b"canonical mint commitment", true));
        assert!(!crc_case::<Fq>(b"canonical mint commitment", true));
    }

    #[test]
    fn canonical_assembler_matches_model_plaintext_including_padding_and_crc() {
        use iroha_data_model::kagemusha::{
            KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1, KagemushaCreditOpeningV1,
            kagemusha_credit_opening_canonical_layout_v1,
        };
        let opening = KagemushaCreditOpeningV1 {
            version: 1,
            credit_id: [11; 32],
            amount: 17,
            credit_commitment_opening: [21; 32],
            recipient_binding_opening: [31; 32],
            recovery_nonce: [41; 32],
        };
        let canonical = opening.canonical_bytes().expect("model plaintext");
        let layout = kagemusha_credit_opening_canonical_layout_v1().expect("model layout");
        let mut builder = BaseCircuitBuilder::<Fp>::default()
            .use_k(13)
            .use_lookup_bits(12)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let fields = KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1
            .iter()
            .map(|r| assign_bytes(ctx, &range, &canonical[r.clone()]))
            .collect::<Vec<_>>();
        let fields = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let bytes = assemble_canonical_preimage_v1(
            ctx,
            &range,
            &layout,
            &KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1,
            &fields,
        )
        .expect("canonical assembler");
        assert_eq!(
            bytes
                .iter()
                .map(|byte| byte.test_value())
                .collect::<Vec<_>>(),
            canonical
        );
        let actual = bytes
            .iter()
            .map(|byte| {
                range
                    .gate()
                    .mul(ctx, byte.quantum_cell(), QuantumCell::Constant(Fp::ONE))
            })
            .collect::<Vec<_>>();
        builder.assigned_instances = vec![actual];
        builder.calculate_params(Some(9));
        let instances = canonical
            .into_iter()
            .map(|byte| Fp::from(u64::from(byte)))
            .collect::<Vec<_>>();
        MockProver::run(13, &builder, vec![instances])
            .expect("canonical assembler mock prover")
            .assert_satisfied();
    }

    #[test]
    fn canonical_assembler_rejects_holes_overlaps_and_witness_lengths() {
        use iroha_data_model::kagemusha::{
            KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1,
            kagemusha_credit_opening_canonical_layout_v1,
        };
        let mut builder = BaseCircuitBuilder::<Fp>::default()
            .use_k(12)
            .use_lookup_bits(11);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let layout = kagemusha_credit_opening_canonical_layout_v1().unwrap();
        let ranges = KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1;
        let fields = ranges
            .iter()
            .map(|r| vec![PastaSha256ByteV1::constant(1); r.len()])
            .collect::<Vec<_>>();
        let slices = fields.iter().map(Vec::as_slice).collect::<Vec<_>>();
        assert!(
            assemble_canonical_preimage_v1(ctx, &range, &layout, &ranges[..5], &slices[..5])
                .is_err()
        );
        let mut overlap = ranges.clone();
        overlap[3] = overlap[1].clone();
        assert!(assemble_canonical_preimage_v1(ctx, &range, &layout, &overlap, &slices).is_err());
        let mut dynamic_length = layout;
        dynamic_length[23] = None;
        assert!(
            assemble_canonical_preimage_v1(ctx, &range, &dynamic_length, &ranges, &slices).is_err()
        );
        let mut reversed = ranges.clone();
        reversed[0] = Range {
            start: 100,
            end: 99,
        };
        let mut empty_field = slices.clone();
        empty_field[0] = &[];
        assert!(
            assemble_canonical_preimage_v1(ctx, &range, &layout, &reversed, &empty_field).is_err()
        );
    }
}
