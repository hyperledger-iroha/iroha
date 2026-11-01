//! Projection-message products taken from the actual constrained SHA state.
use super::*;
use crate::privacy_engines::zk_x509::projection_air::hash_bus;

/// Four new private cyclic products follow the original SHA auxiliaries.
pub const AUX_START_V1: usize = 78;
/// Payload-word event selector; SHA padding bytes are masked out separately.
pub const FIX_WORD_V1: usize = 134;
/// Exact message length event on the call's final row.
pub const FIX_LENGTH_V1: usize = 135;
/// Actual SHA digest word event.
pub const FIX_DIGEST_V1: usize = 136;
pub(super) const FIX_END_V1: usize = 137;
pub const WORDS_V1: usize = 512;

pub fn is_active_v1(manifest: ZkX509ShaCallManifestV1) -> bool {
    matches!(manifest.role, ZkX509ShaCallRoleV1::Projection(0..=6))
        && manifest.activation == ZkX509ShaCallActivationV1::Required
}

pub(super) fn emit_fixed_v1(
    manifest: ZkX509ShaCallManifestV1,
    word: &[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1],
    mut emit: impl FnMut(usize, F),
) {
    if !is_active_v1(manifest) {
        return;
    }
    emit(
        FIX_WORD_V1,
        F(u64::from(
            word[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE
                && word[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0 < WORDS_V1 as u64,
        )),
    );
    emit(FIX_LENGTH_V1, word[SHA_WORD_CAPACITY_CALL_LAST_V1]);
    emit(FIX_DIGEST_V1, word[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1]);
}

pub(super) fn row_factor_v1<A: PolynomialAirFieldV1>(
    base: &[A; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    message_count: A,
    fixed: &[A; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    lane: ZkX509ShaCallBusLaneChallengesV1,
) -> A {
    let invocation = fixed[ZK_X509_SHA_FIXED_SLOT_V1];
    let mut payload_word = A::ZERO;
    for byte in 0..4 {
        let bits = 1 + (3 - byte) * 8;
        let value = pack_sha_rfc_bits_v1(&base[bits..bits + 8]);
        payload_word = payload_word.add(
            value
                .mul(base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte])
                .mul(A::from_base(F(1_u64 << (8 * (3 - byte))))),
        );
    }
    let word = hash_bus::factor_v1(
        invocation,
        A::from_base(F(hash_bus::PADDED_MESSAGE_KIND_V1)),
        fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1],
        payload_word,
        lane,
    );
    let length = hash_bus::factor_v1(
        invocation,
        A::from_base(F(hash_bus::MESSAGE_LENGTH_KIND_V1)),
        A::ZERO,
        message_count,
        lane,
    );
    let digest = hash_bus::factor_v1(
        invocation,
        A::from_base(F(hash_bus::DIGEST_KIND_V1)),
        fixed[SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1],
        base[0],
        lane,
    );
    A::ONE
        .add(fixed[FIX_WORD_V1].mul(word.sub(A::ONE)))
        .add(fixed[FIX_LENGTH_V1].mul(length.sub(A::ONE)))
        .add(fixed[FIX_DIGEST_V1].mul(digest.sub(A::ONE)))
}

/// A bounded prepass permits a streamed row-zero terminal without retaining a
/// whole auxiliary matrix. The AIR independently binds every factor to rows.
pub(super) fn segment_terminal_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1],
    segment: usize,
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<zeroize::Zeroizing<[F; 4]>, ZkX509ShaCallBusStarkErrorV1> {
    if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let mut products = zeroize::Zeroizing::new([F::ONE; 4]);
    for manifest in schedule.calls().iter().copied().filter(|call| {
        call.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1 == segment && is_active_v1(*call)
    }) {
        let witness = witnesses
            .get(usize::from(manifest.call))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        if witness.role != manifest.role || witness.message.len() > WORDS_V1 * 4 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let invocation = F(u64::from(manifest.role.slot()));
        for index in 0..WORDS_V1 {
            let mut value = zeroize::Zeroizing::new(F::ZERO);
            for byte in 0..4 {
                *value = value.mul(F(256)).add(F(u64::from(
                    witness.message.get(4 * index + byte).copied().unwrap_or(0),
                )));
            }
            for lane in 0..4 {
                products[lane] = products[lane].mul(hash_bus::factor_v1(
                    invocation,
                    F(hash_bus::PADDED_MESSAGE_KIND_V1),
                    F(index as u64),
                    *value,
                    challenges.lanes[lane],
                ));
            }
        }
        for lane in 0..4 {
            products[lane] = products[lane].mul(hash_bus::factor_v1(
                invocation,
                F(hash_bus::MESSAGE_LENGTH_KIND_V1),
                F::ZERO,
                F(witness.message.len() as u64),
                challenges.lanes[lane],
            ));
        }
        for index in 0..8 {
            let value = zeroize::Zeroizing::new(F(u64::from(u32::from_be_bytes(
                witness.digest[index * 4..index * 4 + 4]
                    .try_into()
                    .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Topology)?,
            ))));
            for lane in 0..4 {
                products[lane] = products[lane].mul(hash_bus::factor_v1(
                    invocation,
                    F(hash_bus::DIGEST_KIND_V1),
                    F(index as u64),
                    *value,
                    challenges.lanes[lane],
                ));
            }
        }
    }
    Ok(products)
}

pub(super) fn push_residues_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509ShaBatchRowV1<A>,
    next: &ZkX509ShaBatchRowV1<A>,
    challenges: ZkX509ShaCallBusChallengesV1,
    residues: &mut impl super::ShaResidueSinkV1<A>,
) {
    let first = current.fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1];
    for lane in 0..4 {
        let factor = row_factor_v1(
            &current.base,
            current.aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1],
            &current.fixed,
            challenges.lanes[lane],
        );
        residues.push(first.mul(next.aux[AUX_START_V1 + lane].sub(A::ONE)));
        residues.push(
            A::ONE.sub(first).mul(
                next.aux[AUX_START_V1 + lane].sub(current.aux[AUX_START_V1 + lane].mul(factor)),
            ),
        );
    }
}
