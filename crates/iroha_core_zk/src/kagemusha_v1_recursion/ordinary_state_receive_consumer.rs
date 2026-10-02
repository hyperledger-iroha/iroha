//! Ordinary Receive83 scalar consumer, separate from the OEM Terminal column reader.
//!
//! The released Wrapper proves its entire sender State/Terminal/Guard history and the full
//! signed receiver request. This relation reconstructs that exact request and output, proves
//! recipient plaintext knowledge, and inserts the same deterministic credit identity/value.
//! Full immutable DATA assertion/finality and historical request custody remain separate
//! closed Native/service capabilities; their data hashes do not create those capabilities.
use super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    generation::KagemushaOrdinaryRecursiveReceiveIncomingOpeningV1,
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned, hash},
    ordinary_cash_opening::{OrdinaryCashClockCellsV1, clock_payload},
    ordinary_guard_data_binding::KagemushaOrdinaryGuardDataBindingV1,
    ordinary_receiver_request_opening::{
        OrdinaryReceiverRequestOpeningV1, reconstruct_ordinary_request_canonical_v1,
    },
    ordinary_send_output_opening::{
        OrdinarySendOutputSourcesV1, constrain_ordinary_send_output_opening_v1,
    },
    state_relation::KagemushaAssignedStateRelationV1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
};
use iroha_data_model::kagemusha::*;
use std::collections::BTreeMap;
type Bytes<F> = [PastaSha256ByteV1<F>; 32];

/// Mathematical early column, copied to the exact subsequently recursively verified column.
/// These cells and queued hashes alone give no proof admission or monetary authority.
pub(super) struct OrdinaryReceiveOpeningV1<F: KagemushaPoseidonFieldV1> {
    pub(super) column: Vec<AssignedValue<F>>,
    pub(super) request_digest: Bytes<F>,
    pub(super) semantic_digest: Bytes<F>,
    pub(super) encrypted_raw_sha256: Bytes<F>,
}
fn bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    raw: [u8; 32],
) -> Bytes<F> {
    assign_bytes(ctx, range, &raw)
        .try_into()
        .expect("fixed digest32")
}
fn limbs<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: [AssignedValue<F>; 2],
) -> Bytes<F> {
    assigned_digest_bytes_v1(ctx, range.gate(), value)
        .try_into()
        .expect("fixed digest32")
}
fn selected<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enable: AssignedValue<F>,
    raw: Bytes<F>,
) -> Bytes<F> {
    raw.map(|b| {
        let v = range.gate().mul(ctx, enable, b.quantum_cell());
        PastaSha256ByteV1::range_checked(ctx, range, v)
    })
}
fn equal_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enable: AssignedValue<F>,
    left: &[PastaSha256ByteV1<F>],
    right: &[PastaSha256ByteV1<F>],
) -> Result<(), String> {
    if left.len() != right.len() {
        return Err("ordinary Receive semantic width differs".into());
    }
    for (a, b) in left.iter().zip(right) {
        let d = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        let invalid = range.gate().mul(ctx, d, enable);
        range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
    }
    Ok(())
}
fn copy_limbs_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enable: AssignedValue<F>,
    left: [AssignedValue<F>; 2],
    right: [AssignedValue<F>; 2],
) {
    for (a, b) in left.into_iter().zip(right) {
        let d = range.gate().sub(ctx, a, b);
        let bad = range.gate().mul(ctx, d, enable);
        range.gate().assert_is_const(ctx, &bad, &F::ZERO);
    }
}
fn clock<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    c: &KagemushaOrdinaryCashClockContextV1,
) -> Result<OrdinaryCashClockCellsV1<F>, String> {
    c.validate_shape()?;
    Ok(OrdinaryCashClockCellsV1 {
        nonce: bytes(ctx, range, c.request_nonce),
        signed_observations_original_digest: bytes(
            ctx,
            range,
            c.signed_observations_original_digest,
        ),
        lower_at_ms: ctx.load_witness(F::from(c.lower_at_ms)),
        upper_at_ms: ctx.load_witness(F::from(c.upper_at_ms)),
    })
}
fn column_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    at: usize,
) -> Result<Bytes<F>, String> {
    let pair = column
        .get(at..at + 2)
        .ok_or("ordinary Receive83 digest column short")?;
    Ok(limbs(ctx, range, [pair[0], pair[1]]))
}

