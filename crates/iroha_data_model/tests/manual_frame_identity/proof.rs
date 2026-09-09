//! Captured public proof frames and checked reconstruction contracts.

use std::fmt::Debug;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Hash, HashOf, MerkleProof, privacy::LaneCommitmentId};
use iroha_data_model::{
    kaigi::scalar::KaigiAuthorizationScalarV1,
    nexus::{
        LANE_PRIVACY_MAX_MERKLE_DEPTH_V1, LanePrivacyMerkleWitness, LanePrivacyProof,
        LanePrivacyWitness,
    },
    proof::{
        PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1,
        PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1, ProofAttachment, ProofAttachmentList,
        ProofAttachmentListError, ProofBox, VerifyingKeyId,
    },
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{DecodeAll as _, Encode as _},
    core as ncore,
    json::{JsonDeserialize, JsonSerialize, Value},
};

use crate::frame_identity_test_support::record;

fn family<T>(rows: &mut Vec<Value>, name: &str, values: &[T])
where
    T: norito::NoritoSchema
        + Clone
        + Debug
        + PartialEq
        + NoritoSerialize
        + for<'de> NoritoDeserialize<'de>
        + JsonSerialize
        + JsonDeserialize,
{
    assert!(!values.is_empty());
    for (index, value) in values.iter().enumerate() {
        assert!(
            !values[..index].contains(value),
            "distinct populated variants"
        );
        record(rows, &format!("{name}/root_{index}"), value);
        record(
            rows,
            &format!("{name}/option_{index}"),
            &Some(value.clone()),
        );
    }
    record(rows, &format!("{name}/option_none"), &None::<T>);
    record(rows, &format!("{name}/vec_empty"), &Vec::<T>::new());
    record(rows, &format!("{name}/vec_all"), &values.to_vec());
}

fn assert_json<T>(value: &T)
where
    T: Debug + PartialEq + NoritoSerialize + JsonSerialize + JsonDeserialize,
{
    let json = norito::json::to_json(value).unwrap();
    let decoded: T = norito::json::from_json(&json).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(decoded.encode(), value.encode());
    assert_eq!(norito::json::to_json(&decoded).unwrap(), json);
}

fn reject_payload<T>(bytes: &[u8])
where
    T: Debug + NoritoSerialize + for<'de> NoritoDeserialize<'de>,
{
    let flags = ncore::default_encode_flags();
    let frame = ncore::frame_bare_with_header_flags::<T>(bytes, flags)
        .expect("frame invalid fields with their correct header, length and checksum");
    let view = ncore::from_bytes_view(&frame).expect("authenticate the malformed-field frame");
    assert_eq!(view.as_bytes(), bytes);
    view.decode_exact_with(ncore::decode_field_canonical::<T>)
        .expect_err("checked reconstruction must reject malformed fields");
    assert!(norito::decode_canonical::<T>(&frame).is_err());
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let _payload = ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), flags);
    let archived = ncore::from_bytes::<T>(&frame).expect("authenticate typed archive metadata");
    <T as ncore::DeserializePayload<'_>>::try_deserialize(archived)
        .expect_err("the owner's fallible decoder must reject without panicking");
}

fn attachment(bytes: Vec<u8>) -> ProofAttachment {
    let value = ProofAttachment::new_ref(
        "halo2/ipa".into(),
        ProofBox::new("halo2/ipa".into(), bytes),
        VerifyingKeyId::new("halo2/ipa", "capture_vk"),
    );
    assert_eq!(value.structural_error(), None);
    value
}

