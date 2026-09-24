//! Assigned canonical openings of outgoing private journal and recovery commitments.
//!
//! The State proof carries the preparation ID and two sealed-stream digests. The outgoing opening
//! hashes exact bytes and the preparation transcript against those carriers, then derives the
//! durable and body commitments. Its live typed claim still requires complete proof and release
//! qualification before these relations grant production outgoing authority.

use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1, KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1,
    KAGEMUSHA_WIRE_VERSION_V1,
};

use super::{
    KagemushaPoseidonFieldV1,
    canonical_preimage::{
        assemble_canonical_preimage_v1, assemble_terminal_recovery_frame_v1,
        canonical_compact_length_u14_stream_v1, stream::KagemushaBoundedByteStreamV1,
    },
    guard_bundle::{
        KagemushaAssignedGuardBundleV1, assign_bytes, constant_bytes, digest_limbs_assigned, hash,
    },
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

pub(super) const TERMINAL_JOURNAL_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:terminal-private-journal";
pub(super) const TERMINAL_RECOVERY_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:terminal-private-recovery";
pub(super) const SEALED_TRANSITION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:sealed-transition-inputs";
pub(super) const SEALED_RECOVERY_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:sealed-recovery-seeds";
pub(super) const PREPARATION_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:outgoing-preparation";

/// Prepared-intent fields that must be authenticated in the same terminal relation.
///
/// This input is assembled only after the ID transcript opens its State candidate carrier in the
/// same terminal relation. A host digest or fresh witness does not establish provenance.
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

/// Hash the exact canonical Norito journal preimage from assigned prepared-intent cells.
///
/// The Norito header and padding are pinned by the native prepared-intent model; its CRC64-XZ is
/// derived from the assigned payload. The SHA message uses the same big-endian domain and frame
/// lengths as `canonical_sha256_digest`. An absent prepared-intent source fails before proving.
fn hash_terminal_journal_commitment_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: Option<&KagemushaAssignedTerminalJournalPreparedInputsV1<F>>,
) -> Result<[AssignedValue<F>; 2], String> {
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
    Ok(digest_limbs_assigned(ctx, &actual))
}

/// Bind the canonical journal SHA output to an existing terminal-body commitment in unit tests.
#[cfg(test)]
pub(super) fn constrain_terminal_journal_commitment_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: Option<&KagemushaAssignedTerminalJournalPreparedInputsV1<F>>,
    terminal_body_journal_commitment: [AssignedValue<F>; 2],
) -> Result<(), String> {
    for (actual, expected) in hash_terminal_journal_commitment_v1(ctx, range, jobs, prepared)?
        .into_iter()
        .zip(terminal_body_journal_commitment)
    {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(())
}

/// Private prepared-intent cells required for the sealed recovery commitment.
///
/// The bounded byte streams gain authority only when the same terminal circuit hashes them
/// against the verified State candidate's digest carriers. The preparation ID additionally
/// requires its exact transcript opening; fresh witness cells alone do not establish provenance.
pub(super) struct KagemushaAssignedTerminalRecoveryPreparedInputsV1<F: KagemushaPoseidonFieldV1> {
    preparation_id: [AssignedValue<F>; 2],
    prepared_one_use_authorization_digest: [AssignedValue<F>; 2],
    sealed_transition_inputs: KagemushaBoundedByteStreamV1<F>,
    sealed_recovery_seeds: KagemushaBoundedByteStreamV1<F>,
}

/// A second stream copy for isolated equality tests, including fixed-capacity tails.
///
/// The production opening hashes exact bytes against State candidate digest carriers instead of
/// trusting another host copy. This type is deliberately unavailable to production code.
#[cfg(test)]
pub(super) struct KagemushaVerifiedTerminalRecoveryStreamsV1<F: KagemushaPoseidonFieldV1> {
    sealed_transition_inputs: KagemushaBoundedByteStreamV1<F>,
    sealed_recovery_seeds: KagemushaBoundedByteStreamV1<F>,
}

#[cfg(test)]
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

/// Open one length-prefixed sealed stream against a digest carried by a verified State proof.
///
/// The expected cells must be taken from the candidate column only after proof and history-fold
/// verification. This helper alone cannot authenticate its caller or the preparation-ID
/// transcript. The outgoing terminal relation queues this bounded job with the preparation-ID
/// transcript; complete typed-claim and fixed k=16 qualification remain required.
fn constrain_sealed_stream_digest_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    stream: &KagemushaBoundedByteStreamV1<F>,
    verified_candidate_digest: Option<[AssignedValue<F>; 2]>,
    domain: &[u8],
    capacity: usize,
) -> Result<(), String> {
    let expected = verified_candidate_digest
        .ok_or_else(|| "sealed stream lacks a recursively verified candidate digest".to_owned())?;
    let maximum = if domain == SEALED_TRANSITION_DIGEST_DOMAIN_V1 {
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize
    } else if domain == SEALED_RECOVERY_DIGEST_DOMAIN_V1 {
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize
    } else {
        return Err("sealed stream has an unknown digest domain".to_owned());
    };
    if capacity == 0 || capacity > maximum || stream.bytes().len() != capacity {
        return Err("sealed stream fixed capacity disagrees with profile".to_owned());
    }
    let gate = range.gate();
    let empty = gate.is_zero(ctx, stream.actual_len());
    gate.assert_is_const(ctx, &empty, &F::ZERO);
    let length_bits = PastaSha256BitV1::decompose(ctx, gate, stream.actual_len(), 64);
    let mut prefix = constant_bytes(domain);
    prefix.extend(constant_bytes(&[0]));
    prefix.extend(
        length_bits
            .chunks_exact(8)
            .map(|bits| PastaSha256ByteV1::from_bits_le(ctx, gate, bits)),
    );
    let prefix = fixed_stream_v1(ctx, range, prefix)?;
    let message = append_stream_v1(ctx, range, prefix, stream)?;
    let actual = bounded_hash_v1(ctx, range, jobs, &message)?;
    for (actual, expected) in digest_limbs_assigned(ctx, &actual)
        .into_iter()
        .zip(expected)
    {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(())
}

/// Assign exact prepared outgoing bytes and SHA-open both streams to verified State carriers.
///
/// The producer's bytes are ordinary private witness data, not an attestation. Their authority
/// comes only from the in-circuit length/byte hashing against the recursively verified candidate
/// carriers in the same typed claim. The live outgoing relation invokes this through its complete
/// opening.
fn assign_prepared_outgoing_sealed_streams_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    sources: &KagemushaTerminalPreparedSourceCellsV1<F>,
    producer_bytes: Option<&[Vec<u8>; 2]>,
    transition_capacity: usize,
    recovery_capacity: usize,
) -> Result<[KagemushaBoundedByteStreamV1<F>; 2], String> {
    let producer_bytes = producer_bytes
        .ok_or_else(|| "prepared outgoing lacks sealed-byte producer witness".to_owned())?;
    let candidate = sources.candidate_preparation_transcript.ok_or_else(|| {
        "prepared outgoing lacks a recursively verified State candidate".to_owned()
    })?;
    let carriers = sources.candidate_stream_digest_carriers.ok_or_else(|| {
        "prepared outgoing lacks recursively verified sealed stream carriers".to_owned()
    })?;
    for (bytes, capacity, maximum) in [
        (
            &producer_bytes[0],
            transition_capacity,
            KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize,
        ),
        (
            &producer_bytes[1],
            recovery_capacity,
            KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize,
        ),
    ] {
        if capacity == 0 || capacity > maximum || bytes.is_empty() || bytes.len() > capacity {
            return Err(
                "prepared outgoing sealed-byte witness exceeds its fixed profile".to_owned(),
            );
        }
    }
    let gate = range.gate();
    let send = gate.is_equal(
        ctx,
        candidate.operation_tag,
        QuantumCell::Constant(F::from(2)),
    );
    let redeem = gate.is_equal(
        ctx,
        candidate.operation_tag,
        QuantumCell::Constant(F::from(4)),
    );
    let outgoing = gate.or(ctx, send, redeem);
    gate.assert_is_const(ctx, &outgoing, &F::ONE);
    let mut assigned = Vec::with_capacity(2);
    for (bytes, capacity, carrier, domain) in [
        (
            &producer_bytes[0],
            transition_capacity,
            carriers[0],
            SEALED_TRANSITION_DIGEST_DOMAIN_V1,
        ),
        (
            &producer_bytes[1],
            recovery_capacity,
            carriers[1],
            SEALED_RECOVERY_DIGEST_DOMAIN_V1,
        ),
    ] {
        let mut padded = vec![0_u8; capacity];
        padded[..bytes.len()].copy_from_slice(bytes);
        let assigned_bytes = assign_bytes(ctx, range, &padded);
        let length = ctx.load_witness(F::from(
            u64::try_from(bytes.len()).expect("fixed sealed-stream profile fits u64"),
        ));
        let stream = KagemushaBoundedByteStreamV1::constrain(ctx, range, assigned_bytes, length)?;
        constrain_sealed_stream_digest_v1(
            ctx,
            range,
            jobs,
            &stream,
            Some(carrier),
            domain,
            capacity,
        )?;
        assigned.push(stream);
    }
    Ok(assigned
        .try_into()
        .expect("two prepared outgoing sealed streams"))
}

