//! Real P-256 certificates over an explicitly simulated non-forking checkpoint register.
//! This validates Core publication/recovery semantics, not physical hardware qualification.

use super::*;
use crate::zk::kagemusha_v1_state::sparse_merkle::authenticated_history::{
    KagemushaHistoryNodeRecordV1, KagemushaHistoryRootSelectionV1,
};
use crate::zk::kagemusha_v1_state::tests::{
    AcceptSnapshotRecursiveVerifierV1, coordinator_operation_store_tests,
    snapshot_device_public_key, snapshot_device_signature,
};
use p256::ecdsa::SigningKey;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

type Machine = KagemushaStateMachineV1<AcceptSnapshotRecursiveVerifierV1, SimulatedHardware>;

#[derive(Clone)]
struct SimulatedHardware {
    key: Rc<SigningKey>,
    register: Rc<RefCell<Register>>,
    material_available: Rc<Cell<bool>>,
    challenge_counter: Rc<Cell<u64>>,
}

#[derive(Default)]
struct Register {
    current: Option<DurabilityAnchorStatementV1>,
    terminals: BTreeMap<
        DigestV1,
        (
            KagemushaRecoveryCheckpointStatementV1,
            Vec<u8>,
            KagemushaRecoveryJournalsV1,
        ),
    >,
}

fn signing_bytes(statement: &KagemushaRecoveryCheckpointStatementV1) -> Vec<u8> {
    let mut bytes = b"test-only:offline:metadata-cas\0".to_vec();
    bytes.extend(norito::encode_canonical(statement).unwrap());
    bytes
}

impl SimulatedHardware {
    fn commit(
        &self,
        candidate: &KagemushaRecoveryCheckpointCandidateV1,
    ) -> Result<Vec<u8>, String> {
        let statement = candidate.statement();
        let mut register = self.register.borrow_mut();
        if let Some((previous, certificate, journals)) =
            register.terminals.get(&statement.operation_id)
        {
            if previous != statement || journals != &candidate.snapshot.recovery_metadata.journals {
                return Err("conflicting checkpoint identity".into());
            }
            return Ok(certificate.clone());
        }
        let predecessor = register.current.as_ref().map_or(
            KagemushaRecoveryCheckpointIdentityV1::INITIAL,
            KagemushaRecoveryCheckpointIdentityV1::from_anchor,
        );
        if predecessor != statement.previous {
            return Err("stale hardware predecessor".into());
        }
        let signature = snapshot_device_signature(&self.key, &signing_bytes(statement));
        let certificate = norito::encode_canonical(&signature).unwrap();
        register.current = Some(statement.successor.clone());
        register.terminals.insert(
            statement.operation_id,
            (
                statement.clone(),
                certificate.clone(),
                candidate.snapshot.recovery_metadata.journals.clone(),
            ),
        );
        Ok(certificate)
    }
}

impl KagemushaGuardBundleVerifierV1 for SimulatedHardware {
    fn verify_bootstrap(
        &self,
        _: &BootstrapStatementV1,
        _: &KagemushaNormalizedGuardStatementV1,
        _: &[u8],
    ) -> Result<(), String> {
        Err("unused test bootstrap proof hook".into())
    }
    fn verify_transition(
        &self,
        _: &HardwareTransitionStatementV1,
        _: &TransitionProofStatementV1,
        _: &KagemushaNormalizedGuardStatementV1,
        _: &[u8],
    ) -> Result<(), String> {
        Err("unused test monetary hook".into())
    }
    fn verify_credit_stage(&self, _: &CreditStageStatementV1, _: &[u8]) -> Result<(), String> {
        Err("unused test credit hook".into())
    }
    fn verify_durability_anchor(
        &self,
        statement: &DurabilityAnchorStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        let register = self.register.borrow();
        let (cas, original, _) = register
            .terminals
            .values()
            .find(|(cas, _, _)| &cas.successor == statement)
            .ok_or("unissued checkpoint")?;
        if original != bytes {
            return Err("changed original certificate".into());
        }
        let signature: KagemushaDeviceSignatureV1 =
            norito::decode_from_bytes(bytes).map_err(|e| e.to_string())?;
        signature
            .verify(&snapshot_device_public_key(&self.key), &signing_bytes(cas))
            .map_err(|e| e.to_string())
    }
    fn verify_recovery_checkpoint_cas(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        let register = self.register.borrow();
        let (original, original_bytes, _) = register
            .terminals
            .get(&statement.operation_id)
            .ok_or("CAS never executed")?;
        if original != statement || original_bytes != bytes {
            return Err("CAS certificate conflict".into());
        }
        let signature: KagemushaDeviceSignatureV1 =
            norito::decode_from_bytes(bytes).map_err(|e| e.to_string())?;
        signature
            .verify(
                &snapshot_device_public_key(&self.key),
                &signing_bytes(statement),
            )
            .map_err(|e| e.to_string())
    }
    fn verify_current_recovery_checkpoint(
        &self,
        statement: &DurabilityAnchorStatementV1,
        journals: &KagemushaRecoveryJournalsV1,
    ) -> Result<(), String> {
        if !self.material_available.get() {
            return Err("native recovery files unavailable".into());
        }
        let register = self.register.borrow();
        if register.current.as_ref() != Some(statement) {
            return Err("historical checkpoint is not current".into());
        }
        if !register
            .terminals
            .values()
            .any(|(cas, _, actual)| &cas.successor == statement && actual == journals)
        {
            return Err("native journal material mismatch".into());
        }
        // Freshness is owned by this simulated device exchange, never supplied by the caller.
        let challenge = self
            .challenge_counter
            .get()
            .checked_add(1)
            .ok_or("test challenge overflow")?;
        self.challenge_counter.set(challenge);
        let mut message = b"test-only:offline:current-checkpoint\0".to_vec();
        message.extend(challenge.to_le_bytes());
        message.extend(norito::encode_canonical(statement).unwrap());
        snapshot_device_signature(&self.key, &message)
            .verify(&snapshot_device_public_key(&self.key), &message)
            .map_err(|e| e.to_string())
    }
}

