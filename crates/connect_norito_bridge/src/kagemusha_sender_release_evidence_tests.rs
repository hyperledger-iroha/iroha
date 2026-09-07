//! Actual canonical sender-command parser fixtures; all proof material is synthetic.

use super::*;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderPublicInputPreimageV1, SenderWalletContextV1, canonical_command_body_for_tests,
    canonical_hardware_authorization_for_tests, terminal_envelope_digest_v1,
};
use iroha_core::zk::kagemusha_v1_state::KagemushaRedemptionTerminalReceiptV1;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::consensus_v2::HeightContextId,
    kagemusha::{
        KagemushaAcknowledgementV1, KagemushaCommitEvidenceV1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
        KagemushaHardwarePlatformClassV1, KagemushaHardwareProfileV1, KagemushaLifecycleBindingV1,
        KagemushaMonotonicLeaseV1, KagemushaOperationKindV1, KagemushaRedemptionProofV1,
        KagemushaRedemptionStatementV1, KagemushaTrustedCommitTimeV1,
        kagemusha_ciphertext_digest_v1, kagemusha_liability_pool_id_v1,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

fn base_command() -> SenderCommandV1 {
    SenderCommandV1::decode_canonical_exact(
        12,
        [7; 32],
        &canonical_command_body_for_tests(12).unwrap(),
    )
    .unwrap()
}

fn digest(value: &Value, key: &str) -> [u8; 32] {
    hex::decode(value.get(key).unwrap().as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap()
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).unwrap().as_u64().unwrap()
}

fn credential(value: &Value) -> KagemushaHardwareCredentialV1 {
    KagemushaHardwareCredentialV1 {
        version: 1,
        credential_id: digest(value, "credential_id"),
        network_id: HashOf::from_untyped_unchecked(Hash::prehashed(digest(value, "network_id")))
            .into(),
        hardware_profile_id: digest(value, "hardware_profile_id"),
        suite_id: digest(value, "suite_id"),
        firmware_policy_digest: digest(value, "firmware_policy_digest"),
        policy_epoch: number(value, "policy_epoch"),
        lane_commitment: digest(value, "lane_commitment"),
        hardware_epoch_id: digest(value, "hardware_epoch_id"),
        hardware_epoch_generation: number(value, "hardware_epoch_generation"),
        device_public_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            &hex::decode(value.get("device_public_key").unwrap().as_str().unwrap()).unwrap(),
        )
        .unwrap(),
        device_key_reference: digest(value, "device_key_reference"),
        issued_at_ms: number(value, "issued_at_ms"),
        expires_at_ms: number(value, "expires_at_ms"),
        governance_signature: KagemushaDeviceSignatureV1::from_raw_bytes(
            &hex::decode(value.get("governance_signature").unwrap().as_str().unwrap()).unwrap(),
        )
        .unwrap(),
    }
}

