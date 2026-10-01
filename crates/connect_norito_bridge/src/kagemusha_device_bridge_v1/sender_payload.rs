//! Bridge path for the single shared canonical sender wire implementation.
//! Re-exporting public codecs grants no native lease, signature custody or monetary authority.
pub use iroha_core_zk::kagemusha_sender_wire::*;
#[cfg(test)]
use iroha_core_zk::kagemusha_v1_state::{
    DevicePolicyBindingV1, HardwareEpochV1, KagemushaLaneIdV1, KagemushaStateContextV1,
    KagemushaTransitionKindV1,
};
#[cfg(test)]
use iroha_data_model::{
    account::AccountId,
    kagemusha::{KagemushaDeviceSignatureV1, KagemushaOperationKindV1, KagemushaPaymentRequestV1},
};
#[cfg(test)]
use norito::codec::{Decode, Encode};
#[cfg(test)]
use p256::ecdsa::{Signature as P256Signature, signature::Signer as _};
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sender_selector_frames_keep_root_projection_and_container_identity_distinct() {
        let selector = SenderPreparationSelectorV1 {
            inputs_digest: [7; 32],
            preparation_id: [9; 32],
        };
        let frame = norito::to_bytes(&selector).unwrap();
        let header = norito::core::Header::read(&mut &frame[..]).unwrap();
        assert_eq!(
            header.schema,
            norito::core::schema_hash_for_name(
                "iroha.kagemusha.device.v1.sender-preparation-selector"
            )
        );
        assert_eq!(
            norito::decode_from_bytes::<SenderPreparationSelectorV1>(&frame).unwrap(),
            selector
        );

        let selectors = vec![selector];
        let frame = norito::to_bytes(&selectors).unwrap();
        let header = norito::core::Header::read(&mut &frame[..]).unwrap();
        assert_eq!(
            header.schema,
            norito::core::schema_hash_for_name(
                "alloc::vec::Vec<connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderPreparationSelectorV1>"
            )
        );
        assert_eq!(
            norito::decode_from_bytes::<Vec<SenderPreparationSelectorV1>>(&frame).unwrap(),
            selectors
        );
    }

    #[test]
    fn sender_context_and_preimages_match_core_and_shared_archive() {
        use crate::kagemusha_core_coordinator_v1::KagemushaCoreSenderPreparationArchiveV1;
        use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputPreimageV1;

        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/kagemusha_core_coordinator_archives_v1.json"
        )))
        .unwrap();
        let bytes = hex::decode(fixture["preparation"]["norito_hex"].as_str().unwrap()).unwrap();
        let preparation =
            KagemushaCoreSenderPreparationArchiveV1::decode_canonical_exact(&bytes).unwrap();
        assert_eq!(preparation.encode_canonical().unwrap(), bytes);
        let command = SenderCommandV1::decode_canonical_exact(
            5,
            preparation.operation_id,
            &canonical_command_body_for_tests(5).unwrap(),
        )
        .unwrap();
        assert_eq!(command.context, preparation.context);
        assert_eq!(
            norito::encode_canonical(&command.context).unwrap(),
            norito::encode_canonical(&preparation.context).unwrap(),
        );
        let SenderCommandBodyV1::Prepare { inputs: send } = command.body else {
            panic!("prepare fixture body")
        };
        let SenderPublicInputsV1::SendSplit { request } = &send else {
            panic!("send fixture public inputs")
        };
        let beneficiary = KagemushaPaymentRequestV1::decode_canonical_exact(request)
            .unwrap()
            .recipient;
        let redemption = SenderPublicInputsV1::RedeemSplit {
            amount: 7,
            beneficiary,
        };
        for inputs in [send, redemption] {
            let bridge = SenderPublicInputPreimageV1 {
                version: VERSION,
                operation_id: preparation.operation_id,
                context: preparation.context.clone(),
                inputs,
            };
            let encoded = norito::encode_canonical(&bridge).unwrap();
            let core: KagemushaOutgoingPublicInputPreimageV1 =
                norito::decode_canonical(&encoded).unwrap();
            assert_eq!(core.context, bridge.context);
            assert_eq!(norito::encode_canonical(&core).unwrap(), encoded);
            let digest = core.canonical_digest().unwrap();
            assert_eq!(digest, bridge.canonical_digest().unwrap());
            if matches!(bridge.inputs, SenderPublicInputsV1::SendSplit { .. }) {
                assert_eq!(digest, preparation.inputs_digest);
            }
            let mut substituted = core.clone();
            substituted.context.core_authorization_key_reference[0] ^= 1;
            assert!(substituted.context.validate_shape().is_ok());
            assert!(
                substituted
                    .context
                    .validate_against_native(&core.context)
                    .is_err()
            );
            assert_ne!(substituted.canonical_digest().unwrap(), digest);
            substituted.context.core_authorization_key_reference = [0; 32];
            assert!(substituted.canonical_digest().is_err());
        }
    }

    #[test]
    fn sender_reservation_binding_matches_core_and_both_mobile_sdks() {
        use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1;

        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/kagemusha_sender_reservation_v1.json"
        )))
        .unwrap();
        let bytes = |field: &str| hex::decode(fixture[field].as_str().unwrap()).unwrap();
        let request_bytes = bytes("send_request_hex");
        KagemushaPaymentRequestV1::decode_canonical_exact(&request_bytes).unwrap();
        let beneficiary_bytes = bytes("redeem_beneficiary_payload_hex");
        let beneficiary = AccountId::decode(&mut beneficiary_bytes.as_slice()).unwrap();
        assert_eq!(beneficiary.encode(), beneficiary_bytes);
        let cases = [
            (
                "send_binding_hex",
                SenderPublicInputsV1::SendSplit {
                    request: request_bytes.clone(),
                },
                KagemushaOutgoingPublicInputsV1::SendSplit {
                    request: request_bytes.clone(),
                },
            ),
            (
                "redeem_binding_hex",
                SenderPublicInputsV1::RedeemSplit {
                    amount: fixture["redeem_amount_decimal"]
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap(),
                    beneficiary: beneficiary.clone(),
                },
                KagemushaOutgoingPublicInputsV1::RedeemSplit {
                    amount: fixture["redeem_amount_decimal"]
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap(),
                    beneficiary,
                },
            ),
        ];
        for (field, bridge, core) in cases {
            let shared = bytes(field);
            assert_eq!(norito::encode_canonical(&bridge).unwrap(), shared);
            assert_eq!(norito::encode_canonical(&core).unwrap(), shared);
            assert_eq!(
                norito::decode_canonical::<SenderPublicInputsV1>(&shared).unwrap(),
                bridge
            );
            assert_eq!(
                norito::decode_canonical::<KagemushaOutgoingPublicInputsV1>(&shared).unwrap(),
                core
            );
            let mut trailing = shared.clone();
            trailing.push(0);
            assert!(norito::decode_canonical::<SenderPublicInputsV1>(&trailing).is_err());
        }
        // A reservation is a tagged input, never the old untyped request/amount concatenation.
        assert!(norito::decode_canonical::<SenderPublicInputsV1>(&request_bytes).is_err());
        let mut retired_redemption = 7_u128.to_le_bytes().to_vec();
        retired_redemption.extend(bytes("redeem_beneficiary_payload_hex"));
        assert!(norito::decode_canonical::<SenderPublicInputsV1>(&retired_redemption).is_err());
    }

    #[test]
    fn sender_commit_rejects_resigned_zero_amount_for_both_monetary_kinds() {
        use crate::kagemusha_core_coordinator_v1::{
            KagemushaCoreSenderCandidateArchiveV1, KagemushaCoreSenderPreparationArchiveV1,
        };

        let bytes = canonical_command_body_for_tests(7).expect("signed commit fixture");
        let original = SenderCommandV1::decode_canonical_exact(7, [7; 32], &bytes)
            .expect("canonical commit fixture");
        let SenderCommandBodyV1::Commit {
            selector,
            candidate_digest,
            hardware_authorization,
        } = &original.body
        else {
            panic!("commit fixture body")
        };
        let original_authorization =
            SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
                .expect("canonical signed authorization");
        let (signing_key, _) = hardware_authorization_test_key().expect("fixture signing key");

        for kind in [
            KagemushaTransitionKindV1::SendSplit,
            KagemushaTransitionKindV1::RedeemSplit,
        ] {
            for amount in [1, 0] {
                let mut authorization = original_authorization.clone();
                authorization.hardware_transition_statement.kind = kind;
                authorization.hardware_transition_statement.amount = amount;
                authorization.authorization_id = authorization
                    .unsigned_preimage()
                    .authorization_id()
                    .expect("mutated authorization preimage");
                let signature: P256Signature = signing_key.sign(&authorization.authorization_id);
                let signature = signature.normalize_s().unwrap_or(signature);
                authorization.authenticator =
                    KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
                        .expect("canonical low-S signature");
                let authorization_bytes = norito::encode_canonical(&authorization)
                    .expect("canonical mutated authorization");
                // The zero case must fail monetary validation, not signature or codec checks.
                SenderHardwareAuthorizationV1::decode_canonical_exact(&authorization_bytes)
                    .expect("re-signed authorization must authenticate for either amount");

                let mut command = original.clone();
                command.body = SenderCommandBodyV1::Commit {
                    selector: selector.clone(),
                    candidate_digest: *candidate_digest,
                    hardware_authorization: authorization_bytes.clone(),
                };
                let candidate = KagemushaCoreSenderCandidateArchiveV1 {
                    version: VERSION,
                    preparation: KagemushaCoreSenderPreparationArchiveV1 {
                        version: VERSION,
                        operation_id: command.operation_id,
                        context: command.context.clone(),
                        inputs_digest: selector.inputs_digest,
                    },
                    selector: selector.clone(),
                    candidate_digest: *candidate_digest,
                    hardware_commit_authorization: authorization_bytes,
                };
                let expected_valid = amount != 0;
                assert_eq!(
                    validate_hardware_authorization_statement(
                        &authorization,
                        &command.context,
                        None,
                    )
                    .is_ok(),
                    expected_valid,
                );
                assert_eq!(command.validate_shape().is_ok(), expected_valid);
                assert_eq!(command.encode_canonical().is_ok(), expected_valid);
                let raw_command = norito::encode_canonical(&command).unwrap();
                assert_eq!(
                    SenderCommandV1::decode_canonical_exact(7, [7; 32], &raw_command).is_ok(),
                    expected_valid,
                );
                assert_eq!(candidate.validate_shape().is_ok(), expected_valid);
                assert_eq!(candidate.encode_canonical().is_ok(), expected_valid);
                let raw_candidate = norito::encode_canonical(&candidate).unwrap();
                assert_eq!(
                    KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&raw_candidate)
                        .is_ok(),
                    expected_valid,
                );
            }
        }
    }

    fn fixture_bytes(name: &str) -> Vec<u8> {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/kagemusha_v1.json"
        )))
        .expect("shared KAGEMUSHA fixture must decode");
        hex::decode(
            fixture[name]["norito_hex"]
                .as_str()
                .expect("fixture entry must carry canonical bytes"),
        )
        .expect("fixture bytes must be hexadecimal")
    }

    fn installed_sender_record() -> SenderRecordV1 {
        let request_bytes = fixture_bytes("payment_request");
        let request = KagemushaPaymentRequestV1::decode_canonical_exact(&request_bytes)
            .expect("fixture request must decode");
        let context = SenderWalletContextV1 {
            lane: KagemushaLaneIdV1 {
                network_id: request.network_id.clone(),
                device_lane_id: [0x41; 32],
                asset: request.asset.clone(),
                scale: request.scale,
            },
            release: KagemushaStateContextV1 {
                protocol_version: VERSION,
                suite_id: request.hardware_credential.suite_id,
                vk_digest: [0x42; 32],
                release_id: request.release_id,
                asset_incarnation: request.asset_incarnation,
                hardware_profile_id: request.hardware_credential.hardware_profile_id,
                policy_epoch: request.hardware_credential.policy_epoch,
            },
            credential_id: [0x43; 32],
            hardware_epoch: HardwareEpochV1 {
                generation: u128::from(request.hardware_credential.hardware_epoch_generation),
                epoch_id: request.hardware_credential.hardware_epoch_id,
            },
            device_policy_binding: DevicePolicyBindingV1 {
                device_key_reference: [0x44; 32],
                hardware_policy_id: [0x45; 32],
            },
            core_authorization_key_reference: [0x4d; 32],
        };
        let operation_id = [0x46; 32];
        let inputs = SenderPublicInputsV1::SendSplit {
            request: request_bytes,
        };
        let inputs_digest = SenderPublicInputPreimageV1 {
            version: VERSION,
            operation_id,
            context: context.clone(),
            inputs: inputs.clone(),
        }
        .canonical_digest()
        .expect("fixture input preimage must validate");
        let record = SenderRecordV1 {
            operation_id,
            context,
            inputs_digest,
            operation_kind: KagemushaOperationKindV1::SendSplit,
            preparation_id: [0x47; 32],
            outbox_reservation_id: [0x48; 32],
            outcome_id: [0x49; 32],
            phase: SenderPhaseV1::Installed,
            record_revision: 4,
            inputs: Some(inputs),
            candidate_digest: Some([0x4a; 32]),
            commit_certificate_digest: Some([0x4b; 32]),
            envelope_digest: Some([0x4c; 32]),
            terminal_receipt_digest: None,
        };
        record
            .validate_shape(&record.context)
            .expect("installed fixture record must validate");
        record
    }

    #[test]
    fn sender_operation_inventory_uses_frozen_codes() {
        let request = SenderPublicInputsV1::SendSplit { request: vec![1] };
        let selector = SenderPreparationSelectorV1 {
            inputs_digest: [1; 32],
            preparation_id: [2; 32],
        };
        assert_eq!(
            SenderCommandBodyV1::Prepare {
                inputs: request.clone()
            }
            .operation(),
            5
        );
        assert_eq!(
            SenderCommandBodyV1::RecoverPrepared {
                inputs_digest: [1; 32]
            }
            .operation(),
            6
        );
        assert_eq!(
            SenderCommandBodyV1::Commit {
                selector: selector.clone(),
                candidate_digest: [3; 32],
                hardware_authorization: vec![1],
            }
            .operation(),
            7
        );
        assert_eq!(
            SenderCommandBodyV1::RecoverTerminal {
                inputs_digest: [1; 32]
            }
            .operation(),
            8
        );
        assert_eq!(
            SenderCommandBodyV1::Install {
                selector,
                candidate_digest: [3; 32],
                inputs: request.clone(),
                envelope: vec![1]
            }
            .operation(),
            9
        );
        assert_eq!(
            SenderCommandBodyV1::RecoverInstalled {
                selector: SenderRecoverySelectorV1::Lookup {
                    inputs_digest: [1; 32],
                },
            }
            .operation(),
            10
        );
        assert_eq!(
            SenderCommandBodyV1::Release {
                inputs_digest: [1; 32],
                envelope_digest: [4; 32],
                inputs: request,
                envelope: vec![1],
                terminal_receipt: SenderTerminalReceiptV1::PaymentAcknowledgement(vec![1]),
                hardware_authorization: vec![1],
            }
            .operation(),
            12
        );
    }

    #[test]
    fn terminal_digest_rejects_empty_bytes() {
        assert_eq!(terminal_envelope_digest_v1(&[]), Err(SenderErrorV1::Size));
    }

    #[test]
    fn operation_12_exact_duplicate_terminal_receipt_is_idempotent() {
        let mut released = installed_sender_record();
        released.phase = SenderPhaseV1::Released;
        released.record_revision += 1;
        released.inputs = None;
        released.terminal_receipt_digest = Some([0x4d; 32]);
        released
            .validate_shape(&released.context)
            .expect("released fixture record must validate");

        assert_eq!(validate_record_progress_v1(&released, &released), Ok(()));
    }

    #[test]
    fn operation_12_conflicting_terminal_receipt_is_rejected() {
        let mut released = installed_sender_record();
        released.phase = SenderPhaseV1::Released;
        released.record_revision += 1;
        released.inputs = None;
        released.terminal_receipt_digest = Some([0x4d; 32]);
        let mut conflict = released.clone();
        conflict.terminal_receipt_digest = Some([0x4e; 32]);

        assert_eq!(
            validate_record_progress_v1(&released, &conflict),
            Err(SenderErrorV1::Conflict)
        );
    }
}

