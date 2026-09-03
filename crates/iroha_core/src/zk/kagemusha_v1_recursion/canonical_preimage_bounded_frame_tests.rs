//! Bounded canonical Norito framing and compact-length constraint regressions.

use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
use halo2_proofs::{
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
};

use super::*;
use crate::zk::kagemusha_v1_recursion::guard_bundle::assign_bytes;

const FRAME_TEST_K: u32 = 13;
const FRAME_TEST_LOOKUP_BITS: usize = 8;

fn framing_template_and_payload(frame: &[u8]) -> (Vec<Option<u8>>, Vec<u8>) {
    assert!(frame.len() >= HEADER_BYTES);
    let payload_len = usize::try_from(u64::from_le_bytes(
        frame[PAYLOAD_LENGTH_RANGE]
            .try_into()
            .expect("eight-byte payload length"),
    ))
    .expect("test payload length fits usize");
    let payload_start = frame
        .len()
        .checked_sub(payload_len)
        .expect("declared payload fits frame");
    assert!(payload_start >= HEADER_BYTES);
    assert!(
        frame[HEADER_BYTES..payload_start]
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(
        u64::from_le_bytes(
            frame[CHECKSUM_RANGE]
                .try_into()
                .expect("eight-byte checksum"),
        ),
        norito::crc64_fallback(&frame[payload_start..]),
    );
    let mut template = frame[..payload_start]
        .iter()
        .copied()
        .map(Some)
        .collect::<Vec<_>>();
    template[PAYLOAD_LENGTH_RANGE].fill(None);
    template[CHECKSUM_RANGE].fill(None);
    (template, frame[payload_start..].to_vec())
}

fn bounded_frame_builder<F: KagemushaPoseidonFieldV1>(
    template: &[Option<u8>],
    payload: &[u8],
    actual_len: F,
) -> BaseCircuitBuilder<F> {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(FRAME_TEST_K as usize)
        .use_lookup_bits(FRAME_TEST_LOOKUP_BITS)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let payload_bytes = assign_bytes(ctx, &range, payload);
    let assigned_len = ctx.load_witness(actual_len);
    let payload = KagemushaBoundedByteStreamV1::constrain(ctx, &range, payload_bytes, assigned_len)
        .expect("bounded payload");
    let frame = assemble_bounded_canonical_frame_template_v1(ctx, &range, template, &payload)
        .expect("bounded canonical frame");
    let mut instances = vec![assigned_len, frame.actual_len()];
    instances.extend(
        frame
            .bytes()
            .iter()
            .map(|byte| byte.assigned().expect("assigned frame byte")),
    );
    builder.assigned_instances = vec![instances];
    builder.calculate_params(Some(9));
    builder
}

fn bounded_frame_case<F: KagemushaPoseidonFieldV1>(
    template: &[Option<u8>],
    payload: &[u8],
    actual_len: usize,
    public_payload_len: usize,
    public_frame: &[u8],
) -> bool {
    let builder = bounded_frame_builder(template, payload, F::from(actual_len as u64));
    let capacity = template.len() + payload.len();
    let mut expected_frame = public_frame.to_vec();
    expected_frame.resize(capacity, 0);
    let mut instances = vec![
        F::from(public_payload_len as u64),
        F::from((template.len() + public_payload_len) as u64),
    ];
    instances.extend(
        expected_frame
            .into_iter()
            .map(|byte| F::from(u64::from(byte))),
    );
    MockProver::run(FRAME_TEST_K, &builder, vec![instances])
        .expect("bounded-frame mock prover")
        .verify()
        .is_ok()
}

fn assert_frames_match_canonical_encoder<F: KagemushaPoseidonFieldV1>() {
    let unit_frame = norito::encode_canonical(&()).expect("canonical unit frame");
    let (unit_template, unit_payload) = framing_template_and_payload(&unit_frame);
    assert!(unit_payload.is_empty());
    assert!(bounded_frame_case::<F>(
        &unit_template,
        &unit_payload,
        0,
        0,
        &unit_frame,
    ));

    // u128 requires eight bytes of root alignment padding after the forty-byte header.
    let aligned_frame = norito::encode_canonical(&u128::MAX).expect("canonical aligned frame");
    let (aligned_template, aligned_payload) = framing_template_and_payload(&aligned_frame);
    assert_eq!(aligned_template.len(), HEADER_BYTES + 8);
    assert!(bounded_frame_case::<F>(
        &aligned_template,
        &aligned_payload,
        aligned_payload.len(),
        aligned_payload.len(),
        &aligned_frame,
    ));

    let frames = ["", "x", "fixed topology payload"]
        .map(|value| norito::encode_canonical(&value.to_owned()).expect("canonical string frame"));
    let parts = frames
        .iter()
        .map(|frame| framing_template_and_payload(frame))
        .collect::<Vec<_>>();
    let template = &parts[0].0;
    assert!(parts.iter().all(|(candidate, _)| candidate == template));
    let capacity = parts
        .iter()
        .map(|(_, payload)| payload.len())
        .max()
        .expect("string fixtures");
    for (frame, (_, active_payload)) in frames.iter().zip(&parts) {
        let mut payload = active_payload.clone();
        payload.resize(capacity, 0);
        assert!(bounded_frame_case::<F>(
            template,
            &payload,
            active_payload.len(),
            active_payload.len(),
            frame,
        ));
    }
}

fn assert_model_descriptor_drives_frame<F: KagemushaPoseidonFieldV1>() {
    use iroha_data_model::kagemusha::{
        KagemushaPairedProofV1, kagemusha_canonical_mint_frame_prefix_v1,
    };

    let proof = KagemushaPairedProofV1 {
        version: 1,
        eq_protocol_digest: [1; 32],
        ep_protocol_digest: [2; 32],
        semantic_digest: [3; 32],
        guard_eq_credential_audit: [4; 32],
        guard_ep_credential_audit: [5; 32],
        eq_deferred_audit: [6; 32],
        ep_deferred_audit: [7; 32],
        eq_proof: Vec::new(),
        ep_proof: Vec::new(),
        eq_history: Vec::new(),
        ep_history: Vec::new(),
    };
    let canonical = norito::encode_canonical(&proof).expect("canonical paired-proof frame");
    let (template, payload_bytes) = framing_template_and_payload(&canonical);
    let framing =
        kagemusha_canonical_mint_frame_prefix_v1(&proof).expect("model framing descriptor");
    assert_eq!(framing.bytes(), template);

    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(16)
        .use_lookup_bits(FRAME_TEST_LOOKUP_BITS);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let assigned_payload = assign_bytes(ctx, &range, &payload_bytes);
    let payload_len = ctx.load_witness(F::from(payload_bytes.len() as u64));
    let payload =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, assigned_payload, payload_len)
            .expect("model payload");
    let assembled = assemble_bounded_canonical_frame_v1(ctx, &range, &framing, &payload)
        .expect("model-owned framing");
    assert_eq!(
        assembled
            .bytes()
            .iter()
            .map(|byte| byte.test_value())
            .collect::<Vec<_>>(),
        canonical,
    );
}

