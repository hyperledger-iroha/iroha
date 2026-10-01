//! Selector, qualification and historical recovery projection tests only.
//! Public fixtures below do not create a Core owner or qualify physical hardware.
use super::*;
use crate::kagemusha_core_coordinator_v1::startup_qualification::tests as fixture;
use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderCommandV1, SenderWalletContextV1, canonical_command_body_for_tests,
};
use iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingOperationPhaseV1 as Phase;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::kagemusha::KagemushaOperationKindV1 as Kind;

fn context(
    generation: u64,
) -> (
    SenderWalletContextV1,
    crate::kagemusha_device_bridge_v1::QualificationProjectionV1,
) {
    let qualification = fixture::qualification(generation);
    let command = canonical_command_body_for_tests(6).unwrap();
    let command: SenderCommandV1 = norito::decode_from_bytes(&command).unwrap();
    let mut current = command.context;
    current.lane.network_id = qualification.credential.network_id;
    current.lane.device_lane_id = qualification.credential.lane_commitment;
    current.release.release_id = qualification.release_id;
    current.release.hardware_profile_id = qualification.profile.hardware_profile_id;
    current.release.suite_id = qualification.credential.suite_id;
    current.release.policy_epoch = qualification.profile.policy_epoch;
    current.credential_id = qualification.credential.credential_id;
    current.hardware_epoch.generation =
        u128::from(qualification.credential.hardware_epoch_generation);
    current.hardware_epoch.epoch_id = qualification.credential.hardware_epoch_id;
    current.device_policy_binding.device_key_reference =
        qualification.credential.device_key_reference;
    current.device_policy_binding.hardware_policy_id = [0x91; 32];
    current.core_authorization_key_reference = qualification.core_authorization_key_reference;
    (current, qualification)
}
fn archive(current: SenderWalletContextV1) -> KagemushaCoreSenderRecoveryArchiveV1 {
    KagemushaCoreSenderRecoveryArchiveV1 {
        version: 1,
        operation_id: [0x71; 32],
        terminal_id: [0x72; 32],
        context: current,
        inputs_digest: [0x73; 32],
    }
}

