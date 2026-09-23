//! Assigned canonical openings of outgoing private journal and recovery commitments.
//!
//! The preparation ID, transition digest and sealed recovery bytes still require an authenticated
//! prepared-intent opening. These relations grant no outgoing monetary authority until those
//! sources are wired.

use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1, KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
};

use super::{
    KagemushaPoseidonFieldV1,
    canonical_preimage::{
        assemble_canonical_preimage_v1, assemble_terminal_recovery_frame_v1,
        canonical_compact_length_u14_stream_v1, stream::KagemushaBoundedByteStreamV1,
    },
    guard_bundle::{constant_bytes, digest_limbs_assigned, hash},
    terminal_authorization::KagemushaTerminalPreparedSourceCellsV1,
    terminal_body_commitment::{
        KagemushaAssignedTerminalBodyFieldsV1, KagemushaAuthenticatedTerminalBodyDurableSourcesV1,
        KagemushaAuthenticatedTerminalBodyPrefixV1,
        constrain_terminal_body_commitment_from_sources_v1,
    },
};
use crate::zk::{
    kagemusha_v1_state::terminal_journal_canonical_layout_v1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};

const TERMINAL_JOURNAL_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-private-journal";
const TERMINAL_RECOVERY_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-private-recovery";

/// Prepared-intent fields that must be authenticated elsewhere in the same recursive relation.
///
/// The fields are private and there is no production constructor until the prepared-intent proof
/// exports these assigned cells. A host digest or a fresh witness would not establish provenance.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "prepared-intent recursive opening is not installed"
    )
)]
pub(super) struct KagemushaAssignedTerminalJournalPreparedInputsV1<F: KagemushaPoseidonFieldV1> {
    preparation_id: [AssignedValue<F>; 2],
    candidate_envelope_digest: [AssignedValue<F>; 2],
    state_transition_digest: [AssignedValue<F>; 2],
    outbox_reservation_commitment: [AssignedValue<F>; 2],
    journal_revision_after: AssignedValue<F>,
}

fn digest_bytes_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: [AssignedValue<F>; 2],
) -> Vec<PastaSha256ByteV1<F>> {
    let mut bytes = Vec::with_capacity(32);
    for limb in digest {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), limb, 128);
        bytes.extend(
            bits.chunks_exact(8)
                .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), bits)),
        );
    }
    bytes
}

/// Hash the exact canonical Norito journal preimage and bind it to the terminal-body field.
///
/// The Norito header and padding are pinned by the native prepared-intent model; its CRC64-XZ is
/// derived from the assigned payload. The SHA message uses the same big-endian domain and frame
/// lengths as `canonical_sha256_digest`. An absent prepared-intent source fails before proving.
/// TODO: export the preparation ID, transition digest and journal revision from the verified
/// prepared-intent relation, then construct this input from those cells in both Pasta parities.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "prepared-intent recursive opening is not installed"
    )
)]
pub(super) fn constrain_terminal_journal_commitment_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: Option<&KagemushaAssignedTerminalJournalPreparedInputsV1<F>>,
    terminal_body_journal_commitment: [AssignedValue<F>; 2],
) -> Result<(), String> {
    let prepared = prepared.ok_or_else(|| {
        "terminal journal lacks an authenticated prepared-intent source".to_owned()
    })?;
    let (layout, ranges) = terminal_journal_canonical_layout_v1()
        .map_err(|_| "terminal journal canonical layout changed".to_owned())?;
    let preparation_id = digest_bytes_v1(ctx, range, prepared.preparation_id);
    let candidate = digest_bytes_v1(ctx, range, prepared.candidate_envelope_digest);
    let transition = digest_bytes_v1(ctx, range, prepared.state_transition_digest);
    let reservation = digest_bytes_v1(ctx, range, prepared.outbox_reservation_commitment);
    let revision_bits =
        PastaSha256BitV1::decompose(ctx, range.gate(), prepared.journal_revision_after, 128);
    let revision = revision_bits
        .chunks_exact(8)
        .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), bits))
        .collect::<Vec<_>>();
    let frame = assemble_canonical_preimage_v1(
        ctx,
        range,
        &layout,
        &ranges,
        &[
            &preparation_id,
            &candidate,
            &transition,
            &reservation,
            &revision,
        ],
    )?;
    let frame_len = u64::try_from(frame.len())
        .map_err(|_| "terminal journal canonical frame length exceeds u64".to_owned())?;
    let domain_len = u64::try_from(TERMINAL_JOURNAL_DOMAIN_V1.len())
        .map_err(|_| "terminal journal domain length exceeds u64".to_owned())?;
    let mut message = constant_bytes(&domain_len.to_be_bytes());
    message.extend(constant_bytes(TERMINAL_JOURNAL_DOMAIN_V1));
    message.extend(constant_bytes(&frame_len.to_be_bytes()));
    message.extend(frame);
    let actual = hash(ctx, jobs, message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &actual)
        .into_iter()
        .zip(terminal_body_journal_commitment)
    {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(())
}