/// Explicit ordinary83 reader. Sender app profile/epoch and logical terminal record are not
/// recipient profile/epoch or an OEM certificate. Protocol slots are always pinned, including
/// inactive genuine proof padding; monetary scope/output equalities use the actual Receive bit.
pub(super) fn constrain_ordinary_receive_column_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    state: &KagemushaAssignedStateRelationV1<F>,
    protocols: [[AssignedValue<F>; 2]; 2],
) -> Result<(), String> {
    if column.len() != 83 {
        return Err("ordinary Receive requires exact Wrapper49+34".into());
    }
    let enable = state.receive_credit.active;
    for (at, value) in [
        (0, ctx.load_constant(F::from(2))),
        (1, state.successor.protocol_version),
        (14, state.successor.scale),
        (36, state.amount),
    ] {
        let d = range.gate().sub(ctx, column[at], value);
        let bad = range.gate().mul(ctx, d, enable);
        range.gate().assert_is_const(ctx, &bad, &F::ZERO);
    }
    for (at, value) in [
        (2, state.successor.suite_id),
        (4, state.successor.vk_digest),
        (6, state.successor.release_id),
        (8, state.successor.network_id),
        (10, state.successor.asset_id),
        (12, state.successor.asset_incarnation),
        (15, state.successor.liability_pool_id),
    ] {
        copy_limbs_if(ctx, range, enable, [column[at], column[at + 1]], value);
    }
    for (at, value) in [(45, protocols[0]), (47, protocols[1])] {
        for (actual, expected) in [column[at], column[at + 1]].into_iter().zip(value) {
            ctx.constrain_equal(&actual, &expected);
        }
    }
    // Redeem's artifact-manifest slot cannot enter an ordinary received Send.
    for value in &column[39..41] {
        let bad = range.gate().mul(ctx, *value, enable);
        range.gate().assert_is_const(ctx, &bad, &F::ZERO);
    }
    Ok(())
}

/// Data-only inactive canonical request operand. Its evidence is empty public codec padding;
/// neither a signature equation nor a Native receiver/finality owner is manufactured here.
fn inactive_request(
    state: &super::state_relation::KagemushaStateRelationWitnessV1,
    c: &KagemushaOrdinaryAppCredentialV1,
    w: &KagemushaAppOperationApprovalV1,
) -> Result<KagemushaOrdinaryPaymentRequestV1, String> {
    let clock = KagemushaOrdinaryCashClockContextV1 {
        version: 1,
        request_nonce: w.challenge.nonce,
        signed_observations_original_digest: [1; 32],
        lower_at_ms: w.challenge.issued_at_ms,
        upper_at_ms: w.challenge.issued_at_ms,
    };
    let body = KagemushaOrdinaryPaymentRequestBodyV1 {
        version: 1,
        release_id: c.subject.release_id,
        network_id: c.subject.network_id,
        normalized_asset_id: kagemusha_asset_identity_digest_v1(&state.successor.lane.asset)
            .map_err(|e| e.to_string())?,
        asset_incarnation: *state.successor.asset_incarnation.as_bytes(),
        scale: state.successor.lane.scale,
        reserve_pool_id: state.successor.liability_pool_id,
        recipient_account_binding: c.subject.account_binding,
        amount: 1,
        recipient_encryption_key: [9; 32],
        recipient_credential_digest: c.canonical_digest()?,
        recipient_lane_id: c.subject.lane_id,
        request_id: [1; 32],
        clock_context: clock,
        issued_at_ms: w.challenge.issued_at_ms,
        expires_at_ms: w.challenge.expires_at_ms,
    };
    Ok(KagemushaOrdinaryPaymentRequestV1 {
        body,
        evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: vec![0x5a; 8],
        },
    })
}

