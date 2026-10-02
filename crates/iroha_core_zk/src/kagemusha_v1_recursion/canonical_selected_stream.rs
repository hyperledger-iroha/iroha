//! Shared selected canonical original assembly for model-owned variable evidence framing.
//! This reconstructs one complete stream and CRC/SHA, never one full proof per byte width.
use super::{crc64_xz_prefix_bytes_v1, stream::KagemushaBoundedByteStreamV1};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, QuantumCell,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use std::collections::BTreeMap;
/// One sole-encoder framing variant and its already constrained semantic bytes.
pub(in crate::kagemusha_v1_recursion) struct CanonicalSelectedStreamVariantV1<
    F: KagemushaPoseidonFieldV1,
> {
    pub(in crate::kagemusha_v1_recursion) selector: AssignedValue<F>,
    pub(in crate::kagemusha_v1_recursion) raw_length: usize,
    pub(in crate::kagemusha_v1_recursion) prefix: Vec<Option<u8>>,
    pub(in crate::kagemusha_v1_recursion) suffix: Vec<Option<u8>>,
    pub(in crate::kagemusha_v1_recursion) complete_length: usize,
    pub(in crate::kagemusha_v1_recursion) header_crc_bytes: [usize; 8],
    pub(in crate::kagemusha_v1_recursion) archive_payload_start: usize,
    pub(in crate::kagemusha_v1_recursion) semantic_bytes: BTreeMap<usize, PastaSha256ByteV1<F>>,
}
fn select_byte<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    selectors: &[AssignedValue<F>],
    bytes: &[PastaSha256ByteV1<F>],
) -> PastaSha256ByteV1<F> {
    let first = bytes[0].quantum_cell();
    let same = bytes.iter().all(|b| match (first, b.quantum_cell()) {
        (QuantumCell::Constant(a), QuantumCell::Constant(b)) => a == b,
        (QuantumCell::Existing(a), QuantumCell::Existing(b)) => a.cell == b.cell,
        _ => false,
    });
    if same {
        return bytes[0];
    }
    let value = range.gate().inner_product(
        ctx,
        selectors.iter().copied(),
        bytes.iter().map(|b| b.quantum_cell()),
    );
    PastaSha256ByteV1::range_checked(ctx, range, value)
}
/// Reconstruct every syntax/semantic/evidence byte and CRC of one actual selected full original.
#[allow(clippy::too_many_arguments)]
pub(in crate::kagemusha_v1_recursion) fn reconstruct_selected_canonical_stream_v1<
    F: KagemushaPoseidonFieldV1,