/// Authenticated prepared-intent cells required for the sealed recovery commitment.
///
/// The bounded streams must be exported from the same verified prepared-intent relation that
/// supplies the preparation ID and one-use authorization digest. Neither a native digest nor
/// fresh byte witnesses are acceptable substitutes for those cells.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "prepared-intent recursive opening is not installed"
    )
)]
pub(super) struct KagemushaAssignedTerminalRecoveryPreparedInputsV1<F: KagemushaPoseidonFieldV1> {
    preparation_id: [AssignedValue<F>; 2],
    prepared_one_use_authorization_digest: [AssignedValue<F>; 2],
    sealed_transition_inputs: KagemushaBoundedByteStreamV1<F>,
    sealed_recovery_seeds: KagemushaBoundedByteStreamV1<F>,
}

/// Streams exported by a verified prepared-intent relation, including their fixed-capacity tails.
///
/// The current terminal fold has no such export. Supplying a second host copy of the prepared
/// bytes would not authenticate them, so production construction remains unavailable.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "verified prepared-intent streams are not exported"
    )
)]
pub(super) struct KagemushaVerifiedTerminalRecoveryStreamsV1<F: KagemushaPoseidonFieldV1> {
    sealed_transition_inputs: KagemushaBoundedByteStreamV1<F>,
    sealed_recovery_seeds: KagemushaBoundedByteStreamV1<F>,
}

fn constrain_terminal_recovery_stream_identity_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    prepared: &KagemushaAssignedTerminalRecoveryPreparedInputsV1<F>,
    verified: Option<&KagemushaVerifiedTerminalRecoveryStreamsV1<F>>,
) -> Result<(), String> {
    let verified = verified.ok_or_else(|| {
        "terminal recovery lacks recursively verified sealed stream sources".to_owned()
    })?;
    for (prepared, verified) in [
        (
            &prepared.sealed_transition_inputs,
            &verified.sealed_transition_inputs,
        ),
        (
            &prepared.sealed_recovery_seeds,
            &verified.sealed_recovery_seeds,
        ),
    ] {
        if prepared.bytes().len() != verified.bytes().len() {
            return Err("terminal recovery verified stream capacity mismatch".to_owned());
        }
        ctx.constrain_equal(&prepared.actual_len(), &verified.actual_len());
        for (prepared_byte, verified_byte) in prepared.bytes().iter().zip(verified.bytes()) {
            let difference = range.gate().sub(
                ctx,
                prepared_byte.quantum_cell(),
                verified_byte.quantum_cell(),
            );
            range.gate().assert_is_const(ctx, &difference, &F::ZERO);
        }
    }
    Ok(())
}

fn fixed_stream_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let len = u64::try_from(bytes.len())
        .map_err(|_| "terminal recovery fixed stream length exceeds u64".to_owned())?;
    let len = ctx.load_constant(F::from(len));
    KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, len)
}

fn sequence_count_stream_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    count: AssignedValue<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let bits = PastaSha256BitV1::decompose(ctx, range.gate(), count, 64);
    let bytes = bits
        .chunks_exact(8)
        .map(|byte| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), byte))
        .collect::<Vec<_>>();
    fixed_stream_v1(ctx, range, bytes)
}

fn append_stream_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    left: KagemushaBoundedByteStreamV1<F>,
    right: &KagemushaBoundedByteStreamV1<F>,
) -> Result<KagemushaBoundedByteStreamV1<F>, String> {
    let capacity = left
        .bytes()
        .len()
        .checked_add(right.bytes().len())
        .ok_or_else(|| "terminal recovery stream capacity overflow".to_owned())?;
    left.concat(ctx, range, right, capacity)
}

fn bounded_hash_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    message: &KagemushaBoundedByteStreamV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let words =
        jobs.digest_bounded_constrained(ctx, range, message.bytes(), message.actual_len())?;
    let mut bytes = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    Ok(bytes.try_into().expect("SHA-256 digest width"))
}