fn fixture() -> (Machine, SimulatedHardware) {
    let original = coordinator_operation_store_tests::machine().0;
    let hardware = SimulatedHardware {
        key: Rc::new(SigningKey::from_bytes((&[119; 32]).into()).unwrap()),
        register: Rc::new(RefCell::new(Register::default())),
        material_available: Rc::new(Cell::new(true)),
        challenge_counter: Rc::new(Cell::new(0)),
    };
    let KagemushaStateMachineV1 {
        recovery_metadata,
        state,
        journal_revision,
        inbox_revision,
        pending_credits,
        accepted_recipient_bindings,
        accepted_payment_receipts,
        mint_inbox,
        consumed_credits,
        authenticated_history,
        receiver_inbox_capacity,
        sender_outbox_capacity,
        outgoing_candidate_journal,
        proof_release,
        recursive_verifier,
        ..
    } = original;
    let machine = KagemushaStateMachineV1 {
        recovery_metadata,
        published_checkpoint: None,
        state,
        journal_revision,
        inbox_revision,
        pending_credits,
        accepted_recipient_bindings,
        accepted_payment_receipts,
        mint_inbox,
        consumed_credits,
        authenticated_history,
        receiver_inbox_capacity,
        sender_outbox_capacity,
        outgoing_candidate_journal,
        proof_release,
        recursive_verifier,
        guard_verifier: hardware.clone(),
    };
    let mut machine = machine;
    let snapshot = machine.snapshot().unwrap();
    let candidate = KagemushaRecoveryCheckpointCandidateV1 {
        before_snapshot_commitment: snapshot.snapshot_commitment,
        statement: snapshot
            .recovery_metadata
            .checkpoint_statement(snapshot.recovery_anchor()),
        snapshot,
    };
    let certificate = hardware.commit(&candidate).unwrap();
    machine
        .install_recovery_checkpoint(&candidate, certificate)
        .unwrap();
    (machine, hardware)
}

fn resign(mut credential: KagemushaHardwareCredentialV1) -> KagemushaHardwareCredentialV1 {
    let issuer = SigningKey::from_bytes((&[8; 32]).into()).unwrap();
    credential.credential_id = credential.expected_credential_id().unwrap();
    credential.governance_signature =
        snapshot_device_signature(&issuer, &credential.canonical_signing_bytes().unwrap());
    credential
}

fn renewal(machine: &Machine, issued_at_ms: u64) -> KagemushaHardwareCredentialV1 {
    let mut credential = machine.accepted_credential_floor().credential.clone();
    credential.issued_at_ms = issued_at_ms;
    resign(credential)
}

fn candidate(machine: &Machine, tag: u8) -> KagemushaRecoveryCheckpointCandidateV1 {
    machine
        .prepare_recovery_checkpoint([tag; 32], machine.recovery_metadata.journals.clone())
        .unwrap()
}

fn restore(
    machine: &Machine,
    snapshot: KagemushaStateSnapshotV1,
    anchor: &DurabilityAnchorV1,
) -> Result<Machine, KagemushaStateErrorV1> {
    Machine::restore(
        snapshot,
        anchor,
        machine.proof_release.clone(),
        machine.proof_release.clone(),
        machine.enrollment_binding(),
        machine.authenticated_history.clone().into_store(),
        AcceptSnapshotRecursiveVerifierV1,
        machine.guard_verifier.clone(),
    )
}

