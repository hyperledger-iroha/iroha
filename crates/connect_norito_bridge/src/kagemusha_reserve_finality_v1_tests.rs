//! Reserve-finality FFI negative boundaries and metadata-only lookup tests.
//!
//! TODO: Run this module with the coordinator's admitted native build and add valid applied
//! top-up/redemption fixtures plus forged-certificate/context/request cases before release.
//! These pending/malformed cases alone cannot qualify positive monetary verification.

use super::*;
use iroha_data_model::isi::kagemusha_v1::KagemushaOperationStatusV1;

fn pending_json() -> Vec<u8> {
    norito::json::to_vec(&KagemushaOperationStatusV1 {
        version: 1,
        operation_id: [7; 32],
        kind: KagemushaOperationKindV1::TopUp,
        state: KagemushaOperationStateV1::Pending,
        result: None,
        rejection: None,
    })
    .expect("pending status JSON")
}

fn anchor() -> KagemushaFinalityTrustAnchorV1 {
    KagemushaFinalityTrustAnchorV1 {
        network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::prehashed([3; 32]),
        )),
        block_height: 7,
        height_context_id: HeightContextId(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::prehashed([5; 32]),
        )),
    }
}

#[test]
fn trusted_coordinates_are_preserved_exactly_with_full_unsigned_height() {
    let value = trusted_anchor([3; 32], u64::MAX, [5; 32]).expect("exact marked hashes");
    assert_eq!(value.network_id.as_bytes(), &[3; 32]);
    assert_eq!(value.height_context_id.0.as_ref(), &[5; 32]);
    assert_eq!(value.block_height, u64::MAX);
}

#[test]
fn trusted_coordinates_reject_unmarked_hashes_without_normalization() {
    for invalid in [[0; 32], [4; 32]] {
        assert!(trusted_anchor(invalid, 7, [5; 32]).is_err());
        assert!(trusted_anchor([3; 32], 7, invalid).is_err());
    }
}

#[test]
fn trusted_coordinates_reject_zero_height() {
    assert!(trusted_anchor([3; 32], 0, [5; 32]).is_err());
}

#[test]
fn pending_hint_is_null_and_contains_no_money() {
    assert_eq!(
        anchor_hint_json(&pending_json()).expect("metadata"),
        b"null"
    );
}

#[test]
fn malformed_hint_cannot_release_an_untrusted_projection() {
    for bytes in [b"".as_slice(), b"{}", b"null", b"{\"state\":\"applied\"}"] {
        assert!(anchor_hint_json(bytes).is_err());
    }
}

#[test]
fn expected_intent_rejects_unknown_kind_and_noncanonical_requests() {
    for kind in [0, 1, 2, 255] {
        assert!(expected_request(kind, b"{}").is_err());
    }
}

#[test]
fn pending_status_cannot_be_promoted_to_verified_value() {
    let anchor = anchor();
    let expected = ExpectedRequest {
        kind: KagemushaOperationKindV1::TopUp,
        operation_id: [7; 32],
        network_id: anchor.network_id,
        canonical: b"unused",
    };
    assert!(verified_payload(&pending_json(), &expected, &anchor).is_err());
}

#[test]
fn zero_or_different_external_anchor_cannot_release_value() {
    let anchor = anchor();
    let expected = ExpectedRequest {
        kind: KagemushaOperationKindV1::TopUp,
        operation_id: [7; 32],
        network_id: anchor.network_id,
        canonical: b"unused",
    };
    let mut wrong = anchor;
    wrong.block_height = 0;
    assert!(verified_payload(&pending_json(), &expected, &wrong).is_err());
    wrong = anchor;
    wrong.network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::prehashed([9; 32]),
    ));
    assert!(verified_payload(&pending_json(), &expected, &wrong).is_err());
}

#[test]
fn hint_ffi_returns_owned_null_json() {
    let response = pending_json();
    let mut output = ptr::null_mut();
    let mut length = 0;
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_hint_v1(
            response.as_ptr(),
            response.len() as c_ulong,
            &mut output,
            &mut length,
        )
    };
    assert_eq!(status, 0);
    assert!(!output.is_null());
    assert_eq!(
        unsafe { slice::from_raw_parts(output, length as usize) },
        b"null"
    );
    connect_norito_free(output);
}