fn proof_values(rows: &mut Vec<Value>) -> Vec<ProofAttachment> {
    // ProofBox deliberately carries opaque backend bytes. This is a wire-shape
    // fixture, not evidence that a zero-knowledge proof verifies in an engine.
    let plain = attachment(vec![1, 2, 3, 4]);
    let mut commitment = plain.clone();
    commitment.vk_commitment = Some([0x17; 32]);
    let mut envelope = plain.clone();
    envelope.envelope_hash = Some(Hash::new(&envelope.proof.bytes).into());
    let lane = LanePrivacyProof::merkle_from_raw_path(
        LaneCommitmentId::new(5),
        [0x21; 32],
        0,
        vec![Some(Hash::new([0x35; 32]).into())],
    )
    .expect("complete canonical Merkle witness shape");
    lane.validate_structure_v1().unwrap();
    let mut lane_only = plain.clone();
    lane_only.lane_privacy = Some(lane.clone());
    let mut complete = envelope.clone();
    complete.vk_commitment = commitment.vk_commitment;
    complete.lane_privacy = Some(lane);
    let values = vec![plain, commitment, envelope, lane_only, complete];
    for value in &values {
        assert_eq!(value.structural_error(), None);
        assert_json(value);
    }
    family(rows, "proof_attachment", &values);
    values
}

fn append_field<T: ncore::SerializePayload>(bytes: &mut Vec<u8>, value: &T) {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let mut field = Vec::new();
    ncore::serialize_to_buffer(value, &mut field).unwrap();
    ncore::write_len_header_to_vec(bytes, u64::try_from(field.len()).unwrap());
    bytes.extend_from_slice(&field);
}

fn proof_rejections(values: &[ProofAttachment]) {
    let plain = &values[0];
    let mut invalid = Vec::new();
    let mut value = plain.clone();
    value.proof.backend = "stark/fri".into();
    invalid.push(value);
    let mut value = plain.clone();
    value.vk_ref.backend = "stark/fri".into();
    invalid.push(value);
    for name in ["", "invalid key name"] {
        let mut value = plain.clone();
        value.vk_ref.name = name.into();
        invalid.push(value);
    }
    let mut value = plain.clone();
    value.proof.bytes.clear();
    invalid.push(value);
    let mut value = plain.clone();
    value.vk_commitment = Some([0; 32]);
    invalid.push(value);
    for hash in [[0; 32], Hash::new([0x99; 8]).into()] {
        let mut value = plain.clone();
        assert_ne!(Some(hash), Some(Hash::new(&value.proof.bytes).into()));
        value.envelope_hash = Some(hash);
        invalid.push(value);
    }
    let sibling = HashOf::<[u8; 32]>::new(&[0x39; 32]);
    for (index, path) in [
        (0, Vec::new()),
        (0, vec![None]),
        (2, vec![Some(sibling)]),
        (0, vec![Some(sibling); LANE_PRIVACY_MAX_MERKLE_DEPTH_V1 + 1]),
    ] {
        let mut value = plain.clone();
        value.lane_privacy = Some(LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(5),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0x21; 32],
                proof: MerkleProof::from_audit_path(index, path),
            }),
        });
        invalid.push(value);
    }
    for value in invalid {
        assert!(value.structural_error().is_some());
        reject_payload::<ProofAttachment>(&value.encode());
        let json = norito::json::to_json(&value).unwrap();
        assert!(norito::json::from_json::<ProofAttachment>(&json).is_err());
    }
    // Each added None is a complete, well-formed field, but a redundant tail
    // would give the same attachment a second binary spelling.
    for value in &values[..3] {
        let mut redundant = value.encode();
        if value.envelope_hash.is_some() {
            append_field(&mut redundant, &None::<LanePrivacyProof>);
        } else {
            append_field(&mut redundant, &None::<[u8; 32]>);
        }
        reject_payload::<ProofAttachment>(&redundant);
    }
    let mut missing_key = Vec::new();
    append_field(&mut missing_key, &plain.backend);
    append_field(&mut missing_key, &plain.proof);
    reject_payload::<ProofAttachment>(&missing_key);
}