>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    variants: &[CanonicalSelectedStreamVariantV1<F>],
    repeated_raw_byte_unit: &[Option<u8>],
    maximum_prefix: usize,
    maximum_suffix: usize,
    maximum_stream: usize,
    raw: &KagemushaBoundedByteStreamV1<F>,
    raw_length_bits: usize,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let first = variants
        .first()
        .ok_or("canonical selected stream variants absent")?;
    let encoded_capacity = raw
        .bytes()
        .len()
        .checked_sub(1)
        .and_then(|n| n.checked_mul(repeated_raw_byte_unit.len()))
        .and_then(|n| n.checked_add(1));
    if encoded_capacity.is_none()
        || maximum_stream == 0
        || first.archive_payload_start > maximum_stream
        || raw_length_bits == 0
        || raw_length_bits > 32
        || raw.bytes().len() >= (1_usize << raw_length_bits)
        || raw.bytes().is_empty()
        || repeated_raw_byte_unit.first() != Some(&None)
        || repeated_raw_byte_unit.iter().skip(1).any(Option::is_none)
        || variants.iter().any(|v| {
            v.raw_length == 0
                || v.raw_length > raw.bytes().len()
                || v.header_crc_bytes != first.header_crc_bytes
                || v.archive_payload_start != first.archive_payload_start
                || v.prefix.len() > maximum_prefix
                || v.suffix.len() > maximum_suffix
                || v.complete_length > maximum_stream
                || v.archive_payload_start > v.complete_length
                || v.header_crc_bytes
                    .iter()
                    .any(|i| *i >= v.prefix.len() || *i >= v.archive_payload_start)
                || v.header_crc_bytes
                    .iter()
                    .enumerate()
                    .any(|(i, position)| v.header_crc_bytes[..i].contains(position))
                || v.semantic_bytes.keys().any(|position| {
                    *position >= v.complete_length
                        || v.header_crc_bytes.contains(position)
                        || (*position >= v.prefix.len()
                            && *position < v.complete_length - v.suffix.len())
                })
                || v.raw_length
                    .checked_sub(1)
                    .and_then(|n| n.checked_mul(repeated_raw_byte_unit.len()))
                    .and_then(|n| n.checked_add(1))
                    .and_then(|n| n.checked_add(v.prefix.len()))
                    .and_then(|n| n.checked_add(v.suffix.len()))
                    != Some(v.complete_length)
        })
    {
        return Err("canonical selected stream fixed topology differs".into());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    range.range_check(ctx, raw.actual_len(), raw_length_bits);
    let selectors = variants.iter().map(|v| v.selector).collect::<Vec<_>>();
    for variant in variants {
        gate.assert_bit(ctx, variant.selector);
        let difference = gate.sub(
            ctx,
            raw.actual_len(),
            QuantumCell::Constant(F::from(variant.raw_length as u64)),
        );
        let selected = gate.mul(ctx, difference, variant.selector);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let sum = gate.sum(ctx, selectors.iter().copied());
    gate.assert_is_const(ctx, &sum, &F::ONE);
    let segment = |ctx: &mut halo2_base::Context<F>,
                   prefix: bool|
     -> Result<KagemushaBoundedByteStreamV1<F>, String> {
        let capacity = if prefix {
            maximum_prefix
        } else {
            maximum_suffix
        };
        let mut bytes = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let choices = variants
                .iter()
                .map(|v| {
                    let template = if prefix { &v.prefix } else { &v.suffix };
                    let Some(byte) = template.get(i) else {
                        return Ok(PastaSha256ByteV1::constant(0));
                    };
                    let global = if prefix {
                        i
                    } else {
                        v.complete_length - v.suffix.len() + i
                    };
                    if let Some(&value) = v.semantic_bytes.get(&global) {
                        return Ok(value);
                    }
                    if v.header_crc_bytes.contains(&global) {
                        return Ok(PastaSha256ByteV1::constant(0));
                    }
                    byte.map(PastaSha256ByteV1::constant)
                        .ok_or_else(|| "canonical selected stream unassigned byte".into())
                })
                .collect::<Result<Vec<_>, String>>()?;
            bytes.push(select_byte(ctx, &range, &selectors, &choices));
        }
        let length = gate.inner_product(
            ctx,
            selectors.iter().copied(),
            variants.iter().map(|v| {
                QuantumCell::Constant(F::from(if prefix {
                    v.prefix.len()
                } else {
                    v.suffix.len()
                } as u64))
            }),
        );
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, bytes, length)
    };
    let prefix = segment(ctx, true)?;
    let suffix = segment(ctx, false)?;
    let stride = repeated_raw_byte_unit.len();
    let raw_capacity = encoded_capacity.ok_or("canonical encoded capacity overflow")?;
    let mut encoded = Vec::with_capacity(raw_capacity);
    for (i, raw_byte) in raw.bytes().iter().enumerate() {
        let active = range.is_less_than(
            ctx,
            QuantumCell::Constant(F::from(i as u64)),
            raw.actual_len(),
            raw_length_bits,
        );
        let value = gate.mul(ctx, raw_byte.quantum_cell(), active);
        encoded.push(PastaSha256ByteV1::range_checked(ctx, &range, value));
        if i + 1 < raw.bytes().len() {
            let continues = range.is_less_than(
                ctx,
                QuantumCell::Constant(F::from((i + 1) as u64)),
                raw.actual_len(),
                raw_length_bits,
            );
            for fixed in repeated_raw_byte_unit.iter().skip(1) {
                let value = gate.mul(
                    ctx,
                    QuantumCell::Constant(F::from(u64::from(
                        fixed.ok_or("canonical byte unit hole")?,
                    ))),
                    continues,
                );
                encoded.push(PastaSha256ByteV1::range_checked(ctx, &range, value));
            }
        }
    }
    let one_less = gate.sub(ctx, raw.actual_len(), QuantumCell::Constant(F::ONE));
    let length = gate.mul_add(
        ctx,
        one_less,
        QuantumCell::Constant(F::from(stride as u64)),
        QuantumCell::Constant(F::ONE),
    );
    let encoded = KagemushaBoundedByteStreamV1::constrain(ctx, &range, encoded, length)?;
    let stream = prefix
        .concat(ctx, &range, &encoded, maximum_stream)?
        .concat(ctx, &range, &suffix, maximum_stream)?;
    let expected_length = gate.inner_product(
        ctx,
        selectors.iter().copied(),
        variants
            .iter()
            .map(|v| QuantumCell::Constant(F::from(v.complete_length as u64))),
    );
    ctx.constrain_equal(&stream.actual_len(), &expected_length);
    let payload_length = gate.sub(
        ctx,
        stream.actual_len(),
        QuantumCell::Constant(F::from(first.archive_payload_start as u64)),
    );
    let payload = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        stream.bytes()[first.archive_payload_start..].to_vec(),
        payload_length,
    )?;
    let checksum = crc64_xz_prefix_bytes_v1(ctx, &range, payload.bytes(), payload.actual_len())?;
    let mut message = stream.bytes().to_vec();
    for (position, byte) in first.header_crc_bytes.iter().zip(checksum) {
        message[*position] = byte
    }
    let words = jobs.digest_bounded_constrained(ctx, &range, &message, stream.actual_len())?;
    let mut digest = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, gate, word, 32);
        for offset in [24, 16, 8, 0] {
            digest.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                gate,
                &bits[offset..offset + 8],
            ));
        }
    }
    digest
        .try_into()
        .map_err(|_| "canonical selected stream SHA width differs".into())
}