#[test]
fn hint_ffi_clears_stale_output_on_malformed_input() {
    let response = b"{}";
    let mut output = ptr::without_provenance_mut(1);
    let mut length = 99;
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_hint_v1(
            response.as_ptr(),
            response.len() as c_ulong,
            &mut output,
            &mut length,
        )
    };
    assert_ne!(status, 0);
    assert!(output.is_null());
    assert_eq!(length, 0);
}

#[test]
fn hint_ffi_rejects_oversized_declared_length_before_reading() {
    let one = [0u8];
    let mut output = ptr::without_provenance_mut(1);
    let mut length = 99;
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_hint_v1(
            one.as_ptr(),
            (KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1 + 1) as c_ulong,
            &mut output,
            &mut length,
        )
    };
    assert_ne!(status, 0);
    assert!(output.is_null());
    assert_eq!(length, 0);
}

#[test]
fn hint_ffi_clears_the_available_output_when_other_output_is_null() {
    let response = pending_json();
    let mut output = ptr::without_provenance_mut(1);
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_hint_v1(
            response.as_ptr(),
            response.len() as c_ulong,
            &mut output,
            ptr::null_mut(),
        )
    };
    assert_ne!(status, 0);
    assert!(output.is_null());
}

#[test]
fn verifier_ffi_rejects_invalid_kind_and_clears_output_before_input_access() {
    let mut output = ptr::without_provenance_mut(1);
    let mut length = 99;
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_verify_v1(
            ptr::null(),
            0,
            255,
            ptr::null(),
            0,
            ptr::null(),
            0,
            0,
            ptr::null(),
            0,
            &mut output,
            &mut length,
        )
    };
    assert_ne!(status, 0);
    assert!(output.is_null());
    assert_eq!(length, 0);
}

#[test]
fn verifier_ffi_rejects_reserved_anchor_before_request_decode() {
    let response = pending_json();
    let request = b"{}";
    let network = [3u8; 32];
    let context = [5u8; 32];
    let mut output = ptr::without_provenance_mut(1);
    let mut length = 99;
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_verify_v1(
            response.as_ptr(),
            response.len() as c_ulong,
            0,
            request.as_ptr(),
            request.len() as c_ulong,
            network.as_ptr(),
            32,
            0,
            context.as_ptr(),
            32,
            &mut output,
            &mut length,
        )
    };
    assert_ne!(status, 0);
    assert!(output.is_null());
    assert_eq!(length, 0);
}

#[test]
fn verifier_ffi_rejects_invalid_request_and_never_returns_pending_result() {
    let response = pending_json();
    let request = b"{}";
    let network = [3u8; 32];
    let context = [5u8; 32];
    let mut output = ptr::without_provenance_mut(1);
    let mut length = 99;
    let status = unsafe {
        connect_norito_kagemusha_reserve_finality_verify_v1(
            response.as_ptr(),
            response.len() as c_ulong,
            0,
            request.as_ptr(),
            request.len() as c_ulong,
            network.as_ptr(),
            32,
            7,
            context.as_ptr(),
            32,
            &mut output,
            &mut length,
        )
    };
    assert_ne!(status, 0);
    assert!(output.is_null());
    assert_eq!(length, 0);
}

