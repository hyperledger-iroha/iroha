//! Custody admission preserves the existing native authorization and zeroizing owners.

use super::*;

#[test]
fn reservation_roundtrip_keeps_exact_original_inputs_and_rejects_changed_intent() {
    let (intent, _, _, original) = atomic_private_settlement_reservation_fixture_v1(None);
    let (material, expected) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).unwrap();
    assert_eq!(
        expected.input_ids.len(),
        1,
        "virtual dummy inputs cannot reserve another note"
    );
    let saved = Zeroizing::new(material.to_vec());
    let (restored, observed) =
        decode_atomic_private_settlement_reservation_v1(material, &intent, 0).unwrap();
    assert_eq!(expected, observed);
    assert_eq!(restored.plaintext, original.plaintext);
    let (encoded, again) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &restored).unwrap();
    assert_eq!(*saved, *encoded);
    assert_eq!(expected, again);
    assert!(decode_atomic_private_settlement_reservation_v1(encoded, &intent, 1).is_err());
    let mut changed = intent;
    changed.material.expiry_height += 1;
    assert!(decode_atomic_private_settlement_reservation_v1(saved, &changed, 0).is_err());
}

#[test]
fn reservation_rejects_substituted_secret_or_authorizations_before_encoding() {
    let (intent, _, _, mut original) = atomic_private_settlement_reservation_fixture_v1(None);
    original.inputs[0].spending_secret[0] ^= 1;
    assert!(encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).is_err());
    let (intent, _, _, mut original) = atomic_private_settlement_reservation_fixture_v1(None);
    original.plaintext.payer_authorization.signatures.clear();
    assert!(encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).is_err());
    let (intent, _, _, mut original) = atomic_private_settlement_reservation_fixture_v1(None);
    original.plaintext.outputs[0]
        .view_key_authorization
        .signatures
        .clear();
    assert!(encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).is_err());
}

#[test]
fn reservation_canonical_frame_rejects_corruption_and_oversize() {
    let (intent, _, _, original) = atomic_private_settlement_reservation_fixture_v1(None);
    let (mut material, _) =
        encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).unwrap();
    let last = material.len() - 1;
    material[last] ^= 1;
    assert!(decode_atomic_private_settlement_reservation_v1(material, &intent, 0).is_err());
    assert!(
        decode_atomic_private_settlement_reservation_v1(
            Zeroizing::new(vec![
                0;
                ATOMIC_PRIVATE_SETTLEMENT_RESERVATION_MAX_BYTES_V1 + 1
            ]),
            &intent,
            0
        )
        .is_err()
    );
}

#[test]
fn active_input_identity_excludes_bundle_and_route_but_includes_pool_namespace() {
    let (intent, _, _, original) = atomic_private_settlement_reservation_fixture_v1(None);
    let commitment = original.plaintext.inputs[0].commitment;
    let original_id = input_id(&intent, 0, commitment).unwrap();
    let mut changed = intent.clone();
    changed.bundle_id = Hash::new(b"another pending payment");
    changed.material.legs[0].route.lane_incarnation = Hash::new(b"later committee incarnation");
    assert_eq!(original_id, input_id(&changed, 0, commitment).unwrap());
    changed.material.legs[0].pool_id = iroha_data_model::privacy::PrivacyPoolIdV1::new([92; 32]);
    assert_ne!(original_id, input_id(&changed, 0, commitment).unwrap());
}

#[test]
fn partially_decoded_secret_fields_are_wiped_before_outer_record_exists() {
    struct Refusal;
    impl SerializePayload for Refusal {
        fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
            0_u8.serialize(encoder)
        }
    }
    impl<'a> DeserializePayload<'a> for Refusal {
        fn deserialize(_: &'a Archived<Self>) -> Self {
            panic!("test only uses fallible decode")
        }
        fn try_deserialize(_: &'a Archived<Self>) -> Result<Self, norito::Error> {
            Err(norito::Error::Message(
                "deliberate later field refusal".into(),
            ))
        }
    }
    impl<'a> DecodeFromSlice<'a> for Refusal {
        fn decode_from_slice(_: &'a [u8]) -> Result<(Self, usize), norito::Error> {
            Err(norito::Error::Message(
                "deliberate later field refusal".into(),
            ))
        }
    }
    #[derive(Encode, Decode, norito::NoritoSchema)]
    #[norito_schema(name = "iroha_core::atomic_private_settlement::wallet::tests::InterruptedV1")]
    struct Interrupted {
        secret: [SecretByte; 32],
        later: Refusal,
    }
    let original = Interrupted {
        secret: core::array::from_fn(|_| SecretByte(0x81)),
        later: Refusal,
    };
    let material = Zeroizing::new(norito::encode_canonical(&original).unwrap());
    DESTROYED_SECRET_BYTES.with(|count| count.set(0));
    assert!(norito::decode_canonical::<Interrupted>(&material).is_err());
    assert_eq!(
        DESTROYED_SECRET_BYTES.with(std::cell::Cell::get),
        32,
        "all first-field bytes must be owned and wiped when decoding the later field fails"
    );
}

#[test]
fn reservation_checks_real_payer_and_view_signatures_after_shape_validation() {
    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    let wrong = KeyPair::from_seed(vec![0xEF; 32], Algorithm::Ed25519);
    for payer in [true, false] {
        let (intent, _, _, mut original) = atomic_private_settlement_reservation_fixture_v1(None);
        if payer {
            let auth = &mut original.plaintext.payer_authorization;
            auth.signatures[0].signature =
                SignatureOf::try_new(wrong.private_key(), &auth.body).unwrap();
        } else {
            let auth = &mut original.plaintext.outputs[0].view_key_authorization;
            auth.signatures[0].signature =
                SignatureOf::try_new(wrong.private_key(), &auth.body).unwrap();
        }
        original
            .plaintext
            .validate_preparation_against_proof_intent(&intent)
            .unwrap();
        assert!(encode_atomic_private_settlement_reservation_v1(&intent, 0, &original).is_err());
    }
}
