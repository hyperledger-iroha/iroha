//! Wire-contract tests for the exact-eight-slot confidential memo envelope.

use iroha_data_model::confidential::{
    CONFIDENTIAL_MEMO_ML_KEM_768_CIPHERTEXT_BYTES_V1,
    CONFIDENTIAL_MEMO_ML_KEM_1024_CIPHERTEXT_BYTES_V1, CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1,
    CONFIDENTIAL_MEMO_WIRE_MAGIC_V1, CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1,
    CONFIDENTIAL_MEMO_XCHACHA_NONCE_BYTES_V1, CONFIDENTIAL_MEMO_XCHACHA_TAG_BYTES_V1,
    ConfidentialMemoEnvelopeV1, ConfidentialMemoRecipientSlotV1, ConfidentialMemoSuiteV1,
};
use norito::{codec::decode_adaptive, core::DecodeFromSlice};

fn slot(index: u8, suite: ConfidentialMemoSuiteV1) -> ConfidentialMemoRecipientSlotV1 {
    ConfidentialMemoRecipientSlotV1::new(
        suite,
        vec![index + 1; suite.encapsulation_bytes()],
        [index + 17; CONFIDENTIAL_MEMO_XCHACHA_NONCE_BYTES_V1],
        [index + 33; CONFIDENTIAL_MEMO_WRAPPED_KEY_BYTES_V1],
    )
    .expect("deterministic slot must be canonical")
}

fn envelope() -> ConfidentialMemoEnvelopeV1 {
    ConfidentialMemoEnvelopeV1::new(
        core::array::from_fn(|index| {
            let suite = if index % 2 == 0 {
                ConfidentialMemoSuiteV1::MlKem768XChaCha20Poly1305
            } else {
                ConfidentialMemoSuiteV1::MlKem1024XChaCha20Poly1305
            };
            slot(u8::try_from(index).expect("slot index fits u8"), suite)
        }),
        [0xA5; CONFIDENTIAL_MEMO_XCHACHA_NONCE_BYTES_V1],
        vec![0x5A; CONFIDENTIAL_MEMO_XCHACHA_TAG_BYTES_V1 + 32],
    )
    .expect("deterministic envelope must be canonical")
}

#[test]
fn adaptive_codec_roundtrips_the_exact_v1_shape() {
    let expected = envelope();
    let bytes = norito::codec::encode_adaptive(&expected);
    let actual: ConfidentialMemoEnvelopeV1 =
        decode_adaptive(&bytes).expect("canonical memo envelope must decode");
    assert_eq!(actual, expected);
    assert_eq!(actual.slots().len(), CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1);
    assert_eq!(
        actual.slots()[0].encapsulation().len(),
        CONFIDENTIAL_MEMO_ML_KEM_768_CIPHERTEXT_BYTES_V1
    );
    assert_eq!(
        actual.slots()[1].encapsulation().len(),
        CONFIDENTIAL_MEMO_ML_KEM_1024_CIPHERTEXT_BYTES_V1
    );
}

#[test]
fn json_roundtrips_the_named_exact_eight_slot_shape() {
    let expected = envelope();
    let json = norito::json::to_json(&expected).expect("encode exact-eight memo JSON");
    let actual: ConfidentialMemoEnvelopeV1 =
        norito::json::from_json(&json).expect("decode exact-eight memo JSON");
    assert_eq!(actual, expected);

    let value = norito::json::to_value(&expected).expect("encode exact-eight memo JSON value");
    let slots = value
        .get("slots")
        .and_then(norito::json::Value::as_object)
        .expect("memo JSON must contain a named slots object");
    assert_eq!(slots.len(), CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1);
    for index in 0..CONFIDENTIAL_MEMO_RECIPIENT_SLOTS_V1 {
        assert!(slots.contains_key(&format!("slot_{index}")));
    }
}

#[test]
fn json_rejects_seven_or_nine_slot_objects() {
    let expected = envelope();
    let mut seven = norito::json::to_value(&expected).expect("encode seven-slot rejection fixture");
    let seven_slots = seven
        .as_object_mut()
        .expect("memo JSON object")
        .get_mut("slots")
        .and_then(norito::json::Value::as_object_mut)
        .expect("named memo slots object");
    assert!(seven_slots.remove("slot_7").is_some());
    assert!(
        norito::json::from_value::<ConfidentialMemoEnvelopeV1>(seven).is_err(),
        "a missing eighth slot must not enter a variable-length compatibility path"
    );

    let mut nine = norito::json::to_value(&expected).expect("encode nine-slot rejection fixture");
    let nine_slots = nine
        .as_object_mut()
        .expect("memo JSON object")
        .get_mut("slots")
        .and_then(norito::json::Value::as_object_mut)
        .expect("named memo slots object");
    let extra = nine_slots
        .get("slot_7")
        .expect("canonical eighth slot")
        .clone();
    assert!(nine_slots.insert("slot_8".to_owned(), extra).is_none());
    assert!(
        norito::json::from_value::<ConfidentialMemoEnvelopeV1>(nine).is_err(),
        "an extra ninth slot must not enter a variable-length compatibility path"
    );
}

#[test]
fn old_single_recipient_wire_is_not_a_v1_candidate() {
    let mut old_wire = vec![1];
    old_wire.extend_from_slice(&[7; 32]);
    old_wire.extend_from_slice(&[2; 24]);
    old_wire.push(16);
    old_wire.extend_from_slice(&[3; 16]);
    let error = ConfidentialMemoEnvelopeV1::decode_from_slice(&old_wire)
        .expect_err("retired wire must fail rather than enter a compatibility decoder");
    assert!(error.to_string().contains("wire magic"));
}

#[test]
fn wire_magic_is_not_an_ordinal_alias() {
    assert_eq!(CONFIDENTIAL_MEMO_WIRE_MAGIC_V1.len(), 8);
    assert_ne!(CONFIDENTIAL_MEMO_WIRE_MAGIC_V1[0], 1);
}
