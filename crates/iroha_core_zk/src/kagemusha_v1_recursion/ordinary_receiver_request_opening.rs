//! Complete ordinary receiver C, request-signing equation and canonical request original.
//!
//! This fixed topology proves governed issuer admission and both platform signature branches.
//! It does not create an enrolled receiver, financial custodian or Native clock capability.
//! Shipping callers must lend these originals from the independent retained receiver owner.
use super::{
    canonical_preimage::selected_stream::{
        CanonicalSelectedStreamVariantV1, reconstruct_selected_canonical_stream_v1,
    },
    composite::assigned_uint_bytes_v1,
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned, hash},
    ordinary_app_guard_binding::OrdinaryCredentialIssuerCellsV1,
    ordinary_cash_opening::{OrdinaryCashClockCellsV1, clock_payload, fill_transcript},
    ordinary_credential_union::{
        assign_ordinary_credential_union_v1, reconstruct_ordinary_credential_union_v1,
    },
    ordinary_integrity_union::constrain_ordinary_integrity_union_v1,
    ordinary_issuer_config::{ORDINARY_ISSUER_SLOTS, OrdinaryIssuerTableV1},
    ordinary_issuer_equation::constrain_ordinary_issuer_original_v1,
    ordinary_platform_union::constrain_ordinary_signed_message_union_v1,
};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs},
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1, KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_DOMAIN_V1,
    KAGEMUSHA_REQUEST_MAX_TTL_MS_V1, KagemushaOrdinaryAppCredentialV1,
    KagemushaOrdinaryPaymentRequestV1, KagemushaPlayIntegrityRefreshLeaseV1,
};
use std::collections::BTreeMap;
type Bytes<F> = [PastaSha256ByteV1<F>; 32];
/// Exact Native-selected originals. `enabled` affects witness assignment only; the actual
/// operation cell controls every circuit selection and public receiver commitment.
pub(super) struct OrdinaryReceiverRequestWitnessV1<'a> {
    pub(super) request: &'a KagemushaOrdinaryPaymentRequestV1,
    pub(super) credential: &'a KagemushaOrdinaryAppCredentialV1,
    pub(super) integrity_lease: Option<&'a KagemushaPlayIntegrityRefreshLeaseV1>,
    pub(super) previous_app_attest_counter: Option<u32>,
    pub(super) enabled: bool,
}
/// These come from the genuine verified candidate and captured purpose2 Native selection.
/// Request clock originals remain independently signed by the receiver key; the preparation
/// interval is separately lent from the actual sender Native clock owner.
pub(super) struct OrdinaryReceiverRequestSourcesV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) release: Bytes<F>,
    pub(super) network: Bytes<F>,
    pub(super) normalized_asset: Bytes<F>,
    pub(super) incarnation: Bytes<F>,
    pub(super) scale: AssignedValue<F>,
    pub(super) reserve_pool: Bytes<F>,
    pub(super) amount: AssignedValue<F>,
    pub(super) preparation_clock: &'a OrdinaryCashClockCellsV1<F>,
    pub(super) expected_request_digest: Bytes<F>,
    pub(super) expected_recipient_credential_digest: Bytes<F>,
}
/// Exact signed receiver commitment and same requested amount/encryption point for outgoing
/// output/decryption selection. Disabled results are all zero; mathematical codec pads never
/// enter a public receiver original digest.
pub(super) struct OrdinaryReceiverRequestOpeningV1<F: KagemushaPoseidonFieldV1> {
    pub(super) request_digest: Bytes<F>,
    pub(super) credential_digest: Bytes<F>,
    pub(super) encryption_key: Bytes<F>,
    pub(super) recipient_lane: Bytes<F>,
    pub(super) request_id: Bytes<F>,
}
fn equal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    a: &[PastaSha256ByteV1<F>],
    b: &[PastaSha256ByteV1<F>],
) -> Result<(), String> {
    if a.len() != b.len() {
        return Err("ordinary receiver field width differs".into());
    }
    for (a, b) in a.iter().zip(b) {
        let difference = range.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        range.gate().assert_is_const(ctx, &difference, &F::ZERO);
    }
    Ok(())
}
fn integer<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
    bits: usize,
) -> Result<AssignedValue<F>, String> {
    if bits % 8 != 0 || bytes.len() != bits / 8 || bits > 128 {
        return Err("receiver integer width differs".into());
    }
    let value = range.gate().inner_product(
        ctx,
        bytes.iter().map(|b| b.quantum_cell()),
        (0..bytes.len()).map(|i| QuantumCell::Constant(F::from(256).pow_vartime([i as u64]))),
    );
    range.range_check(ctx, value, bits);
    Ok(value)
}
/// Pin the whole governed issuer point and profile together to every release table row.
/// This Base-gate relation commits all64 rows in the key; no table/key comes from a witness.
fn release_issuer<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    table: &OrdinaryIssuerTableV1,
    native_profile: [u8; 32],
    profile: Bytes<F>,
) -> Result<([u8; 65], [AssignedValue<F>; 65]), String> {
    let selected = table.selected(native_profile)?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let selectors = (0..ORDINARY_ISSUER_SLOTS)
        .map(|i| {
            let bit = ctx.load_witness(F::from(u64::from(i == selected)));
            gate.assert_bit(ctx, bit);
            if table.slots[i].profile_id == [0; 32] {
                gate.assert_is_const(ctx, &bit, &F::ZERO);
            }
            bit
        })
        .collect::<Vec<_>>();
    let sum = gate.sum(ctx, selectors.iter().copied());
    gate.assert_is_const(ctx, &sum, &F::ONE);
    let limbs = digest_limbs_assigned(ctx, &profile);
    for (i, actual) in limbs.into_iter().enumerate() {
        let expected = gate.inner_product(
            ctx,
            selectors.iter().copied(),
            table
                .slots
                .iter()
                .map(|row| QuantumCell::Constant(digest_limbs::<F>(row.profile_id)[i])),
        );
        ctx.constrain_equal(&actual, &expected);
    }
    let key = core::array::from_fn(|i| {
        gate.inner_product(
            ctx,
            selectors.iter().copied(),
            table
                .slots
                .iter()
                .map(|row| QuantumCell::Constant(F::from(u64::from(row.issuer_sec1[i])))),
        )
    });
    Ok((table.slots[selected].issuer_sec1, key))
}
fn select<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    enabled: AssignedValue<F>,
    bytes: Bytes<F>,
) -> Bytes<F> {
    bytes.map(|byte| {
        let value = range.gate().mul(ctx, byte.quantum_cell(), enabled);
        PastaSha256ByteV1::range_checked(ctx, range, value)
    })
}
/// Full original and signature opening. The requested amount/scope are joined to the actual
/// verified outgoing State; every signature and original reconstruction job remains queued
/// for the enclosing complete typed SHA consumer and both reciprocal audits.
pub(super) fn constrain_ordinary_receiver_request_opening_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    table: &OrdinaryIssuerTableV1,
    sources: OrdinaryReceiverRequestSourcesV1<'_, F>,
    witness: OrdinaryReceiverRequestWitnessV1<'_>,
) -> Result<OrdinaryReceiverRequestOpeningV1<F>, String> {
    witness.request.body.validate_shape()?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let enabled = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(2)));
    // Witness routing may select genuine originals vs public pads, but never grants Send.
    let native_enabled = ctx.load_witness(F::from(u64::from(witness.enabled)));
    gate.assert_bit(ctx, native_enabled);
    ctx.constrain_equal(&enabled, &native_enabled);
    let mut union = assign_ordinary_credential_union_v1(builder, witness.credential)?;
    let ed_digest =
        reconstruct_ordinary_credential_union_v1(builder, jobs, witness.credential, &union, false)?;
    let (issuer_raw, issuer_cells) = release_issuer(
        builder,
        table,
        witness.credential.subject.hardware_profile_id,
        union.cells.fixed_digests[7],
    )?;
    let issuer_signature = assign_bytes(
        builder.main(0),
        &range,
        witness
            .credential
            .circuit_admission
            .signature
            .as_raw_bytes(),
    )
    .try_into()
    .map_err(|_| "receiver issuer signature width")?;
    constrain_ordinary_issuer_original_v1(
        builder,
        jobs,
        1,
        &union.cells.fixed_digests[6],
        &union.cells.fixed_digests[7],
        &ed_digest,
        &issuer_signature,
        &witness.credential.circuit_admission,
        &issuer_raw,
        &issuer_cells,
    )?;
    union.cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed_digest,
        signature: issuer_signature,
    });
    let credential_digest =
        reconstruct_ordinary_credential_union_v1(builder, jobs, witness.credential, &union, true)?;
    let native_transcript = witness.request.body.binding_transcript();
    let parts = native_transcript
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
        .collect::<Vec<_>>();
    let fields: BTreeMap<_, _> = parts.iter().cloned().collect();
    let field = |name: &str| -> Result<&[PastaSha256ByteV1<F>], String> {
        fields
            .get(name)
            .map(Vec::as_slice)
            .ok_or_else(|| format!("receiver semantic field {name} absent"))
    };
    let ctx = builder.main(0);
    let gate = range.gate();
    equal(
        ctx,
        &range,
        field("version")?,
        &constant_bytes(&1_u16.to_le_bytes()),
    )?;
    equal(ctx, &range, field("release_id")?, &sources.release)?;
    equal(ctx, &range, field("network_id")?, &sources.network)?;
    equal(
        ctx,
        &range,
        field("normalized_asset_id")?,
        &sources.normalized_asset,
    )?;
    equal(
        ctx,
        &range,
        field("asset_incarnation")?,
        &sources.incarnation,
    )?;
    equal(
        ctx,
        &range,
        field("reserve_pool_id")?,
        &sources.reserve_pool,
    )?;
    let scale_bytes = assigned_uint_bytes_v1(ctx, gate, sources.scale, 32);
    equal(ctx, &range, field("scale")?, &scale_bytes)?;
    let amount_bytes = assigned_uint_bytes_v1(ctx, gate, sources.amount, 128);
    equal(ctx, &range, field("amount")?, &amount_bytes)?;
    equal(
        ctx,
        &range,
        field("recipient_credential_digest")?,
        &credential_digest,
    )?;
    equal(
        ctx,
        &range,
        field("recipient_account_binding")?,
        &union.cells.fixed_digests[3],
    )?;
    equal(
        ctx,
        &range,
        field("recipient_lane_id")?,
        &union.cells.fixed_digests[5],
    )?;
    equal(ctx, &range, &union.cells.fixed_digests[6], &sources.release)?;
    equal(ctx, &range, &union.cells.fixed_digests[4], &sources.network)?;
    equal(
        ctx,
        &range,
        &union.cells.version,
        &constant_bytes(&1_u16.to_le_bytes()),
    )?;
    let issued = integer(ctx, &range, field("issued_at_ms")?, 64)?;
    let expires = integer(ctx, &range, field("expires_at_ms")?, 64)?;
    range.check_less_than(ctx, issued, expires, 64);
    let ttl = gate.sub(ctx, expires, issued);
    range.range_check(ctx, ttl, 64);
    let excessive = range.is_less_than(
        ctx,
        QuantumCell::Constant(F::from(KAGEMUSHA_REQUEST_MAX_TTL_MS_V1)),
        ttl,
        64,
    );
    gate.assert_is_const(ctx, &excessive, &F::ZERO);
    let zero = gate.is_zero(ctx, issued);
    gate.assert_is_const(ctx, &zero, &F::ZERO);
    let credential_issued = integer(ctx, &range, &union.cells.scalars[2], 64)?;
    let credential_expires = integer(ctx, &range, &union.cells.scalars[3], 64)?;
    for (lower, upper) in [
        (credential_issued, issued),
        (expires, credential_expires),
        (issued, sources.preparation_clock.lower_at_ms),
    ] {
        let bad = range.is_less_than(ctx, upper, lower, 64);
        let selected = gate.mul(ctx, bad, enabled);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    let timely = range.is_less_than(ctx, sources.preparation_clock.upper_at_ms, expires, 64);
    let late = gate.mul_not(ctx, timely, enabled);
    gate.assert_is_const(ctx, &late, &F::ZERO);
    // The full receiver C and optional issuer-signed current lease use the same interval and key.
    constrain_ordinary_integrity_union_v1(
        builder,
        jobs,
        &union.cells,
        &credential_digest,
        union.integrity,
        witness.integrity_lease,
        Some((&issuer_raw, &issuer_cells)),
        issued,
        expires,
    )?;
    let clock_specimen = &witness.request.body.clock_context;
    let ctx = builder.main(0);
    let clock = OrdinaryCashClockCellsV1 {
        nonce: assign_bytes(ctx, &range, &clock_specimen.request_nonce)
            .try_into()
            .map_err(|_| "request clock nonce width")?,
        signed_observations_original_digest: assign_bytes(
            ctx,
            &range,
            &clock_specimen.signed_observations_original_digest,
        )
        .try_into()
        .map_err(|_| "request clock originals width")?,
        lower_at_ms: ctx.load_witness(F::from(clock_specimen.lower_at_ms)),
        upper_at_ms: ctx.load_witness(F::from(clock_specimen.upper_at_ms)),
    };
    let clock_raw = clock_payload(ctx, &range, &clock, clock_specimen)?;
    let mut clock_message = constant_bytes(KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1);
    clock_message.extend_from_slice(&clock_raw);
    let clock_digest = hash(ctx, jobs, clock_message)?;
    equal(ctx, &range, field("clock_context_digest")?, &clock_digest)?;
    let before = range.is_less_than(ctx, clock.lower_at_ms, issued, 64);
    range.gate().assert_is_const(ctx, &before, &F::ZERO);
    range.check_less_than(ctx, clock.upper_at_ms, expires, 64);
    let mut signing_prefix = KAGEMUSHA_ORDINARY_PAYMENT_REQUEST_DOMAIN_V1.to_vec();
    signing_prefix.extend_from_slice(&390_u64.to_le_bytes());
    let (message, _) = fill_transcript(native_transcript, &signing_prefix, parts)?;
    let native_message = witness.request.body.canonical_signing_bytes()?;
    let streams = constrain_ordinary_signed_message_union_v1(
        builder,
        jobs,
        &union.cells,
        witness.credential.subject.app_public_key.as_sec1_bytes(),
        witness.credential.subject.app_release_digest,
        &witness.request.evidence,
        &native_message,
        &message,
        union.apple,
        witness.previous_app_attest_counter,
        Some((enabled, witness.enabled)),
        false,
    )?;
    let original_digest = reconstruct_ordinary_request_canonical_v1(
        builder,
        jobs,
        witness.request,
        &fields,
        &clock,
        &streams.mathematical_codec_original,
        union.apple,
    )?;
    let ctx = builder.main(0);
    let request_digest = select(ctx, &range, enabled, original_digest);
    let credential_digest = select(ctx, &range, enabled, credential_digest);
    equal(
        ctx,
        &range,
        &request_digest,
        &sources.expected_request_digest,
    )?;
    equal(
        ctx,
        &range,
        &credential_digest,
        &sources.expected_recipient_credential_digest,
    )?;
    let as_digest = |name: &str| -> Result<Bytes<F>, String> {
        field(name)?
            .try_into()
            .map_err(|_| "receiver digest width".into())
    };
    Ok(OrdinaryReceiverRequestOpeningV1 {
        request_digest,
        credential_digest,
        encryption_key: select(ctx, &range, enabled, as_digest("recipient_encryption_key")?),
        recipient_lane: select(ctx, &range, enabled, as_digest("recipient_lane_id")?),
        request_id: select(ctx, &range, enabled, as_digest("request_id")?),
    })
}