fn constrain_terminal_recovery_with_capacity_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: &KagemushaAssignedTerminalRecoveryPreparedInputsV1<F>,
    expected: [AssignedValue<F>; 2],
    transition_capacity: usize,
    seeds_capacity: usize,
) -> Result<(), String> {
    if transition_capacity == 0
        || seeds_capacity == 0
        || transition_capacity > KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize
        || seeds_capacity > KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize
        || prepared.sealed_transition_inputs.bytes().len() != transition_capacity
        || prepared.sealed_recovery_seeds.bytes().len() != seeds_capacity
    {
        return Err("terminal recovery fixed stream capacity disagrees with profile".to_owned());
    }
    let gate = range.gate();
    for length in [
        prepared.sealed_transition_inputs.actual_len(),
        prepared.sealed_recovery_seeds.actual_len(),
    ] {
        let is_zero = gate.is_zero(ctx, length);
        gate.assert_is_const(ctx, &is_zero, &F::ZERO);
    }

    let preparation_id = digest_bytes_v1(ctx, range, prepared.preparation_id);
    let authorization = digest_bytes_v1(ctx, range, prepared.prepared_one_use_authorization_digest);
    // A Norito Vec<u8> field is length-prefixed as a whole. Its payload begins with a fixed
    // little-endian u64 sequence count, then the raw bytes. The compact field length is 8+count.
    let transition_count = prepared.sealed_transition_inputs.actual_len();
    let transition_payload_length =
        gate.add(ctx, transition_count, QuantumCell::Constant(F::from(8)));
    let transition_length =
        canonical_compact_length_u14_stream_v1(ctx, range, transition_payload_length)?;
    let transition_count = sequence_count_stream_v1(ctx, range, transition_count)?;
    let seeds_count = prepared.sealed_recovery_seeds.actual_len();
    let seeds_payload_length = gate.add(ctx, seeds_count, QuantumCell::Constant(F::from(8)));
    let seeds_length = canonical_compact_length_u14_stream_v1(ctx, range, seeds_payload_length)?;
    let seeds_count = sequence_count_stream_v1(ctx, range, seeds_count)?;
    let mut payload = fixed_stream_v1(ctx, range, constant_bytes(&[32]))?;
    let preparation_id = fixed_stream_v1(ctx, range, preparation_id)?;
    payload = append_stream_v1(ctx, range, payload, &preparation_id)?;
    let digest_prefix = fixed_stream_v1(ctx, range, constant_bytes(&[32]))?;
    payload = append_stream_v1(ctx, range, payload, &digest_prefix)?;
    let authorization = fixed_stream_v1(ctx, range, authorization)?;
    payload = append_stream_v1(ctx, range, payload, &authorization)?;
    payload = append_stream_v1(ctx, range, payload, &transition_length)?;
    payload = append_stream_v1(ctx, range, payload, &transition_count)?;
    payload = append_stream_v1(ctx, range, payload, &prepared.sealed_transition_inputs)?;
    payload = append_stream_v1(ctx, range, payload, &seeds_length)?;
    payload = append_stream_v1(ctx, range, payload, &seeds_count)?;
    payload = append_stream_v1(ctx, range, payload, &prepared.sealed_recovery_seeds)?;
    let frame = assemble_terminal_recovery_frame_v1(ctx, range, &payload)?;

    let frame_length_bits = PastaSha256BitV1::decompose(ctx, gate, frame.actual_len(), 64);
    let frame_length_bytes = (0..8)
        .map(|index| {
            let start = (7 - index) * 8;
            PastaSha256ByteV1::from_bits_le(ctx, gate, &frame_length_bits[start..start + 8])
        })
        .collect::<Vec<_>>();
    let domain_len = u64::try_from(TERMINAL_RECOVERY_DOMAIN_V1.len())
        .map_err(|_| "terminal recovery domain length exceeds u64".to_owned())?;
    let mut prefix = Vec::with_capacity(16 + TERMINAL_RECOVERY_DOMAIN_V1.len());
    prefix.extend(constant_bytes(&domain_len.to_be_bytes()));
    prefix.extend(constant_bytes(TERMINAL_RECOVERY_DOMAIN_V1));
    prefix.extend(frame_length_bytes);
    let prefix = fixed_stream_v1(ctx, range, prefix)?;
    let message = append_stream_v1(ctx, range, prefix, &frame)?;
    let digest = bounded_hash_v1(ctx, range, jobs, &message)?;
    for (actual, claimed) in digest_limbs_assigned(ctx, &digest)
        .into_iter()
        .zip(expected)
    {
        ctx.constrain_equal(&actual, &claimed);
    }
    Ok(())
}

/// Constrain the exact canonical private-recovery commitment from authenticated prepared cells.
///
/// Both sealed streams have the V1 maximum capacity regardless of their active lengths, so key
/// shape cannot depend on a witness. Both active lengths and all fixed-capacity bytes must match
/// verified prepared-intent sources before the SHA job is queued. Missing authority fails closed.
/// TODO: export the prepared-intent proof's exact ID, one-use digest and sealed byte streams
/// into the recursive relation in both Pasta parities.
pub(super) fn constrain_terminal_recovery_commitment_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: Option<&KagemushaAssignedTerminalRecoveryPreparedInputsV1<F>>,
    verified_streams: Option<&KagemushaVerifiedTerminalRecoveryStreamsV1<F>>,
    terminal_body_recovery_commitment: [AssignedValue<F>; 2],
) -> Result<(), String> {
    let prepared = prepared.ok_or_else(|| {
        "terminal recovery lacks an authenticated prepared-intent source".to_owned()
    })?;
    constrain_terminal_recovery_stream_identity_v1(ctx, range, prepared, verified_streams)?;
    constrain_terminal_recovery_with_capacity_v1(
        ctx,
        range,
        jobs,
        prepared,
        terminal_body_recovery_commitment,
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize,
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize,
    )
}