#[test]
fn sender_recovery_selectors_preserve_the_closed_identity_axis() {
    for selector in [0, 1] {
        for kind in [0_u32, 1] {
            let mut fields = vec![vec![selector], vec![0x71; 32], kind.to_le_bytes().to_vec()];
            fields.extend(vec![Vec::new(); 5]);
            let (selected, selected_kind) = sender_recovery_selector(&fields).unwrap();
            assert_eq!(
                selected,
                if selector == 0 {
                    NativeSenderRecoverySelectorV1::Terminal([0x71; 32])
                } else {
                    NativeSenderRecoverySelectorV1::Operation([0x71; 32])
                }
            );
            assert_eq!(
                selected_kind,
                if kind == 0 {
                    Kind::SendSplit
                } else {
                    Kind::RedeemSplit
                }
            );
            for mutation in 0..5 {
                let mut changed = fields.clone();
                match mutation {
                    0 => changed[0] = vec![2],
                    1 => changed[0].push(0),
                    2 => changed[1] = vec![0; 32],
                    3 => changed[2] = vec![2, 0, 0, 0],
                    _ => {
                        changed.pop();
                    }
                }
                assert!(sender_recovery_selector(&changed).is_err());
            }
        }
    }
}
#[test]
fn sender_recovery_qualification_requires_the_actual_complete_current_context() {
    let (current, qualification) = context(2);
    let root = current.device_policy_binding.hardware_policy_id;
    require_sender_recovery_qualification(&current, &qualification, root).unwrap();
    for mutation in 0..13 {
        let mut changed = current.clone();
        match mutation {
            0 => changed.credential_id[0] ^= 1,
            1 => changed.release.release_id[0] ^= 1,
            2 => changed.release.hardware_profile_id[0] ^= 1,
            3 => changed.release.suite_id[0] ^= 1,
            4 => changed.release.policy_epoch += 1,
            5 => changed.device_policy_binding.hardware_policy_id[0] ^= 1,
            6 => changed.core_authorization_key_reference[0] ^= 1,
            7 => changed.lane.device_lane_id[0] ^= 1,
            8 => changed.hardware_epoch.generation += 1,
            9 => changed.hardware_epoch.epoch_id[0] ^= 1,
            10 => changed.device_policy_binding.device_key_reference[0] ^= 1,
            11 => {
                changed.lane.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"other-recovery-genesis")),
                )
            }
            _ => {
                assert!(
                    require_sender_recovery_qualification(&changed, &qualification, [0x92; 32])
                        .is_err()
                );
                continue;
            }
        }
        assert!(require_sender_recovery_qualification(&changed, &qualification, root).is_err());
    }
}
#[test]
fn sender_recovery_keeps_original_historical_archive_and_rejects_swapped_axes_or_kind() {
    let (current, _) = context(2);
    let (old, _) = context(1);
    let original = archive(old);
    let retained = sender_recovery_projection(
        NativeSenderRecoverySelectorV1::Operation(original.operation_id),
        Kind::SendSplit,
        Kind::SendSplit,
        Phase::Installed,
        original.clone(),
        &current,
    )
    .unwrap()
    .unwrap();
    assert_eq!(retained, original);
    assert_eq!(
        retained.encode_canonical().unwrap(),
        original.encode_canonical().unwrap()
    );
    for selector in [
        NativeSenderRecoverySelectorV1::Terminal(original.operation_id),
        NativeSenderRecoverySelectorV1::Operation(original.terminal_id),
    ] {
        assert!(
            sender_recovery_projection(
                selector,
                Kind::SendSplit,
                Kind::SendSplit,
                Phase::Installed,
                original.clone(),
                &current
            )
            .is_err()
        );
    }
    assert!(
        sender_recovery_projection(
            NativeSenderRecoverySelectorV1::Terminal(original.terminal_id),
            Kind::RedeemSplit,
            Kind::SendSplit,
            Phase::Installed,
            original.clone(),
            &current
        )
        .is_err()
    );
    let mut future = original;
    future.context.hardware_epoch.generation = current.hardware_epoch.generation + 1;
    assert!(
        sender_recovery_projection(
            NativeSenderRecoverySelectorV1::Operation(future.operation_id),
            Kind::SendSplit,
            Kind::SendSplit,
            Phase::Installed,
            future,
            &current
        )
        .is_err()
    );
}
#[test]
fn unfinished_sender_is_an_error_and_only_verified_released_tombstone_is_absent() {
    let (current, _) = context(1);
    let original = archive(current.clone());
    let selected = NativeSenderRecoverySelectorV1::Operation(original.operation_id);
    for phase in [Phase::Prepared, Phase::CandidatePersisted, Phase::Committed] {
        assert!(
            sender_recovery_projection(
                selected,
                Kind::RedeemSplit,
                Kind::RedeemSplit,
                phase,
                original.clone(),
                &current
            )
            .is_err()
        );
    }
    assert_eq!(
        sender_recovery_projection(
            selected,
            Kind::RedeemSplit,
            Kind::RedeemSplit,
            Phase::Released,
            original,
            &current
        )
        .unwrap(),
        None
    );
}