/// Hash the exact native pre-proof preparation transcript against its State candidate carrier.
///
/// Candidate fields come from the recursively verified State column; the request, reservation,
/// and one-use authorization come from terminal relations. The two lengths must be the active
/// lengths of the streams already SHA-opened against that same candidate. The redemption manifest
/// comes from the terminal public instance fixed by the authenticated release verifier; the
/// caller must fail closed if that source is absent rather than assign a host digest here.
/// The fixed 32-job typed claim and k=16 geometry still require compiled qualification.
fn constrain_preparation_id_transcript_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    sources: &KagemushaTerminalPreparedSourceCellsV1<F>,
    sealed_transition_inputs_len: AssignedValue<F>,
    sealed_recovery_seeds_len: AssignedValue<F>,
) -> Result<[AssignedValue<F>; 2], String> {
    let candidate = sources.candidate_preparation_transcript.ok_or_else(|| {
        "preparation transcript lacks a recursively verified State candidate".to_owned()
    })?;
    let transition = sources.verified_state_transition_digest.ok_or_else(|| {
        "preparation transcript lacks a recursively verified transition digest".to_owned()
    })?;
    let stream_digests = sources.candidate_stream_digest_carriers.ok_or_else(|| {
        "preparation transcript lacks recursively verified sealed stream digests".to_owned()
    })?;
    let manifest = sources.verified_artifact_manifest_digest.ok_or_else(|| {
        "preparation transcript lacks an authenticated redemption artifact manifest".to_owned()
    })?;
    let carrier = sources.candidate_preparation_id_carrier.ok_or_else(|| {
        "preparation transcript lacks a recursively verified preparation ID carrier".to_owned()
    })?;
    let gate = range.gate();
    let send = gate.is_equal(
        ctx,
        candidate.operation_tag,
        QuantumCell::Constant(F::from(2)),
    );
    let redeem = gate.is_equal(
        ctx,
        candidate.operation_tag,
        QuantumCell::Constant(F::from(4)),
    );
    let outgoing = gate.or(ctx, send, redeem);
    gate.assert_is_const(ctx, &outgoing, &F::ONE);
    for (digest, expected_nonzero) in [(manifest, redeem), (sources.request_digest, send)] {
        let low_zero = gate.is_zero(ctx, digest[0]);
        let high_zero = gate.is_zero(ctx, digest[1]);
        let zero = gate.and(ctx, low_zero, high_zero);
        let nonzero = gate.not(ctx, zero);
        ctx.constrain_equal(&nonzero, &expected_nonzero);
    }

    let operation_bits = PastaSha256BitV1::decompose(ctx, gate, candidate.operation_tag, 8);
    let mut transcript = constant_bytes(PREPARATION_ID_DOMAIN_V1);
    transcript.extend(constant_bytes(&[0]));
    transcript.extend(constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()));
    transcript.push(PastaSha256ByteV1::from_bits_le(ctx, gate, &operation_bits));
    for digest in [
        candidate.predecessor_state_commitment,
        candidate.successor_state_commitment,
        transition,
        candidate.prepared_transition_binding_digest,
        candidate.projection_semantic_digest,
        candidate.lifecycle_binding_digest,
        sources.request_digest,
        manifest,
        candidate.normalized_guard_statement_digest,
        sources.outbox_reservation_commitment,
    ] {
        transcript.extend(digest_bytes_v1(ctx, range, digest));
    }
    transcript.extend(sources.prepared_one_use_authorization_digest);
    for (length, digest) in [
        (sealed_transition_inputs_len, stream_digests[0]),
        (sealed_recovery_seeds_len, stream_digests[1]),
    ] {
        let bits = PastaSha256BitV1::decompose(ctx, gate, length, 64);
        transcript.extend(
            bits.chunks_exact(8)
                .map(|byte| PastaSha256ByteV1::from_bits_le(ctx, gate, byte)),
        );
        transcript.extend(digest_bytes_v1(ctx, range, digest));
    }
    let message = fixed_stream_v1(ctx, range, transcript)?;
    let actual = bounded_hash_v1(ctx, range, jobs, &message)?;
    let actual_limbs = digest_limbs_assigned(ctx, &actual);
    for (actual, expected) in actual_limbs.iter().copied().zip(carrier) {
        ctx.constrain_equal(&actual, &expected);
    }
    Ok(actual_limbs)
}

fn hash_terminal_recovery_with_capacity_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: &KagemushaAssignedTerminalRecoveryPreparedInputsV1<F>,
    transition_capacity: usize,
    seeds_capacity: usize,
) -> Result<[AssignedValue<F>; 2], String> {
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
    Ok(digest_limbs_assigned(ctx, &digest))
}

#[cfg(test)]
fn constrain_terminal_recovery_with_capacity_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    prepared: &KagemushaAssignedTerminalRecoveryPreparedInputsV1<F>,
    expected: [AssignedValue<F>; 2],
    transition_capacity: usize,
    seeds_capacity: usize,
) -> Result<(), String> {
    for (actual, claimed) in hash_terminal_recovery_with_capacity_v1(
        ctx,
        range,
        jobs,
        prepared,
        transition_capacity,
        seeds_capacity,
    )?
    .into_iter()
    .zip(expected)
    {
        ctx.constrain_equal(&actual, &claimed);
    }
    Ok(())
}