/// Complete outgoing terminal opening required by the production monetary relation.
///
/// The fields are private and there is deliberately no production constructor: an opening may
/// only be added here after the prepared-intent fields, selected terminal body, and signed
/// certificate digest have been exported as cells from recursively verified sources. Host-assigned
/// values cannot construct this authority. Experimental testnet construction does not call this
/// production-only helper while the prepared-intent proof is unavailable.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "authenticated outgoing opening is not exported")
)]
pub(super) struct KagemushaAuthenticatedTerminalRecoveryOpeningV1<F: KagemushaPoseidonFieldV1> {
    journal_prepared: KagemushaAssignedTerminalJournalPreparedInputsV1<F>,
    recovery_prepared: KagemushaAssignedTerminalRecoveryPreparedInputsV1<F>,
    prepared_sources: KagemushaTerminalPreparedSourceCellsV1<F>,
    /// Both fixed-capacity streams from the same verified prepared-intent relation.
    verified_recovery_streams: Option<KagemushaVerifiedTerminalRecoveryStreamsV1<F>>,
    body: KagemushaAssignedTerminalBodyFieldsV1<F>,
    prefix: KagemushaAuthenticatedTerminalBodyPrefixV1<F>,
    certificate_commitment: [AssignedValue<F>; 2],
}

fn constrain_terminal_prepared_opening_identity_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    sources: &KagemushaTerminalPreparedSourceCellsV1<F>,
    preparation_ids: ([AssignedValue<F>; 2], [AssignedValue<F>; 2]),
    candidates: ([AssignedValue<F>; 2], [AssignedValue<F>; 2]),
    reservations: ([AssignedValue<F>; 2], [AssignedValue<F>; 2]),
    journal_transition_digest: [AssignedValue<F>; 2],
    recovery_authorization: [AssignedValue<F>; 2],
    journal_revision_after: AssignedValue<F>,
) -> Result<(), String> {
    let verified_preparation_id = sources.verified_preparation_id.ok_or_else(|| {
        "terminal durable opening lacks a recursively verified preparation ID".to_owned()
    })?;
    let verified_state_transition_digest =
        sources.verified_state_transition_digest.ok_or_else(|| {
            "terminal durable opening lacks a recursively verified transition digest".to_owned()
        })?;
    let terminal = ctx.load_constant(F::ONE);
    ctx.constrain_equal(&sources.terminal_branch, &terminal);
    ctx.constrain_equal(&journal_revision_after, &sources.journal_revision_after);
    for (left, right) in [
        preparation_ids,
        (preparation_ids.0, verified_preparation_id),
        candidates,
        (candidates.1, sources.candidate_envelope_digest),
        reservations,
        (reservations.1, sources.outbox_reservation_commitment),
        (journal_transition_digest, verified_state_transition_digest),
        (
            recovery_authorization,
            digest_limbs_assigned(ctx, &sources.prepared_one_use_authorization_digest),
        ),
    ] {
        for (left, right) in left.into_iter().zip(right) {
            ctx.constrain_equal(&left, &right);
        }
    }
    Ok(())
}