#[test]
fn installed_reply_envelope_contract_matches_shared_op9_and_op10_wire() {
    use crate::kagemusha_device_bridge_v1::sender_payload::{
        SenderCommandBodyV1, SenderRecordV1, SenderRecoveryItemV1, SenderRecoverySelectorV1,
        SenderReplyBodyV1, SenderReplyV1, terminal_envelope_digest_v1,
    };
    use iroha_data_model::kagemusha::KagemushaPaymentV1;
    // Public codecs and fixtures only. These results do not admit a physical owner or proof.
    let bytes = canonical_command_body_for_tests(9).unwrap();
    let install = SenderCommandV1::decode_canonical_exact(9, [7; 32], &bytes).unwrap();
    let SenderCommandBodyV1::Install {
        selector,
        candidate_digest,
        inputs,
        envelope,
    } = &install.body
    else {
        panic!("original install fixture");
    };
    let crate::kagemusha_device_bridge_v1::sender_payload::SenderPublicInputsV1::SendSplit {
        request,
    } = inputs
    else {
        panic!("original payment input fixture");
    };
    let request =
        iroha_data_model::kagemusha::KagemushaPaymentRequestV1::decode_canonical_exact(request)
            .unwrap();
    let payment =
        KagemushaPaymentV1::decode_canonical_shape_exact_against(envelope, &request).unwrap();
    let record = SenderRecordV1 {
        operation_id: install.operation_id,
        context: install.context.clone(),
        inputs_digest: selector.inputs_digest,
        operation_kind: Kind::SendSplit,
        preparation_id: selector.preparation_id,
        outbox_reservation_id: [0x74; 32],
        outcome_id: payment.output.credit_id,
        phase: SenderPhaseV1::Installed,
        record_revision: 4,
        inputs: Some(inputs.clone()),
        candidate_digest: Some(*candidate_digest),
        commit_certificate_digest: Some(payment.proof.commit_certificate_digest),
        envelope_digest: Some(terminal_envelope_digest_v1(envelope).unwrap()),
        terminal_receipt_digest: None,
    };
    let reply = SenderReplyV1 {
        version: 1,
        operation: 9,
        request_id: install.operation_id,
        context: install.context.clone(),
        index_revision: 4,
        body: SenderReplyBodyV1::Lookup(Some(SenderRecoveryItemV1 {
            record,
            canonical_envelope: Vec::new(),
        })),
    };
    reply.validate_against(&install, &install.context).unwrap();
    require_installed_reply_envelope(&install, &[], envelope).unwrap();
    let mut wrong_install = reply.clone();
    let SenderReplyBodyV1::Lookup(Some(item)) = &mut wrong_install.body else {
        unreachable!()
    };
    item.canonical_envelope = envelope.clone();
    assert!(
        wrong_install
            .validate_against(&install, &install.context)
            .is_err()
    );
    assert!(require_installed_reply_envelope(&install, envelope, envelope).is_err());
    let recover = SenderCommandV1 {
        version: 1,
        operation: 10,
        operation_id: install.operation_id,
        context: install.context.clone(),
        body: SenderCommandBodyV1::RecoverInstalled {
            selector: SenderRecoverySelectorV1::Lookup {
                inputs_digest: selector.inputs_digest,
            },
        },
    };
    let mut recovered = wrong_install;
    recovered.operation = 10;
    recovered
        .validate_against(&recover, &recover.context)
        .unwrap();
    require_installed_reply_envelope(&recover, envelope, envelope).unwrap();
    let mut changed = envelope.clone();
    changed[0] ^= 1;
    assert!(require_installed_reply_envelope(&recover, &changed, envelope).is_err());
    assert!(require_installed_reply_envelope(&recover, &[], envelope).is_err());
    let mut changed_command = install.clone();
    let SenderCommandBodyV1::Install {
        envelope: command_envelope,
        ..
    } = &mut changed_command.body
    else {
        unreachable!()
    };
    command_envelope[0] ^= 1;
    assert!(require_installed_reply_envelope(&changed_command, &[], envelope).is_err());
    for operation in [0, 8, 11, 12] {
        let mut wrong_operation = recover.clone();
        wrong_operation.operation = operation;
        assert!(require_installed_reply_envelope(&wrong_operation, envelope, envelope).is_err());
    }
    assert!(require_installed_reply_envelope(&install, &[], &[]).is_err());
}