/// Exercise the canonical private-recovery commitment with a second stream copy in tests.
///
/// The production opening hashes both exact streams against recursively verified candidate
/// carriers and derives the recovery SHA output directly. Comparing two host copies does not
/// authenticate either one, so this helper is unavailable in production.
#[cfg(test)]
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
/// The source cells come from the verified State, Guard and terminal relations. The exact sealed
/// bytes are private producer witnesses and gain authority only through the six queued SHA jobs.
/// The live outgoing terminal relation calls this helper after candidate verification.
pub(super) struct KagemushaAuthenticatedTerminalRecoveryOpeningV1<'a, F: KagemushaPoseidonFieldV1> {
    outgoing_sealed_streams: &'a [Vec<u8>; 2],
    prepared_sources: KagemushaTerminalPreparedSourceCellsV1<F>,
    prefix: KagemushaAuthenticatedTerminalBodyPrefixV1<F>,
}

impl<'a, F: KagemushaPoseidonFieldV1> KagemushaAuthenticatedTerminalRecoveryOpeningV1<'a, F> {
    /// Retain only already-assigned proof sources and exact outgoing producer bytes.
    ///
    /// The caller must invoke this after recursively verifying State and Guard and before
    /// consuming the terminal SHA claim. No host-computed journal, recovery or body digest is
    /// accepted; those three values are derived by the six-job opening below.
    pub(super) fn from_verified_assigned_sources_v1(
        prepared_sources: KagemushaTerminalPreparedSourceCellsV1<F>,
        guard: &KagemushaAssignedGuardBundleV1<F>,
        outgoing_sealed_streams: Option<&'a [Vec<u8>; 2]>,
    ) -> Result<Self, String> {
        if prepared_sources.verified_preparation_id.is_some() {
            return Err("outgoing opening cannot preload a verified preparation ID".to_owned());
        }
        let candidate = prepared_sources
            .candidate_preparation_transcript
            .ok_or_else(|| {
                "outgoing opening lacks recursively verified State candidate cells".to_owned()
            })?;
        let state_hardware_profile_id =
            prepared_sources
                .candidate_hardware_profile_id
                .ok_or_else(|| {
                    "outgoing opening lacks recursively verified State hardware profile".to_owned()
                })?;
        let state_policy_epoch = prepared_sources.candidate_policy_epoch.ok_or_else(|| {
            "outgoing opening lacks recursively verified State policy epoch".to_owned()
        })?;
        if prepared_sources.verified_state_transition_digest.is_none()
            || prepared_sources.candidate_preparation_id_carrier.is_none()
            || prepared_sources.candidate_stream_digest_carriers.is_none()
            || prepared_sources.verified_artifact_manifest_digest.is_none()
        {
            return Err(
                "outgoing opening lacks a complete verified candidate transcript".to_owned(),
            );
        }
        let derived = prepared_sources.derived_commit_cells.ok_or_else(|| {
            "outgoing opening lacks assigned terminal certificate cells".to_owned()
        })?;
        let outgoing_sealed_streams = outgoing_sealed_streams.ok_or_else(|| {
            "outgoing opening lacks exact sealed-byte producer witness".to_owned()
        })?;
        for (bytes, maximum) in [
            (
                &outgoing_sealed_streams[0],
                KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize,
            ),
            (
                &outgoing_sealed_streams[1],
                KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize,
            ),
        ] {
            if bytes.is_empty() || bytes.len() > maximum {
                return Err("outgoing opening sealed-byte witness exceeds its profile".to_owned());
            }
        }
        let prefix = KagemushaAuthenticatedTerminalBodyPrefixV1 {
            candidate_envelope_digest: prepared_sources.candidate_envelope_digest,
            state_lifecycle_binding_digest: candidate.lifecycle_binding_digest,
            guard_lifecycle_binding_digest: guard.lifecycle_binding_digest,
            derived_transition_nullifier: derived.transition_nullifier,
            derived_outbox_reservation_commitment: prepared_sources.outbox_reservation_commitment,
            derived_evidence_tag: derived.evidence_tag,
            derived_evidence_commitment: derived.evidence_commitment,
            state_hardware_profile_id,
            guard_hardware_profile_id: guard.hardware_profile_id,
            state_policy_epoch,
            guard_policy_epoch: guard.policy_epoch,
            state_successor_commitment: candidate.successor_state_commitment,
            guard_successor_commitment: guard.successor_state,
        };
        Ok(Self {
            outgoing_sealed_streams,
            prepared_sources,
            prefix,
        })
    }
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
    let candidate_preparation_id_carrier =
        sources.candidate_preparation_id_carrier.ok_or_else(|| {
            "terminal durable opening lacks a recursively verified candidate preparation ID carrier"
                .to_owned()
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
        (preparation_ids.0, candidate_preparation_id_carrier),
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
/// The journal and recovery preparation IDs must be identical and match the candidate carrier.
/// The journal's candidate and reservation cells must match the terminal prefix, its transition
/// digest must match the verified State candidate, and both sealed streams must open their
/// candidate digest carriers. The live outgoing state builder includes this helper. Its complete
/// 32-job typed claim and circuit geometry still require compiled qualification.
pub(super) fn constrain_outgoing_terminal_recovery_opening_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    opening: Option<&KagemushaAuthenticatedTerminalRecoveryOpeningV1<'_, F>>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let opening = opening.ok_or_else(|| {
        "outgoing terminal recovery lacks authenticated prepared-intent and body sources".to_owned()
    })?;
    if opening.prepared_sources.verified_preparation_id.is_some() {
        return Err("outgoing opening cannot preload a verified preparation ID".to_owned());
    }
    // These bounded SHA jobs join the same typed claim as the durable openings. The complete
    // 32-job circuit still needs both-parity proof, geometry and release qualification before
    // its result can authorize a production monetary transition.
    let [sealed_transition_inputs, sealed_recovery_seeds] =
        assign_prepared_outgoing_sealed_streams_v1(
            ctx,
            range,
            jobs,
            &opening.prepared_sources,
            Some(opening.outgoing_sealed_streams),
            KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize,
            KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize,
        )?;
    let prepared_authorization = digest_limbs_assigned(
        ctx,
        &opening
            .prepared_sources
            .prepared_one_use_authorization_digest,
    );
    let opened_preparation_id = constrain_preparation_id_transcript_v1(
        ctx,
        range,
        jobs,
        &opening.prepared_sources,
        sealed_transition_inputs.actual_len(),
        sealed_recovery_seeds.actual_len(),
    )?;
    let recovery_prepared = KagemushaAssignedTerminalRecoveryPreparedInputsV1 {
        preparation_id: opened_preparation_id,
        prepared_one_use_authorization_digest: prepared_authorization,
        sealed_transition_inputs,
        sealed_recovery_seeds,
    };
    // Only this complete in-circuit opening may promote the recursively verified candidate
    // carrier into a verified ID; the input cannot preload that result.
    let mut opened_sources = opening.prepared_sources;
    opened_sources.verified_preparation_id = Some(opened_preparation_id);
    let journal_prepared = KagemushaAssignedTerminalJournalPreparedInputsV1 {
        preparation_id: opened_preparation_id,
        candidate_envelope_digest: opened_sources.candidate_envelope_digest,
        state_transition_digest: opened_sources.verified_state_transition_digest.ok_or_else(
            || "outgoing opening lacks a verified State transition digest".to_owned(),
        )?,
        outbox_reservation_commitment: opened_sources.outbox_reservation_commitment,
        journal_revision_after: opened_sources.journal_revision_after,
    };
    constrain_terminal_prepared_opening_identity_v1(
        ctx,
        &opened_sources,
        (
            journal_prepared.preparation_id,
            recovery_prepared.preparation_id,
        ),
        (
            journal_prepared.candidate_envelope_digest,
            opening.prefix.candidate_envelope_digest,
        ),
        (
            journal_prepared.outbox_reservation_commitment,
            opening.prefix.derived_outbox_reservation_commitment,
        ),
        journal_prepared.state_transition_digest,
        recovery_prepared.prepared_one_use_authorization_digest,
        journal_prepared.journal_revision_after,
    )?;
    let private_journal_commitment =
        hash_terminal_journal_commitment_v1(ctx, range, jobs, Some(&journal_prepared))?;
    let private_recovery_commitment = hash_terminal_recovery_with_capacity_v1(
        ctx,
        range,
        jobs,
        &recovery_prepared,
        KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1 as usize,
        KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1 as usize,
    )?;
    let candidate_bytes = digest_bytes_v1(ctx, range, opening.prefix.candidate_envelope_digest)
        .try_into()
        .expect("candidate envelope digest has 32 bytes");
    let body = KagemushaAssignedTerminalBodyFieldsV1 {
        candidate_envelope_digest: candidate_bytes,
        state_lifecycle_binding_digest: opening.prefix.state_lifecycle_binding_digest,
        guard_lifecycle_binding_digest: opening.prefix.guard_lifecycle_binding_digest,
        transition_nullifier: opening.prefix.derived_transition_nullifier,
        outbox_reservation_commitment: opening.prefix.derived_outbox_reservation_commitment,
        evidence_tag: opening.prefix.derived_evidence_tag,
        evidence_commitment: opening.prefix.derived_evidence_commitment,
        state_hardware_profile_id: opening.prefix.state_hardware_profile_id,
        guard_hardware_profile_id: opening.prefix.guard_hardware_profile_id,
        state_policy_epoch: opening.prefix.state_policy_epoch,
        guard_policy_epoch: opening.prefix.guard_policy_epoch,
        state_successor_commitment: opening.prefix.state_successor_commitment,
        guard_successor_commitment: opening.prefix.guard_successor_commitment,
        private_journal_commitment,
        private_recovery_commitment,
    };
    let durable = KagemushaAuthenticatedTerminalBodyDurableSourcesV1 {
        private_journal_commitment,
        private_recovery_commitment,
    };
    let certificate_commitment = opened_sources
        .derived_commit_cells
        .ok_or_else(|| "outgoing opening lacks assigned terminal certificate cells".to_owned())?
        .hardware_terminal_commitment;
    constrain_terminal_body_commitment_from_sources_v1(
        ctx,
        range,
        jobs,
        &body,
        &opening.prefix,
        Some(&durable),
        certificate_commitment,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::kagemusha_v1_recursion::guard_bundle::assign_bytes;
    use crate::zk::kagemusha_v1_recursion::terminal_body_commitment::constrain_apple_signed_terminal_body_commitment_v1;
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
    fn outgoing_opening_constructor_retains_only_assigned_sources_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            let mut builder = BaseCircuitBuilder::<F>::new(false).use_k(9);
            let ctx = builder.main(0);
            let assign = |ctx: &mut Context<F>, value: u64| ctx.load_witness(F::from(value));
            let zero = ctx.load_constant(F::ZERO);
            let digest = [zero; 2];
            let bytes = [PastaSha256ByteV1::constant(0); 32];
            let guard_lifecycle = [assign(ctx, 8), assign(ctx, 9)];
            let guard_profile = [assign(ctx, 14), assign(ctx, 15)];
            let guard_epoch = assign(ctx, 16);
            let guard_successor = [assign(ctx, 17), assign(ctx, 18)];
            let guard = KagemushaAssignedGuardBundleV1 {
                guard_digest: bytes,
                credential_digests: [bytes; 2],
                credential_issuance_digests: [bytes; 2],
                credential_app_policy_binding_digests: [bytes; 2],
                credential_device_public_keys: [Vec::new(), Vec::new()],
                protocol_version: zero,
                predecessor_suite_id: digest,
                predecessor_vk_digest: digest,
                successor_suite_id: digest,
                successor_vk_digest: digest,
                operation: zero,
                amount: zero,
                peer_credit_id: digest,
                recipient_encryption_key_binding: digest,
                mint_finality_proof_binding_digest: digest,
                predecessor_release_id: digest,
                release_id: digest,
                network_id: digest,
                asset_id: digest,
                asset_incarnation: digest,
                asset_scale: zero,
                liability_pool_id: digest,
                hardware_profile_id: guard_profile,
                policy_epoch: guard_epoch,
                lane_id: digest,
                predecessor_state: digest,
                successor_state: guard_successor,
                predecessor_nonce: digest,
                successor_nonce: digest,
                predecessor_sequence: zero,
                successor_sequence: zero,
                predecessor_generation: zero,
                successor_generation: zero,
                predecessor_epoch: digest,
                successor_epoch: digest,
                predecessor_key: digest,
                successor_key: digest,
                predecessor_policy: digest,
                successor_policy: digest,
                journal_before: zero,
                journal_after: zero,
                lifecycle_binding_digest: guard_lifecycle,
                prepared_transition_binding_digest: digest,
                terminal_commit_binding_digest: digest,
                sender_one_time_authorization_digest: digest,
                receive_credit_binding_digest: digest,
                transition_intent: digest,
                transition_effect: digest,
                recovery_record: digest,
                durable_inbox_effect: digest,
                durable_outbox_effect: digest,
            };
            let state_lifecycle = [assign(ctx, 4), assign(ctx, 5)];
            let state_successor = [assign(ctx, 6), assign(ctx, 7)];
            let state_profile = [assign(ctx, 11), assign(ctx, 12)];
            let state_epoch = assign(ctx, 13);
            let candidate = super::super::terminal_authorization::KagemushaCandidatePreparationTranscriptCellsV1 {
                operation_tag: assign(ctx, 2),
                predecessor_state_commitment: digest,
                successor_state_commitment: state_successor,
                prepared_transition_binding_digest: digest,
                projection_semantic_digest: digest,
                lifecycle_binding_digest: state_lifecycle,
                normalized_guard_statement_digest: digest,
            };
            let sources = KagemushaTerminalPreparedSourceCellsV1 {
                terminal_branch: ctx.load_constant(F::ONE),
                derived_commit_cells: Some(
                    super::super::terminal_authorization::KagemushaTerminalDerivedCommitCellsV1 {
                        transition_nullifier: digest,
                        evidence_tag: zero,
                        evidence_commitment: digest,
                        hardware_terminal_commitment: digest,
                    },
                ),
                verified_preparation_id: None,
                verified_state_transition_digest: Some(digest),
                candidate_preparation_id_carrier: Some(digest),
                candidate_stream_digest_carriers: Some([digest; 2]),
                candidate_preparation_transcript: Some(candidate),
                candidate_hardware_profile_id: Some(state_profile),
                candidate_policy_epoch: Some(state_epoch),
                request_digest: digest,
                verified_artifact_manifest_digest: Some(digest),
                candidate_envelope_digest: digest,
                outbox_reservation_commitment: digest,
                prepared_one_use_authorization_digest: bytes,
                journal_revision_after: zero,
            };
            let streams = [vec![1], vec![2]];
            let opening =
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    sources,
                    &guard,
                    Some(&streams),
                )
                .expect("complete assigned opening sources");
            for (actual, expected) in [
                (opening.prefix.state_lifecycle_binding_digest, [4_u64, 5]),
                (opening.prefix.guard_lifecycle_binding_digest, [8, 9]),
                (opening.prefix.state_hardware_profile_id, [11, 12]),
                (opening.prefix.guard_hardware_profile_id, [14, 15]),
                (opening.prefix.state_successor_commitment, [6, 7]),
                (opening.prefix.guard_successor_commitment, [17, 18]),
            ] {
                for (cell, value) in actual.into_iter().zip(expected) {
                    assert_eq!(*cell.value(), F::from(value));
                }
            }
            assert_eq!(*opening.prefix.state_policy_epoch.value(), F::from(13));
            assert_eq!(*opening.prefix.guard_policy_epoch.value(), F::from(16));
            assert!(std::ptr::eq(opening.outgoing_sealed_streams, &streams));