fn profile(value: &Value) -> KagemushaHardwareProfileV1 {
    KagemushaHardwareProfileV1 {
        version: 1,
        protocol_version: 1,
        hardware_profile_id: digest(value, "hardware_profile_id"),
        provider_id: digest(value, "provider_id"),
        platform_class: match value.get("platform_class").unwrap().as_str().unwrap() {
            "android_oem_service" => KagemushaHardwarePlatformClassV1::AndroidOemService,
            "dedicated_secure_element" => KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
            _ => panic!("unsupported synthetic fixture class"),
        },
        product_class_digest: digest(value, "product_class_digest"),
        firmware_policy_digest: digest(value, "firmware_policy_digest"),
        enrollment_attestation_verifier_digest: digest(
            value,
            "enrollment_attestation_verifier_digest",
        ),
        attestation_trust_roots_digest: digest(value, "attestation_trust_roots_digest"),
        allowed_suite_commitment: digest(value, "allowed_suite_commitment"),
        policy_epoch: number(value, "policy_epoch"),
        governance_credential_public_key: KagemushaDevicePublicKeyV1::from_sec1_bytes(
            &hex::decode(
                value
                    .get("governance_credential_public_key")
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
        capability_mask: number(value, "capability_mask").try_into().unwrap(),
        qualification_report_digest: digest(value, "qualification_report_digest"),
        valid_from_ms: number(value, "valid_from_ms"),
        expires_at_ms: number(value, "expires_at_ms"),
    }
}

fn sign_fixture(bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
    // Public diagnostic scalar two, matching the physical fixture's device key.
    sign_fixture_scalar(bytes, 2)
}

fn sign_fixture_scalar(bytes: &[u8], last_byte: u8) -> KagemushaDeviceSignatureV1 {
    let mut scalar = [0; 32];
    scalar[31] = last_byte;
    let key = SigningKey::from_bytes((&scalar).into()).unwrap();
    let signature: Signature = key.sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref()).unwrap()
}

fn lifecycle(
    context: &SenderWalletContextV1,
    request: &KagemushaPaymentRequestV1,
    payment: &KagemushaPaymentV1,
    redeem: bool,
) -> KagemushaLifecycleBindingV1 {
    KagemushaLifecycleBindingV1 {
        version: 1,
        network_id: context.lane.network_id,
        protocol_version: context.release.protocol_version,
        suite_id: context.release.suite_id,
        vk_digest: context.release.vk_digest,
        release_id: context.release.release_id,
        asset: context.lane.asset.clone(),
        asset_incarnation: context.release.asset_incarnation,
        scale: context.lane.scale,
        liability_pool_id: request.liability_pool_id,
        hardware_profile_id: context.release.hardware_profile_id,
        policy_epoch: context.release.policy_epoch,
        operation_kind: if redeem {
            KagemushaOperationKindV1::RedeemSplit
        } else {
            KagemushaOperationKindV1::SendSplit
        },
        request_id: if redeem { [0; 32] } else { request.request_id },
        receiver_lane_commitment: if redeem {
            [0; 32]
        } else {
            request.hardware_credential.lane_commitment
        },
        credit_id: if redeem {
            [0; 32]
        } else {
            payment.output.credit_id
        },
        ciphertext_digest: if redeem {
            [0; 32]
        } else {
            kagemusha_ciphertext_digest_v1(&payment.encrypted_credit)
        },
    }
}

/// Build a shape-valid command using actual model encoders and diagnostic signatures.
/// The optional contexts are public test data only; no executable or signing key is loaded.
fn fixture(
    operation_id: [u8; 32],
    redeem: bool,
    configuration: Option<(&Value, &Value)>,
) -> SenderCommandV1 {
    let mut command = base_command();
    let SenderCommandBodyV1::Release {
        inputs,
        envelope,
        terminal_receipt,
        hardware_authorization,
        ..
    } = &command.body
    else {
        unreachable!()
    };
    let SenderPublicInputsV1::SendSplit { request } = inputs else {
        unreachable!()
    };
    let mut request = KagemushaPaymentRequestV1::decode_canonical_exact(request).unwrap();
    let mut payment =
        KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &request).unwrap();
    let SenderTerminalReceiptV1::PaymentAcknowledgement(ack_bytes) = terminal_receipt else {
        unreachable!()
    };
    let mut acknowledgement = KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
        ack_bytes, &request, &payment,
    )
    .unwrap();
    let old_authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
    let mut preparation_id = old_authorization.preparation_id;
    let mut candidate_digest = old_authorization.candidate_digest;
    // A synthetic manifest selector, distinct from the candidate artifact-set identity.
    let artifact_digest = [0x83; 32];
    if let Some((configuration, attempt)) = configuration {
        let credentials = configuration
            .get("credentials")
            .unwrap()
            .as_array()
            .unwrap();
        let sender = credential(&credentials[0]);
        let profile = profile(configuration.get("hardware_profile").unwrap());
        profile.validate().unwrap();
        sender.validate_against_profile(&profile).unwrap();
        assert_eq!(sender.credential_id, digest(attempt, "credential_id"));
        let context = &mut command.context;
        context.lane.network_id = sender.network_id;
        context.lane.device_lane_id = sender.lane_commitment;
        context.release.suite_id = sender.suite_id;
        context.release.vk_digest = digest(configuration, "vk_digest");
        context.release.hardware_profile_id = sender.hardware_profile_id;
        context.release.policy_epoch = sender.policy_epoch;
        context.credential_id = sender.credential_id;
        context.hardware_epoch.epoch_id = sender.hardware_epoch_id;
        context.hardware_epoch.generation = sender.hardware_epoch_generation.into();
        context.device_policy_binding.device_key_reference = sender.device_key_reference;
        context.device_policy_binding.hardware_policy_id =
            digest(configuration, "hardware_policy_id");
        preparation_id = digest(attempt, "preparation_sha256");
        candidate_digest = digest(attempt, "candidate_sha256");
        // A peer request needs its own receiver credential covering the request window.
        // Extend only this synthetic peer credential, signed with public test issuer scalar
        // one. The sender context retains the independently checked short-lived credential.
        let mut receiver = sender;
        receiver.lane_commitment = Sha256::digest(
            [
                b"synthetic-parser-receiver-lane".as_slice(),
                &sender.lane_commitment,
            ]
            .concat(),
        )
        .into();
        if !redeem {
            receiver.issued_at_ms = number(attempt, "request_start_ms");
            receiver.expires_at_ms = number(attempt, "request_end_ms");
        }
        receiver = receiver.seal_credential_id().unwrap();
        receiver.governance_signature =
            sign_fixture_scalar(&receiver.canonical_signing_bytes().unwrap(), 1);
        receiver.validate_against_profile(&profile).unwrap();
        request.network_id = context.lane.network_id;
        request.hardware_credential = receiver;
        request.liability_pool_id = kagemusha_liability_pool_id_v1(
            &request.network_id,
            &request.asset,
            request.asset_incarnation,
        )
        .unwrap();
        request.issued_at_ms = receiver.issued_at_ms;
        request.expires_at_ms = receiver.expires_at_ms;
        request.request_id = operation_id;
        request.signature = sign_fixture(&request.canonical_signing_bytes().unwrap());
        request.validate_against_profile(&profile).unwrap();
        payment.output.request_digest = request.canonical_digest().unwrap();
        payment.output.transition_nullifier = Sha256::digest(
            [
                b"synthetic-native-parser-send-nullifier".as_slice(),
                &operation_id,
            ]
            .concat(),
        )
        .into();
        payment.output.committed_at_ms = number(attempt, "decision_trusted_time_ms");
        payment.output = payment.output.seal_credit_id_against(&request).unwrap();
        payment.output.commit_evidence =
            if attempt.get("source").unwrap().as_str().unwrap() == "monotonic_lease" {
                KagemushaCommitEvidenceV1::MonotonicLease(KagemushaMonotonicLeaseV1 {
                    lease_evidence_commitment: digest(attempt, "commit_evidence_commitment"),
                })
            } else {
                KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
                    time_evidence_commitment: digest(attempt, "commit_evidence_commitment"),
                })
            };
    }
    let context = &command.context;
    let (inputs, envelope, terminal_receipt, receipt_digest, outcome_id, transition_nullifier) =
        if redeem {
            let statement = KagemushaRedemptionStatementV1 {
                version: 1,
                lifecycle: lifecycle(context, &request, &payment, true),
                amount: request.amount,
                beneficiary: request.recipient.clone(),
                terminal_nullifier: Sha256::digest(
                    [
                        b"synthetic-native-parser-redemption-nullifier".as_slice(),
                        &operation_id,
                    ]
                    .concat(),
                )
                .into(),
                redemption_commitment: [0x82; 32],
                redemption_id: [0; 32],
                commit_evidence: payment.output.commit_evidence,
            }
            .seal_redemption_id()
            .unwrap();
            let mut certificate = payment.commit_certificate;
            certificate.lifecycle_binding_digest = statement.lifecycle.canonical_digest().unwrap();
            certificate.transition_nullifier = statement.terminal_nullifier;
            certificate.commit_evidence = statement.commit_evidence;
            certificate.candidate_envelope_digest = candidate_digest;
            certificate.hardware_profile_id = context.release.hardware_profile_id;
            certificate.policy_epoch = context.release.policy_epoch;
            certificate = certificate.seal_certificate_id().unwrap();
            let proof = KagemushaRedemptionProofV1 {
                version: 1,
                eq_protocol_digest: payment.proof.eq_protocol_digest,
                ep_protocol_digest: payment.proof.ep_protocol_digest,
                semantic_digest: statement.canonical_digest().unwrap(),
                candidate_envelope_digest: candidate_digest,
                commit_certificate_digest: certificate.canonical_digest().unwrap(),
                eq_deferred_audit: payment.proof.eq_deferred_audit,
                ep_deferred_audit: payment.proof.ep_deferred_audit,
                eq_proof: payment.proof.eq_proof,
                ep_proof: payment.proof.ep_proof,
                eq_history: payment.proof.eq_history,
                ep_history: payment.proof.ep_history,
            };
            let voucher = KagemushaRedemptionVoucherV1 {
                version: 1,
                statement,
                commit_certificate: certificate,
                proof,
                artifact_manifest_digest: artifact_digest,
            };
            voucher.validate_shape().unwrap();
            let envelope = canonical(&voucher).unwrap();
            let receipt = KagemushaRedemptionTerminalReceiptV1 {
                version: 1,
                network_id: context.lane.network_id,
                operation_id,
                redemption_id: voucher.statement.redemption_id,
                terminal_nullifier: voucher.statement.terminal_nullifier,
                envelope_digest: terminal_envelope_digest_v1(&envelope).unwrap(),
                reserve_receipt_digest: [0x84; 32],
                authenticated_status_digest: [0x85; 32],
                finalized_block_height: 17,
                height_context_id: HeightContextId(HashOf::from_untyped_unchecked(
                    Hash::prehashed([0x86; 32]),
                )),
            };
            let digest = receipt.canonical_digest().unwrap();
            (
                SenderPublicInputsV1::RedeemSplit {
                    amount: voucher.statement.amount,
                    beneficiary: voucher.statement.beneficiary,
                },
                envelope,
                SenderTerminalReceiptV1::RedemptionSettlement(receipt),
                digest,
                voucher.statement.redemption_id,
                voucher.statement.terminal_nullifier,
            )
        } else {
            if configuration.is_some() {
                payment.commit_certificate.lifecycle_binding_digest =
                    lifecycle(context, &request, &payment, false)
                        .canonical_digest()
                        .unwrap();
                payment.commit_certificate.candidate_envelope_digest = candidate_digest;
                payment.commit_certificate.transition_nullifier =
                    payment.output.transition_nullifier;
                payment.commit_certificate.hardware_profile_id =
                    context.release.hardware_profile_id;
                payment.commit_certificate.policy_epoch = context.release.policy_epoch;
                payment.commit_certificate.commit_evidence = payment.output.commit_evidence;
                payment.commit_certificate =
                    payment.commit_certificate.seal_certificate_id().unwrap();
                payment.proof.candidate_envelope_digest = candidate_digest;
                payment.proof.commit_certificate_digest =
                    payment.commit_certificate.canonical_digest().unwrap();
                payment.proof.semantic_digest = payment.body_digest_against(&request).unwrap();
                payment.validate_shape_against(&request).unwrap();
                acknowledgement.request_digest = request.canonical_digest().unwrap();
                acknowledgement.payment_digest =
                    payment.canonical_digest_against(&request).unwrap();
                acknowledgement.inbox_receipt.credit_id = payment.output.credit_id;
                acknowledgement.signature =
                    sign_fixture(&acknowledgement.canonical_signing_bytes().unwrap());
            }
            acknowledgement
                .validate_shape_against(&request, &payment)
                .unwrap();
            let receipt = canonical(&acknowledgement).unwrap();
            let mut hash = Sha256::new();
            hash.update(b"iroha:kagemusha:device:v1:accepted-acknowledgement");
            hash.update([0]);
            hash.update((receipt.len() as u64).to_le_bytes());
            hash.update(&receipt);
            (
                SenderPublicInputsV1::SendSplit {
                    request: canonical(&request).unwrap(),
                },
                canonical(&payment).unwrap(),
                SenderTerminalReceiptV1::PaymentAcknowledgement(receipt),
                hash.finalize().into(),
                payment.output.credit_id,
                payment.output.transition_nullifier,
            )
        };
    let inputs_digest = SenderPublicInputPreimageV1 {
        version: 1,
        operation_id,
        context: context.clone(),
        inputs: inputs.clone(),
    }
    .canonical_digest()
    .unwrap();
    let envelope_digest = terminal_envelope_digest_v1(&envelope).unwrap();
    let hardware_authorization = canonical_hardware_authorization_for_tests(
        SenderHardwareAuthorizationPurposeV1::Release,
        operation_id,
        inputs_digest,
        preparation_id,
        candidate_digest,
        context,
        &inputs,
        outcome_id,
        transition_nullifier,
        Some(envelope_digest),
        Some(receipt_digest),
    )
    .unwrap();
    command.operation_id = operation_id;
    command.body = SenderCommandBodyV1::Release {
        inputs_digest,
        envelope_digest,
        inputs,
        envelope,
        terminal_receipt,
        hardware_authorization,
    };
    command.validate_shape().unwrap();
    command
}

