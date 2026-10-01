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
    ordinary_issuer_equation::constrain_ordinary_issuer_original_v1,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
};
use iroha_data_model::kagemusha::{
    KagemushaPlayIntegrityRefreshLeaseOriginalLayoutV1, KagemushaPlayIntegrityRefreshLeaseV1,
};
use sha2::{Digest as _, Sha256};

pub(super) struct OrdinaryIntegrityLeaseCellsV1<F: KagemushaPoseidonFieldV1> {
    version: [PastaSha256ByteV1<F>; 2],
    fields: [[PastaSha256ByteV1<F>; 32]; 11],
    scalars: [[PastaSha256ByteV1<F>; 8]; 6],
    signature: [PastaSha256ByteV1<F>; 64],
    possession: Vec<PastaSha256ByteV1<F>>,
    issuer_admission: Option<OrdinaryCredentialIssuerCellsV1<F>>,
}

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

/// Authenticate the precise reservation-selected refresh lease and constrain its private scope
/// and interval to the same original credential and actual platform approval signing frame.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_integrity_lease_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    credential_digest: &[PastaSha256ByteV1<F>; 32],
    raw: &KagemushaPlayIntegrityRefreshLeaseV1,
    governed_key: &[u8; 65],
    governed_key_cells: &[AssignedValue<F>; 65],
    approval_issued: AssignedValue<F>,
    approval_expires: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    constrain_ordinary_integrity_lease_inner_v1(
        builder,
        jobs,
        credential,
        credential_digest,
        raw,
        Some((governed_key, governed_key_cells)),
        approval_issued,
        approval_expires,
    )
}

/// Same-original State-side copying of the complete selected lease. This never authenticates
/// its issuer; the enclosing State must consume the corresponding genuine ordinary Guard.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_integrity_lease_data_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    credential_digest: &[PastaSha256ByteV1<F>; 32],
    raw: &KagemushaPlayIntegrityRefreshLeaseV1,
    approval_issued: AssignedValue<F>,
    approval_expires: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    constrain_ordinary_integrity_lease_inner_v1(
        builder,
        jobs,
        credential,
        credential_digest,
        raw,
        None,
        approval_issued,
        approval_expires,
    )
}

#[allow(clippy::too_many_arguments)]
fn constrain_ordinary_integrity_lease_inner_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    credential: &OrdinaryCredentialOriginalCellsV1<F>,
    credential_digest: &[PastaSha256ByteV1<F>; 32],
    raw: &KagemushaPlayIntegrityRefreshLeaseV1,
    issuer: Option<(&[u8; 65], &[AssignedValue<F>; 65])>,
    approval_issued: AssignedValue<F>,
    approval_expires: AssignedValue<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let layout = raw.ed_only_preimage_layout()?;
    let bytes = raw.ed_only_canonical_bytes()?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut positions = |indices: &[usize]| {
        assign_bytes(
            ctx,
            &range,
            &indices.iter().map(|i| bytes[*i]).collect::<Vec<_>>(),
        )
    };
    let mut cells = OrdinaryIntegrityLeaseCellsV1 {
        version: positions(&layout.version_bytes)
            .try_into()
            .map_err(|_| "ordinary lease version width")?,
        fields: core::array::from_fn(|i| {
            positions(&layout.fixed_digest_bytes[i])
                .try_into()
                .expect("lease raw32")
        }),
        scalars: core::array::from_fn(|i| {
            positions(&layout.scalar_bytes[i])
                .try_into()
                .expect("lease raw64")
        }),
        signature: positions(&layout.signature_bytes)
            .try_into()
            .map_err(|_| "ordinary lease Ed signature width")?,
        possession: positions(&layout.possession_signature_bytes),
        issuer_admission: None,
    };
    let ed_digest = reconstruct(builder, jobs, &layout, &cells)?;
    let signature = assign_bytes(
        builder.main(0),
        &range,
        raw.circuit_admission.signature.as_raw_bytes(),
    )
    .try_into()
    .map_err(|_| "ordinary lease issuer signature width")?;
    if let Some((governed_key, governed_key_cells)) = issuer {
        constrain_ordinary_issuer_original_v1(
            builder,
            jobs,
            2,
            &credential.fixed_digests[6],
            &credential.fixed_digests[7],
            &ed_digest,
            &signature,
            &raw.circuit_admission,
            governed_key,
            governed_key_cells,
        )?;
    }
    cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed_digest,
        signature,
    });
    let ctx = builder.main(0);
    for (actual, expected) in [
        (&cells.fields[0], credential_digest),
        (&cells.fields[2], &credential.fixed_digests[13]),
        (&cells.fields[3], &credential.fixed_digests[6]),
        (&cells.fields[4], &credential.fixed_digests[7]),
        (&cells.fields[5], &credential.fixed_digests[9]),
        (&cells.fields[6], &credential.fixed_digests[10]),
    ] {
        equal(ctx, &range, actual, expected)?;
    }
    let pi = credential
        .play_integrity_fields
        .as_ref()
        .ok_or("ordinary lease lacks original governed Integrity policy")?;
    equal(ctx, &range, &cells.fields[9], &pi[2])?;
    equal(ctx, &range, &cells.scalars[0], &credential.scalars[0])?;
    equal(ctx, &range, &cells.scalars[1], &credential.scalars[1])?;
    equal(
        ctx,
        &range,
        &cells.version,
        &[
            PastaSha256ByteV1::constant(1),
            PastaSha256ByteV1::constant(0),
        ],
    )?;
    let time = core::array::from_fn::<_, 4, _>(|i| {
        uint64(ctx, &range, &cells.scalars[i + 2]).expect("lease uint64")
    });
    let credential_issued = uint64(ctx, &range, &credential.scalars[2])?;
    let credential_expires = uint64(ctx, &range, &credential.scalars[3])?;
    // Google verdict <= issuer lease <= exact approval; all exclusive deadlines stay bounded
    // by both the unchanged credential and the exact original refresh/lease signature.
    for (before, after) in [
        (time[0], time[2]),
        (time[2], approval_issued),
        (credential_issued, time[2]),
        (approval_expires, time[1]),
        (approval_expires, time[3]),
        (time[3], time[1]),
        (time[3], credential_expires),
    ] {
        let bad = range.is_less_than(ctx, after, before, 64);
        range.gate().assert_is_const(ctx, &bad, &F::ZERO);
    }
    let live = range.is_less_than(ctx, time[2], time[3], 64);
    range.gate().assert_is_const(ctx, &live, &F::ONE);
    let der_sha = hash(ctx, jobs, cells.possession.clone())?;
    equal(ctx, &range, &der_sha, &cells.fields[10])?;
    // The model-issued P256 admission attests this complete Ed+possession original. The native
    // lease admission still independently verifies the actual Core/Ed/app/Google originals.
    let expected: DigestV1 = Sha256::digest(bytes).into();
    let expected = assign_bytes(ctx, &range, &expected);
    equal(ctx, &range, &ed_digest, &expected)?;
    reconstruct(builder, jobs, &raw.original_preimage_layout()?, &cells)
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