mod top_up_submission_binding_tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        Level,
        account::AccountId,
        asset::AssetDefinitionId,
        block::BlockHeader,
        isi::{
            Log,
            kagemusha_v1::{KAGEMUSHA_CHAIN_VERSION_V1, KagemushaTopUpRequestV1, TopUpKagemushaV1},
        },
        kagemusha::*,
        nexus::AxtAssetIncarnationV1,
        testing::kagemusha::KagemushaFixtureSignerV1,
        transaction::{FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder},
    };
    use iroha_model_base::domain::DomainId;
    const FIXTURE_TOP_UP_PUBLIC_KEY_HEX: &str = "04209c317b637935dd3da1c54f63495dfb31f97d293df085710320595c9aacb83fdde4c69fc17a0c74c20cc692662f049892ba37a4ba47d2c70cd8a99986391f9b";
    fn network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"torii-shared-kagemusha-v1",
        )))
    }
    fn asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        )
    }
    fn asset_incarnation(seed: u8) -> AxtAssetIncarnationV1 {
        AxtAssetIncarnationV1::try_from_bytes(*Hash::new([seed]).as_ref())
            .expect("canonical asset incarnation")
    }
    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }
    fn fixture_public_key(encoded: &str) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            &hex::decode(encoded).expect("decode fixed P-256 public key"),
        )
        .expect("canonical fixed P-256 public key")
    }
    fn signing_key(seed: u8) -> KagemushaFixtureSignerV1 {
        KagemushaFixtureSignerV1::from_repeated_byte(seed)
    }
    fn sign_device(key: &KagemushaFixtureSignerV1, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
        key.sign(bytes)
    }
    fn hardware_credential(
        network_id: NetworkId,
        lane_commitment: [u8; 32],
        device_public_key: KagemushaDevicePublicKeyV1,
        tag: u8,
    ) -> KagemushaHardwareCredentialV1 {
        let credential = KagemushaHardwareCredentialV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id,
            hardware_profile_id: [tag; 32],
            suite_id: suite_id(),
            firmware_policy_digest: [tag.wrapping_add(1); 32],
            policy_epoch: 1,
            lane_commitment,
            hardware_epoch_id: [tag.wrapping_add(2); 32],
            hardware_epoch_generation: 1,
            device_public_key,
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 500,
            expires_at_ms: 90_000,
            governance_signature: sign_device(&signing_key(3), b"fixture governance credential"),
        }
        .seal_credential_id()
        .expect("seal hardware credential identity");
        credential
            .validate_shape()
            .expect("valid hardware credential shape");
        credential
    }
    fn recipient_encryption_key(tag: u8) -> [u8; 32] {
        let mut key = [0; 32];
        key[0] = tag;
        key
    }
    fn encrypted_credit(recipient_key: [u8; 32], tag: u8) -> Vec<u8> {
        let mut ephemeral_x25519_public_key = [0; 32];
        ephemeral_x25519_public_key[0] = tag.wrapping_add(1);
        KagemushaEncryptedCreditEnvelopeV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            ephemeral_x25519_public_key,
            nonce: [tag; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
            ciphertext_and_tag: vec![
                tag;
                kagemusha_credit_opening_canonical_len_v1()
                    .expect("credit opening length")
                    + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
            ],
        }
        .canonical_bytes_against_recipient_key(recipient_key)
        .expect("canonical encrypted credit")
    }
    fn paired_proof(semantic_digest: [u8; 32], tag: u8) -> KagemushaPairedProofV1 {
        KagemushaPairedProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: [tag; 32],
            ep_protocol_digest: [tag.wrapping_add(1); 32],
            semantic_digest,
            guard_eq_credential_audit: [tag.wrapping_add(2); 32],
            guard_ep_credential_audit: [tag.wrapping_add(3); 32],
            eq_deferred_audit: [tag.wrapping_add(4); 32],
            ep_deferred_audit: [tag.wrapping_add(5); 32],
            eq_proof: vec![tag.wrapping_add(6); 128],
            ep_proof: vec![tag.wrapping_add(7); 128],
            eq_history: vec![tag.wrapping_add(8); KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![tag.wrapping_add(9); KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        }
    }
    fn top_up_request_for_network(network_id: NetworkId) -> KagemushaTopUpRequestV1 {
        let recipient_public_key = fixture_public_key(FIXTURE_TOP_UP_PUBLIC_KEY_HEX);
        let asset = asset();
        let asset_incarnation = asset_incarnation(1);
        let recipient_lane_id = [0x23; 32];
        let recipient_one_time_key = recipient_encryption_key(0x29);
        let request = KagemushaTopUpRequestV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id: [0x21; 32],
            issuance_commitment: [0; 32],
            credit_id: [0; 32],
            release_id: [0x22; 32],
            suite_id: suite_id(),
            vk_digest: [0x24; 32],
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 4,
            amount: 25_000,
            liability_pool_id: kagemusha_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            payer: account(0x31),
            recipient: account(0x32),
            hardware_credential: hardware_credential(
                network_id,
                recipient_lane_id,
                recipient_public_key,
                0x71,
            ),
            recipient_credential_commitment: [0x25; 32],
            credit_commitment: [0x26; 32],
            recipient_one_time_key,
            encrypted_credit: encrypted_credit(recipient_one_time_key, 0x27),
            artifact_manifest_digest: [0x28; 32],
            mint_authorization: None,
        }
        .seal_identifiers()
        .expect("seal top-up identifiers");
        let statement = request
            .mint_authorization_statement()
            .expect("mint authorization statement");
        let proof = paired_proof(
            statement
                .canonical_digest()
                .expect("mint authorization semantic digest"),
            0x81,
        );
        request
            .attach_mint_authorization(KagemushaMintAuthorizationV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                statement,
                proof,
            })
            .expect("attach mint authorization")
    }
    fn top_up_request() -> KagemushaTopUpRequestV1 {
        top_up_request_for_network(network_id())
    }
    const fn suite_id() -> [u8; 32] {
        [0x10; 32]
    }

    fn reseal_request(mut value: KagemushaTopUpRequestV1) -> KagemushaTopUpRequestV1 {
        value.mint_authorization = None;
        let value = value
            .seal_identifiers()
            .expect("reseal complete changed intent");
        let statement = value.mint_authorization_statement().expect("new statement");
        let proof = paired_proof(statement.canonical_digest().expect("digest"), 0x81);
        value
            .attach_mint_authorization(KagemushaMintAuthorizationV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                statement,
                proof,
            })
            .expect("valid changed authorization shape")
    }
    fn request_bytes(value: &KagemushaTopUpRequestV1) -> Vec<u8> {
        norito::encode_canonical(value).expect("canonical reviewed intent")
    }
    fn signed_request(
        value: &KagemushaTopUpRequestV1,
        admission: TransactionAdmissionIntent,
    ) -> Vec<u8> {
        let key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
        TransactionBuilder::new(
            value.network_id,
            value.payer.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([TopUpKagemushaV1::new(value.clone()).expect("instruction")])
        .with_admission_intent(admission)
        .try_sign(key.private_key())
        .expect("sign")
        .encode_wire_v1()
        .expect("canonical signed wire")
    }
    #[test]
    fn canonical_signed_request_is_bound_before_dispatch() {
        let expected = top_up_request();
        let signed = signed_request(&expected, TransactionAdmissionIntent::QueuePlanSynced);
        validate_top_up_submission(&signed, &request_bytes(&expected))
            .expect("native bound submission");
        assert_eq!(
            unsafe {
                connect_norito_kagemusha_top_up_signed_request_validate_v1(
                    signed.as_ptr(),
                    signed.len() as c_ulong,
                    request_bytes(&expected).as_ptr(),
                    request_bytes(&expected).len() as c_ulong,
                )
            },
            0
        );
    }
    #[test]
    fn different_valid_reviewed_requests_cannot_authorize_signed_bytes() {
        let expected = top_up_request();
        let signed = signed_request(&expected, TransactionAdmissionIntent::QueuePlanSynced);
        for change in 0..5 {
            let mut other = expected.clone();
            match change {
                0 => other.operation_id = [0x51; 32],
                1 => other.recipient = account(0x34),
                2 => other.amount += 1,
                3 => {
                    other.asset = AssetDefinitionId::derive_from_components(
                        DomainId::try_new("wonderland", "universal").unwrap(),
                        "other".parse().unwrap(),
                    );
                    other.liability_pool_id = kagemusha_liability_pool_id_v1(
                        &other.network_id,
                        &other.asset,
                        other.asset_incarnation,
                    )
                    .unwrap();
                }
                _ => {
                    let credential = other.hardware_credential.clone();
                    other.hardware_credential = hardware_credential(
                        other.network_id,
                        credential.lane_commitment,
                        credential.device_public_key,
                        0x72,
                    );
                }
            }
            let other = reseal_request(other);
            let canonical = request_bytes(&other);
            decode_kagemusha_top_up_request_v1(&canonical)
                .expect("valid independent reviewed intent");
            assert_ne!(canonical, request_bytes(&expected));
            // Both requests and the signed transaction are individually valid. Only exact full
            // request equality rejects same-operation amount/asset/credential substitutions.
            validate_top_up_submission(&signed, &request_bytes(&expected))
                .expect("original still valid");
            assert!(
                validate_top_up_submission(&signed, &canonical).is_err(),
                "change {change}"
            );
        }
    }
    #[test]
    fn ordinary_admission_cannot_prepare_a_top_up() {
        let request = top_up_request();
        let signed = signed_request(&request, TransactionAdmissionIntent::Ordinary);
        assert!(validate_top_up_submission(&signed, &request_bytes(&request)).is_err());
    }
    #[test]
    fn invalid_signature_cannot_prepare_a_top_up() {
        let request = top_up_request();
        let transaction = TransactionBuilder::new(
            request.network_id,
            request.payer.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([TopUpKagemushaV1::new(request.clone()).unwrap()])
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
        .build_with_signature(Signature::from_bytes(&[]));
        assert!(
            validate_top_up_submission(
                &transaction.encode_wire_v1().unwrap(),
                &request_bytes(&request)
            )
            .is_err()
        );
    }
    #[test]
    fn wrong_network_and_payer_cannot_prepare_a_top_up() {
        let request = top_up_request();
        let other = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
        let wrong_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other-network")),
        );
        for (network, payer, key) in [
            (
                wrong_network,
                request.payer.clone(),
                KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519),
            ),
            (
                request.network_id,
                AccountId::new(other.public_key().clone()),
                other,
            ),
        ] {
            let signed = TransactionBuilder::new(
                network,
                payer,
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([TopUpKagemushaV1::new(request.clone()).unwrap()])
            .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
            .try_sign(key.private_key())
            .unwrap()
            .encode_wire_v1()
            .unwrap();
            assert!(validate_top_up_submission(&signed, &request_bytes(&request)).is_err());
        }
    }
    #[test]
    fn extra_or_wrong_instruction_cannot_prepare_a_top_up() {
        let request = top_up_request();
        let key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
        let topup = TopUpKagemushaV1::new(request.clone()).unwrap();
        let transactions = [
            TransactionBuilder::new(
                request.network_id,
                request.payer.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([topup.clone(), topup])
            .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
            .try_sign(key.private_key())
            .unwrap(),
            TransactionBuilder::new(
                request.network_id,
                request.payer.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(Level::INFO, "not a top-up".to_owned())])
            .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
            .try_sign(key.private_key())
            .unwrap(),
        ];
        for transaction in transactions {
            assert!(
                validate_top_up_submission(
                    &transaction.encode_wire_v1().unwrap(),
                    &request_bytes(&request)
                )
                .is_err()
            );
        }
    }
    #[test]
    fn malformed_noncanonical_and_oversized_wire_cannot_pass() {
        let request = top_up_request();
        let expected = request_bytes(&request);
        let mut signed = signed_request(&request, TransactionAdmissionIntent::QueuePlanSynced);
        signed.push(0);
        for bytes in [
            vec![],
            vec![1, 2, 3],
            signed,
            vec![0; MAX_TOP_UP_SIGNED_TRANSACTION_BYTES + 1],
        ] {
            assert!(validate_top_up_submission(&bytes, &expected).is_err());
        }
    }
    #[test]
    fn ffi_rejects_missing_and_out_of_bounds_input_before_reading() {
        let pointer = ptr::without_provenance::<c_uchar>(1);
        for (tx, tx_len, request, request_len) in [
            (ptr::null(), 1, pointer, 1),
            (pointer, c_ulong::MAX, pointer, 1),
            (pointer, 1, ptr::null(), 1),
            (pointer, 1, pointer, c_ulong::MAX),
        ] {
            // A valid readable signed byte is required before testing request bounds.
            let byte = [0u8];
            let tx = if tx_len == 1 && !tx.is_null() {
                byte.as_ptr()
            } else {
                tx
            };
            assert_ne!(
                unsafe {
                    connect_norito_kagemusha_top_up_signed_request_validate_v1(
                        tx,
                        tx_len,
                        request,
                        request_len,
                    )
                },
                0
            );
        }
    }
}