/// Same fixed request/output/plaintext graph on every ordinary State operation. The selected
/// request signature was already verified by the actual released Wrapper. This reconstructs
/// its complete canonical bytes and copies recipient C/account/lane to the fresh State Guard.
/// All byte jobs must be consumed by the enclosing complete typed SHA claim and parity audits.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn constrain_ordinary_receive_opening_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    assigned: &KagemushaAssignedStateRelationV1<F>,
    state: &super::state_relation::KagemushaStateRelationWitnessV1,
    own: &KagemushaOrdinaryGuardDataBindingV1<F>,
    c: &KagemushaOrdinaryAppCredentialV1,
    w: &KagemushaAppOperationApprovalV1,
    native_column: &[F],
    source: Option<KagemushaOrdinaryRecursiveReceiveIncomingOpeningV1<'_>>,
) -> Result<OrdinaryReceiveOpeningV1<F>, String> {
    if native_column.len() != 83 {
        return Err("ordinary Receive early Wrapper83 column differs".into());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let enabled = assigned.receive_credit.active;
    let present = ctx.load_witness(F::from(u64::from(source.is_some())));
    gate.assert_bit(ctx, present);
    ctx.constrain_equal(&present, &enabled);
    let column = native_column
        .iter()
        .map(|v| {
            let a = ctx.load_witness(*v);
            range.range_check(ctx, a, 128);
            a
        })
        .collect::<Vec<_>>();
    let pad_request;
    let pad_output;
    let request: &KagemushaOrdinaryPaymentRequestV1;
    let output;
    let preparation_clock;
    let encrypted;
    let opening;
    let sender_index;
    let sender_epoch;
    let sender_lane;
    match source {
        Some(s) => {
            let super::KagemushaOrdinaryLineageOutgoingOriginalsV1::Send {
                request: r,
                output: o,
                encrypted_credit: e,
                preparation_clock: p,
            } = s.outgoing.outgoing_originals()
            else {
                return Err("ordinary Receive source is not an actual sender Send".into());
            };
            request = r.as_ref();
            output = o;
            preparation_clock = p;
            encrypted = e.as_slice();
            opening = Some(s.credit_opening);
            let approved = s.outgoing.approval()?;
            sender_index = approved.challenge.subject.secure_index_before;
            sender_epoch = approved.challenge.subject.hardware_epoch_id;
            sender_lane = s.outgoing.normalized_preparation().lane_id;
        }
        None => {
            pad_request = inactive_request(state, c, w)?;
            request = &pad_request;
            preparation_clock = &request.body.clock_context;
            pad_output = KagemushaOrdinaryPaymentOutputV1 {
                version: 1,
                request_digest: [0; 32],
                amount: 0,
                sender_before_commitment: [0; 32],
                sender_after_commitment: [0; 32],
                transition_nullifier: [0; 32],
                credit_id: [0; 32],
                ciphertext_commitment: [0; 32],
                encrypted_credit_digest: [0; 32],
                clock_context_digest: [0; 32],
                prepared_at_ms: 0,
            };
            output = &pad_output;
            encrypted = &[];
            opening = None;
            sender_index = 0;
            sender_epoch = [0; 32];
            sender_lane = [0; 32];
        }
    }
    let native_transcript = request.body.binding_transcript();
    let fields = native_transcript
        .fields
        .iter()
        .map(|f| {
            (
                f.name,
                assign_bytes(
                    builder.main(0),
                    &range,
                    &native_transcript.bytes[f.range.clone()],
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let field = |name: &str| -> Result<&[PastaSha256ByteV1<F>], String> {
        fields
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("ordinary received request field {name} absent"))
    };
    let ctx = builder.main(0);
    let gate = range.gate();
    let prep_clock = clock(ctx, &range, preparation_clock)?;
    let request_clock = clock(ctx, &range, &request.body.clock_context)?;
    let request_clock_raw =
        clock_payload(ctx, &range, &request_clock, &request.body.clock_context)?;
    let mut request_clock_message = constant_bytes(KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1);
    request_clock_message.extend(request_clock_raw);
    let request_clock_digest = hash(ctx, jobs, request_clock_message)?;
    equal_if(
        ctx,
        &range,
        enabled,
        field("clock_context_digest")?,
        &request_clock_digest,
    )?;
    for (name, value) in [
        ("release_id", assigned.successor.release_id),
        ("network_id", assigned.successor.network_id),
        ("normalized_asset_id", assigned.successor.asset_id),
        ("asset_incarnation", assigned.successor.asset_incarnation),
        ("reserve_pool_id", assigned.successor.liability_pool_id),
        ("recipient_lane_id", assigned.successor.lane_id),
    ] {
        let bytes = limbs(ctx, &range, value);
        equal_if(ctx, &range, enabled, field(name)?, &bytes)?;
    }
    equal_if(
        ctx,
        &range,
        enabled,
        field("recipient_credential_digest")?,
        &own.digests[1],
    )?;
    equal_if(
        ctx,
        &range,
        enabled,
        field("recipient_account_binding")?,
        &own.account_binding,
    )?;
    let amount = assigned_uint_bytes_v1(ctx, gate, assigned.amount, 128);
    equal_if(ctx, &range, enabled, field("amount")?, &amount)?;
    let scale = assigned_uint_bytes_v1(ctx, gate, assigned.successor.scale, 32);
    equal_if(ctx, &range, enabled, field("scale")?, &scale)?;
    let version = constant_bytes(&1_u16.to_le_bytes());
    equal_if(ctx, &range, enabled, field("version")?, &version)?;
    // Exact original request validity at the original sender preparation interval, not at
    // a later incoming approval or a fabricated fresh clock for the expired request.
    let issued = ctx.load_witness(F::from(request.body.issued_at_ms));
    let expires = ctx.load_witness(F::from(request.body.expires_at_ms));
    for (name, value) in [("issued_at_ms", issued), ("expires_at_ms", expires)] {
        let bytes = assigned_uint_bytes_v1(ctx, gate, value, 64);
        equal_if(ctx, &range, enabled, field(name)?, &bytes)?;
    }
    let early = range.is_less_than(ctx, prep_clock.lower_at_ms, issued, 64);
    let bad = gate.mul(ctx, early, enabled);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let valid = range.is_less_than(ctx, prep_clock.upper_at_ms, expires, 64);
    let bad = gate.mul_not(ctx, valid, enabled);
    gate.assert_is_const(ctx, &bad, &F::ZERO);
    let (apple, raw) = match &request.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            (false, signature_der.as_slice())
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
            (true, raw_assertion.as_slice())
        }
    };
    if raw.len() > 311 {
        return Err("ordinary received request original evidence exceeds grammar".into());
    }
    let mut padded = raw.to_vec();
    padded.resize(311, 0);
    let codec = assign_bytes(ctx, &range, &padded);
    let len = ctx.load_witness(F::from(raw.len() as u64));
    let codec = KagemushaBoundedByteStreamV1::constrain(ctx, &range, codec, len)?;
    let apple = ctx.load_witness(F::from(u64::from(apple)));
    gate.assert_bit(ctx, apple);
    let request_raw_digest = reconstruct_ordinary_request_canonical_v1(
        builder,
        jobs,
        request,
        &fields,
        &request_clock,
        &codec,
        apple,
    )?;
    let ctx = builder.main(0);
    let request_digest = selected(ctx, &range, enabled, request_raw_digest);
    let recipient = selected(ctx, &range, enabled, own.digests[1]);
    let request_public = column_digest(ctx, &range, &column, 30)?;
    let recipient_public = column_digest(ctx, &range, &column, 32)?;
    equal_if(ctx, &range, enabled, &request_digest, &request_public)?;
    equal_if(ctx, &range, enabled, &recipient, &recipient_public)?;
    let field_bytes = |name: &str| -> Result<Bytes<F>, String> {
        field(name)?
            .try_into()
            .map_err(|_| "ordinary received request digest width".into())
    };
    let receiver = OrdinaryReceiverRequestOpeningV1 {
        request_digest,
        credential_digest: recipient,
        encryption_key: selected(
            ctx,
            &range,
            enabled,
            field_bytes("recipient_encryption_key")?,
        ),
        recipient_lane: selected(ctx, &range, enabled, field_bytes("recipient_lane_id")?),
        request_id: selected(ctx, &range, enabled, field_bytes("request_id")?),
    };
    let mut raw = encrypted.to_vec();
    raw.resize(KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1, 0);
    if encrypted.len() > KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1 {
        return Err("ordinary received encrypted credit exceeds maintained kernel".into());
    }
    let assigned_raw = assign_bytes(ctx, &range, &raw);
    let len = ctx.load_witness(F::from(encrypted.len() as u64));
    let encrypted_stream = KagemushaBoundedByteStreamV1::constrain(ctx, &range, assigned_raw, len)?;
    let raw_sha = jobs.digest_bounded_constrained(
        ctx,
        &range,
        encrypted_stream.bytes(),
        encrypted_stream.actual_len(),
    )?;
    let mut raw_sha_bytes = Vec::with_capacity(32);
    for word in raw_sha {
        let bits = PastaSha256BitV1::decompose(ctx, gate, word, 32);
        for start in [24, 16, 8, 0] {
            raw_sha_bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                gate,
                &bits[start..start + 8],
            ));
        }
    }
    let encrypted_raw_sha256 = raw_sha_bytes
        .try_into()
        .map_err(|_| "ordinary encrypted raw SHA width")?;
    let before = bytes(ctx, &range, output.sender_before_commitment);
    let after = bytes(ctx, &range, output.sender_after_commitment);
    let epoch = bytes(ctx, &range, sender_epoch);
    let lane = bytes(ctx, &range, sender_lane);
    let index = ctx.load_witness(from_u128::<F>(sender_index));
    let network = limbs(ctx, &range, assigned.successor.network_id);
    let pool = limbs(ctx, &range, assigned.successor.liability_pool_id);
    let credit = limbs(ctx, &range, assigned.replay_credit_id);
    let credit = selected(ctx, &range, enabled, credit);
    let expected_output = bytes(
        ctx,
        &range,
        if source.is_some() {
            output.binding_digest()?
        } else {
            [0; 32]
        },
    );
    let expected_encrypted = bytes(ctx, &range, output.encrypted_credit_digest);
    let expected_nullifier = bytes(
        ctx,
        &range,
        if source.is_some() {
            output.transition_nullifier
        } else {
            kagemusha_ordinary_transition_nullifier_v1(
                [0; 32],
                0,
                [0; 32],
                *state.successor.lane.network_id.as_bytes(),
                [0; 32],
                state.successor.liability_pool_id,
            )?
        },
    );
    let expected_commitment = bytes(ctx, &range, output.ciphertext_commitment);
    let send_operation = gate.mul(ctx, enabled, QuantumCell::Constant(F::from(2)));
    let received = constrain_ordinary_send_output_opening_v1(
        ctx,
        &range,
        jobs,
        OrdinarySendOutputSourcesV1 {
            operation: send_operation,
            amount: assigned.amount,
            before,
            after,
            predecessor_secure_index: index,
            predecessor_epoch: epoch,
            network,
            sender_lane: lane,
            reserve_pool: pool,
            receiver: &receiver,
            selected_credit_id: credit,
            encrypted_credit: &encrypted_stream,
            preparation_clock: &prep_clock,
            preparation_clock_specimen: preparation_clock,
            expected_output_digest: expected_output,
            expected_encrypted_digest: expected_encrypted,
            expected_nullifier,
            expected_ciphertext_commitment: expected_commitment,
        },
        output,
        opening,
    )?;
    let nullifier = column_digest(ctx, &range, &column, 28)?;
    equal_if(
        ctx,
        &range,
        enabled,
        &received.transition_nullifier,
        &nullifier,
    )?;
    let ciphertext = column_digest(ctx, &range, &column, 34)?;
    equal_if(
        ctx,
        &range,
        enabled,
        &received.ciphertext_commitment,
        &ciphertext,
    )?;
    let candidate = column_digest(ctx, &range, &column, 24)?;
    let record = column_digest(ctx, &range, &column, 26)?;
    let mut binding = constant_bytes(KAGEMUSHA_ORDINARY_OUTPUT_BINDING_DOMAIN_V1);
    binding.extend(received.output_digest);
    binding.extend(candidate);
    binding.extend(record);
    let binding = hash(ctx, jobs, binding)?;
    let expected = column_digest(ctx, &range, &column, 37)?;
    equal_if(ctx, &range, enabled, &binding, &expected)?;
    let semantic = received.output_digest;
    for (actual, expected) in [
        (
            receiver.request_digest,
            assigned.receive_credit.request_digest,
        ),
        (
            receiver.encryption_key,
            assigned.receive_credit.recipient_encryption_key,
        ),
        (
            receiver.recipient_lane,
            assigned.receive_credit.recipient_lane_id,
        ),
        (
            receiver.credential_digest,
            assigned.receive_credit.receiver_binding_digest,
        ),
        (received.credit_id, assigned.receive_credit.credit_id),
        (
            received.ciphertext_commitment,
            assigned.receive_credit.ciphertext_commitment,
        ),
        (semantic, assigned.receive_credit.payment_output_digest),
    ] {
        let actual = digest_limbs_assigned(ctx, &actual);
        copy_limbs_if(ctx, &range, enabled, actual, expected);
    }
    let secret_originals = opening.map_or([[0; 32]; 3], |o| {
        [
            o.credit_commitment_opening,
            o.recipient_binding_opening,
            o.recovery_nonce,
        ]
    });
    for (raw, expected) in secret_originals.into_iter().zip([
        assigned.receive_credit.credit_commitment_opening,
        assigned.receive_credit.recipient_binding_opening,
        assigned.receive_credit.recovery_nonce,
    ]) {
        let actual = bytes(ctx, &range, raw);
        let actual = digest_limbs_assigned(ctx, &actual);
        copy_limbs_if(ctx, &range, enabled, actual, expected);
    }
    let actual = digest_limbs_assigned(ctx, &received.transition_nullifier);
    copy_limbs_if(
        ctx,
        &range,
        enabled,
        actual,
        assigned.receive_credit.transition_nullifier,
    );
    // These shared internal positions carry explicit ordinary body/candidate commitments;
    // they do not claim an OEM certificate or prepared-payment decoder relation.
    copy_limbs_if(
        ctx,
        &range,
        enabled,
        [column[22], column[23]],
        assigned.receive_credit.prepared_transfer_digest,
    );
    copy_limbs_if(
        ctx,
        &range,
        enabled,
        [column[24], column[25]],
        assigned.receive_credit.incoming_proof_binding_digest,
    );
    Ok(OrdinaryReceiveOpeningV1 {
        column,
        request_digest,
        semantic_digest: semantic,
        encrypted_raw_sha256,
    })
}

