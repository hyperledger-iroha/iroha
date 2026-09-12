//! Ordinary attacks on the simulated register; no proof generator or accepting Core mock is used.

use super::*;

fn fixture() -> (
    DiagnosticCheckpointRegister,
    KagemushaRecoveryCheckpointStatementV1,
) {
    let material = core_bound_mint_recipient_material(
        digest(b"checkpoint-register-release", 0),
        digest(b"vk-set", 0),
        digest(b"checkpoint-register-manifest", 0),
        1_000,
    );
    let preview = bootstrap_preview(&material, provisional_artifacts(&material));
    let hardware = DiagnosticCheckpointRegister::new(&material, &preview.state).unwrap();
    let state = &preview.state;
    let statement = KagemushaRecoveryCheckpointStatementV1 {
        operation_id: digest(b"checkpoint-register-operation", 0),
        previous: KagemushaRecoveryCheckpointIdentityV1 {
            revision: 0,
            snapshot_commitment: [0; 32],
        },
        successor: DurabilityAnchorStatementV1 {
            metadata_revision: 1,
            version: state.version,
            lane: state.lane.clone(),
            state_commitment: state.state_commitment,
            hardware_epoch: state.hardware_epoch,
            device_policy_binding: state.device_policy_binding,
            state_nonce_commitment: state.state_nonce_commitment,
            logical_sequence: state.logical_sequence,
            journal_revision: 0,
            inbox_revision: 0,
            snapshot_commitment: digest(b"checkpoint-register-opaque-snapshot", 0),
        },
    };
    (hardware, statement)
}

fn retain(
    hardware: &DiagnosticCheckpointRegister,
    statement: &KagemushaRecoveryCheckpointStatementV1,
) {
    // These are opaque storage-protocol bytes, deliberately not a Core snapshot or proof.
    // The production diagnostic caller can only use retain_pending with an opaque Core stage.
    hardware
        .retain_bytes(
            statement,
            norito::encode_canonical(&(1_u16, statement.clone())).unwrap(),
        )
        .unwrap();
}

