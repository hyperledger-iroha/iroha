//! One encoder-derived bounded stream for every permitted original lease DER width.
//!
//! The length selects codec metadata only. Semantic bytes, complete issuer admission and CRC
//! remain real constrained cells. This module grants no lease or signature authority.

use super::{
    canonical_preimage::{crc64_xz_prefix_bytes_v1, stream::KagemushaBoundedByteStreamV1},
    ordinary_integrity_binding::OrdinaryIntegrityLeaseCellsV1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, QuantumCell,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::{
    KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1,
    KagemushaPlayIntegrityRefreshLeaseStreamLengthV1,
};
use std::collections::BTreeMap;

fn positions<F: KagemushaPoseidonFieldV1>(
    variant: &KagemushaPlayIntegrityRefreshLeaseStreamLengthV1,
    cells: &OrdinaryIntegrityLeaseCellsV1<F>,
) -> Result<BTreeMap<usize, PastaSha256ByteV1<F>>, String> {
    let mut values = BTreeMap::new();
    let mut put = |indices: &[usize], bytes: &[PastaSha256ByteV1<F>]| {
        if indices.len() != bytes.len() {
            return Err("ordinary lease stream field width differs".to_owned());
        }
        for (&index, &byte) in indices.iter().zip(bytes) {
            if values.insert(index, byte).is_some() {
                return Err("ordinary lease stream semantic positions overlap".to_owned());
            }
        }
        Ok(())
    };
    let layout = &variant.layout;
    put(&layout.version_bytes, &cells.version)?;
    for (indices, bytes) in layout.fixed_digest_bytes.iter().zip(&cells.fields) {
        put(indices, bytes)?;
    }
    for (indices, bytes) in layout.scalar_bytes.iter().zip(&cells.scalars) {
        put(indices, bytes)?;
    }
    put(&layout.signature_bytes, &cells.signature)?;
    if let Some(issuer_layout) = &layout.issuer_admission_layout {
        let issuer = cells
            .issuer_admission
            .as_ref()
            .ok_or("ordinary lease stream issuer absent")?;
        put(
            &issuer_layout.version_bytes,
            &[
                PastaSha256ByteV1::constant(1),
                PastaSha256ByteV1::constant(0),
            ],
        )?;
        put(
            &[issuer_layout.purpose_byte],
            &[PastaSha256ByteV1::constant(2)],
        )?;
        for (indices, bytes) in issuer_layout.fixed_digest_bytes.iter().zip([
            &cells.fields[3],
            &cells.fields[4],
            &issuer.ed_original_sha256,
        ]) {
            put(indices, bytes)?;
        }
        put(&issuer_layout.signature_bytes, &issuer.signature)?;
    }
    Ok(values)
}

/// Select exact grammar bytes. Common constants/cells are shared rather than repeated 65 times.
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
    let byte = range.gate().inner_product(
        ctx,
        selectors.iter().copied(),
        bytes.iter().map(|b| b.quantum_cell()),
    );
    PastaSha256ByteV1::range_checked(ctx, range, byte)
}

