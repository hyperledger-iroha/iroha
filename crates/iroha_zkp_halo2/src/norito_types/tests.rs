//! Payload-only decoding and original IPA envelope frame contracts.
use super::*;

fn assert_bare_prefix<T>(value: &T)
where
    T: norito::core::SerializePayload
        + for<'de> norito::core::DeserializePayload<'de>
        + for<'de> norito::core::DecodeFromSlice<'de>,
{
    let mut bytes = Vec::new();
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::core::serialize_to_buffer(value, &mut bytes).unwrap();
    for end in 0..bytes.len() {
        assert!(
            T::decode_from_slice(&bytes[..end]).is_err(),
            "truncation at {end}"
        );
    }
    let mut with_tail = bytes.clone();
    with_tail.extend_from_slice(&[0xa5, 0x5a]);
    let (decoded, used) = T::decode_from_slice(&with_tail).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(&with_tail[used..], &[0xa5, 0x5a]);
    let mut reencoded = Vec::new();
    norito::core::serialize_to_buffer(&decoded, &mut reencoded).unwrap();
    assert_eq!(reencoded, bytes);
    let limits = norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, usize::MAX);
    assert!(matches!(
        norito::with_decode_limits(limits, || T::decode_from_slice(&bytes)),
        Err(norito::Error::FieldLengthExceeded { limit: 0, .. })
    ));
    assert!(matches!(
        norito::codec::decode_exact_from_slice::<T>(&with_tail),
        Err(norito::Error::LengthMismatch)
    ));
}

#[test]
fn bare_prefix_decoders_preserve_consumption_and_exact_boundary_limits() {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../tests/fixtures/open_verify_frame.v1.json"
    ))
    .unwrap();
    let frame = hex::decode(fixture["frame_hex"].as_str().unwrap()).unwrap();
    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&frame).unwrap();
    assert_bare_prefix(&envelope.params);
    assert_bare_prefix(&envelope.public);
    assert_bare_prefix(&envelope.proof);
}

#[test]
fn outer_envelope_preserves_captured_identity_frames_and_valid_proof() {
    let fixture: norito::json::Value = norito::json::from_str(include_str!(
        "../../tests/fixtures/open_verify_frame.v1.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["schema"].as_str(),
        Some("iroha.zkp-halo2.open-verify-frame.v1")
    );
    assert_eq!(
        fixture["layout_flags"].as_u64(),
        Some(u64::from(norito::core::default_encode_flags()))
    );
    assert_eq!(
        <OpenVerifyEnvelope as norito::NoritoSchema>::nominal_name(),
        fixture["nominal"].as_str().unwrap()
    );
    assert_eq!(
        <OpenVerifyEnvelope as norito::NoritoSchema>::frame_name(),
        fixture["root"].as_str().unwrap()
    );
    let bytes = |key: &str| hex::decode(fixture[key].as_str().unwrap()).unwrap();
    let hash = norito::schema::identity::frame_hash::<OpenVerifyEnvelope>();
    assert_eq!(hash.as_slice(), bytes("serialize_schema_hash"));
    assert_eq!(hash.as_slice(), bytes("deserialize_schema_hash"));
    let frame = bytes("frame_hex");
    let envelope: OpenVerifyEnvelope = norito::decode_from_bytes(&frame).unwrap();
    assert_eq!(norito::to_bytes(&envelope).unwrap(), frame);
    assert_eq!(envelope.params.encode_bytes(), bytes("params_bare_hex"));
    assert_eq!(envelope.public.encode_bytes(), bytes("public_bare_hex"));
    assert_eq!(envelope.proof.encode_bytes(), bytes("proof_bare_hex"));
    assert!(
        crate::batch::verify_open_batch(&[envelope.clone()])[0]
            .as_ref()
            .unwrap()
    );
    assert_eq!(
        norito::to_bytes(&None::<OpenVerifyEnvelope>).unwrap(),
        bytes("option_none_hex")
    );
    assert_eq!(
        norito::to_bytes(&Some(envelope.clone())).unwrap(),
        bytes("option_some_hex")
    );
    assert!(
        norito::decode_from_bytes::<Option<OpenVerifyEnvelope>>(&bytes("option_none_hex"))
            .unwrap()
            .is_none()
    );
    let some = norito::decode_from_bytes::<Option<OpenVerifyEnvelope>>(&bytes("option_some_hex"))
        .unwrap()
        .unwrap();
    assert_eq!(norito::to_bytes(&some).unwrap(), frame);
    assert_eq!(
        norito::to_bytes(&Vec::<OpenVerifyEnvelope>::new()).unwrap(),
        bytes("vec_empty_hex")
    );
    assert_eq!(
        norito::to_bytes(&vec![envelope.clone(), envelope]).unwrap(),
        bytes("vec_two_hex")
    );
    assert!(
        norito::decode_from_bytes::<Vec<OpenVerifyEnvelope>>(&bytes("vec_empty_hex"))
            .unwrap()
            .is_empty()
    );
    let batch =
        norito::decode_from_bytes::<Vec<OpenVerifyEnvelope>>(&bytes("vec_two_hex")).unwrap();
    assert_eq!(batch.len(), 2);
    for value in &batch {
        assert_eq!(norito::to_bytes(value).unwrap(), frame);
    }
    assert!(
        crate::batch::verify_open_batch(&batch)
            .iter()
            .all(|result| matches!(result, Ok(true)))
    );
    let mut wrong_identity = frame.clone();
    wrong_identity[6] ^= 1;
    assert!(matches!(
        norito::decode_from_bytes::<OpenVerifyEnvelope>(&wrong_identity),
        Err(norito::Error::SchemaMismatch)
    ));
    for end in 0..frame.len() {
        assert!(
            norito::decode_from_bytes::<OpenVerifyEnvelope>(&frame[..end]).is_err(),
            "truncation at {end}"
        );
    }
}