#[test]
fn simulated_checkpoint_enrollment_pins_recipient_and_complete_runtime() {
    let material = core_bound_mint_recipient_material(
        digest(b"checkpoint-enrollment-release", 0),
        digest(b"vk-set", 0),
        digest(b"checkpoint-enrollment-manifest", 0),
        1_000,
    );
    let preview = bootstrap_preview(&material, provisional_artifacts(&material));
    let binding = diagnostic_enrollment_binding(&material, &preview.state).unwrap();
    assert_eq!(binding.owner.account_id, material.recipient);
    assert_eq!(binding.owner.runtime.fi_id.as_ref(), "diagnostic-fi");
    assert_eq!(
        binding.owner.runtime.authentication_namespace.as_ref(),
        "diagnostic-auth"
    );
    assert_eq!(
        binding.owner.runtime.ledger_dataspace_id,
        iroha_model_base::topology::DataSpaceId::new(10)
    );
    assert_eq!(
        binding.owner.runtime.network_id,
        preview.state.lane.network_id
    );
    assert_eq!(binding.owner.runtime.asset, preview.state.lane.asset);
    assert_eq!(
        binding.owner.runtime.asset_incarnation,
        preview.state.asset_incarnation
    );
    assert_eq!(binding.owner.runtime.scale, preview.state.lane.scale);
    assert_eq!(binding.owner.lane_id, preview.state.lane.device_lane_id);
    assert_eq!(
        binding.enrollment_id,
        binding.owner.enrollment_id().unwrap()
    );
    let hardware = DiagnosticCheckpointRegister::new(&material, &preview.state).unwrap();
    assert_eq!(hardware.enrollment, binding);

    let mut foreign_material = material;
    foreign_material.recipient = AccountId::new(
        KeyPair::from_seed(vec![92; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let foreign = DiagnosticCheckpointRegister::new(&foreign_material, &preview.state).unwrap();
    assert_eq!(
        foreign.enrollment.owner.account_id,
        foreign_material.recipient
    );
    assert_ne!(foreign.enrollment.enrollment_id, binding.enrollment_id);
    assert_ne!(foreign.identity, hardware.identity);

    let mut invalid = preview.state;
    invalid.lane.device_lane_id = [0; 32];
    assert!(diagnostic_enrollment_binding(&foreign_material, &invalid).is_err());
}

#[test]
fn simulated_checkpoint_pins_original_governed_credential_and_policy() {
    let material = core_bound_mint_recipient_material(
        digest(b"checkpoint-identity-release", 0),
        digest(b"vk-set", 0),
        digest(b"checkpoint-identity-manifest", 0),
        1_000,
    );
    let preview = bootstrap_preview(&material, provisional_artifacts(&material));
    let hardware = DiagnosticCheckpointRegister::new(&material, &preview.state).unwrap();
    assert_eq!(hardware.credential, material.hardware_credential);
    type Mutation = fn(&mut KagemushaStateV1);
    let mutations: [Mutation; 9] = [
        |state| {
            let mut bytes = *state.lane.network_id.as_bytes();
            bytes[0] ^= 1;
            state.lane.network_id = iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(bytes)),
            );
        },
        |state| state.lane.device_lane_id[0] ^= 1,
        |state| state.hardware_epoch.epoch_id[0] ^= 1,
        |state| state.hardware_epoch.generation += 1,
        |state| state.device_policy_binding.device_key_reference[0] ^= 1,
        |state| state.device_policy_binding.hardware_policy_id[0] ^= 1,
        |state| state.hardware_profile_id[0] ^= 1,
        |state| state.policy_epoch += 1,
        |state| state.release_id[0] ^= 1,
    ];
    for mutate in mutations {
        let mut changed = preview.state.clone();
        mutate(&mut changed);
        assert!(DiagnosticCheckpointRegister::new(&material, &changed).is_err());
    }
    let mut changed = material;
    changed.hardware_credential.expires_at_ms -= 1;
    assert!(DiagnosticCheckpointRegister::new(&changed, &preview.state).is_err());
}

#[test]
fn simulated_checkpoint_signature_without_persisted_cas_is_rejected() {
    let (hardware, statement) = fixture();
    let signature = sign_journal(
        &hardware.key,
        CAS_DOMAIN,
        &(hardware.identity, statement.clone()),
    );
    verify_journal(
        &device_public_key(&hardware.key),
        CAS_DOMAIN,
        &(hardware.identity, statement.clone()),
        &signature,
    )
    .unwrap();
    assert!(hardware.verify_cas(&statement, &signature).is_err());
    assert!(hardware.commit(&statement).is_err());
    retain(&hardware, &statement);
    assert!(hardware.verify_cas(&statement, &signature).is_err());
    let original = hardware.commit(&statement).unwrap();
    hardware.verify_cas(&statement, &original).unwrap();
    let mut foreign_identity = hardware.clone();
    foreign_identity.identity[0] ^= 1;
    assert!(foreign_identity.verify_cas(&statement, &original).is_err());
    hardware
        .verify_anchor(&statement.successor, &original)
        .unwrap();
    let mut changed = original;
    changed[0] ^= 1;
    assert!(hardware.verify_cas(&statement, &changed).is_err());
    assert!(
        hardware
            .verify_anchor(&statement.successor, &changed)
            .is_err()
    );
    let mut unissued = statement.successor;
    unissued.snapshot_commitment[0] ^= 1;
    assert!(hardware.verify_anchor(&unissued, &signature).is_err());
}

#[test]
fn simulated_checkpoint_exact_lost_reply_retry_keeps_original_bytes() {
    let (hardware, statement) = fixture();
    retain(&hardware, &statement);
    let original = hardware.commit(&statement).unwrap();
    let clone = hardware.clone();
    retain(&clone, &statement);
    assert_eq!(clone.commit(&statement).unwrap(), original);
    assert_eq!(hardware.register.borrow().terminals.len(), 1);
    assert!(clone.retain_bytes(&statement, vec![9]).is_err());
    let mut changed = statement.clone();
    changed.successor.snapshot_commitment[0] ^= 1;
    assert!(clone.retain_bytes(&changed, vec![9]).is_err());
    assert!(clone.commit(&changed).is_err());
    hardware
        .verify_current(&statement.successor, &hardware.journals())
        .unwrap();
    clone
        .verify_current(&statement.successor, &clone.journals())
        .unwrap();
    assert_eq!(hardware.register.borrow().next_challenge, 2);
    assert_eq!(hardware.commit(&statement).unwrap(), original);
}

#[test]
fn simulated_checkpoint_race_and_historical_current_selection_are_rejected() {
    let (hardware, first) = fixture();
    let mut competing = first.clone();
    competing.operation_id[0] ^= 1;
    competing.successor.snapshot_commitment[0] ^= 1;
    retain(&hardware, &first);
    retain(&hardware, &competing);
    let original = hardware.commit(&first).unwrap();
    assert!(hardware.clone().commit(&competing).is_err());
    let mut next = first.clone();
    next.operation_id[1] ^= 1;
    next.previous = checkpoint_identity(&first.successor);
    next.successor.metadata_revision = 2;
    next.successor.snapshot_commitment[1] ^= 1;
    retain(&hardware, &next);
    let next_certificate = hardware.commit(&next).unwrap();
    hardware.verify_cas(&first, &original).unwrap();
    assert_eq!(hardware.commit(&first).unwrap(), original);
    assert!(
        hardware
            .verify_current(&first.successor, &hardware.journals())
            .is_err()
    );
    hardware.verify_cas(&next, &next_certificate).unwrap();
    hardware
        .verify_current(&next.successor, &hardware.journals())
        .unwrap();
}

#[test]
fn simulated_checkpoint_rejects_noncanonical_predecessors_and_foreign_wallets() {
    let (hardware, statement) = fixture();
    type Mutation = fn(&mut KagemushaRecoveryCheckpointStatementV1);
    let mutations: [Mutation; 11] = [
        |cas| cas.operation_id = [0; 32],
        |cas| cas.previous.snapshot_commitment = [1; 32],
        |cas| {
            cas.previous.revision = 1;
            cas.successor.metadata_revision = 2;
        },
        |cas| {
            cas.previous.revision = u128::MAX;
            cas.previous.snapshot_commitment = [1; 32];
        },
        |cas| cas.successor.metadata_revision = 3,
        |cas| cas.successor.snapshot_commitment = [0; 32],
        |cas| cas.successor.version += 1,
        |cas| cas.successor.lane.device_lane_id[0] ^= 1,
        |cas| cas.successor.hardware_epoch.epoch_id[0] ^= 1,
        |cas| cas.successor.device_policy_binding.device_key_reference[0] ^= 1,
        |cas| cas.successor.device_policy_binding.hardware_policy_id[0] ^= 1,
    ];
    for mutate in mutations {
        let mut changed = statement.clone();
        mutate(&mut changed);
        assert!(hardware.retain_bytes(&changed, vec![1]).is_err());
        assert!(hardware.commit(&changed).is_err());
    }
    assert!(hardware.retain_bytes(&statement, Vec::new()).is_err());
    assert!(hardware.register.borrow().material.is_empty());
}

#[test]
fn simulated_checkpoint_checks_actual_snapshot_and_journal_bytes() {
    let (hardware, statement) = fixture();
    retain(&hardware, &statement);
    let saved_before_cas = hardware
        .register
        .borrow()
        .material
        .get(&statement.operation_id)
        .unwrap()
        .clone();
    hardware
        .register
        .borrow_mut()
        .material
        .get_mut(&statement.operation_id)
        .unwrap()
        .snapshot[0] ^= 1;
    assert!(
        hardware.commit(&statement).is_err(),
        "pre-CAS storage corruption cannot become the original snapshot"
    );
    assert!(hardware.register.borrow().current.is_none());
    assert!(hardware.register.borrow().terminals.is_empty());
    hardware
        .register
        .borrow_mut()
        .material
        .insert(statement.operation_id, saved_before_cas);
    let original = hardware.commit(&statement).unwrap();
    let saved = hardware
        .register
        .borrow()
        .material
        .get(&statement.operation_id)
        .unwrap()
        .clone();
    hardware
        .register
        .borrow_mut()
        .material
        .remove(&statement.operation_id);
    assert!(
        hardware
            .verify_current(&statement.successor, &hardware.journals())
            .is_err()
    );
    hardware
        .register
        .borrow_mut()
        .material
        .insert(statement.operation_id, saved.clone());
    hardware
        .register
        .borrow_mut()
        .material
        .get_mut(&statement.operation_id)
        .unwrap()
        .snapshot[0] ^= 1;
    assert!(hardware.verify_cas(&statement, &original).is_err());
    assert!(
        hardware
            .verify_current(&statement.successor, &hardware.journals())
            .is_err()
    );
    hardware
        .register
        .borrow_mut()
        .material
        .insert(statement.operation_id, saved);
    let coordinator = hardware.register.borrow().coordinator.clone();
    let responses = hardware.register.borrow().responses.clone();
    for role in 0..2 {
        for mutation in 0..3 {
            {
                let mut register = hardware.register.borrow_mut();
                let bytes = if role == 0 {
                    &mut register.coordinator
                } else {
                    &mut register.responses
                };
                match mutation {
                    0 => bytes[0] ^= 1,
                    1 => {
                        bytes.pop();
                    }
                    2 => bytes.push(0), // Every speculative suffix is rejected by this fixture.
                    _ => unreachable!(),
                }
            }
            assert!(
                hardware
                    .verify_current(&statement.successor, &hardware.journals())
                    .is_err()
            );
            assert!(hardware.commit(&statement).is_err());
            hardware.register.borrow_mut().coordinator = coordinator.clone();
            hardware.register.borrow_mut().responses = responses.clone();
        }
    }
    hardware
        .verify_current(&statement.successor, &hardware.journals())
        .unwrap();
}

#[test]
fn simulated_checkpoint_rejects_changed_selected_journal_projections() {
    let (hardware, statement) = fixture();
    retain(&hardware, &statement);
    hardware.commit(&statement).unwrap();
    type Mutation = fn(&mut KagemushaRecoveryJournalsV1);
    let mutations: [Mutation; 8] = [
        |journals| journals.coordinator.sequence += 1,
        |journals| journals.coordinator.head[0] ^= 1,
        |journals| journals.coordinator.byte_len += 1,
        |journals| journals.responses.sequence += 1,
        |journals| journals.responses.head[0] ^= 1,
        |journals| journals.responses.byte_len += 1,
        |journals| journals.response_history_root[0] ^= 1,
        |journals| journals.retirement_transition_id[0] ^= 1,
    ];
    for mutate in mutations {
        let mut changed = hardware.journals();
        mutate(&mut changed);
        assert!(
            hardware
                .verify_current(&statement.successor, &changed)
                .is_err()
        );
    }
}

#[test]
fn simulated_checkpoint_freshness_cannot_reuse_challenge_or_foreign_signature() {
    let (hardware, statement) = fixture();
    assert!(
        hardware
            .verify_current(&statement.successor, &hardware.journals())
            .is_err()
    );
    retain(&hardware, &statement);
    hardware.commit(&statement).unwrap();
    let response = hardware
        .current_response(&hardware.register.borrow(), 7)
        .unwrap();
    hardware
        .verify_current_response(7, &statement.successor, &hardware.journals(), &response)
        .unwrap();
    assert!(
        hardware
            .verify_current_response(8, &statement.successor, &hardware.journals(), &response)
            .is_err()
    );
    assert!(
        hardware
            .verify_current_response(0, &statement.successor, &hardware.journals(), &response)
            .is_err()
    );
    let mut foreign = response;
    foreign.1 = sign_journal(
        &deterministic_signing_key(0x7711),
        CURRENT_DOMAIN,
        &(
            hardware.identity,
            7_u64,
            foreign.0.clone(),
            hardware.journals(),
        ),
    );
    assert!(
        hardware
            .verify_current_response(7, &statement.successor, &hardware.journals(), &foreign)
            .is_err()
    );
    hardware.register.borrow_mut().next_challenge = u64::MAX;
    assert!(
        hardware
            .verify_current(&statement.successor, &hardware.journals())
            .is_err()
    );
}

#[test]
fn simulated_checkpoint_native_bootstrap_selection_does_not_authorize_recovery() {
    let (hardware, statement) = fixture();
    let mut journals = hardware.journals();
    journals.coordinator.head[0] ^= 1;
    journals.responses.head[0] ^= 1;
    journals.retirement_transition_id = statement.operation_id;
    // Exercise only the private storage protocol. No Core stage or real WAL is claimed here.
    let bytes = norito::encode_canonical(&(1_u16, statement.clone())).unwrap();
    hardware
        .retain_material(
            &statement,
            bytes.clone(),
            journals,
            JournalSource::CoreBootstrap,
        )
        .unwrap();
    let certificate = hardware.commit(&statement).unwrap();
    hardware.verify_cas(&statement, &certificate).unwrap();
    hardware
        .verify_current(&statement.successor, &journals)
        .unwrap();
    assert!(
        hardware
            .verify_current(&statement.successor, &hardware.journals())
            .is_err()
    );
    assert!(
        hardware
            .verify_anchor(&statement.successor, &certificate)
            .is_err(),
        "bootstrap descriptor checks do not supply a native restoration owner"
    );
    assert!(
        hardware
            .retain_material(
                &statement,
                bytes.clone(),
                hardware.journals(),
                JournalSource::CoreBootstrap
            )
            .is_err()
    );
    assert!(
        hardware
            .retain_material(&statement, bytes, journals, JournalSource::Simulated)
            .is_err()
    );
    hardware
        .register
        .borrow_mut()
        .material
        .get_mut(&statement.operation_id)
        .unwrap()
        .snapshot[0] ^= 1;
    assert!(
        hardware
            .verify_current(&statement.successor, &journals)
            .is_err()
    );
}