#[test]
fn bounded_frame_matches_actual_canonical_frames_at_boundaries_in_both_fields() {
    assert_frames_match_canonical_encoder::<Fp>();
    assert_frames_match_canonical_encoder::<Fq>();
    assert_model_descriptor_drives_frame::<Fp>();
    assert_model_descriptor_drives_frame::<Fq>();
}

fn assert_frame_substitutions_fail<F: KagemushaPoseidonFieldV1>() {
    let frame = norito::encode_canonical(&"canonical frame".to_owned())
        .expect("canonical substitution fixture");
    let (template, active_payload) = framing_template_and_payload(&frame);
    let mut payload = active_payload.clone();
    payload.resize(active_payload.len() + 3, 0);
    assert!(bounded_frame_case::<F>(
        &template,
        &payload,
        active_payload.len(),
        active_payload.len(),
        &frame,
    ));

    let mut substituted_payload = payload.clone();
    let last = active_payload.len() - 1;
    substituted_payload[last] ^= 0x40;
    assert!(!bounded_frame_case::<F>(
        &template,
        &substituted_payload,
        active_payload.len(),
        active_payload.len(),
        &frame,
    ));

    let mut substituted_length = frame.clone();
    substituted_length[PAYLOAD_LENGTH_RANGE.start] ^= 1;
    assert!(!bounded_frame_case::<F>(
        &template,
        &payload,
        active_payload.len(),
        active_payload.len(),
        &substituted_length,
    ));

    let mut substituted_checksum = frame.clone();
    substituted_checksum[CHECKSUM_RANGE.start + 3] ^= 0x80;
    assert!(!bounded_frame_case::<F>(
        &template,
        &payload,
        active_payload.len(),
        active_payload.len(),
        &substituted_checksum,
    ));

    let mut nonzero_tail = payload;
    nonzero_tail[active_payload.len()] = 1;
    assert!(!bounded_frame_case::<F>(
        &template,
        &nonzero_tail,
        active_payload.len(),
        active_payload.len(),
        &frame,
    ));
}

#[test]
fn bounded_frame_rejects_payload_length_checksum_and_tail_substitution_in_both_fields() {
    assert_frame_substitutions_fail::<Fp>();
    assert_frame_substitutions_fail::<Fq>();
}