#[cfg(test)]
mod explicit_schema_identity_tests {
    use super::*;

    macro_rules! identity {
        ($root:ty, $nominal:literal, $frame:literal) => {
            assert_eq!(<$root as norito::NoritoSchema>::nominal_name(), $nominal);
            assert_eq!(<$root as norito::NoritoSchema>::frame_name(), $frame);
            assert_eq!(
                norito::schema::identity::frame_hash::<$root>(),
                norito::core::schema_hash_for_name($frame)
            );
            assert_eq!(
                <Vec<$root> as norito::NoritoSchema>::nominal_name(),
                format!("alloc::vec::Vec<{}>", $nominal)
            );
        };
    }

    fn roundtrip<T>(value: &T) -> Vec<u8>
    where
        T: norito::NoritoSerialize,
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        let frame = norito::encode_canonical(value).expect("canonical fixture frame");
        let header = norito::core::Header::read(frame.as_slice()).expect("typed frame header");
        assert_eq!(header.schema, norito::schema::identity::frame_hash::<T>());
        let decoded: T = norito::decode_canonical(&frame).expect("same root canonical replay");
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
        assert!(matches!(
            norito::decode_canonical::<Vec<T>>(&frame),
            Err(norito::Error::SchemaMismatch)
        ));
        let mut trailing = frame.clone();
        trailing.push(0);
        assert!(norito::decode_canonical::<T>(&trailing).is_err());
        frame
    }

    #[test]
    fn framed_roots_keep_nominal_and_protocol_identities() {
        identity!(
            SenderPublicInputsV1,
            "connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderPublicInputsV1",
            "iroha.kagemusha.device.v1.sender-public-inputs"
        );
        identity!(
            SenderPublicInputPreimageV1,
            "connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderPublicInputPreimageV1",
            "iroha.kagemusha.device.v1.sender-public-input-preimage"
        );
        identity!(
            SenderHardwareAuthorizationPreimageV1,
            "connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderHardwareAuthorizationPreimageV1",
            "iroha.kagemusha.device.v1.sender-hardware-authorization-preimage"
        );
        identity!(
            SenderHardwareAuthorizationV1,
            "connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderHardwareAuthorizationV1",
            "iroha.kagemusha.device.v1.sender-hardware-authorization"
        );
        identity!(
            SenderCommandV1,
            "connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderCommandV1",
            "iroha.kagemusha.device.v1.sender-command"
        );
        identity!(
            SenderReplyV1,
            "connect_norito_bridge::kagemusha_device_bridge_v1::sender_payload::SenderReplyV1",
            "iroha.kagemusha.device.v1.sender-reply"
        );

        let bytes = canonical_command_body_for_tests(5).unwrap();
        let command = SenderCommandV1::decode_canonical_exact(5, [7; 32], &bytes).unwrap();
        assert_eq!(roundtrip(&command), bytes);
        let SenderCommandBodyV1::Prepare { inputs } = command.body else {
            panic!("prepare fixture")
        };
        let inputs_frame = roundtrip(&inputs);
        let core_inputs: iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1 =
            norito::decode_canonical(&inputs_frame).expect("same canonical Core input projection");
        assert_eq!(
            norito::encode_canonical(&core_inputs).unwrap(),
            inputs_frame
        );
        assert_ne!(
            <SenderPublicInputsV1 as norito::NoritoSchema>::nominal_name(),
            <iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputsV1 as norito::NoritoSchema>::nominal_name(),
        );
        let preimage = SenderPublicInputPreimageV1 {
            version: VERSION,
            operation_id: command.operation_id,
            context: command.context,
            inputs,
        };
        let frame = roundtrip(&preimage);
        let core: iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingPublicInputPreimageV1 =
            norito::decode_canonical(&frame).unwrap();
        assert_eq!(norito::encode_canonical(&core).unwrap(), frame);
        assert_eq!(
            core.canonical_digest().unwrap(),
            preimage.canonical_digest().unwrap()
        );
        let commit = SenderCommandV1::decode_canonical_exact(
            7,
            [7; 32],
            &canonical_command_body_for_tests(7).unwrap(),
        )
        .unwrap();
        let SenderCommandBodyV1::Commit {
            hardware_authorization,
            ..
        } = commit.body
        else {
            panic!("commit fixture")
        };
        let authorization =
            SenderHardwareAuthorizationV1::decode_canonical_exact(&hardware_authorization).unwrap();
        assert_eq!(roundtrip(&authorization), hardware_authorization);
        assert_eq!(
            authorization
                .unsigned_preimage()
                .authorization_id()
                .unwrap(),
            authorization.authorization_id
        );
    }
}