            let mut missing = sources;
            missing.candidate_hardware_profile_id = None;
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    missing,
                    &guard,
                    Some(&streams),
                )
                .is_err()
            );
            let mut missing = sources;
            missing.candidate_policy_epoch = None;
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    missing,
                    &guard,
                    Some(&streams),
                )
                .is_err()
            );
            let mut missing = sources;
            missing.verified_artifact_manifest_digest = None;
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    missing,
                    &guard,
                    Some(&streams),
                )
                .is_err()
            );
            let mut missing = sources;
            missing.derived_commit_cells = None;
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    missing,
                    &guard,
                    Some(&streams),
                )
                .is_err()
            );
            let mut missing = sources;
            missing.candidate_stream_digest_carriers = None;
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    missing,
                    &guard,
                    Some(&streams),
                )
                .is_err()
            );
            let mut preloaded = sources;
            preloaded.verified_preparation_id = Some(digest);
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    preloaded,
                    &guard,
                    Some(&streams),
                )
                .is_err()
            );
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    sources, &guard, None,
                )
                .is_err()
            );
            let empty = [Vec::new(), vec![2]];
            assert!(
                KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                    sources,
                    &guard,
                    Some(&empty),
                )
                .is_err()
            );
        }
        check::<Fp>();
        check::<Fq>();
    }

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
            let candidate_preparation_id_carrier =
                (mutation != 14).then(|| assign(ctx, [1 + u64::from(mutation == 15), 2]));
            let sources = KagemushaTerminalPreparedSourceCellsV1 {
                terminal_branch: ctx.load_witness(F::from(u64::from(mutation != 6))),
                derived_commit_cells: None,
                verified_preparation_id,
                // The isolated equality test supplies synthetic verified cells; production
                // installs these only from the recursively verified State candidate column.
                verified_state_transition_digest: verified_transition,
                candidate_preparation_id_carrier,
                candidate_stream_digest_carriers: None,
                candidate_preparation_transcript: None,
                candidate_hardware_profile_id: None,
                candidate_policy_epoch: None,
                request_digest: journal_preparation,
                verified_artifact_manifest_digest: None,
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
                assert!(
                    sources.candidate_preparation_id_carrier.is_some(),
                    "carrier-only outgoing opening must exercise the unverified-ID gate"
                );
                assert_eq!(
                    result.expect_err("candidate carrier alone cannot authorize outgoing journal"),
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
            if mutation == 14 {
                assert_eq!(
                    result.expect_err("missing candidate carrier must fail closed"),
                    "terminal durable opening lacks a recursively verified candidate preparation ID carrier"
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
        for mutation in 0..=15 {
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

    #[derive(Clone, Copy, Debug)]
    enum CompleteOutgoingMutation {
        None,
        SealedTransitionByte,
        PreparationCarrier,
        JournalRevision,
        TerminalBodyCommitment,
        SignedTerminalBody,
    }

    fn complete_outgoing_opening_circuit<F: KagemushaPoseidonFieldV1>(
        mutation: CompleteOutgoingMutation,
    ) -> Result<TestCircuit<F>, String> {
        use iroha_data_model::kagemusha::{
            KagemushaCommitEvidenceV1, KagemushaHardwareSelectionSigningLayoutV1,
            KagemushaHardwareTerminalBodyV1, KagemushaTrustedCommitTimeV1,
        };
        use sha2::{Digest as _, Sha256};

        const SEND_PREPARATION_ID: [u8; 32] = [
            0xe4, 0xc4, 0xd7, 0xd7, 0x55, 0xb8, 0x67, 0xf7, 0xb7, 0x09, 0x3c, 0x33, 0xfb, 0xaa,
            0x6c, 0x57, 0x72, 0xbf, 0x9d, 0x9b, 0x1b, 0x04, 0x8b, 0x54, 0xe1, 0x0e, 0x4d, 0x38,
            0xec, 0xf1, 0xdd, 0x4a,
        ];
        fn stream_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update(domain);
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            hasher.finalize().into()
        }

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(K as usize - 1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let assign_digest = |ctx: &mut Context<F>, value: [u8; 32]| {
            digest_limbs::<F>(value).map(|limb| ctx.load_witness(limb))
        };
        let mut streams = [vec![0x11, 0x22, 0x33], vec![0x44, 0x55]];
        let transition_carrier = stream_digest(SEALED_TRANSITION_DIGEST_DOMAIN_V1, &streams[0]);
        let recovery_carrier = stream_digest(SEALED_RECOVERY_DIGEST_DOMAIN_V1, &streams[1]);
        let journal = terminal_journal_commitment_v1(
            SEND_PREPARATION_ID,
            [12; 32],
            [3; 32],
            [9; 32],
            REVISION,
        )
        .map_err(|_| "native complete-opening journal commitment failed".to_owned())?;
        let recovery = terminal_recovery_commitment_v1(
            SEND_PREPARATION_ID,
            [10; 32],
            &streams[0],
            &streams[1],
        )
        .map_err(|_| "native complete-opening recovery commitment failed".to_owned())?;
        let body = KagemushaHardwareTerminalBodyV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            candidate_envelope_digest: [12; 32],
            lifecycle_binding_digest: [6; 32],
            transition_nullifier: [16; 32],
            outbox_reservation_commitment: [9; 32],
            commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
                time_evidence_commitment: [15; 32],
            }),
            hardware_profile_id: [13; 32],
            policy_epoch: 14,
            private_successor_commitment: [2; 32],
            private_journal_commitment: journal,
            private_recovery_commitment: recovery,
        };
        let signed_body_commitment = body
            .canonical_commitment()
            .map_err(|_| "native complete-opening terminal body failed".to_owned())?;
        let mut body_commitment = signed_body_commitment;
        let mut preparation_carrier = SEND_PREPARATION_ID;
        let revision = if matches!(mutation, CompleteOutgoingMutation::JournalRevision) {
            REVISION + 1
        } else {
            REVISION
        };
        match mutation {
            CompleteOutgoingMutation::SealedTransitionByte => streams[0][0] ^= 1,
            CompleteOutgoingMutation::PreparationCarrier => preparation_carrier[0] ^= 1,
            CompleteOutgoingMutation::TerminalBodyCommitment => body_commitment[0] ^= 1,
            CompleteOutgoingMutation::None
            | CompleteOutgoingMutation::JournalRevision
            | CompleteOutgoingMutation::SignedTerminalBody => {}
        }

        let zero = ctx.load_constant(F::ZERO);
        let zero_digest = [zero; 2];
        let zero_bytes = [PastaSha256ByteV1::constant(0); 32];
        let lifecycle = assign_digest(ctx, [6; 32]);
        let successor = assign_digest(ctx, [2; 32]);
        let profile = assign_digest(ctx, [13; 32]);
        let epoch = ctx.load_witness(F::from(14));
        let guard = KagemushaAssignedGuardBundleV1 {
            guard_digest: zero_bytes,
            credential_digests: [zero_bytes; 2],
            credential_issuance_digests: [zero_bytes; 2],
            credential_app_policy_binding_digests: [zero_bytes; 2],
            credential_device_public_keys: [Vec::new(), Vec::new()],
            protocol_version: zero,
            predecessor_suite_id: zero_digest,
            predecessor_vk_digest: zero_digest,
            successor_suite_id: zero_digest,
            successor_vk_digest: zero_digest,
            operation: zero,
            amount: zero,
            peer_credit_id: zero_digest,
            recipient_encryption_key_binding: zero_digest,
            mint_finality_proof_binding_digest: zero_digest,
            predecessor_release_id: zero_digest,
            release_id: zero_digest,
            network_id: zero_digest,
            asset_id: zero_digest,
            asset_incarnation: zero_digest,
            asset_scale: zero,
            liability_pool_id: zero_digest,
            hardware_profile_id: profile,
            policy_epoch: epoch,
            lane_id: zero_digest,
            predecessor_state: zero_digest,
            successor_state: successor,
            predecessor_nonce: zero_digest,
            successor_nonce: zero_digest,
            predecessor_sequence: zero,
            successor_sequence: zero,
            predecessor_generation: zero,
            successor_generation: zero,
            predecessor_epoch: zero_digest,
            successor_epoch: zero_digest,
            predecessor_key: zero_digest,
            successor_key: zero_digest,
            predecessor_policy: zero_digest,
            successor_policy: zero_digest,
            journal_before: zero,
            journal_after: zero,
            lifecycle_binding_digest: lifecycle,
            prepared_transition_binding_digest: zero_digest,
            terminal_commit_binding_digest: zero_digest,
            sender_one_time_authorization_digest: zero_digest,
            receive_credit_binding_digest: zero_digest,
            transition_intent: zero_digest,
            transition_effect: zero_digest,
            recovery_record: zero_digest,
            durable_inbox_effect: zero_digest,
            durable_outbox_effect: zero_digest,
        };
        let candidate =
            super::super::terminal_authorization::KagemushaCandidatePreparationTranscriptCellsV1 {
                operation_tag: ctx.load_witness(F::from(2)),
                predecessor_state_commitment: assign_digest(ctx, [1; 32]),
                successor_state_commitment: successor,
                prepared_transition_binding_digest: assign_digest(ctx, [4; 32]),
                projection_semantic_digest: assign_digest(ctx, [5; 32]),
                lifecycle_binding_digest: lifecycle,
                normalized_guard_statement_digest: assign_digest(ctx, [8; 32]),
            };
        let one_use_bytes: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &[10; 32])
            .try_into()
            .expect("one-use digest width");
        let sources = KagemushaTerminalPreparedSourceCellsV1 {
            terminal_branch: ctx.load_constant(F::ONE),
            derived_commit_cells: Some(
                super::super::terminal_authorization::KagemushaTerminalDerivedCommitCellsV1 {
                    transition_nullifier: assign_digest(ctx, [16; 32]),
                    evidence_tag: zero,
                    evidence_commitment: assign_digest(ctx, [15; 32]),
                    hardware_terminal_commitment: assign_digest(ctx, body_commitment),
                },
            ),
            verified_preparation_id: None,
            verified_state_transition_digest: Some(assign_digest(ctx, [3; 32])),
            candidate_preparation_id_carrier: Some(assign_digest(ctx, preparation_carrier)),
            candidate_stream_digest_carriers: Some([
                assign_digest(ctx, transition_carrier),
                assign_digest(ctx, recovery_carrier),
            ]),
            candidate_preparation_transcript: Some(candidate),
            candidate_hardware_profile_id: Some(profile),
            candidate_policy_epoch: Some(epoch),
            request_digest: assign_digest(ctx, [7; 32]),
            verified_artifact_manifest_digest: Some(zero_digest),
            candidate_envelope_digest: assign_digest(ctx, [12; 32]),
            outbox_reservation_commitment: assign_digest(ctx, [9; 32]),
            prepared_one_use_authorization_digest: one_use_bytes,
            journal_revision_after: ctx.load_witness(F::from_u128(revision)),
        };
        let opening =
            KagemushaAuthenticatedTerminalRecoveryOpeningV1::from_verified_assigned_sources_v1(
                sources,
                &guard,
                Some(&streams),
            )?;
        let mut jobs = PastaSha256JobsV1::default();
        let derived_body = constrain_outgoing_terminal_recovery_opening_v1(
            ctx,
            &range,
            &mut jobs,
            Some(&opening),
        )?;
        // Keep the signed-field check on the bytes derived by the complete outgoing opening.
        // The raw assertion signature and issuer enrollment still need the live recursive fold.
        let mut signed_subject = [0_u8; KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES];
        signed_subject[KagemushaHardwareSelectionSigningLayoutV1::TERMINAL_BODY_COMMITMENT]
            .copy_from_slice(&signed_body_commitment);
        if matches!(mutation, CompleteOutgoingMutation::SignedTerminalBody) {
            signed_subject
                [KagemushaHardwareSelectionSigningLayoutV1::TERMINAL_BODY_COMMITMENT.start + 15] ^=
                1;
        }
        let signed_subject = std::array::from_fn(|index| {
            ctx.load_witness(F::from(u64::from(signed_subject[index])))
        });
        constrain_apple_signed_terminal_body_commitment_v1(
            ctx,
            &range,
            &signed_subject,
            &derived_body,
        );
        assert_eq!(jobs.typed_claim_jobs()?.len(), 6);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        Ok(TestCircuit { builder, jobs })
    }

    #[test]
    fn complete_outgoing_opening_binds_six_sha_jobs_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            MockProver::run(
                K,
                &complete_outgoing_opening_circuit::<F>(CompleteOutgoingMutation::None)
                    .expect("native-consistent complete outgoing opening"),
                vec![],
            )
            .expect("complete outgoing opening mock prover")
            .assert_satisfied();
            for mutation in [
                CompleteOutgoingMutation::SealedTransitionByte,
                CompleteOutgoingMutation::PreparationCarrier,
                CompleteOutgoingMutation::JournalRevision,
                CompleteOutgoingMutation::TerminalBodyCommitment,
                CompleteOutgoingMutation::SignedTerminalBody,
            ] {
                assert!(
                    MockProver::run(
                        K,
                        &complete_outgoing_opening_circuit::<F>(mutation)
                            .expect("mutated complete outgoing opening"),
                        vec![],
                    )
                    .expect("mutated complete outgoing opening mock prover")
                    .verify()
                    .is_err(),
                    "accepted altered complete outgoing source: {mutation:?}"
                );
            }
        }
        check::<Fp>();
        check::<Fq>();
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

    #[derive(Clone, Copy, Debug)]
    enum StreamDigestMutation {
        None,
        TransitionByte,
        TransitionLength,
        TransitionCarrier,
        RecoveryByte,
        RecoveryLength,
        RecoveryCarrier,
        MissingTransitionCarrier,
        MissingRecoveryCarrier,
    }

    fn sealed_stream_digest_test_circuit<F: KagemushaPoseidonFieldV1>(
        mutation: StreamDigestMutation,
    ) -> Result<TestCircuit<F>, String> {
        use sha2::{Digest as _, Sha256};

        fn native_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update(domain);
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            hasher.finalize().into()
        }

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(K as usize - 1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let mut transition = [0x11, 0x22, 0, 0];
        let mut transition_len = 2_u64;
        if matches!(mutation, StreamDigestMutation::TransitionByte) {
            transition[0] ^= 1;
        }
        if matches!(mutation, StreamDigestMutation::TransitionLength) {
            transition[1] = 0;
            transition_len = 1;
        }
        let transition_bytes = assign_bytes(ctx, &range, &transition);
        let transition_length = ctx.load_witness(F::from(transition_len));
        let transition = KagemushaBoundedByteStreamV1::constrain(
            ctx,
            &range,
            transition_bytes,
            transition_length,
        )?;
        let mut recovery = [0x33, 0x44, 0x55, 0];
        let mut recovery_len = 3_u64;
        if matches!(mutation, StreamDigestMutation::RecoveryByte) {
            recovery[0] ^= 1;
        }
        if matches!(mutation, StreamDigestMutation::RecoveryLength) {
            recovery[2] = 0;
            recovery_len = 2;
        }
        let recovery_bytes = assign_bytes(ctx, &range, &recovery);
        let recovery_length = ctx.load_witness(F::from(recovery_len));
        let recovery =
            KagemushaBoundedByteStreamV1::constrain(ctx, &range, recovery_bytes, recovery_length)?;
        let mut transition_digest =
            native_digest(SEALED_TRANSITION_DIGEST_DOMAIN_V1, &[0x11, 0x22]);
        let mut recovery_digest =
            native_digest(SEALED_RECOVERY_DIGEST_DOMAIN_V1, &[0x33, 0x44, 0x55]);
        if matches!(mutation, StreamDigestMutation::TransitionCarrier) {
            transition_digest[0] ^= 1;
        }
        if matches!(mutation, StreamDigestMutation::RecoveryCarrier) {
            recovery_digest[0] ^= 1;
        }
        let transition_carrier =
            digest_limbs::<F>(transition_digest).map(|value| ctx.load_witness(value));
        let recovery_carrier =
            digest_limbs::<F>(recovery_digest).map(|value| ctx.load_witness(value));
        let mut jobs = PastaSha256JobsV1::default();
        constrain_sealed_stream_digest_v1(
            ctx,
            &range,
            &mut jobs,
            &transition,
            (!matches!(mutation, StreamDigestMutation::MissingTransitionCarrier))
                .then_some(transition_carrier),
            SEALED_TRANSITION_DIGEST_DOMAIN_V1,
            4,
        )?;
        constrain_sealed_stream_digest_v1(
            ctx,
            &range,
            &mut jobs,
            &recovery,
            (!matches!(mutation, StreamDigestMutation::MissingRecoveryCarrier))
                .then_some(recovery_carrier),
            SEALED_RECOVERY_DIGEST_DOMAIN_V1,
            4,
        )?;
        assert_eq!(jobs.typed_claim_jobs()?.len(), 2);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        Ok(TestCircuit { builder, jobs })
    }

    #[test]
    fn sealed_stream_carriers_require_exact_length_and_bytes_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            MockProver::run(
                K,
                &sealed_stream_digest_test_circuit::<F>(StreamDigestMutation::None)
                    .expect("both sealed stream carriers"),
                vec![],
            )
            .expect("sealed stream digest mock prover")
            .assert_satisfied();
            for mutation in [
                StreamDigestMutation::TransitionByte,
                StreamDigestMutation::TransitionLength,
                StreamDigestMutation::TransitionCarrier,
                StreamDigestMutation::RecoveryByte,
                StreamDigestMutation::RecoveryLength,
                StreamDigestMutation::RecoveryCarrier,
            ] {
                assert!(
                    MockProver::run(
                        K,
                        &sealed_stream_digest_test_circuit::<F>(mutation)
                            .expect("mutated stream carrier"),
                        vec![],
                    )
                    .expect("mutated stream digest mock prover")
                    .verify()
                    .is_err(),
                    "accepted altered sealed stream or candidate carrier: {mutation:?}"
                );
            }
            for mutation in [
                StreamDigestMutation::MissingTransitionCarrier,
                StreamDigestMutation::MissingRecoveryCarrier,
            ] {
                assert_eq!(
                    sealed_stream_digest_test_circuit::<F>(mutation)
                        .err()
                        .expect("missing verified candidate carrier must close opening"),
                    "sealed stream lacks a recursively verified candidate digest"
                );
            }
        }
        check::<Fp>();
        check::<Fq>();
    }

    fn preparation_transcript_test_circuit<F: KagemushaPoseidonFieldV1>(
        redeem: bool,
        mutation: Option<usize>,
        manifest_present: bool,
        producer_present: bool,
    ) -> Result<TestCircuit<F>, String> {
        use sha2::{Digest as _, Sha256};

        fn stream_digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
            let mut hasher = Sha256::new();
            hasher.update(domain);
            hasher.update([0]);
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
            hasher.finalize().into()
        }

        // The send vector is also pinned by candidate_lifecycle; the redemption vector uses
        // operation 4, zero request, and a nonzero artifact manifest with the same digest order.
        const SEND_ID: [u8; 32] = [
            0xe4, 0xc4, 0xd7, 0xd7, 0x55, 0xb8, 0x67, 0xf7, 0xb7, 0x09, 0x3c, 0x33, 0xfb, 0xaa,
            0x6c, 0x57, 0x72, 0xbf, 0x9d, 0x9b, 0x1b, 0x04, 0x8b, 0x54, 0xe1, 0x0e, 0x4d, 0x38,
            0xec, 0xf1, 0xdd, 0x4a,
        ];
        const REDEEM_ID: [u8; 32] = [
            0x74, 0x9a, 0x17, 0x02, 0xc4, 0x9b, 0x34, 0x3c, 0xbd, 0x76, 0x5c, 0x2a, 0x02, 0x09,
            0x68, 0xd4, 0xf8, 0x0a, 0x39, 0x67, 0x9a, 0x2f, 0xc3, 0x63, 0x9b, 0x1e, 0x2c, 0xdd,
            0x74, 0xde, 0xbb, 0xfd,
        ];
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits(K as usize - 1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let assign_digest = |ctx: &mut Context<F>, digest: [u8; 32]| {
            digest_limbs::<F>(digest).map(|limb| ctx.load_witness(limb))
        };
        let mut operation = if redeem { 4_u64 } else { 2_u64 };
        let mut digests = [
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            [5; 32],
            [6; 32],
            if redeem { [0; 32] } else { [7; 32] },
            if redeem { [11; 32] } else { [0; 32] },
            [8; 32],
            [9; 32],
            [10; 32],
        ];
        let mut producer_bytes = [vec![0x11, 0x22, 0x33], vec![0x44, 0x55]];
        let mut transition_digest =
            stream_digest(SEALED_TRANSITION_DIGEST_DOMAIN_V1, &[0x11, 0x22, 0x33]);
        let mut recovery_digest = stream_digest(SEALED_RECOVERY_DIGEST_DOMAIN_V1, &[0x44, 0x55]);
        let mut carrier = if redeem { REDEEM_ID } else { SEND_ID };
        if let Some(index) = mutation {
            match index {
                0 => operation ^= 6,
                1..=11 => digests[index - 1][0] ^= 1,
                12 => producer_bytes[0].push(0x66),
                13 => transition_digest[0] ^= 1,
                14 => producer_bytes[1].push(0x66),
                15 => recovery_digest[0] ^= 1,
                16 => carrier[0] ^= 1,
                17 => producer_bytes[0][0] ^= 1,
                18 => producer_bytes[1][0] ^= 1,
                19 => producer_bytes[0].clear(),
                20 => producer_bytes[1].extend_from_slice(&[0x66, 0x77, 0x88]),
                21 | 22 => {}
                23 => operation = 3,
                _ => unreachable!(),
            }
        }
        let candidate =
            super::super::terminal_authorization::KagemushaCandidatePreparationTranscriptCellsV1 {
                operation_tag: ctx.load_witness(F::from(operation)),
                predecessor_state_commitment: assign_digest(ctx, digests[0]),
                successor_state_commitment: assign_digest(ctx, digests[1]),
                prepared_transition_binding_digest: assign_digest(ctx, digests[3]),
                projection_semantic_digest: assign_digest(ctx, digests[4]),
                lifecycle_binding_digest: assign_digest(ctx, digests[5]),
                normalized_guard_statement_digest: assign_digest(ctx, digests[8]),
            };
        let one_use_bytes: [PastaSha256ByteV1<F>; 32] = assign_bytes(ctx, &range, &digests[10])
            .try_into()
            .expect("digest width");
        let zero = ctx.load_constant(F::ZERO);
        let mut sources = KagemushaTerminalPreparedSourceCellsV1 {
            terminal_branch: ctx.load_witness(F::ONE),
            derived_commit_cells: None,
            verified_preparation_id: None,
            verified_state_transition_digest: Some(assign_digest(ctx, digests[2])),
            candidate_preparation_id_carrier: Some(assign_digest(ctx, carrier)),
            candidate_stream_digest_carriers: Some([
                assign_digest(ctx, transition_digest),
                assign_digest(ctx, recovery_digest),
            ]),
            candidate_preparation_transcript: Some(candidate),
            candidate_hardware_profile_id: None,
            candidate_policy_epoch: None,
            request_digest: assign_digest(ctx, digests[6]),
            // Synthetic source for the isolated hash vector only. Production must derive
            // this cell from the release-pinned terminal public instance.
            verified_artifact_manifest_digest: manifest_present
                .then(|| assign_digest(ctx, digests[7])),
            candidate_envelope_digest: [zero; 2],
            outbox_reservation_commitment: assign_digest(ctx, digests[9]),
            prepared_one_use_authorization_digest: one_use_bytes,
            journal_revision_after: zero,
        };
        if matches!(mutation, Some(21)) {
            sources.candidate_stream_digest_carriers = None;
        }
        if matches!(mutation, Some(22)) {
            sources.candidate_preparation_transcript = None;
        }
        let mut jobs = PastaSha256JobsV1::default();
        let [transition, recovery] = assign_prepared_outgoing_sealed_streams_v1(
            ctx,
            &range,
            &mut jobs,
            &sources,
            producer_present.then_some(&producer_bytes),
            4,
            4,
        )?;
        let (transition_len, recovery_len) = (transition.actual_len(), recovery.actual_len());
        constrain_preparation_id_transcript_v1(
            ctx,
            &range,
            &mut jobs,
            &sources,
            transition_len,
            recovery_len,
        )?;
        assert_eq!(jobs.typed_claim_jobs()?.len(), 3);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        Ok(TestCircuit { builder, jobs })
    }

    #[test]
    fn preparation_transcript_matches_native_vectors_and_rejects_mutations_in_both_parities() {
        fn check<F: KagemushaPoseidonFieldV1>() {
            for redeem in [false, true] {
                MockProver::run(
                    K,
                    &preparation_transcript_test_circuit::<F>(redeem, None, true, true)
                        .expect("complete exact preparation transcript"),
                    vec![],
                )
                .expect("preparation transcript mock prover")
                .assert_satisfied();
                assert_eq!(
                    preparation_transcript_test_circuit::<F>(redeem, None, false, true)
                        .err()
                        .expect("missing authenticated manifest must close the transcript"),
                    "preparation transcript lacks an authenticated redemption artifact manifest"
                );
                assert_eq!(
                    preparation_transcript_test_circuit::<F>(redeem, None, true, false)
                        .err()
                        .expect("missing sealed-byte producer must fail closed"),
                    "prepared outgoing lacks sealed-byte producer witness"
                );
                for mutation in 0..=16 {
                    assert!(
                        MockProver::run(
                            K,
                            &preparation_transcript_test_circuit::<F>(
                                redeem,
                                Some(mutation),
                                true,
                                true,
                            )
                            .expect("mutated preparation transcript"),
                            vec![],
                        )
                        .expect("mutated transcript mock prover")
                        .verify()
                        .is_err(),
                        "accepted preparation transcript mutation {mutation}, redeem={redeem}"
                    );
                }
                for mutation in [17, 18] {
                    assert!(
                        MockProver::run(
                            K,
                            &preparation_transcript_test_circuit::<F>(
                                redeem,
                                Some(mutation),
                                true,
                                true,
                            )
                            .expect("mutated producer sealed bytes"),
                            vec![],
                        )
                        .expect("mutated producer mock prover")
                        .verify()
                        .is_err(),
                        "accepted prepared outgoing sealed-byte mutation {mutation}, redeem={redeem}"
                    );
                }
                for mutation in [19, 20] {
                    assert_eq!(
                        preparation_transcript_test_circuit::<F>(
                            redeem,
                            Some(mutation),
                            true,
                            true,
                        )
                        .err()
                        .expect("invalid producer stream must fail before hashing"),
                        "prepared outgoing sealed-byte witness exceeds its fixed profile"
                    );
                }
                for (mutation, expected) in [
                    (
                        21,
                        "prepared outgoing lacks recursively verified sealed stream carriers",
                    ),
                    (
                        22,
                        "prepared outgoing lacks a recursively verified State candidate",
                    ),
                ] {
                    assert_eq!(
                        preparation_transcript_test_circuit::<F>(
                            redeem,
                            Some(mutation),
                            true,
                            true,
                        )
                        .err()
                        .expect("missing verified source must close outgoing opening"),
                        expected
                    );
                }
                assert!(
                    MockProver::run(
                        K,
                        &preparation_transcript_test_circuit::<F>(redeem, Some(23), true, true)
                            .expect("non-outgoing operation witness"),
                        vec![],
                    )
                    .expect("non-outgoing operation mock prover")
                    .verify()
                    .is_err(),
                    "accepted a non-outgoing stream opening, redeem={redeem}"
                );
            }
        }
        check::<Fp>();
        check::<Fq>();
    }
}