fn list_payload(attachments: &Vec<ProofAttachment>) -> Vec<u8> {
    let field = attachments.encode();
    // The control decoder must accept the entire supplied element sequence.
    // Invalid list cardinality or size cannot be hidden behind a truncated Vec.
    let mut input = field.as_slice();
    let decoded = Vec::<ProofAttachment>::decode_all(&mut input).unwrap();
    assert_eq!(&decoded, attachments);
    assert!(input.is_empty());
    let mut bytes = Vec::new();
    append_field(&mut bytes, attachments);
    bytes
}

fn reject_list_json(payload: &[u8]) {
    let frame = ncore::frame_bare_with_header_flags::<ProofAttachmentList>(
        payload,
        ncore::default_encode_flags(),
    )
    .unwrap();
    let json = norito::json::to_json(&STANDARD.encode(frame)).unwrap();
    assert!(norito::json::from_json::<ProofAttachmentList>(&json).is_err());
}

fn list_values_and_cardinality(rows: &mut Vec<Value>, values: &[ProofAttachment]) {
    // An empty ProofAttachmentList is invalid; absent/empty outer containers
    // are recorded by family without manufacturing an invalid list value.
    assert!(matches!(
        ProofAttachmentList::try_from(Vec::new()),
        Err(ProofAttachmentListError::Empty)
    ));
    let empty = list_payload(&Vec::new());
    reject_payload::<ProofAttachmentList>(&empty);
    reject_list_json(&empty);
    let singleton = ProofAttachmentList::try_from(vec![values[0].clone()]).unwrap();
    let populated = ProofAttachmentList::try_from(values.to_vec()).unwrap();
    assert_eq!(singleton.as_slice(), &values[..1]);
    assert_eq!(populated.as_slice(), values);
    for list in [&singleton, &populated] {
        assert!(!list.is_empty());
        assert_json(list);
        assert_eq!(list.encode(), list_payload(&list.clone().into_vec()));
    }
    family(rows, "proof_attachment_list", &[singleton, populated]);
    let mut maximum = ProofAttachmentList::try_from(vec![
        values[0].clone();
        PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1
    ])
    .unwrap();
    let before = norito::encode_canonical(&maximum).unwrap();
    assert_eq!(
        norito::decode_canonical::<ProofAttachmentList>(&before).unwrap(),
        maximum
    );
    let excessive = vec![values[0].clone(); PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1];
    assert!(matches!(
        ProofAttachmentList::try_from(excessive.clone()),
        Err(ProofAttachmentListError::TooMany { actual, maximum })
            if actual == PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1 + 1
                && maximum == PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1
    ));
    let bytes = list_payload(&excessive);
    reject_payload::<ProofAttachmentList>(&bytes);
    reject_list_json(&bytes);
    assert!(matches!(
        maximum.try_push(values[1].clone()),
        Err(ProofAttachmentListError::TooMany { .. })
    ));
    assert_eq!(norito::encode_canonical(&maximum).unwrap(), before);
    assert_eq!(maximum.len(), PROOF_ATTACHMENT_LIST_MAX_ATTACHMENTS_V1);
}