#[test]
fn projects_actual_send_and_redemption_commands_deterministically() {
    for (id, redeem) in [(0x91, false), (0x92, true)] {
        let command = fixture([id; 32], redeem, None);
        let bytes = command.encode_canonical().unwrap();
        let projection =
            kagemusha_sender_release_command_projection_v1(command.operation_id, &bytes).unwrap();
        assert_eq!(
            projection,
            kagemusha_sender_release_command_projection_v1(command.operation_id, &bytes).unwrap()
        );
        let report: Value = norito::json::from_slice(&projection).unwrap();
        assert_eq!(
            report.get("command_sha256").unwrap().as_str(),
            Some(sha256(&bytes).as_str())
        );
        assert_eq!(report.get("structural_only"), Some(&Value::Bool(true)));
        assert_eq!(
            report.get("receipt_kind").unwrap().as_str(),
            Some(if redeem {
                "finalized_redemption_selector"
            } else {
                "payment_acknowledgement"
            })
        );
        assert_eq!(
            report.get("artifact_manifest_digest").unwrap().is_null(),
            !redeem
        );
    }
}

#[test]
fn canonical_decoder_rejects_resigned_release_candidate_substitution() {
    for redeem in [false, true] {
        let mut command = fixture([0x93; 32], redeem, None);
        let SenderCommandBodyV1::Release {
            inputs,
            hardware_authorization,
            ..
        } = &mut command.body
        else {
            unreachable!()
        };
        let auth =
            SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
        let mut wrong_candidate = auth.candidate_digest;
        wrong_candidate[0] ^= 1;
        *hardware_authorization = canonical_hardware_authorization_for_tests(
            auth.purpose,
            auth.operation_id,
            auth.inputs_digest,
            auth.preparation_id,
            wrong_candidate,
            &command.context,
            inputs,
            auth.outcome_id,
            auth.transition_nullifier,
            auth.envelope_digest,
            auth.terminal_receipt_digest,
        )
        .unwrap();
        // The substituted authorization itself has a genuine valid P-256 signature.
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
        assert!(command.validate_shape().is_err());
        let raw = canonical(&command).unwrap();
        assert!(SenderCommandV1::decode_canonical_exact(12, command.operation_id, &raw).is_err());
        assert!(
            kagemusha_sender_release_command_projection_v1(command.operation_id, &raw).is_err()
        );
    }
}

