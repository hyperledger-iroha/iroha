//! Typed original-message and digest products shared by Projection and SHA.
//!
//! Each active call contributes512 zero-padded payload words, one exact length,
//! and eight digest words. Product terminals stay private at native row zero.
use super::*;
use crate::privacy_engines::zk_x509::sha_call_bus_stark::{
    ZkX509ShaCallBusLaneChallengesV1, compress_sha_call_fields_v1,
};

/// Product columns appended after all original projection auxiliaries.
pub(crate) const AUX_START_V1: usize = 32;
/// Typed zero-padded message word, distinct from SHA compression padding.
pub(crate) const PADDED_MESSAGE_KIND_V1: u64 = 3;
/// Exact message length; trailing zero bytes must not disappear.
pub(crate) const MESSAGE_LENGTH_KIND_V1: u64 = 4;
/// The existing SHA digest word type.
pub(crate) const DIGEST_KIND_V1: u64 = 1;
const FIX_PACK: usize = 25;
const FIX_PACK_FIRST: usize = 26;
const FIX_PACK_LAST: usize = 27;
const FIX_KIND: usize = 28;
const FIX_WORD: usize = 29;
const FIX_LENGTH: usize = 30;
pub(super) const FIX_END_V1: usize = 31;

/// The gamma coefficient is exactly one for every typed factor.
pub(crate) fn factor_v1<A: PolynomialAirFieldV1>(
    invocation: A,
    kind: A,
    index: A,
    value: A,
    lane: ZkX509ShaCallBusLaneChallengesV1,
) -> A {
    compress_sha_call_fields_v1(
        invocation.add(A::from_base(F(5))),
        A::from_base(F(4)),
        invocation,
        kind,
        index,
        value,
        lane,
    )
}

pub(super) fn populate_fixed_v1(
    row: ZkX509ProjectionFixedRowV1,
    fixed: &mut [F; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1],
) {
    let packed = match row {
        ZkX509ProjectionFixedRowV1::Output {
            offset,
            active: true,
            ..
        } => Some((usize::from(offset), PADDED_MESSAGE_KIND_V1)),
        ZkX509ProjectionFixedRowV1::Digest {
            offset,
            active: true,
            ..
        } => Some((usize::from(offset), DIGEST_KIND_V1)),
        ZkX509ProjectionFixedRowV1::MessageLength {
            active: true,
            last: true,
            ..
        } => {
            fixed[FIX_LENGTH] = F::ONE;
            None
        }
        _ => None,
    };
    if let Some((offset, kind)) = packed {
        fixed[FIX_PACK] = F::ONE;
        fixed[FIX_PACK_FIRST] = F(u64::from(offset % 4 == 0));
        fixed[FIX_PACK_LAST] = F(u64::from(offset % 4 == 3));
        fixed[FIX_KIND] = F(kind);
        fixed[FIX_WORD] = F((offset / 4) as u64);
    }
}

pub(super) fn populate_base_v1(
    fixed: ZkX509ProjectionFixedRowV1,
    row: &mut [F; ZK_X509_PROJECTION_BASE_WIDTH_V1],
    packed: &mut F,
) {
    let offset = match fixed {
        ZkX509ProjectionFixedRowV1::Output {
            offset,
            active: true,
            ..
        } => usize::from(offset),
        ZkX509ProjectionFixedRowV1::Digest {
            offset,
            active: true,
            ..
        } => usize::from(offset),
        _ => return,
    };
    if offset % 4 == 0 {
        *packed = F::ZERO;
    }
    row[REGION_BEFORE] = *packed;
    *packed = packed.mul(F(256)).add(row[VALUE]);
    row[REGION_AFTER] = *packed;
}

pub(super) fn row_factor_v1<A: PolynomialAirFieldV1>(
    current: &[A; ZK_X509_PROJECTION_BASE_WIDTH_V1],
    fixed: &[A; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1],
    lane: ZkX509ShaCallBusLaneChallengesV1,
) -> A {
    let invocation = fixed[FIX_INVOCATION].sub(A::ONE);
    let word = factor_v1(
        invocation,
        fixed[FIX_KIND],
        fixed[FIX_WORD],
        current[REGION_AFTER],
        lane,
    );
    let length = factor_v1(
        invocation,
        A::from_base(F(MESSAGE_LENGTH_KIND_V1)),
        A::ZERO,
        current[DECLARED_LENGTH],
        lane,
    );
    A::ONE
        .add(fixed[FIX_PACK_LAST].mul(word.sub(A::ONE)))
        .add(fixed[FIX_LENGTH].mul(length.sub(A::ONE)))
}

pub(super) fn push_residues_v1<A: PolynomialAirFieldV1>(
    current: &[A; ZK_X509_PROJECTION_BASE_WIDTH_V1],
    next: &[A; ZK_X509_PROJECTION_BASE_WIDTH_V1],
    current_aux: &[A; ZK_X509_PROJECTION_AUX_WIDTH_V1],
    next_aux: &[A; ZK_X509_PROJECTION_AUX_WIDTH_V1],
    fixed: &[A; ZK_X509_PROJECTION_STARK_FIXED_WIDTH_V1],
    challenges: ZkX509ProjectionChallengesV1,
    residues: &mut Vec<A>,
) {
    residues.push(fixed[FIX_PACK_FIRST].mul(current[REGION_BEFORE]));
    residues.push(
        fixed[FIX_PACK].mul(
            current[REGION_AFTER].sub(
                current[REGION_BEFORE]
                    .mul(A::from_base(F(256)))
                    .add(current[VALUE]),
            ),
        ),
    );
    residues.push(
        fixed[FIX_PACK]
            .sub(fixed[FIX_PACK_LAST])
            .mul(next[REGION_BEFORE].sub(current[REGION_AFTER])),
    );
    let first = fixed[FIX_FIRST_ROW];
    for lane in 0..COPY_LANES {
        let factor = row_factor_v1(current, fixed, challenges.hash_bus.lanes[lane]);
        residues.push(first.mul(next_aux[AUX_START_V1 + lane].sub(A::ONE)));
        residues.push(
            A::ONE.sub(first).mul(
                next_aux[AUX_START_V1 + lane].sub(current_aux[AUX_START_V1 + lane].mul(factor)),
            ),
        );
    }
}