/// Require both durable openings and the entire signed canonical terminal-body SHA relation.
///
/// The journal and recovery preparation IDs must be identical. The journal's candidate and
/// reservation cells must match the terminal prefix, and its transition digest must match the
/// verified prepared intent. This helper remains outside the live experimental state builder
/// until a verified prepared-intent proof exports every source cell.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "production prepared-intent fold remains closed")
)]
pub(super) fn constrain_outgoing_terminal_recovery_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    opening: Option<&KagemushaAuthenticatedTerminalRecoveryOpeningV1<F>>,
) -> Result<(), String> {
    let opening = opening.ok_or_else(|| {
        "outgoing terminal recovery lacks authenticated prepared-intent and body sources".to_owned()
    })?;
    let verified_recovery_streams =
        opening.verified_recovery_streams.as_ref().ok_or_else(|| {
            "terminal recovery lacks recursively verified sealed stream sources".to_owned()
        })?;
    constrain_terminal_prepared_opening_identity_v1(
        ctx,
        &opening.prepared_sources,
        (
            opening.journal_prepared.preparation_id,
            opening.recovery_prepared.preparation_id,
        ),
        (
            opening.journal_prepared.candidate_envelope_digest,
            opening.prefix.candidate_envelope_digest,
        ),
        (
            opening.journal_prepared.outbox_reservation_commitment,
            opening.prefix.derived_outbox_reservation_commitment,
        ),
        opening.journal_prepared.state_transition_digest,
        opening
            .recovery_prepared
            .prepared_one_use_authorization_digest,
        opening.journal_prepared.journal_revision_after,
    )?;
    constrain_terminal_journal_commitment_v1(
        ctx,
        range,
        jobs,
        Some(&opening.journal_prepared),
        opening.body.private_journal_commitment,
    )?;
    constrain_terminal_recovery_commitment_v1(
        ctx,
        range,
        jobs,
        Some(&opening.recovery_prepared),
        Some(verified_recovery_streams),
        opening.body.private_recovery_commitment,
    )?;
    let durable = KagemushaAuthenticatedTerminalBodyDurableSourcesV1 {
        private_journal_commitment: opening.body.private_journal_commitment,
        private_recovery_commitment: opening.body.private_recovery_commitment,
    };
    constrain_terminal_body_commitment_from_sources_v1(
        ctx,
        range,
        jobs,
        &opening.body,
        &opening.prefix,
        Some(&durable),
        opening.certificate_commitment,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_recursion::guard_bundle::assign_bytes;
    use crate::zk::{
        kagemusha_v1_poseidon::digest_limbs,
        kagemusha_v1_state::{terminal_journal_commitment_v1, terminal_recovery_commitment_v1},
        pasta_sha256::PastaSha256ConfigV1,
    };
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };

    const K: u32 = 16;
    const UNUSABLE_ROWS: usize = 9;
    const REVISION: u128 = (1_u128 << 96) + 0x0102_0304_0506_0708;

    #[test]
    fn outgoing_recovery_requires_authenticated_sources_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(K as usize)
                .use_lookup_bits(K as usize - 1)
                .use_instance_columns(1);
            let range = builder.range_chip();
            let mut jobs = PastaSha256JobsV1::default();
            let error = constrain_outgoing_terminal_recovery_opening_v1(
                builder.main(0),
                &range,
                &mut jobs,
                None,
            )
            .expect_err("missing verified prepared intent must close outgoing construction");
            assert_eq!(
                error,
                "outgoing terminal recovery lacks authenticated prepared-intent and body sources"
            );
            assert_eq!(jobs.compression_blocks().expect("unchanged SHA queue"), 0);
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[test]
    fn prepared_terminal_openings_reject_unverified_sources_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>(mutation: usize) -> bool {
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(9)
                .use_lookup_bits(8)
                .use_instance_columns(1);
            let ctx = builder.main(0);
            let assign = |ctx: &mut Context<F>, pair: [u64; 2]| {
                pair.map(|limb| ctx.load_witness(F::from(limb)))
            };
            let journal_preparation = assign(ctx, [1, 2]);
            let recovery_preparation = assign(ctx, [1 + u64::from(mutation == 1), 2]);
            let journal_candidate = assign(ctx, [3, 4]);
            let prefix_candidate = assign(ctx, [3 + u64::from(mutation == 2), 4]);
            let journal_reservation = assign(ctx, [5, 6]);
            let prefix_reservation = assign(ctx, [5 + u64::from(mutation == 3), 6]);
            let journal_transition = assign(ctx, [11, 12]);
            let verified_transition = (mutation != 11).then(|| {
                assign(
                    ctx,
                    [
                        11 + u64::from(mutation == 12),
                        12 + u64::from(mutation == 13),
                    ],
                )
            });
            let authorization_bytes: [PastaSha256ByteV1<F>; 32] =
                constant_bytes(&[7; 32]).try_into().expect("digest width");
            let mut recovery_authorization = digest_limbs_assigned(ctx, &authorization_bytes);
            if mutation == 4 {
                recovery_authorization[0] = ctx.load_witness(F::ZERO);
            }
            let journal_revision_after = ctx.load_witness(F::from(9 + u64::from(mutation == 5)));
            let verified_preparation_id =
                (mutation != 9).then(|| assign(ctx, [1 + u64::from(mutation == 10), 2]));
            let sources = KagemushaTerminalPreparedSourceCellsV1 {
                terminal_branch: ctx.load_witness(F::from(u64::from(mutation != 6))),
                verified_preparation_id,
                // The isolated equality test supplies synthetic verified cells; production
                // installs these only from the recursively verified State candidate column.
                verified_state_transition_digest: verified_transition,
                candidate_envelope_digest: assign(ctx, [3 + u64::from(mutation == 7), 4]),
                outbox_reservation_commitment: assign(ctx, [5 + u64::from(mutation == 8), 6]),
                prepared_one_use_authorization_digest: authorization_bytes,
                journal_revision_after: ctx.load_witness(F::from(9)),
            };
            let result = constrain_terminal_prepared_opening_identity_v1(
                ctx,
                &sources,
                (journal_preparation, recovery_preparation),
                (journal_candidate, prefix_candidate),
                (journal_reservation, prefix_reservation),
                journal_transition,
                recovery_authorization,
                journal_revision_after,
            );
            if mutation == 9 {
                assert_eq!(
                    result.expect_err("missing recursively verified ID must fail closed"),
                    "terminal durable opening lacks a recursively verified preparation ID"
                );
                return false;
            }
            if mutation == 11 {
                assert_eq!(
                    result.expect_err("missing verified transition digest must fail closed"),
                    "terminal durable opening lacks a recursively verified transition digest"
                );
                return false;
            }
            result.expect("synthetic verified ID is present in this isolated relation test");
            builder.assigned_instances = vec![Vec::new()];
            builder.calculate_params(Some(UNUSABLE_ROWS));
            MockProver::run(9, &builder, vec![Vec::new()])
                .expect("prepared terminal opening identity circuit")
                .verify()
                .is_ok()
        }
        for mutation in 0..=13 {
            for pass in [check::<Fp>(mutation), check::<Fq>(mutation)] {
                assert_eq!(pass, mutation == 0, "mutation {mutation}");
            }
        }
    }

    #[test]
    fn verified_recovery_streams_bind_lengths_and_every_byte_in_both_parities() {
        #[derive(Clone, Copy)]
        enum Mutation {
            None,
            Missing,
            TransitionLength,
            TransitionByte,
            SeedsLength,
            SeedsByte,
        }

        fn check<F: KagemushaPoseidonFieldV1>(mutation: Mutation) -> bool {
            const STREAM_K: u32 = 11;
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(STREAM_K as usize)
                .use_lookup_bits(10)
                .use_instance_columns(1);
            let range = builder.range_chip();
            let ctx = builder.main(0);
            let stream = |ctx: &mut Context<F>, bytes: &[u8], len: u64| {
                let bytes = assign_bytes(ctx, &range, bytes);
                let len = ctx.load_witness(F::from(len));
                KagemushaBoundedByteStreamV1::constrain(ctx, &range, bytes, len)
            };
            let prepared = KagemushaAssignedTerminalRecoveryPreparedInputsV1 {
                preparation_id: [ctx.load_witness(F::ONE), ctx.load_witness(F::ONE)],
                prepared_one_use_authorization_digest: [
                    ctx.load_witness(F::ONE),
                    ctx.load_witness(F::ONE),
                ],
                sealed_transition_inputs: stream(ctx, &[0x11, 0, 0, 0], 2)
                    .expect("prepared transition stream"),
                sealed_recovery_seeds: stream(ctx, &[0x33, 0, 0, 0], 2)
                    .expect("prepared seed stream"),
            };
            let transition_byte = if matches!(mutation, Mutation::TransitionByte) {
                0x12
            } else {
                0x11
            };
            let seeds_byte = if matches!(mutation, Mutation::SeedsByte) {
                0x34
            } else {
                0x33
            };
            let verified = KagemushaVerifiedTerminalRecoveryStreamsV1 {
                sealed_transition_inputs: stream(
                    ctx,
                    &[transition_byte, 0, 0, 0],
                    if matches!(mutation, Mutation::TransitionLength) {
                        1
                    } else {
                        2
                    },
                )
                .expect("verified transition stream"),
                sealed_recovery_seeds: stream(
                    ctx,
                    &[seeds_byte, 0, 0, 0],
                    if matches!(mutation, Mutation::SeedsLength) {
                        1
                    } else {
                        2
                    },
                )
                .expect("verified seed stream"),
            };
            let verified = (!matches!(mutation, Mutation::Missing)).then_some(&verified);
            if matches!(mutation, Mutation::Missing) {
                let mut jobs = PastaSha256JobsV1::default();
                let expected = [ctx.load_witness(F::ONE), ctx.load_witness(F::ONE)];
                let result = constrain_terminal_recovery_commitment_v1(
                    ctx,
                    &range,
                    &mut jobs,
                    Some(&prepared),
                    verified,
                    expected,
                );
                assert_eq!(
                    result.expect_err("missing verified streams must fail before hashing"),
                    "terminal recovery lacks recursively verified sealed stream sources"
                );
                assert_eq!(jobs.compression_blocks().expect("no SHA jobs"), 0);
                return false;
            }
            let result =
                constrain_terminal_recovery_stream_identity_v1(ctx, &range, &prepared, verified);
            result.expect("both fixed-capacity verified stream sources are present");
            builder.assigned_instances = vec![Vec::new()];
            builder.calculate_params(Some(UNUSABLE_ROWS));
            MockProver::run(STREAM_K, &builder, vec![Vec::new()])
                .expect("sealed stream identity circuit")
                .verify()
                .is_ok()
        }

        for mutation in [
            Mutation::None,
            Mutation::Missing,
            Mutation::TransitionLength,
            Mutation::TransitionByte,
            Mutation::SeedsLength,
            Mutation::SeedsByte,
        ] {
            for pass in [check::<Fp>(mutation), check::<Fq>(mutation)] {
                assert_eq!(pass, matches!(mutation, Mutation::None));
            }
        }
    }

    #[test]
    fn canonical_journal_frame_and_digest_match_fixed_wire_vector() {
        let (template, ranges) =
            terminal_journal_canonical_layout_v1().expect("canonical journal layout");
        assert_eq!(template.len(), 197);
        assert_eq!(ranges, [49..81, 82..114, 115..147, 148..180, 181..197]);
        // Independently calculated from Norito V1's five compact field lengths, CRC64-XZ,
        // declared nominal schema identity, and the two big-endian domain/frame lengths.
        assert_eq!(
            terminal_journal_commitment_v1([1; 32], [2; 32], [3; 32], [4; 32], REVISION)
                .expect("native journal commitment"),
            [
                0x17, 0x0d, 0x5b, 0x76, 0xbf, 0x14, 0x1a, 0x3f, 0x70, 0x15, 0x31, 0x0f, 0x1d, 0x84,
                0x38, 0xb8, 0xe5, 0xb5, 0x2d, 0xb5, 0xf3, 0x2f, 0x2a, 0xc8, 0x3e, 0xfb, 0xcc, 0x84,
                0xcd, 0x75, 0xb1, 0x95,
            ]
        );
    }

    #[derive(Clone, Debug)]
    struct Config<F: halo2_base::utils::ScalarField> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct TestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for TestCircuit<F> {
        type Config = Config<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable = (1_usize << params.k) - UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable);
            Config {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("journal opening test uses parameterized Base config")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "journal opening Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << self.builder.config_params.k) - UNUSABLE_ROWS,
            )
        }
    }

    #[derive(Clone, Copy)]
    enum Mutation {
        None,
        Preparation,
        Candidate,
        Transition,
        Reservation,
        Revision,
        Missing,
    }

    fn circuit<F: KagemushaPoseidonFieldV1>(mutation: Mutation) -> Result<TestCircuit<F>, String> {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(15);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let digest = |ctx: &mut Context<F>, value: [u8; 32], changed: bool| {
            let mut assigned = digest_limbs::<F>(value).map(|limb| ctx.load_witness(limb));
            if changed {
                assigned[0] = ctx.load_witness(*assigned[0].value() + F::from(1_u64));
            }
            assigned
        };
        let prepared = KagemushaAssignedTerminalJournalPreparedInputsV1 {
            preparation_id: digest(ctx, [1; 32], matches!(mutation, Mutation::Preparation)),
            candidate_envelope_digest: digest(
                ctx,
                [2; 32],
                matches!(mutation, Mutation::Candidate),
            ),
            state_transition_digest: digest(ctx, [3; 32], matches!(mutation, Mutation::Transition)),
            outbox_reservation_commitment: digest(
                ctx,
                [4; 32],
                matches!(mutation, Mutation::Reservation),
            ),
            journal_revision_after: ctx.load_witness(
                F::from_u128(REVISION) + F::from(u64::from(matches!(mutation, Mutation::Revision))),
            ),
        };
        let expected = terminal_journal_commitment_v1([1; 32], [2; 32], [3; 32], [4; 32], REVISION)
            .map_err(|_| "native journal commitment failed".to_owned())?;
        let expected = digest(ctx, expected, false);
        let mut jobs = PastaSha256JobsV1::default();
        constrain_terminal_journal_commitment_v1(
            ctx,
            &range,
            &mut jobs,
            (!matches!(mutation, Mutation::Missing)).then_some(&prepared),
            expected,
        )?;
        assert_eq!(jobs.compression_blocks().expect("journal SHA geometry"), 5);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        Ok(TestCircuit { builder, jobs })
    }

    #[test]
    fn exact_canonical_journal_opening_rejects_each_field_mutation_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            MockProver::run(
                K,
                &circuit::<F>(Mutation::None).expect("journal sources"),
                vec![],
            )
            .expect("journal mock prover")
            .assert_satisfied();
            assert!(circuit::<F>(Mutation::Missing).is_err());
            for mutation in [
                Mutation::Preparation,
                Mutation::Candidate,
                Mutation::Transition,
                Mutation::Reservation,
                Mutation::Revision,
            ] {
                assert!(
                    MockProver::run(K, &circuit::<F>(mutation).expect("mutated source"), vec![])
                        .expect("mutated journal mock prover")
                        .verify()
                        .is_err()
                );
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    #[derive(Clone, Copy)]
    enum RecoveryMutation {
        None,
        Preparation,
        Authorization,
        TransitionByte,
        TransitionLength,
        SeedsByte,
        SeedsLength,
        Missing,
    }

    fn recovery_circuit<F: KagemushaPoseidonFieldV1>(
        mutation: RecoveryMutation,
    ) -> Result<TestCircuit<F>, String> {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(15);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let assign_digest = |ctx: &mut Context<F>, value: [u8; 32], changed: bool| {
            let mut assigned = digest_limbs::<F>(value).map(|limb| ctx.load_witness(limb));
            if changed {
                assigned[0] = ctx.load_witness(*assigned[0].value() + F::from(1_u64));
            }
            assigned
        };
        let mut transition = [0x11_u8, 0x22, 0, 0];
        let mut seeds = [0x33_u8, 0x44, 0x55, 0];
        if matches!(mutation, RecoveryMutation::TransitionByte) {
            transition[0] ^= 1;
        }
        if matches!(mutation, RecoveryMutation::SeedsByte) {
            seeds[0] ^= 1;
        }
        let transition_len: u64 = if matches!(mutation, RecoveryMutation::TransitionLength) {
            transition[1] = 0;
            1
        } else {
            2
        };
        let seeds_len: u64 = if matches!(mutation, RecoveryMutation::SeedsLength) {
            seeds[2] = 0;
            2
        } else {
            3
        };
        let transition_bytes = assign_bytes(ctx, &range, &transition);
        let transition_length = ctx.load_witness(F::from(transition_len));
        let transition = KagemushaBoundedByteStreamV1::constrain(
            ctx,
            &range,
            transition_bytes,
            transition_length,
        )?;
        let seeds_bytes = assign_bytes(ctx, &range, &seeds);
        let seeds_length = ctx.load_witness(F::from(seeds_len));
        let seeds =
            KagemushaBoundedByteStreamV1::constrain(ctx, &range, seeds_bytes, seeds_length)?;
        let prepared = KagemushaAssignedTerminalRecoveryPreparedInputsV1 {
            preparation_id: assign_digest(
                ctx,
                [1; 32],
                matches!(mutation, RecoveryMutation::Preparation),
            ),
            prepared_one_use_authorization_digest: assign_digest(
                ctx,
                [2; 32],
                matches!(mutation, RecoveryMutation::Authorization),
            ),
            sealed_transition_inputs: transition,
            sealed_recovery_seeds: seeds,
        };
        let expected =
            terminal_recovery_commitment_v1([1; 32], [2; 32], &[0x11, 0x22], &[0x33, 0x44, 0x55])
                .map_err(|_| "native recovery commitment failed".to_owned())?;
        let expected = assign_digest(ctx, expected, false);
        let mut jobs = PastaSha256JobsV1::default();
        if matches!(mutation, RecoveryMutation::Missing) {
            assert!(
                constrain_terminal_recovery_commitment_v1(
                    ctx, &range, &mut jobs, None, None, expected,
                )
                .is_err()
            );
            return Err(
                "terminal recovery lacks an authenticated prepared-intent source".to_owned(),
            );
        }
        constrain_terminal_recovery_with_capacity_v1(
            ctx, &range, &mut jobs, &prepared, expected, 4, 4,
        )?;
        assert_eq!(jobs.compression_blocks().expect("recovery SHA geometry"), 4);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        Ok(TestCircuit { builder, jobs })
    }

    #[test]
    fn bounded_canonical_recovery_opening_rejects_field_and_length_mutations_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            MockProver::run(
                K,
                &recovery_circuit::<F>(RecoveryMutation::None).expect("recovery sources"),
                vec![],
            )
            .expect("recovery mock prover")
            .assert_satisfied();
            assert!(recovery_circuit::<F>(RecoveryMutation::Missing).is_err());
            for mutation in [
                RecoveryMutation::Preparation,
                RecoveryMutation::Authorization,
                RecoveryMutation::TransitionByte,
                RecoveryMutation::TransitionLength,
                RecoveryMutation::SeedsByte,
                RecoveryMutation::SeedsLength,
            ] {
                assert!(
                    MockProver::run(
                        K,
                        &recovery_circuit::<F>(mutation).expect("mutated recovery source"),
                        vec![],
                    )
                    .expect("mutated recovery mock prover")
                    .verify()
                    .is_err()
                );
            }
        }
        check::<Fp>();
        check::<Fq>();
    }
}