/// Exact whole Receive IncomingReservation original. Complete sender DATA assertion and
/// pre-receipt carrier SHA are separately authenticated by the closed source capability; the
/// request, encrypted bytes, credit/opening semantics, recipient lineage and actual full outer
/// predecessor SHA are derived from the real recursive/canonical operands here.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn constrain_ordinary_receive_reservation_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    state: &KagemushaAssignedStateRelationV1<F>,
    own: &KagemushaOrdinaryGuardDataBindingV1<F>,
    opened: &OrdinaryReceiveOpeningV1<F>,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    full_parent_sha: &Bytes<F>,
) -> Result<Bytes<F>, String> {
    use super::canonical_preimage::{
        assemble_bounded_canonical_frame_v1,
        field_stream::{concat_fields_v1, field_v1, framed_hash_v1, struct_payload_v1},
    };
    type Stream<F> = KagemushaBoundedByteStreamV1<F>;
    fn fixed<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        raw: Vec<PastaSha256ByteV1<F>>,
    ) -> Result<Stream<F>, String> {
        let len = ctx.load_constant(F::from(raw.len() as u64));
        Stream::constrain(ctx, range, raw, len)
    }
    fn literal<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        raw: &[u8],
    ) -> Result<Stream<F>, String> {
        fixed(ctx, range, constant_bytes(raw))
    }
    fn raw<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        raw: &[u8],
        capacity: usize,
    ) -> Result<Stream<F>, String> {
        if raw.len() > capacity {
            return Err("ordinary Receive lineage field exceeds installed bound".into());
        }
        let mut bytes = raw.to_vec();
        bytes.resize(capacity, 0);
        let bytes = assign_bytes(ctx, range, &bytes);
        let len = ctx.load_witness(F::from(raw.len() as u64));
        Stream::constrain(ctx, range, bytes, len)
    }
    fn digest<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        value: [AssignedValue<F>; 2],
    ) -> Result<Stream<F>, String> {
        let b = limbs(ctx, range, value);
        fixed(ctx, range, b.to_vec())
    }
    fn integer<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        range: &RangeChip<F>,
        value: AssignedValue<F>,
        bits: usize,
    ) -> Result<Stream<F>, String> {
        range.range_check(ctx, value, bits);
        let b = assigned_uint_bytes_v1(ctx, range.gate(), value, bits);
        fixed(ctx, range, b)
    }
    macro_rules! payload {($g:expr;$($f:expr),*$(,)?)=>{{let fields=[$($f),*];struct_payload_v1(ctx,range,$g,&fields)}};}
    let enable = state.receive_credit.active;
    let s = &reservation.selection;
    let owner = &s.lineage.owner;
    let rt = &owner.runtime;
    let og = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(owner)?;
    let rg = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(rt)?;
    let lg = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(&s.lineage)?;
    let hg = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(&s.predecessor)?;
    let sg = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(s)?;
    let vg = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(reservation)?;
    let account = raw(ctx, range, &og.fields()[0], 4096)?;
    let prefix =
        kagemusha_canonical_mint_frame_prefix_v1(&owner.account_id).map_err(|e| e.to_string())?;
    let frame = assemble_bounded_canonical_frame_v1(ctx, range, &prefix, &account)?;
    let account_sha = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:app-approval-account\0",
        &frame,
    )?;
    equal_if(ctx, range, enable, &account_sha, &own.account_binding)?;
    let asset = raw(ctx, range, &rg.fields()[4], 64)?;
    let prefix = kagemusha_canonical_mint_frame_prefix_v1(&rt.asset).map_err(|e| e.to_string())?;
    let frame = assemble_bounded_canonical_frame_v1(ctx, range, &prefix, &asset)?;
    let asset_sha = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:asset-identity\0",
        &frame,
    )?;
    let expected = limbs(ctx, range, state.successor.asset_id);
    equal_if(ctx, range, enable, &asset_sha, &expected)?;
    let ig = KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(&rt.asset_incarnation)?;
    let incarnation = payload!(&ig;digest(ctx,range,state.successor.asset_incarnation)?)?;
    // FI id/namespace/dataspace are full original data admitted under the installed FI
    // source/lineage owner. State additionally authenticates all financial scope commitments.
    let runtime = payload!(&rg;
        raw(ctx,range,&rg.fields()[0],KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1)?,
        raw(ctx,range,&rg.fields()[1],8)?,
        raw(ctx,range,&rg.fields()[2],KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1)?,
        digest(ctx,range,state.successor.network_id)?,asset,incarnation,integer(ctx,range,state.successor.scale,32)?,
    )?;
    let owner = payload!(&og;account,runtime,digest(ctx,range,state.successor.lane_id)?)?;
    let lineage = payload!(&lg;literal(ctx,range,&1_u16.to_le_bytes())?,owner,digest(ctx,range,state.predecessor.epoch_id)?,fixed(ctx,range,own.financial_authority_commitment.to_vec())?)?;
    let head = payload!(&hg;digest(ctx,range,state.predecessor_outer)?,integer(ctx,range,state.predecessor.sequence,128)?,fixed(ctx,range,full_parent_sha.to_vec())?)?;
    let KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
        sender_commit_transport_original_sha256,
        sender_outgoing_original_sha256,
        ..
    } = s.source
    else {
        return Err("ordinary Receive reservation requires distinct Receive codec".into());
    };
    let immutable_assertion = bytes(ctx, range, sender_commit_transport_original_sha256);
    let immutable_outgoing = bytes(ctx, range, sender_outgoing_original_sha256);
    let specimen =
        norito::codec::encode_adaptive(&KagemushaOrdinaryIncomingSourceSelectionV1::Receive {
            sender_commit_transport_original_sha256: [0; 32],
            sender_outgoing_original_sha256: [0; 32],
            recipient_request_original_digest: [0; 32],
            encrypted_credit_original_sha256: [0; 32],
        });
    let tag = specimen
        .get(..4)
        .ok_or("ordinary Receive sole enum tag absent")?;
    let fields = [
        fixed(ctx, range, immutable_assertion.to_vec())?,
        fixed(ctx, range, immutable_outgoing.to_vec())?,
        fixed(ctx, range, opened.request_digest.to_vec())?,
        fixed(ctx, range, opened.encrypted_raw_sha256.to_vec())?,
    ];
    let payload_fields = fields
        .iter()
        .map(|f| field_v1(ctx, range, f))
        .collect::<Result<Vec<_>, _>>()?;
    let mut source_fields = vec![literal(ctx, range, tag)?];
    source_fields.extend(payload_fields);
    let source = concat_fields_v1(ctx, range, &source_fields)?;
    let old_fi = bytes(ctx, range, s.financial_control_original_sha256);
    let old_clock = bytes(ctx, range, s.clock_context_digest);
    let selection = payload!(&sg;literal(ctx,range,&1_u16.to_le_bytes())?,lineage,fixed(ctx,range,own.approval_operation_id.to_vec())?,head,source,digest(ctx,range,state.replay_credit_id)?,integer(ctx,range,state.amount,128)?,integer(ctx,range,state.successor.scale,32)?,fixed(ctx,range,own.digests[1].to_vec())?,fixed(ctx,range,old_fi.to_vec())?,fixed(ctx,range,old_clock.to_vec())?)?;
    // Both source selectors are the independently authenticated exact originals. They are
    // not a circuit proof of DATA membership; the enclosing admission requires that owner.
    let value = payload!(&vg;selection,fixed(ctx,range,immutable_assertion.to_vec())?,fixed(ctx,range,immutable_outgoing.to_vec())?,fixed(ctx,range,opened.semantic_digest.to_vec())?)?;
    let full = assemble_bounded_canonical_frame_v1(ctx, range, vg.framing(), &value)?;
    let envelope = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ordinary-incoming-reservation\0",
        &full,
    )?;
    let envelope_limbs = digest_limbs_assigned(ctx, &envelope);
    for expected in [
        state.replay_envelope_digest,
        state.transition_effect_digest,
        state.receive_credit_binding_digest,
        state.receive_credit.envelope_digest,
    ] {
        copy_limbs_if(ctx, range, enable, envelope_limbs, expected);
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::super::state_relation::{AssignedState, KagemushaAssignedReceiveFoldCreditV1};
    use super::*;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };
    /// Known public scalar operands isolate the actual ordinary reader, not a State/Native grant.
    fn reader<F: KagemushaPoseidonFieldV1>(enabled: u64, mutation: Option<usize>, pass: bool) {
        let mut b = BaseCircuitBuilder::<F>::new(false)
            .use_k(12)
            .use_lookup_bits(11)
            .use_instance_columns(1);
        let range = b.range_chip();
        let ctx = b.main(0);
        let z = ctx.load_zero();
        let pair = [z, z];
        let mut column = (0..83).map(|i| F::from(i as u64 + 100)).collect::<Vec<_>>();
        column[0] = F::from(2);
        column[1] = F::ONE;
        column[39] = F::ZERO;
        column[40] = F::ZERO;
        let original = column.clone();
        if let Some(i) = mutation {
            column[i] += F::ONE;
        }
        let actual = column
            .into_iter()
            .map(|v| ctx.load_witness(v))
            .collect::<Vec<_>>();
        let source = original
            .into_iter()
            .map(|v| ctx.load_witness(v))
            .collect::<Vec<_>>();
        let successor = AssignedState {
            protocol_version: source[1],
            suite_id: [source[2], source[3]],
            vk_digest: [source[4], source[5]],
            balance: z,
            sequence: z,
            secure_index: z,
            epoch_generation: z,
            epoch_id: pair,
            key_reference: pair,
            policy_id: pair,
            next_one_use_key_reference: pair,
            nonce: pair,
            replay_root: z,
            commitment: z,
            release_id: [source[6], source[7]],
            asset_incarnation: [source[12], source[13]],
            liability_pool_id: [source[15], source[16]],
            hardware_profile_id: pair,
            policy_epoch: z,
            network_id: [source[8], source[9]],
            asset_id: [source[10], source[11]],
            scale: source[14],
            lane_id: pair,
        };
        let receive = KagemushaAssignedReceiveFoldCreditV1 {
            active: ctx.load_witness(F::from(enabled)),
            amount: source[36],
            credit_id: pair,
            recipient_lane_id: pair,
            incoming_proof_binding_digest: pair,
            request_digest: pair,
            prepared_transfer_digest: pair,
            transition_nullifier: pair,
            recipient_encryption_key: pair,
            ciphertext_commitment: pair,
            credit_commitment_opening: pair,
            recipient_binding_opening: pair,
            recovery_nonce: pair,
            receiver_binding_digest: pair,
            payment_output_digest: pair,
            envelope_digest: pair,
        };
        let state = KagemushaAssignedStateRelationV1 {
            operation: ctx.load_witness(F::from(3)),
            amount: source[36],
            predecessor: successor,
            successor,
            predecessor_outer: pair,
            successor_outer: pair,
            guard_digest: pair,
            journal_revision_before: z,
            journal_revision_after: z,
            transition_effect_digest: pair,
            mint_finality_semantic_digest: pair,
            mint_finality_proof_binding_digest: pair,
            peer_credit_id: pair,
            recipient_encryption_key_binding: pair,
            receive_credit: receive,
            receive_credit_binding_digest: pair,
            lifecycle_binding_digest: pair,
            prepared_transition_binding_digest: pair,
            predecessor_eq_components: pair,
            predecessor_ep_components: pair,
            successor_eq_components: pair,
            successor_ep_components: pair,
            replay_credit_id: pair,
            replay_envelope_digest: pair,
        };
        constrain_ordinary_receive_column_v1(
            ctx,
            &range,
            &actual,
            &state,
            [[source[45], source[46]], [source[47], source[48]]],
        )
        .unwrap();
        assert!(
            constrain_ordinary_receive_column_v1(
                ctx,
                &range,
                &actual[..82],
                &state,
                [[source[45], source[46]], [source[47], source[48]]]
            )
            .is_err()
        );
        b.assigned_instances = vec![Vec::new()];
        b.calculate_params(Some(9));
        assert_eq!(
            MockProver::run(12, &b, vec![Vec::new()])
                .unwrap()
                .verify()
                .is_ok(),
            pass,
            "Receive83 mutation {mutation:?}, enabled {enabled}"
        );
    }
    fn both<F: KagemushaPoseidonFieldV1>() {
        reader::<F>(1, None, true);
        for offset in [0, 1, 2, 4, 6, 8, 10, 12, 14, 15, 36, 39, 40, 45, 47] {
            reader::<F>(1, Some(offset), false);
        }
        // Sender app profile and policy epoch can differ from the receiver's own C.
        reader::<F>(1, Some(17), true);
        reader::<F>(1, Some(19), true);
        reader::<F>(0, Some(36), true);
        reader::<F>(0, Some(45), false);
    }
    #[test]
    fn ordinary_receive83_reader_pins_purpose_scope_amount_manifest_and_actual_protocols() {
        both::<Fp>();
        both::<Fq>();
    }
}