#[test]
fn rejects_valid_commit_authorization_in_a_release_command() {
    for redeem in [false, true] {
        let mut command = fixture([0x96; 32], redeem, None);
        let SenderCommandBodyV1::Release {
            inputs,
            hardware_authorization,
            ..
        } = &mut command.body
        else {
            unreachable!()
        };
        let auth =
            SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
        *hardware_authorization = canonical_hardware_authorization_for_tests(
            SenderHardwareAuthorizationPurposeV1::Commit,
            auth.operation_id,
            auth.inputs_digest,
            auth.preparation_id,
            auth.candidate_digest,
            &command.context,
            inputs,
            auth.outcome_id,
            auth.transition_nullifier,
            None,
            None,
        )
        .unwrap();
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization).unwrap();
        assert!(
            kagemusha_sender_release_command_projection_v1(
                command.operation_id,
                &canonical(&command).unwrap()
            )
            .is_err()
        );
    }
}

#[test]
fn rejects_wrong_selectors_framing_context_and_release_receipts() {
    for redeem in [false, true] {
        let command = fixture([0x94; 32], redeem, None);
        let bytes = command.encode_canonical().unwrap();
        assert!(kagemusha_sender_release_command_projection_v1([0x95; 32], &bytes).is_err());
        assert!(kagemusha_sender_release_command_projection_v1(command.operation_id, &[]).is_err());
        assert!(
            kagemusha_sender_release_command_projection_v1(
                command.operation_id,
                &vec![0; KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1 + 1]
            )
            .is_err()
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(
            kagemusha_sender_release_command_projection_v1(command.operation_id, &trailing)
                .is_err()
        );
        let mut changed = command.clone();
        changed.context.credential_id[0] ^= 1;
        assert!(
            kagemusha_sender_release_command_projection_v1(
                command.operation_id,
                &canonical(&changed).unwrap()
            )
            .is_err()
        );
        let mut changed = command.clone();
        let SenderCommandBodyV1::Release {
            terminal_receipt, ..
        } = &mut changed.body
        else {
            unreachable!()
        };
        match terminal_receipt {
            SenderTerminalReceiptV1::PaymentAcknowledgement(bytes) => {
                bytes[0] ^= 1;
            }
            SenderTerminalReceiptV1::RedemptionSettlement(receipt) => {
                receipt.operation_id[0] ^= 1;
            }
        }
        assert!(
            kagemusha_sender_release_command_projection_v1(
                command.operation_id,
                &canonical(&changed).unwrap()
            )
            .is_err()
        );
    }
    let commit = canonical_command_body_for_tests(7).unwrap();
    assert!(kagemusha_sender_release_command_projection_v1([7; 32], &commit).is_err());
}

#[test]
fn replays_all_tracked_native_sender_release_fixture_bytes() {
    use std::collections::BTreeSet;

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/offline/kagemusha_sender_release_parser_v1.json");
    // Required runtime input: the ignored exporter can run first, but normal validation
    // cannot silently skip a missing or stale reviewed fixture.
    let golden: Value = norito::json::from_slice(
        &std::fs::read(path).expect("required reviewed native sender fixtures"),
    )
    .unwrap();
    assert_eq!(
        golden.get("schema").unwrap().as_str(),
        Some("iroha.kagemusha_v1.sender_release_parser_test_fixtures")
    );
    assert_eq!(golden.get("schema_version").unwrap().as_u64(), Some(1));
    let contexts = golden.get("contexts").unwrap().as_object().unwrap();
    assert_eq!(
        contexts.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "physical_standalone",
            "release_default",
            "release_provider_ab"
        ])
    );
    let base = base_command();
    let mut command_hashes = BTreeSet::new();
    let mut contextual_operations = BTreeSet::new();
    for (name, bundle) in contexts {
        let context = bundle.get("context").unwrap();
        let credentials = context.get("credentials").unwrap().as_array().unwrap();
        assert_eq!(
            credentials.len(),
            1,
            "one original short-lived sender credential"
        );
        let sender = credential(&credentials[0]);
        let profile = profile(context.get("hardware_profile").unwrap());
        profile.validate().unwrap();
        sender.validate_against_profile(&profile).unwrap();
        let rows = bundle.get("fixtures").unwrap().as_array().unwrap();
        assert_eq!(rows.len(), 6);
        let mut cases = BTreeSet::new();
        let mut operations = BTreeSet::new();
        for row in rows {
            let case = row.get("case").unwrap().as_str().unwrap();
            let operation_kind = row.get("operation_kind").unwrap().as_str().unwrap();
            assert!(cases.insert((case, operation_kind)));
            let expected = row.get("projection").unwrap();
            let operation_id = digest(expected, "operation_id");
            assert!(operations.insert(operation_id));
            assert!(contextual_operations.insert((name.clone(), operation_id)));
            let command_hex = row
                .get("canonical_sender_command_hex")
                .unwrap()
                .as_str()
                .unwrap();
            let bytes = hex::decode(command_hex).unwrap();
            assert_eq!(hex::encode(&bytes), command_hex);
            assert!(command_hashes.insert(sha256(&bytes)));
            let actual =
                kagemusha_sender_release_command_projection_v1(operation_id, &bytes).unwrap();
            assert_eq!(
                actual,
                norito::json::to_vec(expected).unwrap(),
                "complete native projection for {name}/{case}/{operation_kind}"
            );
            let command =
                SenderCommandV1::decode_canonical_exact(12, operation_id, &bytes).unwrap();
            assert_eq!(command.encode_canonical().unwrap(), bytes);
            let actual_context = &command.context;
            assert_eq!(actual_context.lane.network_id, sender.network_id);
            assert_eq!(actual_context.lane.device_lane_id, sender.lane_commitment);
            assert_eq!(
                actual_context.release.hardware_profile_id,
                sender.hardware_profile_id
            );
            assert_eq!(actual_context.release.policy_epoch, sender.policy_epoch);
            assert_eq!(actual_context.release.suite_id, sender.suite_id);
            assert_eq!(
                actual_context.release.vk_digest,
                digest(context, "vk_digest")
            );
            assert_eq!(actual_context.credential_id, sender.credential_id);
            assert_eq!(
                actual_context.hardware_epoch.epoch_id,
                sender.hardware_epoch_id
            );
            assert_eq!(
                actual_context.hardware_epoch.generation,
                u128::from(sender.hardware_epoch_generation)
            );
            assert_eq!(
                actual_context.device_policy_binding.device_key_reference,
                sender.device_key_reference
            );
            assert_eq!(
                actual_context.device_policy_binding.hardware_policy_id,
                digest(context, "hardware_policy_id")
            );
            assert_eq!(
                actual_context.release.release_id,
                base.context.release.release_id
            );
            assert_eq!(
                actual_context.release.asset_incarnation,
                base.context.release.asset_incarnation
            );
            assert_eq!(actual_context.lane.asset, base.context.lane.asset);
            assert_eq!(actual_context.lane.scale, base.context.lane.scale);
            assert_eq!(
                actual_context.core_authorization_key_reference,
                base.context.core_authorization_key_reference
            );
            let SenderCommandBodyV1::Release {
                inputs_digest,
                inputs,
                envelope,
                terminal_receipt,
                hardware_authorization,
                ..
            } = &command.body
            else {
                unreachable!()
            };
            assert_eq!(
                *inputs_digest,
                SenderPublicInputPreimageV1 {
                    version: 1,
                    operation_id,
                    context: actual_context.clone(),
                    inputs: inputs.clone()
                }
                .canonical_digest()
                .unwrap()
            );
            assert_eq!(
                row.get("canonical_envelope_hex").unwrap().as_str(),
                Some(hex::encode(envelope).as_str())
            );
            assert_eq!(
                row.get("canonical_hardware_authorization_hex")
                    .unwrap()
                    .as_str(),
                Some(hex::encode(hardware_authorization).as_str())
            );
            let certificate = match inputs {
                SenderPublicInputsV1::SendSplit { request } => {
                    assert_eq!(operation_kind, "send_split");
                    assert_eq!(
                        row.get("canonical_request_hex").unwrap().as_str(),
                        Some(hex::encode(request).as_str())
                    );
                    let request =
                        KagemushaPaymentRequestV1::decode_canonical_exact(request).unwrap();
                    request.validate_against_profile(&profile).unwrap();
                    assert_eq!(number(row, "request_start_ms"), request.issued_at_ms);
                    assert_eq!(number(row, "request_end_ms"), request.expires_at_ms);
                    let payment = KagemushaPaymentV1::decode_canonical_shape_exact_against(
                        envelope, &request,
                    )
                    .unwrap();
                    assert!(
                        (sender.issued_at_ms..sender.expires_at_ms)
                            .contains(&payment.output.committed_at_ms)
                    );
                    assert!(
                        (profile.valid_from_ms..profile.expires_at_ms)
                            .contains(&payment.output.committed_at_ms)
                    );
                    payment.commit_certificate
                }
                SenderPublicInputsV1::RedeemSplit { .. } => {
                    assert_eq!(operation_kind, "redeem_split");
                    assert_eq!(row.get("canonical_request_hex").unwrap().as_str(), Some(""));
                    assert_eq!(number(row, "request_start_ms"), 0);
                    assert_eq!(number(row, "request_end_ms"), 0);
                    KagemushaRedemptionVoucherV1::decode_canonical_shape_exact(envelope)
                        .unwrap()
                        .commit_certificate
                }
            };
            assert_eq!(
                matches!(
                    certificate.commit_evidence,
                    KagemushaCommitEvidenceV1::MonotonicLease(_)
                ),
                case == "lease_end"
            );
            assert_eq!(
                row.get("canonical_certificate_hex").unwrap().as_str(),
                Some(hex::encode(canonical(&certificate).unwrap()).as_str())
            );
            let receipt = match terminal_receipt {
                SenderTerminalReceiptV1::PaymentAcknowledgement(bytes) => bytes.clone(),
                SenderTerminalReceiptV1::RedemptionSettlement(receipt) => {
                    canonical(receipt).unwrap()
                }
            };
            assert_eq!(
                row.get("canonical_terminal_receipt_hex").unwrap().as_str(),
                Some(hex::encode(receipt).as_str())
            );
        }
        assert_eq!(
            cases,
            BTreeSet::from([
                ("trusted_valid", "send_split"),
                ("trusted_valid", "redeem_split"),
                ("lease_end", "send_split"),
                ("lease_end", "redeem_split"),
                ("trusted_before_expiry", "send_split"),
                ("trusted_before_expiry", "redeem_split"),
            ])
        );
    }
    assert_eq!(command_hashes.len(), 18);
    assert_eq!(contextual_operations.len(), 18);
}