/// Hash one exact selected original with one fixed capacity, including its active payload CRC.
pub(super) fn reconstruct_ordinary_integrity_stream_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    grammar: &KagemushaPlayIntegrityRefreshLeaseStreamGrammarV1,
    cells: &OrdinaryIntegrityLeaseCellsV1<F>,
    der_length: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if grammar.lengths.len() != 65
        || cells.possession.len() != 72
        || grammar.der_byte_in_unit != 0
        || grammar.repeated_der_byte_unit.first() != Some(&None)
        || grammar
            .repeated_der_byte_unit
            .iter()
            .skip(1)
            .any(Option::is_none)
    {
        return Err("ordinary lease bounded grammar differs".into());
    }
    let first = &grammar.lengths[0];
    let payload_start = first.archive_payload.start;
    if grammar.lengths.iter().enumerate().any(|(i, v)| {
        v.der_length != i + 8
            || v.header_crc_bytes != first.header_crc_bytes
            || v.header_payload_length_bytes != first.header_payload_length_bytes
            || v.archive_payload.start != payload_start
            || v.layout.original.start != first.layout.original.start
            || v.prefix.len() > grammar.maximum_prefix_bytes
            || v.suffix.len() > grammar.maximum_suffix_bytes
            || v.layout.bytes.len() > grammar.maximum_stream_bytes
    }) {
        return Err("ordinary lease selected grammar topology differs".into());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    range.range_check(ctx, der_length, 7);
    let selectors = (8..=72)
        .map(|n| {
            range
                .gate()
                .is_equal(ctx, der_length, QuantumCell::Constant(F::from(n)))
        })
        .collect::<Vec<_>>();
    let sum = range.gate().sum(ctx, selectors.iter().copied());
    range.gate().assert_is_const(ctx, &sum, &F::ONE);
    let maps = grammar
        .lengths
        .iter()
        .map(|v| positions(v, cells))
        .collect::<Result<Vec<_>, _>>()?;
    let make_segment = |ctx: &mut halo2_base::Context<F>,
                        prefix: bool|
     -> Result<KagemushaBoundedByteStreamV1<F>, String> {
        let capacity = if prefix {
            grammar.maximum_prefix_bytes
        } else {
            grammar.maximum_suffix_bytes
        };
        let mut segment = Vec::with_capacity(capacity);
        for i in 0..capacity {
            let choices = grammar
                .lengths
                .iter()
                .zip(&maps)
                .map(|(v, map)| {
                    let bytes = if prefix { &v.prefix } else { &v.suffix };
                    let Some(template) = bytes.get(i) else {
                        return Ok(PastaSha256ByteV1::constant(0));
                    };
                    let global = if prefix {
                        i
                    } else {
                        v.layout.bytes.len() - v.suffix.len() + i
                    };
                    if let Some(&semantic) = map.get(&global) {
                        return Ok(semantic);
                    }
                    if v.header_crc_bytes.contains(&global) {
                        return Ok(PastaSha256ByteV1::constant(0));
                    }
                    template
                        .map(PastaSha256ByteV1::constant)
                        .ok_or_else(|| "ordinary lease stream has an unassigned byte".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            segment.push(select_byte(ctx, &range, &selectors, &choices));
        }
        let length = range.gate().inner_product(
            ctx,
            selectors.iter().copied(),
            grammar.lengths.iter().map(|v| {
                QuantumCell::Constant(F::from(if prefix {
                    v.prefix.len()
                } else {
                    v.suffix.len()
                } as u64))
            }),
        );
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, segment, length)
    };
    let prefix = make_segment(ctx, true)?;
    let suffix = make_segment(ctx, false)?;
    // The model's repeated framing follows each raw byte except the last. Build all 72 units;
    // circuit selectors keep the sole final raw byte and discard its subsequent framing.
    let stride = grammar.repeated_der_byte_unit.len();
    let der_capacity = 71 * stride + 1;
    let mut der = Vec::with_capacity(der_capacity);
    for i in 0..72 {
        let active =
            range.is_less_than(ctx, QuantumCell::Constant(F::from(i as u64)), der_length, 7);
        let byte = range
            .gate()
            .mul(ctx, cells.possession[i].quantum_cell(), active);
        der.push(PastaSha256ByteV1::range_checked(ctx, &range, byte));
        if i < 71 {
            let continues = range.is_less_than(
                ctx,
                QuantumCell::Constant(F::from((i + 1) as u64)),
                der_length,
                7,
            );
            for fixed in grammar.repeated_der_byte_unit.iter().skip(1) {
                let byte = range.gate().mul(
                    ctx,
                    QuantumCell::Constant(F::from(u64::from(
                        fixed.ok_or("ordinary lease DER unit differs")?,
                    ))),
                    continues,
                );
                der.push(PastaSha256ByteV1::range_checked(ctx, &range, byte));
            }
        }
    }
    let one_less = range
        .gate()
        .sub(ctx, der_length, QuantumCell::Constant(F::ONE));
    let der_size = range.gate().mul_add(
        ctx,
        one_less,
        QuantumCell::Constant(F::from(stride as u64)),
        QuantumCell::Constant(F::ONE),
    );
    let der = KagemushaBoundedByteStreamV1::constrain(ctx, &range, der, der_size)?;
    let stream = prefix
        .concat(ctx, &range, &der, grammar.maximum_stream_bytes)?
        .concat(ctx, &range, &suffix, grammar.maximum_stream_bytes)?;
    let payload_len = range.gate().sub(
        ctx,
        stream.actual_len(),
        QuantumCell::Constant(F::from(payload_start as u64)),
    );
    let payload = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        stream.bytes()[payload_start..].to_vec(),
        payload_len,
    )?;
    let checksum = crc64_xz_prefix_bytes_v1(ctx, &range, payload.bytes(), payload.actual_len())?;
    let mut message = stream.bytes().to_vec();
    for (position, byte) in first.header_crc_bytes.iter().zip(checksum) {
        message[*position] = byte;
    }
    let words = jobs.digest_bounded_constrained(ctx, &range, &message, stream.actual_len())?;
    let mut digest = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            digest.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    digest
        .try_into()
        .map_err(|_| "ordinary lease SHA width differs".into())
}

#[cfg(test)]
#[path = "ordinary_integrity_stream_tests.rs"]
mod tests;