fn list_size_boundary() {
    let ceiling = PROOF_ATTACHMENT_LIST_MAX_CANONICAL_FRAME_BYTES_V1;
    let flags = ncore::default_encode_flags();
    let initial = list_payload(&vec![attachment(vec![0x5a; ceiling])]);
    let initial_frame =
        ncore::frame_bare_with_header_flags::<ProofAttachmentList>(&initial, flags).unwrap();
    let overhead = initial_frame.len().checked_sub(ceiling).unwrap();
    drop(initial_frame);
    drop(initial);
    let proof_len = ceiling.checked_sub(overhead).unwrap();
    let mut exact = ProofAttachmentList::try_from(vec![attachment(vec![0x5a; proof_len])]).unwrap();
    let exact_frame = norito::encode_canonical(&exact).unwrap();
    // The complete emitted frame proves the boundary; no hint can admit it.
    assert_eq!(exact_frame.len(), ceiling);
    assert_eq!(ncore::encoded_frame_len(&exact).unwrap(), ceiling);
    assert_eq!(
        norito::decode_canonical::<ProofAttachmentList>(&exact_frame).unwrap(),
        exact
    );
    let excessive = vec![attachment(vec![0x5a; proof_len + 1])];
    assert!(matches!(
        ProofAttachmentList::try_from(excessive.clone()),
        Err(ProofAttachmentListError::CanonicalFrameTooLarge { actual, maximum })
            if actual == ceiling + 1 && maximum == ceiling
    ));
    let bytes = list_payload(&excessive);
    let frame = ncore::frame_bare_with_header_flags::<ProofAttachmentList>(&bytes, flags).unwrap();
    assert_eq!(frame.len(), ceiling + 1);
    // All proof fields and the complete Vec have already decoded successfully;
    // only the list's intrinsic complete-frame limit disallows this payload.
    reject_payload::<ProofAttachmentList>(&bytes);
    assert!(matches!(
        exact.try_push(attachment(vec![0x44])),
        Err(ProofAttachmentListError::CanonicalFrameTooLarge { .. })
    ));
    assert_eq!(norito::encode_canonical(&exact).unwrap(), exact_frame);
    assert_eq!(exact.len(), 1);
}

fn scalar_values(rows: &mut Vec<Value>) {
    // Source-defined Pasta Fp modulus, not a guessed frame identity or hash.
    let modulus: [u8; 32] =
        hex::decode("01000000ed302d991bf94c09fc98462200000000000000000000000000000040")
            .unwrap()
            .try_into()
            .unwrap();
    let mut maximum = modulus;
    maximum[0] -= 1;
    let values = [[0; 32], [0x24; 32], maximum].map(|bytes| {
        let scalar = KaigiAuthorizationScalarV1::from_le_bytes(bytes).unwrap();
        assert_eq!(scalar.to_le_bytes(), bytes);
        assert_eq!(scalar.encode(), bytes);
        assert_json(&scalar);
        scalar
    });
    assert_eq!(values[0], KaigiAuthorizationScalarV1::default());
    family(rows, "kaigi_authorization_scalar", &values);
    let mut above = modulus;
    above[0] += 1;
    for bytes in [modulus, above, [0xff; 32]] {
        assert!(KaigiAuthorizationScalarV1::from_le_bytes(bytes).is_none());
        reject_payload::<KaigiAuthorizationScalarV1>(&bytes);
        let json = norito::json::to_json(&bytes.to_vec()).unwrap();
        assert!(norito::json::from_json::<KaigiAuthorizationScalarV1>(&json).is_err());
    }
    for length in 0..32 {
        reject_payload::<KaigiAuthorizationScalarV1>(&[0x24; 32][..length]);
    }
    for length in [31, 33] {
        let json = norito::json::to_json(&vec![0_u8; length]).unwrap();
        assert!(norito::json::from_json::<KaigiAuthorizationScalarV1>(&json).is_err());
    }
    let mut marked = values[1].to_le_bytes();
    marked[31] |= 1;
    let marked = KaigiAuthorizationScalarV1::from_le_bytes(marked).unwrap();
    assert_ne!(
        marked, values[1],
        "the hash marker bit is scalar value, not normalization"
    );
}

fn capture_values() -> Vec<Value> {
    let mut rows = Vec::new();
    let values = proof_values(&mut rows);
    proof_rejections(&values);
    list_values_and_cardinality(&mut rows, &values);
    list_size_boundary();
    scalar_values(&mut rows);
    assert_eq!(rows.len(), 29);
    rows
}

#[test]
fn manual_proof_frames_match_capture() {
    let rows = capture_values();
    let evidence = norito::json!({
        "format_version": 1,
        "purpose": "public proof and Kaigi scalar owners before identity declaration",
        "default_encode_flags": (ncore::default_encode_flags()),
        "rows": rows,
    });
    let expected: Value = norito::json::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/manual_proof_identity_frames.json"
    )))
    .expect("immutable pre-declaration proof capture");
    assert_eq!(evidence, expected);
}