#[test]
fn credential_is_not_installed_before_signed_cas_and_fresh_material_selection() {
    let (mut machine, hardware) = fixture();
    let before = machine.snapshot().unwrap();
    let next = renewal(&machine, 20);
    let candidate = machine
        .prepare_credential_checkpoint(
            [31; 32],
            machine.recovery_metadata.journals.clone(),
            next.clone(),
        )
        .unwrap();
    assert_eq!(machine.snapshot().unwrap(), before);
    assert_eq!(
        candidate.snapshot.state.state_commitment,
        before.state.state_commitment
    );
    assert_ne!(
        candidate.snapshot.snapshot_commitment,
        before.snapshot_commitment
    );
    assert!(matches!(
        machine.install_recovery_checkpoint(&candidate, vec![1]),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
    let certificate = hardware.commit(&candidate).unwrap();
    hardware.material_available.set(false);
    assert!(matches!(
        machine.install_recovery_checkpoint(&candidate, certificate.clone()),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
    assert_eq!(machine.snapshot().unwrap(), before);
    hardware.material_available.set(true);
    let anchor = machine
        .install_recovery_checkpoint(&candidate, certificate.clone())
        .unwrap();
    assert_eq!(machine.accepted_credential_floor().credential, next);
    assert_eq!(machine.recovery_metadata.revision, 2);
    assert_eq!(
        machine
            .install_recovery_checkpoint(&candidate, certificate)
            .unwrap(),
        anchor
    );
    assert!(hardware.challenge_counter.get() >= 3);
}

#[test]
fn exact_retry_keeps_original_certificate_and_conflicting_retry_fails() {
    let (mut machine, hardware) = fixture();
    let candidate = candidate(&machine, 32);
    let certificate = hardware.commit(&candidate).unwrap();
    assert_eq!(hardware.commit(&candidate).unwrap(), certificate);
    let anchor = machine
        .install_recovery_checkpoint(&candidate, certificate.clone())
        .unwrap();
    let mut changed = certificate;
    *changed.last_mut().unwrap() ^= 1;
    assert!(matches!(
        machine.install_recovery_checkpoint(&candidate, changed),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    assert_eq!(machine.recovery_checkpoint(), &anchor);
}

#[test]
fn signed_same_epoch_issuance_rollback_and_equal_time_changes_are_rejected() {
    let (mut machine, hardware) = fixture();
    let candidate = machine
        .prepare_credential_checkpoint(
            [33; 32],
            machine.recovery_metadata.journals.clone(),
            renewal(&machine, 30),
        )
        .unwrap();
    let certificate = hardware.commit(&candidate).unwrap();
    machine
        .install_recovery_checkpoint(&candidate, certificate)
        .unwrap();
    assert!(matches!(
        machine.prepare_credential_checkpoint(
            [34; 32],
            machine.recovery_metadata.journals.clone(),
            renewal(&machine, 20)
        ),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    let mut changed = machine.accepted_credential_floor().credential.clone();
    changed.expires_at_ms -= 1;
    let changed = resign(changed);
    assert!(matches!(
        machine.prepare_credential_checkpoint(
            [35; 32],
            machine.recovery_metadata.journals.clone(),
            changed
        ),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    let same = machine
        .prepare_credential_checkpoint(
            [36; 32],
            machine.recovery_metadata.journals.clone(),
            machine.accepted_credential_floor().credential.clone(),
        )
        .unwrap();
    assert_eq!(
        same.snapshot.recovery_metadata.accepted_credential,
        machine.recovery_metadata.accepted_credential
    );
}

#[test]
fn authentic_issuer_cannot_move_floor_to_wrong_wallet_epoch_key_or_profile() {
    let (machine, _) = fixture();
    for index in 0..6 {
        let mut credential = renewal(&machine, 20);
        match index {
            0 => credential.lane_commitment = [88; 32],
            1 => credential.hardware_epoch_generation = 0,
            2 => credential.hardware_epoch_id = [89; 32],
            3 => {
                credential.device_public_key = snapshot_device_public_key(
                    &SigningKey::from_bytes((&[99; 32]).into()).unwrap(),
                );
                credential.device_key_reference =
                    kagemusha_device_key_reference_v1(&credential.device_public_key);
            }
            4 => credential.hardware_profile_id = [90; 32],
            5 => credential.suite_id = [91; 32],
            _ => unreachable!(),
        }
        let credential = resign(credential);
        assert!(
            machine
                .prepare_credential_checkpoint(
                    [37; 32],
                    machine.recovery_metadata.journals.clone(),
                    credential
                )
                .is_err(),
            "case {index}"
        );
    }
}

#[test]
fn changed_current_state_invalidates_prepared_checkpoint_without_installing_metadata() {
    let (mut machine, hardware) = fixture();
    let candidate = candidate(&machine, 38);
    let before = machine.recovery_metadata.clone();
    let certificate = hardware.commit(&candidate).unwrap();
    machine.inbox_revision += 1;
    assert!(matches!(
        machine.install_recovery_checkpoint(&candidate, certificate),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    assert_eq!(machine.recovery_metadata, before);
}

#[test]
fn concurrent_checkpoint_cas_rejects_stale_predecessor() {
    let (mut machine, hardware) = fixture();
    let first = candidate(&machine, 39);
    let second = candidate(&machine, 40);
    let certificate = hardware.commit(&first).unwrap();
    assert!(hardware.commit(&second).is_err());
    machine
        .install_recovery_checkpoint(&first, certificate)
        .unwrap();
    assert!(matches!(
        machine.install_recovery_checkpoint(&second, vec![1]),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
}

#[test]
fn restart_requires_current_hardware_selection_even_for_authentic_old_snapshot() {
    let (mut machine, hardware) = fixture();
    let old = machine.snapshot().unwrap();
    let old_anchor = machine.recovery_checkpoint().clone();
    let candidate = machine
        .prepare_credential_checkpoint(
            [41; 32],
            machine.recovery_metadata.journals.clone(),
            renewal(&machine, 20),
        )
        .unwrap();
    let certificate = hardware.commit(&candidate).unwrap();
    let anchor = machine
        .install_recovery_checkpoint(&candidate, certificate)
        .unwrap();
    assert!(matches!(
        restore(&machine, old, &old_anchor),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
    let restored = restore(&machine, machine.snapshot().unwrap(), &anchor).unwrap();
    assert_eq!(
        restored.accepted_credential_floor(),
        machine.accepted_credential_floor()
    );
    assert!(
        restored
            .prepare_credential_checkpoint(
                [42; 32],
                restored.recovery_metadata.journals.clone(),
                renewal(&restored, 10)
            )
            .is_err()
    );
}

#[test]
fn recomputed_host_metadata_or_missing_journal_material_cannot_restore() {
    let (machine, hardware) = fixture();
    let anchor = machine.recovery_checkpoint().clone();
    let mut tampered = machine.snapshot().unwrap();
    tampered.recovery_metadata.journals.responses.head = [222; 32];
    tampered.recompute_commitment().unwrap();
    assert!(matches!(
        restore(&machine, tampered, &anchor),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    hardware.material_available.set(false);
    assert!(matches!(
        restore(&machine, machine.snapshot().unwrap(), &anchor),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
}

#[test]
fn journal_high_water_marks_cannot_reset_or_change_at_equal_sequence() {
    let (machine, _) = fixture();
    for index in 0..3 {
        let mut journals = machine.recovery_metadata.journals.clone();
        match index {
            0 => journals.coordinator.sequence = 0,
            1 => journals.responses.head = [221; 32],
            2 => journals.response_history_root = [220; 32],
            _ => unreachable!(),
        }
        assert!(
            machine
                .prepare_recovery_checkpoint([43; 32], journals)
                .is_err()
        );
    }
    let mut journals = machine.recovery_metadata.journals.clone();
    journals.responses.sequence += 1;
    journals.responses.byte_len += 88;
    journals.responses.head = [219; 32];
    assert!(
        machine
            .prepare_recovery_checkpoint([43; 32], journals)
            .is_ok()
    );
}

#[test]
fn metadata_revision_never_wraps() {
    let (mut machine, _) = fixture();
    machine.recovery_metadata.revision = u128::MAX;
    machine.recovery_metadata.previous_checkpoint.revision = u128::MAX - 1;
    machine
        .published_checkpoint
        .as_mut()
        .unwrap()
        .anchor
        .statement
        .metadata_revision = u128::MAX;
    assert!(matches!(
        machine.prepare_recovery_checkpoint([44; 32], machine.recovery_metadata.journals.clone()),
        Err(KagemushaStateErrorV1::ArithmeticOverflow)
    ));
}

#[test]
fn restore_requires_opaque_original_floor_catalog_and_original_signature() {
    let (machine, _) = fixture();
    let mut other = machine.proof_release.clone();
    other.artifacts.release_id = [217; 32];
    let snapshot = machine.snapshot().unwrap();
    let anchor = machine.recovery_checkpoint().clone();
    assert!(matches!(
        Machine::restore(
            snapshot,
            &anchor,
            machine.proof_release.clone(),
            other,
            machine.enrollment_binding(),
            machine.authenticated_history.clone().into_store(),
            AcceptSnapshotRecursiveVerifierV1,
            machine.guard_verifier.clone()
        ),
        Err(KagemushaStateErrorV1::InvalidReleaseOrLiabilityPool)
    ));
    let mut changed = renewal(&machine, 20);
    changed.governance_signature = snapshot_device_signature(
        &SigningKey::from_bytes((&[216; 32]).into()).unwrap(),
        &changed.canonical_signing_bytes().unwrap(),
    );
    assert!(
        machine
            .prepare_credential_checkpoint(
                [45; 32],
                machine.recovery_metadata.journals.clone(),
                changed
            )
            .is_err()
    );
}

#[test]
fn snapshot_metadata_roundtrips_canonically_and_omission_is_not_accepted() {
    let (machine, _) = fixture();
    let snapshot = machine.snapshot().unwrap();
    let bytes = norito::encode_canonical(&snapshot).unwrap();
    let decoded: KagemushaStateSnapshotV1 = norito::decode_from_bytes(&bytes).unwrap();
    assert_eq!(decoded, snapshot);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
    assert!(
        norito::decode_from_bytes::<KagemushaStateSnapshotV1>(
            &norito::encode_canonical(&snapshot.state).unwrap()
        )
        .is_err()
    );
}

#[test]
fn production_default_hooks_reject_structural_checkpoint_without_qualified_backend() {
    let (machine, _) = fixture();
    let candidate = candidate(&machine, 46);
    let guard = RejectAllKagemushaGuardBundleVerifierV1;
    assert!(
        guard
            .verify_recovery_checkpoint_cas(candidate.statement(), &[1])
            .is_err()
    );
    assert!(
        guard
            .verify_current_recovery_checkpoint(
                &candidate.statement.successor,
                &candidate.snapshot.recovery_metadata.journals
            )
            .is_err()
    );
}

#[test]
fn exclusive_publication_recovers_lost_acknowledgement_only_from_selected_candidate() {
    let (machine, hardware) = fixture();
    let candidate = candidate(&machine, 47);
    let persisted = candidate.snapshot().clone();
    let release = machine.proof_release.clone();
    let enrollment = machine.enrollment_binding().clone();
    let history = machine.authenticated_history.clone().into_store();
    let certificate = hardware.commit(&candidate).unwrap();
    let anchor = DurabilityAnchorV1 {
        statement: candidate.statement.successor.clone(),
        guard_bundle: certificate.clone(),
    };
    let pending = machine
        .stage_recovery_checkpoint(candidate.clone())
        .unwrap();
    assert_eq!(pending.snapshot(), &persisted);
    assert_eq!(pending.statement(), candidate.statement());
    hardware.material_available.set(false);
    assert!(matches!(
        pending.finish(certificate.clone()),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
    // The old machine was consumed; only fresh authenticated recovery can return another owner.
    hardware.material_available.set(true);
    let restored = Machine::restore(
        persisted,
        &anchor,
        release.clone(),
        release,
        &enrollment,
        history,
        AcceptSnapshotRecursiveVerifierV1,
        hardware.clone(),
    )
    .unwrap();
    let retried = restored
        .stage_recovery_checkpoint(candidate)
        .unwrap()
        .finish(certificate)
        .unwrap();
    assert_eq!(retried.recovery_checkpoint(), &anchor);
}

const BOOTSTRAP_CAPACITY: u64 = 8 * 1024 * 1024;
type BootstrapStage = KagemushaBootstrapJournalStageV1<
    AcceptSnapshotRecursiveVerifierV1,
    SimulatedHardware,
    KagemushaMemoryAuthenticatedHistoryStoreV1,
>;

// This constructs only an explicit structural journal fixture. The public bootstrap constructor
// separately authenticates enrollment, Bootstrap proof and Guard before creating this owner.
fn bootstrap_stage(hardware: SimulatedHardware, change: usize) -> BootstrapStage {
    let original = coordinator_operation_store_tests::machine().0;
    let mut enrollment = original.enrollment_binding().clone();
    let mut credential = original.accepted_credential_floor().credential;
    if change == 1 {
        enrollment.owner.account_id = AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![212; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        enrollment.enrollment_id = enrollment.owner.enrollment_id().unwrap();
    } else if change == 2 {
        credential.issued_at_ms += 1;
        credential = resign(credential);
    } else if change == 3 {
        enrollment.owner.runtime.fi_id = "different-fi".parse().unwrap();
        enrollment.enrollment_id = enrollment.owner.enrollment_id().unwrap();
    }
    let state = KagemushaStateV1::build(
        original.state.context(),
        original.state.liability_pool_id,
        original.state.lane.clone(),
        0,
        0,
        original.state.hardware_epoch,
        original.state.device_policy_binding,
        original.state.state_nonce_commitment,
        ExactConsumedCreditIndex::empty().root(),
    )
    .unwrap();
    KagemushaBootstrapJournalStageV1::new(
        state,
        original.proof_release,
        credential,
        enrollment,
        KagemushaDurableCapacityV1 {
            inbox_bytes: 32 * 1024 * 1024,
            outbox_bytes: BOOTSTRAP_CAPACITY,
        },
        original.authenticated_history,
        AcceptSnapshotRecursiveVerifierV1,
        hardware,
    )
    .unwrap()
}

fn bootstrap_location() -> (tempfile::TempDir, std::path::PathBuf, SimulatedHardware) {
    let temp = tempfile::tempdir().unwrap();
    let bundle = temp.path().canonicalize().unwrap().join("wallet");
    let (_, hardware) = fixture();
    *hardware.register.borrow_mut() = Register::default();
    (temp, bundle, hardware)
}

#[test]
fn bootstrap_partial_private_staging_never_occupies_final_path() {
    let (_temp, bundle, hardware) = bootstrap_location();
    let mut stage = bootstrap_stage(hardware.clone(), 0);
    stage.initialization_failure =
        Some(super::super::bootstrap_checkpoint::BootstrapJournalFailure::AfterFirstJournal);
    assert!(
        stage
            .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [51; 32])
            .is_err()
    );
    assert!(!bundle.exists());
    assert!(
        bootstrap_stage(hardware.clone(), 0)
            .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [51; 32])
            .is_err()
    );
    let pending = bootstrap_stage(hardware.clone(), 0)
        .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [51; 32])
        .unwrap();
    let certificate = hardware.commit(&pending.candidate).unwrap();
    let (machine, coordinator, responses) = pending.finish(certificate).unwrap().into_parts();
    assert_eq!(
        machine.recovery_metadata().journals.coordinator,
        coordinator.recovery_prefix().unwrap()
    );
    assert_eq!(
        machine.recovery_metadata().journals.responses,
        responses.recovery_prefix().unwrap()
    );
}

#[test]
fn bootstrap_rename_interruption_requires_explicit_exact_resume() {
    let (_temp, bundle, hardware) = bootstrap_location();
    let mut stage = bootstrap_stage(hardware.clone(), 0);
    stage.initialization_failure =
        Some(super::super::bootstrap_checkpoint::BootstrapJournalFailure::AfterRename);
    assert!(
        stage
            .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [52; 32])
            .is_err()
    );
    assert!(bundle.join("bootstrap/bootstrap.norito.wal").is_file());
    assert!(
        bootstrap_stage(hardware.clone(), 0)
            .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [52; 32])
            .is_err()
    );
    let pending = bootstrap_stage(hardware.clone(), 0)
        .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [52; 32])
        .unwrap();
    let certificate = hardware.commit(&pending.candidate).unwrap();
    pending.finish(certificate).unwrap();
}

#[test]
fn bootstrap_resume_rejects_changed_owner_credential_operation_and_capacity() {
    let (_temp, bundle, hardware) = bootstrap_location();
    let pending = bootstrap_stage(hardware.clone(), 0)
        .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [53; 32])
        .unwrap();
    let original = pending.snapshot().clone();
    drop(pending);
    for change in 1..=3 {
        assert!(matches!(
            bootstrap_stage(hardware.clone(), change).resume_initialized_journals(
                &bundle,
                BOOTSTRAP_CAPACITY,
                [53; 32]
            ),
            Err(KagemushaStateErrorV1::SnapshotRollback)
        ));
    }
    assert!(
        bootstrap_stage(hardware.clone(), 0)
            .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [54; 32])
            .is_err()
    );
    assert!(
        bootstrap_stage(hardware.clone(), 0)
            .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY + 1, [53; 32])
            .is_err()
    );
    let resumed = bootstrap_stage(hardware, 0)
        .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [53; 32])
        .unwrap();
    assert_eq!(resumed.snapshot(), &original);
}

#[test]
fn bootstrap_pending_holds_both_journals_and_manifest_until_publication() {
    let (_temp, bundle, hardware) = bootstrap_location();
    let pending = bootstrap_stage(hardware.clone(), 0)
        .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [55; 32])
        .unwrap();
    assert!(
        bootstrap_stage(hardware.clone(), 0)
            .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [55; 32])
            .is_err()
    );
    let certificate = hardware.commit(&pending.candidate).unwrap();
    let (machine, coordinator, responses) = pending.finish(certificate).unwrap().into_parts();
    assert!(
        bootstrap_stage(hardware, 0)
            .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [55; 32])
            .is_err()
    );
    assert_eq!(machine.recovery_metadata().revision, 1);
    assert_eq!(coordinator.recovery_prefix().unwrap().sequence, 1);
    assert_eq!(responses.recovery_prefix().unwrap().sequence, 1);
}

#[test]
fn bootstrap_resume_never_recreates_missing_manifest_or_response_journal() {
    for child in [
        "bootstrap/bootstrap.norito.wal",
        "responses/responses.norito.wal",
    ] {
        let (_temp, bundle, hardware) = bootstrap_location();
        drop(
            bootstrap_stage(hardware.clone(), 0)
                .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [56; 32])
                .unwrap(),
        );
        std::fs::remove_file(bundle.join(child)).unwrap();
        assert!(
            bootstrap_stage(hardware, 0)
                .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [56; 32])
                .is_err()
        );
        assert!(!bundle.join(child).exists());
    }
}

#[test]
fn bootstrap_manifest_tamper_before_first_cas_consumes_pending_without_authority() {
    use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
    let (_temp, bundle, hardware) = bootstrap_location();
    let pending = bootstrap_stage(hardware.clone(), 0)
        .initialize_journals(&bundle, BOOTSTRAP_CAPACITY, [57; 32])
        .unwrap();
    let certificate = hardware.commit(&pending.candidate).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(bundle.join("bootstrap/bootstrap.norito.wal"))
        .unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    let mut original = [0];
    file.read_exact(&mut original).unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    file.write_all(&[original[0] ^ 1]).unwrap();
    file.sync_all().unwrap();
    assert!(pending.finish(certificate).is_err());
    assert!(
        bootstrap_stage(hardware, 0)
            .resume_initialized_journals(&bundle, BOOTSTRAP_CAPACITY, [57; 32])
            .is_err()
    );
}

#[test]
fn restore_rejects_different_retail_account_fi_and_authentication_namespace() {
    let (machine, _) = fixture();
    for field in 0..3 {
        let mut expected = machine.enrollment_binding().clone();
        match field {
            0 => {
                expected.owner.account_id = AccountId::new(
                    iroha_crypto::KeyPair::from_seed(
                        vec![212; 32],
                        iroha_crypto::Algorithm::Ed25519,
                    )
                    .public_key()
                    .clone(),
                )
            }
            1 => expected.owner.runtime.fi_id = "other-fi".parse().unwrap(),
            2 => expected.owner.runtime.authentication_namespace = "other-auth".parse().unwrap(),
            _ => unreachable!(),
        }
        expected.enrollment_id = expected.owner.enrollment_id().unwrap();
        assert!(matches!(
            Machine::restore(
                machine.snapshot().unwrap(),
                machine.recovery_checkpoint(),
                machine.proof_release.clone(),
                machine.proof_release.clone(),
                &expected,
                machine.authenticated_history.clone().into_store(),
                AcceptSnapshotRecursiveVerifierV1,
                machine.guard_verifier.clone(),
            ),
            Err(KagemushaStateErrorV1::SnapshotRollback)
        ));
    }
}

#[test]
fn exact_terminal_retry_cannot_publish_a_subsequently_changed_machine() {
    let (machine, hardware) = fixture();
    let candidate = candidate(&machine, 58);
    let certificate = hardware.commit(&candidate).unwrap();
    let mut machine = machine
        .stage_recovery_checkpoint(candidate.clone())
        .unwrap()
        .finish(certificate.clone())
        .unwrap();
    machine.inbox_revision += 1;
    assert!(matches!(
        machine.install_recovery_checkpoint(&candidate, certificate),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    assert!(matches!(
        machine.stage_recovery_checkpoint(candidate),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
}

#[test]
fn sealed_recovery_selection_requires_complete_current_checkpoint_equality() {
    fn select<T: KagemushaCurrentRecoveryOwnerV1>(
        owner: &T,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1> {
        owner.current_recovery_selection()
    }
    let (mut machine, _) = fixture();
    let selected = select(&machine).unwrap();
    assert_eq!(selected.enrollment_binding(), machine.enrollment_binding());
    assert_eq!(
        selected.accepted_credential_floor(),
        machine.accepted_credential_floor()
    );
    assert_eq!(selected.hardware_epoch(), machine.state.hardware_epoch);
    assert_eq!(
        selected.device_policy_binding(),
        machine.state.device_policy_binding
    );
    assert_eq!(selected.checkpoint(), machine.recovery_checkpoint());
    machine.inbox_revision += 1;
    assert!(matches!(
        select(&machine),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
}

#[test]
fn sealed_recovery_selection_rejects_unchanged_local_state_after_hardware_advances() {
    let (machine, hardware) = fixture();
    let local = machine.snapshot().unwrap();
    let challenge_before = hardware.challenge_counter.get();
    assert!(machine.current_recovery_selection().is_ok());
    assert_eq!(hardware.challenge_counter.get(), challenge_before + 1);
    let successor = candidate(&machine, 59);
    hardware.commit(&successor).unwrap();
    assert_eq!(machine.snapshot().unwrap(), local);
    assert!(matches!(
        machine.current_recovery_selection(),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
}

#[test]
fn sealed_recovery_selection_rejects_missing_or_changed_actual_journal_material() {
    let (machine, hardware) = fixture();
    let local = machine.snapshot().unwrap();
    hardware.material_available.set(false);
    assert!(matches!(
        machine.current_recovery_selection(),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
    hardware.material_available.set(true);
    assert!(machine.current_recovery_selection().is_ok());
    hardware
        .register
        .borrow_mut()
        .terminals
        .values_mut()
        .next()
        .unwrap()
        .2
        .responses
        .head = [213; 32];
    assert_eq!(machine.snapshot().unwrap(), local);
    assert!(matches!(
        machine.current_recovery_selection(),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
}

fn speculative_history(machine: &Machine) -> KagemushaPreparedHistoryCasV1 {
    let roots = machine.authenticated_history.store.committed_roots();
    let replay =
        KagemushaHistoryNodeRecordV1::leaf(KagemushaHistoryTreeV1::Replay, [181; 32], [182; 32])
            .unwrap();
    KagemushaPreparedHistoryCasV1::new(
        KagemushaHistoryRootSelectionV1::replay(roots.replay(), replay.content_address().unwrap()),
        vec![replay],
        [183; 32],
    )
    .unwrap()
}

#[test]
fn sealed_recovery_selection_retains_validated_prepare_and_abort_suffixes() {
    for abort in [false, true] {
        let (mut machine, hardware) = fixture();
        let original = machine.snapshot().unwrap();
        let anchor = machine.recovery_checkpoint().clone();
        let transaction = speculative_history(&machine);
        machine
            .authenticated_history
            .store
            .prepare_cas(transaction.clone())
            .unwrap();
        if abort {
            machine
                .authenticated_history
                .store
                .abort_prepared(transaction.transaction_id())
                .unwrap();
        }
        let extended = machine.snapshot().unwrap();
        assert_ne!(
            extended.authenticated_history_commitment,
            original.authenticated_history_commitment
        );
        assert_ne!(extended.recovery_anchor(), anchor.statement);
        assert_eq!(
            machine.current_recovery_selection().unwrap().checkpoint(),
            &anchor
        );
        let mut recovered = restore(&machine, original, &anchor).unwrap();
        assert_eq!(
            recovered.current_recovery_selection().unwrap().checkpoint(),
            &anchor
        );
        assert_eq!(recovered.snapshot().unwrap(), extended);
        // Selecting the old checkpoint never consumes or discards the exact local attempt.
        assert_eq!(
            recovered
                .authenticated_history
                .store
                .prepare_cas(transaction)
                .unwrap(),
            if abort {
                KagemushaHistoryPrepareOutcomeV1::AlreadyAborted
            } else {
                KagemushaHistoryPrepareOutcomeV1::AlreadyPrepared
            }
        );
        let next = candidate(&recovered, 184);
        assert_eq!(
            next.snapshot().authenticated_history_commitment,
            extended.authenticated_history_commitment
        );
        let certificate = hardware.commit(&next).unwrap();
        recovered
            .install_recovery_checkpoint(&next, certificate)
            .unwrap();
        assert_eq!(
            recovered.current_recovery_selection().unwrap().checkpoint(),
            recovered.recovery_checkpoint()
        );
        recovered.inbox_revision += 1;
        assert!(matches!(
            recovered.current_recovery_selection(),
            Err(KagemushaStateErrorV1::SnapshotRollback)
        ));
    }
}

#[test]
fn sealed_recovery_selection_rejects_history_commit_after_selected_checkpoint() {
    let (mut machine, _) = fixture();
    let transaction = speculative_history(&machine);
    machine
        .authenticated_history
        .store
        .prepare_cas(transaction.clone())
        .unwrap();
    machine.current_recovery_selection().unwrap();
    // This is an explicitly simulated history device with a real signed selection certificate.
    let key = SigningKey::from_bytes((&[185; 32]).into()).unwrap();
    let subject = KagemushaHistoryRootSelectionSubjectV1::new(&transaction, [186; 32], 1, 1);
    let certificate = KagemushaHistoryRootSelectionCertificateV1::new(
        subject.clone(),
        snapshot_device_signature(&key, &subject.signing_bytes().unwrap()),
    )
    .verify([186; 32], &snapshot_device_public_key(&key))
    .unwrap();
    machine
        .authenticated_history
        .store
        .commit_prepared(certificate)
        .unwrap();
    assert!(matches!(
        machine.current_recovery_selection(),
        Err(KagemushaStateErrorV1::AuthenticatedHistoryUnavailable)
    ));
}

#[test]
fn sealed_recovery_selection_rejects_rollback_before_selected_history_prefix() {
    let (mut machine, hardware) = fixture();
    let older_store = machine.authenticated_history.store.clone();
    let transaction = speculative_history(&machine);
    machine
        .authenticated_history
        .store
        .prepare_cas(transaction)
        .unwrap();
    let next = candidate(&machine, 187);
    let certificate = hardware.commit(&next).unwrap();
    machine
        .install_recovery_checkpoint(&next, certificate)
        .unwrap();
    machine.current_recovery_selection().unwrap();
    machine.authenticated_history.store = older_store;
    assert!(matches!(
        machine.current_recovery_selection(),
        Err(KagemushaStateErrorV1::AuthenticatedHistoryUnavailable)
    ));
}
