//! Private genuine periodic Integrity lease relation for the same ordinary credential.
//!
//! This consumes the unchanged Ed-only original and mandatory purpose2 issuer signature.
//! The Native holder separately retains its exact Google/possession originals and current clock.
//! A lease cannot select a new credential, key, trust policy, financial epoch or approval deadline.

use super::{
    DigestV1,
    canonical_preimage::assemble_canonical_preimage_v1,
    guard_bundle::{assign_bytes, constant_bytes, hash},
    ordinary_app_guard_binding::{
        OrdinaryCredentialIssuerCellsV1, OrdinaryCredentialOriginalCellsV1,
    },
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1;
use sha2::{Digest as _, Sha256};

pub(super) struct OrdinaryIntegrityLeaseCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) version: [PastaSha256ByteV1<F>; 2],
    pub(super) fields: [[PastaSha256ByteV1<F>; 32]; 11],
    pub(super) scalars: [[PastaSha256ByteV1<F>; 8]; 6],
    pub(super) signature: [PastaSha256ByteV1<F>; 64],
    pub(super) possession: Vec<PastaSha256ByteV1<F>>,
    pub(super) issuer_admission: Option<OrdinaryCredentialIssuerCellsV1<F>>,
}

#[cfg(test)]
fn equal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    a: &[PastaSha256ByteV1<F>],
    b: &[PastaSha256ByteV1<F>],
) -> Result<(), String> {
    if a.len() != b.len() {
        return Err("ordinary lease semantic width differs".into());
    }
    for (a, b) in a.iter().zip(b) {
        let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &d, &F::ZERO);
    }
    Ok(())
}

fn uint64<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) -> Result<AssignedValue<F>, String> {
    if bytes.len() != 8 {
        return Err("ordinary lease integer width differs".into());
    }
    let value = range.gate().inner_product(
        ctx,
        bytes.iter().map(|b| b.quantum_cell()),
        (0..8).map(|i| halo2_base::QuantumCell::Constant(F::from(1u64 << (8 * i)))),
    );
    range.range_check(ctx, value, 64);
    Ok(value)
}

#[cfg(test)]
fn reconstruct<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    layout: &KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1,
    cells: &OrdinaryIntegrityLeaseCellsV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if layout.original.end != layout.bytes.len() {
        return Err("ordinary lease original layout has trailing bytes".into());
    }
    let prefix = layout.bytes[..layout.original.start]
        .iter()
        .copied()
        .collect::<Option<Vec<_>>>()
        .ok_or("ordinary lease digest prefix is not fixed")?;
    // Borrow cells and local issuer framing through one inferred collection lifetime.
    fn raw<'a, F: KagemushaPoseidonFieldV1>(
        original_range: &core::ops::Range<usize>,
        ranges: &mut Vec<core::ops::Range<usize>>,
        fields: &mut Vec<&'a [PastaSha256ByteV1<F>]>,
        indices: &[usize],
        values: &'a [PastaSha256ByteV1<F>],
    ) -> Result<(), String> {
        if indices.len() != values.len() {
            return Err("ordinary lease original field width differs".into());
        }
        for (i, v) in indices.iter().zip(values) {
            let i = i
                .checked_sub(original_range.start)
                .filter(|i| *i < original_range.len())
                .ok_or("ordinary lease original position is outside the frame")?;
            ranges.push(i..i + 1);
            fields.push(core::slice::from_ref(v));
        }
        Ok(())
    }
    let original_range = &layout.original;
    let issuer_version = [
        PastaSha256ByteV1::constant(1),
        PastaSha256ByteV1::constant(0),
    ];
    let issuer_purpose = [PastaSha256ByteV1::constant(2)];
    let mut ranges = Vec::new();
    let mut fields = Vec::new();
    raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.version_bytes,
        &cells.version,
    )?;
    for (indices, values) in layout.fixed_digest_bytes.iter().zip(&cells.fields) {
        raw(original_range, &mut ranges, &mut fields, indices, values)?;
    }
    for (indices, values) in layout.scalar_bytes.iter().zip(&cells.scalars) {
        raw(original_range, &mut ranges, &mut fields, indices, values)?;
    }
    raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.signature_bytes,
        &cells.signature,
    )?;
    raw(
        original_range,
        &mut ranges,
        &mut fields,
        &layout.possession_signature_bytes,
        &cells.possession,
    )?;
    if let Some(layout) = &layout.issuer_admission_layout {
        let issuer = cells
            .issuer_admission
            .as_ref()
            .ok_or("ordinary full lease original lacks issuer cells")?;
        raw(
            original_range,
            &mut ranges,
            &mut fields,
            &layout.version_bytes,
            &issuer_version,
        )?;
        raw(
            original_range,
            &mut ranges,
            &mut fields,
            &[layout.purpose_byte],
            &issuer_purpose,
        )?;
        for (indices, values) in layout.fixed_digest_bytes.iter().zip([
            &cells.fields[3],
            &cells.fields[4],
            &issuer.ed_original_sha256,
        ]) {
            raw(original_range, &mut ranges, &mut fields, indices, values)?;
        }
        raw(
            original_range,
            &mut ranges,
            &mut fields,
            &layout.signature_bytes,
            &issuer.signature,
        )?;
    }
    let range = builder.range_chip();
    let frame = assemble_canonical_preimage_v1(
        builder.main(0),
        &range,
        &layout.bytes[layout.original.clone()],
        &ranges,
        &fields,
    )?;
    let mut bytes = constant_bytes(&prefix);
    bytes.extend(frame);
    hash(builder.main(0), jobs, bytes)
}

/// Initial verdict or Apple validity, separately from a periodic issuer-signed refresh lease.
pub(super) fn constrain_ordinary_initial_integrity_interval_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    integrity_selected: AssignedValue<F>,
    approval_issued: AssignedValue<F>,
    approval_expires: AssignedValue<F>,
) -> Result<(), String> {
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let issued = uint64(ctx, &range, &credential.scalars[2])?;
    let expires = uint64(ctx, &range, &credential.scalars[3])?;
    let too_early = range.is_less_than(ctx, approval_issued, issued, 64);
    range.gate().assert_is_const(ctx, &too_early, &F::ZERO);
    let too_late = range.is_less_than(ctx, expires, approval_expires, 64);
    range.gate().assert_is_const(ctx, &too_late, &F::ZERO);
    if let Some(pi) = &credential.play_integrity_fields {
        let verified = uint64(ctx, &range, &pi[3])?;
        let refresh = uint64(ctx, &range, &pi[4])?;
        for (a, b) in [(verified, approval_issued), (approval_expires, refresh)] {
            let bad = range.is_less_than(ctx, b, a, 64);
            let active_bad = range.gate().mul(ctx, bad, integrity_selected);
            range.gate().assert_is_const(ctx, &active_bad, &F::ZERO);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "ordinary_integrity_interval_tests.rs"]
mod interval_tests;