/// Reconstruct the sole full request original from already-constrained semantic fields and
/// original evidence bytes. Signature authority belongs to the genuine sender Wrapper or the
/// complete receiver signature relation; this byte helper creates neither authority.
pub(super) fn reconstruct_ordinary_request_canonical_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    request: &KagemushaOrdinaryPaymentRequestV1,
    fields: &BTreeMap<&str, Vec<PastaSha256ByteV1<F>>>,
    clock: &OrdinaryCashClockCellsV1<F>,
    codec: &super::canonical_preimage::stream::KagemushaBoundedByteStreamV1<F>,
    apple: AssignedValue<F>,
) -> Result<Bytes<F>, String> {
    let range = builder.range_chip();
    let grammar = request.original_canonical_stream_grammar()?;
    if grammar.variants.len() != 376 {
        return Err("receiver canonical grammar role count differs".into());
    }
    let ctx = builder.main(0);
    let gate = range.gate();
    let mut original_fields = fields.clone();
    original_fields.remove("clock_context_digest");
    original_fields.insert(
        "clock_context.version",
        constant_bytes(&1_u16.to_le_bytes()),
    );
    original_fields.insert("clock_context.request_nonce", clock.nonce.to_vec());
    original_fields.insert(
        "clock_context.signed_observations_original_digest",
        clock.signed_observations_original_digest.to_vec(),
    );
    original_fields.insert(
        "clock_context.lower_at_ms",
        assigned_uint_bytes_v1(ctx, gate, clock.lower_at_ms, 64),
    );
    original_fields.insert(
        "clock_context.upper_at_ms",
        assigned_uint_bytes_v1(ctx, gate, clock.upper_at_ms, 64),
    );
    let variants = grammar
        .variants
        .iter()
        .map(|v| -> Result<_, String> {
            let length_match = gate.is_equal(
                ctx,
                codec.actual_len(),
                QuantumCell::Constant(F::from(v.evidence_length as u64)),
            );
            let platform = if v.apple { apple } else { gate.not(ctx, apple) };
            let selector = gate.mul(ctx, length_match, platform);
            let mut semantic_bytes = BTreeMap::new();
            for f in &v.layout.fields {
                if f.name.starts_with("evidence.") {
                    continue;
                }
                let bytes = original_fields
                    .get(f.name)
                    .ok_or_else(|| format!("receiver original field {} absent", f.name))?;
                if bytes.len() != f.positions.len() {
                    return Err("receiver original field width differs".into());
                }
                for (p, b) in f.positions.iter().copied().zip(bytes.iter().copied()) {
                    if semantic_bytes.insert(p, b).is_some() {
                        return Err("receiver original fields overlap".into());
                    }
                }
            }
            Ok(CanonicalSelectedStreamVariantV1 {
                selector,
                raw_length: v.evidence_length,
                prefix: v.prefix.clone(),
                suffix: v.suffix.clone(),
                complete_length: v.layout.bytes.len(),
                header_crc_bytes: v.header_crc_bytes,
                archive_payload_start: v.archive_payload.start,
                semantic_bytes,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    reconstruct_selected_canonical_stream_v1(
        builder,
        jobs,
        &variants,
        &grammar.repeated_evidence_byte_unit,
        grammar.maximum_prefix_bytes,
        grammar.maximum_suffix_bytes,
        grammar.maximum_stream_bytes,
        codec,
        9,
    )
}

#[cfg(test)]
#[path = "ordinary_receiver_request_opening_tests.rs"]
mod tests;