#[test]
#[ignore = "explicit diagnostic fixture export; requires public input and output file paths"]
fn export_sender_release_parser_physical_fixtures() {
    let input =
        std::env::var("KAGEMUSHA_SENDER_FIXTURE_CONTEXTS").expect("public fixture input path");
    let output = std::env::var("KAGEMUSHA_SENDER_FIXTURE_OUTPUT").expect("fixture output path");
    let config: Value = norito::json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let mut contexts = Map::new();
    for (name, context) in config.get("contexts").unwrap().as_object().unwrap() {
        let attempts = config
            .get("positive_attempts")
            .unwrap()
            .get(name.as_str())
            .unwrap()
            .as_array()
            .unwrap();
        let mut fixtures = Vec::new();
        for attempt in attempts {
            let redeem = attempt.get("operation_kind").unwrap().as_str() == Some("redeem_split");
            let command = fixture(
                digest(attempt, "operation_id"),
                redeem,
                Some((context, attempt)),
            );
            let bytes = command.encode_canonical().unwrap();
            let projection: Value = norito::json::from_slice(
                &kagemusha_sender_release_command_projection_v1(command.operation_id, &bytes)
                    .unwrap(),
            )
            .unwrap();
            let SenderCommandBodyV1::Release {
                inputs,
                envelope,
                terminal_receipt,
                hardware_authorization,
                ..
            } = &command.body
            else {
                unreachable!()
            };
            let (request_bytes, request_start_ms, request_end_ms, certificate) = match inputs {
                SenderPublicInputsV1::SendSplit { request } => {
                    let model = KagemushaPaymentRequestV1::decode_canonical_exact(request).unwrap();
                    (
                        request.clone(),
                        model.issued_at_ms,
                        model.expires_at_ms,
                        KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &model)
                            .unwrap()
                            .commit_certificate,
                    )
                }
                SenderPublicInputsV1::RedeemSplit { .. } => (
                    Vec::new(),
                    0,
                    0,
                    KagemushaRedemptionVoucherV1::decode_canonical_shape_exact(envelope)
                        .unwrap()
                        .commit_certificate,
                ),
            };
            let receipt = match terminal_receipt {
                SenderTerminalReceiptV1::PaymentAcknowledgement(bytes) => bytes.clone(),
                SenderTerminalReceiptV1::RedemptionSettlement(receipt) => {
                    canonical(receipt).unwrap()
                }
            };
            fixtures.push(norito::json!({
                "case": (attempt.get("case").unwrap().clone()), "operation_kind": (attempt.get("operation_kind").unwrap().clone()),
                "canonical_sender_command_hex": (hex::encode(bytes)), "canonical_request_hex": (hex::encode(request_bytes)),
                "request_start_ms": request_start_ms, "request_end_ms": request_end_ms,
                "canonical_envelope_hex": (hex::encode(envelope)), "canonical_certificate_hex": (hex::encode(canonical(&certificate).unwrap())),
                "canonical_terminal_receipt_hex": (hex::encode(receipt)), "canonical_hardware_authorization_hex": (hex::encode(hardware_authorization)),
                "projection": projection
            }));
        }
        contexts.insert(
            name.to_owned(),
            norito::json!({"context": (context.clone()), "fixtures": fixtures}),
        );
    }
    let result = norito::json!({
        "schema": "iroha.kagemusha_v1.sender_release_parser_test_fixtures", "schema_version": 1,
        "scope": "Synthetic proof bytes and diagnostic keys; structurally valid commands do not grant release authority.",
        "contexts": (Value::Object(contexts))
    });
    std::fs::write(output, norito::json::to_vec(&result).unwrap()).unwrap();
}