fn compact_length_builder<F: KagemushaPoseidonFieldV1>(length: F) -> BaseCircuitBuilder<F> {
    let mut builder = BaseCircuitBuilder::<F>::default()
        .use_k(10)
        .use_lookup_bits(FRAME_TEST_LOOKUP_BITS)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let length = ctx.load_witness(length);
    let encoded = canonical_compact_length_u14_stream_v1(ctx, &range, length)
        .expect("canonical compact length");
    let mut instances = vec![encoded.actual_len()];
    instances.extend(
        encoded
            .bytes()
            .iter()
            .map(|byte| byte.assigned().expect("assigned compact byte")),
    );
    builder.assigned_instances = vec![instances];
    builder.calculate_params(Some(9));
    builder
}

fn compact_length_case<F: KagemushaPoseidonFieldV1>(
    length: usize,
    public_len: usize,
    public_bytes: [u8; 2],
) -> bool {
    let builder = compact_length_builder::<F>(F::from(length as u64));
    let instances = vec![
        F::from(public_len as u64),
        F::from(u64::from(public_bytes[0])),
        F::from(u64::from(public_bytes[1])),
    ];
    MockProver::run(10, &builder, vec![instances])
        .expect("compact-length mock prover")
        .verify()
        .is_ok()
}

fn assert_compact_lengths_match_canonical_encoder<F: KagemushaPoseidonFieldV1>() {
    for length in [0, 127, 128, 16_383] {
        let value = "x".repeat(length);
        let frame = norito::encode_canonical(&value).expect("canonical string boundary frame");
        let (_, payload) = framing_template_and_payload(&frame);
        let encoded_len = if length < 128 { 1 } else { 2 };
        let mut expected = [0; 2];
        expected[..encoded_len].copy_from_slice(&payload[..encoded_len]);
        assert!(compact_length_case::<F>(length, encoded_len, expected));
    }
    assert!(!compact_length_case::<F>(128, 2, [0, 1]));
    assert!(!compact_length_case::<F>(16_384, 2, [0x80, 0x80]));
}

#[test]
fn compact_u14_length_matches_canonical_127_128_and_16383_boundaries_in_both_fields() {
    assert_compact_lengths_match_canonical_encoder::<Fp>();
    assert_compact_lengths_match_canonical_encoder::<Fq>();
}

#[test]
fn bounded_frame_template_rejects_every_noncanonical_hole_pattern() {
    let frame = norito::encode_canonical(&()).expect("canonical unit frame");
    let (template, payload_bytes) = framing_template_and_payload(&frame);
    let mut builder = BaseCircuitBuilder::<Fp>::default()
        .use_k(10)
        .use_lookup_bits(FRAME_TEST_LOOKUP_BITS);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let payload_len = ctx.load_witness(Fp::from(0));
    let assigned_payload_bytes = assign_bytes(ctx, &range, &payload_bytes);
    let payload =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, assigned_payload_bytes, payload_len)
            .expect("empty payload");

    let mut missing_fixed_byte = template.clone();
    missing_fixed_byte[0] = None;
    assert!(
        assemble_bounded_canonical_frame_template_v1(ctx, &range, &missing_fixed_byte, &payload)
            .is_err()
    );
    let mut prefilled_length = template.clone();
    prefilled_length[PAYLOAD_LENGTH_RANGE.start] = Some(0);
    assert!(
        assemble_bounded_canonical_frame_template_v1(ctx, &range, &prefilled_length, &payload)
            .is_err()
    );
    let mut prefilled_checksum = template;
    prefilled_checksum[CHECKSUM_RANGE.start] = Some(0);
    assert!(
        assemble_bounded_canonical_frame_template_v1(ctx, &range, &prefilled_checksum, &payload)
            .is_err()
    );
    assert!(
        assemble_bounded_canonical_frame_template_v1(
            ctx,
            &range,
            &[None; HEADER_BYTES - 1],
            &payload,
        )
        .is_err()
    );
    let mut wrong_magic = prefilled_checksum.clone();
    wrong_magic[0] = Some(wrong_magic[0].expect("fixed magic") ^ 1);
    wrong_magic[CHECKSUM_RANGE.start] = None;
    assert!(
        assemble_bounded_canonical_frame_template_v1(ctx, &range, &wrong_magic, &payload).is_err()
    );
    let mut unsupported_flags = prefilled_checksum.clone();
    unsupported_flags[CHECKSUM_RANGE.start] = None;
    unsupported_flags[HEADER_BYTES - 1] = Some(0x80);
    assert!(
        assemble_bounded_canonical_frame_template_v1(ctx, &range, &unsupported_flags, &payload)
            .is_err()
    );
    let aligned_frame = norito::encode_canonical(&u128::MAX).expect("canonical aligned frame");
    let (mut nonzero_alignment, _) = framing_template_and_payload(&aligned_frame);
    nonzero_alignment[HEADER_BYTES] = Some(1);
    assert!(
        assemble_bounded_canonical_frame_template_v1(ctx, &range, &nonzero_alignment, &payload)
            .is_err()
    );
}
