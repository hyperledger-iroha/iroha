//! One encoder-derived bounded stream for every permitted original lease DER width.
//!
//! The length selects codec metadata only. Semantic bytes, complete issuer admission and CRC
//! remain real constrained cells. This module grants no lease or signature authority.

use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    ordinary_integrity_binding::OrdinaryIntegrityLeaseCellsV1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
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
    let raw = KagemushaBoundedByteStreamV1::constrain(
        ctx,
        &range,
        cells.possession.to_vec(),
        der_length,
    )?;
    let variants = grammar
        .lengths
        .iter()
        .zip(selectors)
        .zip(maps)
        .map(|((v, selector), semantic_bytes)| {
            super::canonical_preimage::selected_stream::CanonicalSelectedStreamVariantV1 {
                selector,
                raw_length: v.der_length,
                prefix: v.prefix.clone(),
                suffix: v.suffix.clone(),
                complete_length: v.layout.bytes.len(),
                header_crc_bytes: v.header_crc_bytes,
                archive_payload_start: v.archive_payload.start,
                semantic_bytes,
            }
        })
        .collect::<Vec<_>>();
    super::canonical_preimage::selected_stream::reconstruct_selected_canonical_stream_v1(
        builder,
        jobs,
        &variants,
        &grammar.repeated_der_byte_unit,
        grammar.maximum_prefix_bytes,
        grammar.maximum_suffix_bytes,
        grammar.maximum_stream_bytes,
        &raw,
        7,
    )
}

#[cfg(test)]
#[path = "ordinary_integrity_stream_tests.rs"]
mod tests;
