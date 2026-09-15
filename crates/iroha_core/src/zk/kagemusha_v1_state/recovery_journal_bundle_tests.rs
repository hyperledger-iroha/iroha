//! Real-file paired recovery checks using explicit structural Core verifier fixtures.
//! These tests do not qualify hardware, authenticate response signatures, or execute proofs.

use super::*;
use crate::zk::kagemusha_v1_state::KagemushaResponseEvidenceContextV1;
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use sha2::Sha256;
use std::{cell::Cell, cell::RefCell, rc::Rc};

const RESPONSE_FILE: &str = "responses.norito.wal";
type Pending = KagemushaPendingRecoveryJournalsV1;
type Archive = KagemushaResponseEvidenceArchiveV1;
type SelectedFixture = (
    tempfile::TempDir,
    PathBuf,
    PathBuf,
    Machine,
    KagemushaCoordinatorOperationStoreV1,
    Archive,
);

fn response_context(machine: &Machine) -> KagemushaResponseEvidenceContextV1 {
    KagemushaResponseEvidenceContextV1 {
        canonical_credential: norito::encode_canonical(
            &machine.accepted_credential_floor().credential,
        )
        .unwrap(),
        release_id: machine.state.release_id,
        hardware_policy_id: id(81),
        qualification_report_digest: id(82),
    }
}

fn response_frames(tag: u8) -> (Vec<u8>, Vec<u8>) {
    // Deliberately unqualified authenticator: the archive validates framing only.
    let body = [tag; 32];
    let signature = [tag; 64];
    let mut command = b"IKGMJCM1".to_vec();
    command.extend(1_u16.to_le_bytes());
    command.extend([2, 0]);
    command.extend(id(tag));
    command.extend(32_u32.to_le_bytes());
    command.extend(Sha256::digest(body));
    command.extend(body);
    let mut response = b"IKGMJRS1".to_vec();
    response.extend(1_u16.to_le_bytes());
    response.extend([2, 0]);
    response.extend(id(tag));
    response.extend(32_u32.to_le_bytes());
    response.extend(64_u32.to_le_bytes());
    response.extend(Sha256::digest(body));
    response.extend(Sha256::digest(signature));
    response.extend(body);
    response.extend(signature);
    (command, response)
}

fn append_response(machine: &Machine, archive: &mut Archive, tag: u8) {
    let (command, response) = response_frames(tag);
    assert!(
        archive
            .retain_observed_response(2, id(tag), &command, &response, &response_context(machine))
            .unwrap()
    );
}

fn select_journals(machine: Machine, journals: KagemushaRecoveryJournalsV1, tag: u8) -> Machine {
    let candidate = machine
        .prepare_recovery_checkpoint(id(tag), journals)
        .unwrap();
    machine
        .stage_recovery_checkpoint(candidate)
        .unwrap()
        .finish(vec![207])
        .unwrap()
}

fn restore_selected(machine: &Machine) -> Machine {
    Machine::restore(
        machine.snapshot().unwrap(),
        machine.recovery_checkpoint(),
        machine.proof_release.clone(),
        machine.proof_release.clone(),
        machine.enrollment_binding(),
        machine.authenticated_history.clone().into_store(),
        AcceptSnapshotRecursiveVerifierV1,
        AcceptSnapshotGuardVerifierV1,
    )
    .unwrap()
}

fn advanced(tag: u8) -> SelectedFixture {
    let (mut machine, _, _) = machine();
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().canonicalize().unwrap();
    let coordinator_path = root_path.join("coordinator");
    let response_path = root_path.join("responses");
    let mut coordinator = machine
        .create_coordinator_operation_store(&coordinator_path, CAPACITY)
        .unwrap();
    let mut responses = Archive::create_new(
        &response_path,
        &machine.state.lane,
        machine.state.asset_incarnation,
    )
    .unwrap();
    // Extend the existing structural fixture with an actual response initializer;
    // no synthetic response prefix is carried into this real-file recovery suite.
    machine.recovery_metadata.journals.responses = responses.recovery_prefix().unwrap();
    machine.published_checkpoint = None;
    let machine = snapshot_initial_publish(machine);
    machine
        .reserve_coordinator_operation(&mut coordinator, id(tag), 4, b"exact retained command")
        .unwrap();
    append_response(&machine, &mut responses, tag);
    let mut journals = machine.recovery_metadata.journals;
    journals.coordinator = coordinator.recovery_prefix().unwrap();
    journals.responses = responses.recovery_prefix().unwrap();
    let machine = select_journals(machine, journals, 110);
    assert_eq!(journals.coordinator.sequence, 2);
    assert_eq!(journals.responses.sequence, 2);
    (
        root,
        coordinator_path,
        response_path,
        machine,
        coordinator,
        responses,
    )
}

fn open(machine: &Machine, coordinator: &Path, responses: &Path) -> Pending {
    Pending::open_existing(
        coordinator,
        responses,
        &machine.state.lane,
        machine.state.asset_incarnation,
        0,
    )
    .unwrap()
}

fn original_prefix(bytes: &[u8]) -> &[u8] {
    let payload = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    &bytes[..88 + payload]
}

#[test]
fn advanced_pair_reopens_with_exact_prefixes_and_retries_under_lower_capacity() {
    let (_root, coordinator_path, response_path, original, coordinator, responses) = advanced(21);
    let selected = original.recovery_metadata.journals;
    let coordinator_bytes = fs::read(coordinator_path.join(FILE)).unwrap();
    let response_bytes = fs::read(response_path.join(RESPONSE_FILE)).unwrap();
    let charge = coordinator.live_reserved_bytes();
    let restored = restore_selected(&original);
    let anchor = restored.recovery_checkpoint().clone();
    drop((original, coordinator, responses));
    let pending = open(&restored, &coordinator_path, &response_path);
    let (machine, mut coordinator, mut responses) = pending.bind(restored).unwrap();
    assert_eq!(machine.recovery_checkpoint(), &anchor);
    assert_eq!(coordinator.recovery_prefix().unwrap(), selected.coordinator);
    assert_eq!(responses.recovery_prefix().unwrap(), selected.responses);
    assert_eq!(coordinator.live_reserved_bytes(), charge);
    assert_eq!(
        machine.reserve_coordinator_operation(
            &mut coordinator,
            id(21),
            4,
            b"exact retained command"
        ),
        Ok(id(21))
    );
    let (command, response) = response_frames(21);
    assert!(
        !responses
            .retain_observed_response(2, id(21), &command, &response, &response_context(&machine),)
            .unwrap()
    );
    assert_eq!(
        fs::read(coordinator_path.join(FILE)).unwrap(),
        coordinator_bytes
    );
    assert_eq!(
        fs::read(response_path.join(RESPONSE_FILE)).unwrap(),
        response_bytes
    );
}

#[test]
fn advanced_pair_rejects_each_mixed_same_lane_selected_history_without_writes() {
    for mix_responses in [false, true] {
        let (_a, coordinator_a, response_a, original, store_a, archive_a) = advanced(22);
        let (_b, coordinator_b, response_b, foreign, store_b, archive_b) = advanced(23);
        assert_eq!(original.state.lane, foreign.state.lane);
        assert_eq!(
            original.state.asset_incarnation,
            foreign.state.asset_incarnation
        );
        let restored = restore_selected(&original);
        let coordinator_path = if mix_responses {
            &coordinator_a
        } else {
            &coordinator_b
        };
        let response_path = if mix_responses {
            &response_b
        } else {
            &response_a
        };
        let before_coordinator = fs::read(coordinator_path.join(FILE)).unwrap();
        let before_responses = fs::read(response_path.join(RESPONSE_FILE)).unwrap();
        drop((original, foreign, store_a, archive_a, store_b, archive_b));
        let pending = open(&restored, coordinator_path, response_path);
        assert!(matches!(
            pending.bind(restored),
            Err(KagemushaStateErrorV1::RecoveryMaterial(_))
        ));
        assert_eq!(
            fs::read(coordinator_path.join(FILE)).unwrap(),
            before_coordinator
        );
        assert_eq!(
            fs::read(response_path.join(RESPONSE_FILE)).unwrap(),
            before_responses
        );
    }
}

#[test]
fn advanced_pair_rejects_response_rollback_to_valid_initializer() {
    let (_root, coordinator_path, response_path, original, coordinator, responses) = advanced(24);
    let restored = restore_selected(&original);
    let response_file = response_path.join(RESPONSE_FILE);
    let response_bytes = fs::read(&response_file).unwrap();
    let initial = original_prefix(&response_bytes).to_vec();
    let coordinator_bytes = fs::read(coordinator_path.join(FILE)).unwrap();
    drop((original, coordinator, responses));
    fs::write(&response_file, &initial).unwrap();
    let pending = open(&restored, &coordinator_path, &response_path);
    assert!(matches!(
        pending.bind(restored),
        Err(KagemushaStateErrorV1::RecoveryMaterial(_))
    ));
    assert_eq!(fs::read(&response_file).unwrap(), initial);
    assert_eq!(
        fs::read(coordinator_path.join(FILE)).unwrap(),
        coordinator_bytes
    );
}

#[test]
fn advanced_pair_preserves_valid_appended_suffixes_without_selecting_them() {
    let (_root, coordinator_path, response_path, original, mut coordinator, mut responses) =
        advanced(25);
    let selected = original.recovery_metadata.journals;
    let restored = restore_selected(&original);
    original
        .reserve_coordinator_operation(&mut coordinator, id(26), 4, b"unselected retry")
        .unwrap();
    append_response(&original, &mut responses, 26);
    let complete_coordinator = coordinator.recovery_prefix().unwrap();
    let complete_responses = responses.recovery_prefix().unwrap();
    assert!(complete_coordinator.sequence > selected.coordinator.sequence);
    assert!(complete_responses.sequence > selected.responses.sequence);
    drop((original, coordinator, responses));
    let pending = open(&restored, &coordinator_path, &response_path);
    let (machine, coordinator, responses) = pending.bind(restored).unwrap();
    assert_eq!(machine.recovery_metadata.journals, selected);
    assert_eq!(coordinator.recovery_prefix().unwrap(), complete_coordinator);
    assert_eq!(responses.recovery_prefix().unwrap(), complete_responses);
}

#[test]
fn advanced_pair_reconciles_complete_prepared_core_operation_index_after_restore() {
    let (_root, coordinator_path, response_path, mut machine, mut coordinator, responses) =
        advanced(27);
    let account = super::machine().2;
    let intent = intent(
        &machine,
        machine.accepted_credential_floor().credential.credential_id,
        account,
        id(28),
    );
    machine
        .reserve_coordinator_operation(&mut coordinator, intent.operation_id, 5, &binding(&intent))
        .unwrap();
    machine
        .begin_coordinator_sender_intent(&mut coordinator, &intent)
        .unwrap();
    prepare(&mut machine, &intent);
    let mut journals = machine.recovery_metadata.journals;
    journals.coordinator = coordinator.recovery_prefix().unwrap();
    let original = select_journals(machine, journals, 111);
    let restored = restore_selected(&original);
    let expected = restored
        .outgoing_operation_index()
        .lookup(intent.operation_id)
        .unwrap()
        .clone();
    drop((original, coordinator, responses));
    let pending = open(&restored, &coordinator_path, &response_path);
    let (machine, coordinator, _) = pending.bind(restored).unwrap();
    assert_eq!(
        machine.recover_coordinator_sender_intent(&coordinator, intent.operation_id),
        Ok(Recovery::Indexed(expected))
    );
}

#[test]
fn advanced_pair_rejects_missing_index_operation_even_when_selected_prefix_matches() {
    let (_root, coordinator_path, response_path, mut machine, mut coordinator, responses) =
        advanced(29);
    let before_intent = fs::read(coordinator_path.join(FILE)).unwrap();
    let selected = machine.recovery_metadata.journals;
    let account = super::machine().2;
    let intent = intent(
        &machine,
        machine.accepted_credential_floor().credential.credential_id,
        account,
        id(30),
    );
    machine
        .reserve_coordinator_operation(&mut coordinator, intent.operation_id, 5, &binding(&intent))
        .unwrap();
    machine
        .begin_coordinator_sender_intent(&mut coordinator, &intent)
        .unwrap();
    prepare(&mut machine, &intent);
    // Structural guard fixture deliberately selects an insufficient journal prefix.
    // Pair validation must still cover the entire restored Core operation index.
    let original = select_journals(machine, selected, 112);
    let restored = restore_selected(&original);
    drop((original, coordinator, responses));
    fs::write(coordinator_path.join(FILE), &before_intent).unwrap();
    let pending = open(&restored, &coordinator_path, &response_path);
    assert!(matches!(
        pending.bind(restored),
        Err(KagemushaStateErrorV1::RecoveryMaterial(_))
    ));
    assert_eq!(
        fs::read(coordinator_path.join(FILE)).unwrap(),
        before_intent
    );
}

#[test]
fn advanced_pair_failure_never_appends_pending_sender_retirement() {
    let (_root, coordinator_path, response_path, mut machine, mut coordinator, responses) =
        advanced(31);
    let account = super::machine().2;
    let intent = intent(
        &machine,
        machine.accepted_credential_floor().credential.credential_id,
        account,
        id(32),
    );
    machine
        .reserve_coordinator_operation(&mut coordinator, intent.operation_id, 5, &binding(&intent))
        .unwrap();
    machine
        .begin_coordinator_sender_intent(&mut coordinator, &intent)
        .unwrap();
    prepare(&mut machine, &intent);
    // Existing test-only index fixture isolates retirement accounting. It is not a
    // real monetary proof or a restorable Released snapshot.
    release_index_fixture(&mut machine, intent.operation_id);
    let before = fs::read(coordinator_path.join(FILE)).unwrap();
    let response_bytes = fs::read(response_path.join(RESPONSE_FILE)).unwrap();
    drop((coordinator, responses));
    fs::write(
        response_path.join(RESPONSE_FILE),
        original_prefix(&response_bytes),
    )
    .unwrap();
    let pending = open(&machine, &coordinator_path, &response_path);
    assert!(matches!(
        pending.bind(machine),
        Err(KagemushaStateErrorV1::RecoveryMaterial(_))
    ));
    assert_eq!(fs::read(coordinator_path.join(FILE)).unwrap(), before);
}

#[test]
fn advanced_pair_open_rejects_torn_tails_without_truncating_or_holding_other_lock() {
    for tear_responses in [false, true] {
        let (_root, coordinator_path, response_path, machine, coordinator, responses) =
            advanced(33);
        let coordinator_bytes = fs::read(coordinator_path.join(FILE)).unwrap();
        let response_bytes = fs::read(response_path.join(RESPONSE_FILE)).unwrap();
        drop((coordinator, responses));
        let file = if tear_responses {
            response_path.join(RESPONSE_FILE)
        } else {
            coordinator_path.join(FILE)
        };
        let mut torn = fs::read(&file).unwrap();
        torn.extend([0x44; 17]);
        fs::write(&file, &torn).unwrap();
        assert!(
            Pending::open_existing(
                &coordinator_path,
                &response_path,
                &machine.state.lane,
                machine.state.asset_incarnation,
                CAPACITY
            )
            .is_err()
        );
        assert_eq!(fs::read(&file).unwrap(), torn);
        fs::write(coordinator_path.join(FILE), &coordinator_bytes).unwrap();
        fs::write(response_path.join(RESPONSE_FILE), &response_bytes).unwrap();
        drop(open(&machine, &coordinator_path, &response_path));
    }
}

#[test]
fn advanced_pair_open_rejects_wrong_lane_and_incarnation_before_exposure() {
    let (_root, coordinator_path, response_path, machine, coordinator, responses) = advanced(34);
    drop((coordinator, responses));
    let mut other_lane = machine.state.lane.clone();
    other_lane.device_lane_id[0] ^= 1;
    let other_incarnation = AxtAssetIncarnationV1::try_from_bytes([35; 32]).unwrap();
    for (lane, incarnation) in [
        (&other_lane, machine.state.asset_incarnation),
        (&machine.state.lane, other_incarnation),
    ] {
        assert!(
            Pending::open_existing(
                &coordinator_path,
                &response_path,
                lane,
                incarnation,
                CAPACITY
            )
            .is_err()
        );
    }
    drop(open(&machine, &coordinator_path, &response_path));
}

#[test]
fn response_pair_binding_checks_its_own_initializer_scope() {
    let (_root, _coordinator_path, _response_path, machine, _coordinator, responses) = advanced(39);
    let selected = responses.recovery_prefix().unwrap();
    let mut other_lane = machine.state.lane.clone();
    other_lane.device_lane_id[0] ^= 1;
    // Keep the alternate incarnation canonical so the archive binding check is reached.
    let other_incarnation = AxtAssetIncarnationV1::try_from_bytes([41; 32]).unwrap();
    assert_ne!(other_incarnation, machine.state.asset_incarnation);
    for (lane, incarnation) in [
        (&other_lane, machine.state.asset_incarnation),
        (&machine.state.lane, other_incarnation),
    ] {
        assert_eq!(
            responses.validate_recovery_prefix(lane, incarnation, selected),
            Err(KagemushaResponseEvidenceArchiveErrorV1::InvalidBinding)
        );
    }
    assert_eq!(
        responses.validate_recovery_prefix(
            &machine.state.lane,
            machine.state.asset_incarnation,
            selected
        ),
        Ok(())
    );
}

#[test]
fn advanced_pair_accepts_byte_equivalent_histories_from_distinct_private_paths() {
    let (_a, coordinator_a, response_a, original, store_a, archive_a) = advanced(41);
    let (_b, coordinator_b, response_b, equivalent, store_b, archive_b) = advanced(41);
    assert_eq!(
        fs::read(coordinator_a.join(FILE)).unwrap(),
        fs::read(coordinator_b.join(FILE)).unwrap()
    );
    assert_eq!(
        fs::read(response_a.join(RESPONSE_FILE)).unwrap(),
        fs::read(response_b.join(RESPONSE_FILE)).unwrap()
    );
    let restored = restore_selected(&original);
    let selected = restored.recovery_metadata.journals;
    drop((original, equivalent, store_a, archive_a, store_b, archive_b));
    let pending = open(&restored, &coordinator_b, &response_a);
    let (machine, coordinator, responses) = pending.bind(restored).unwrap();
    assert_eq!(machine.recovery_metadata.journals, selected);
    assert_eq!(coordinator.recovery_prefix().unwrap(), selected.coordinator);
    assert_eq!(responses.recovery_prefix().unwrap(), selected.responses);
}

#[test]
fn advanced_pair_rejects_each_journal_replacement_between_open_and_bind() {
    for replace_response in [false, true] {
        let (_root, coordinator_path, response_path, original, coordinator, responses) =
            advanced(42);
        let restored = restore_selected(&original);
        let lane = restored.state.lane.clone();
        let incarnation = restored.state.asset_incarnation;
        drop((original, coordinator, responses));
        let pending = open(&restored, &coordinator_path, &response_path);
        let path = if replace_response {
            response_path.join(RESPONSE_FILE)
        } else {
            coordinator_path.join(FILE)
        };
        let bytes = fs::read(&path).unwrap();
        let held = path.with_extension("held-original");
        fs::rename(&path, &held).unwrap();
        fs::write(&path, &bytes).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            pending.bind(restored),
            Err(KagemushaStateErrorV1::RecoveryMaterial(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(&path).unwrap();
        fs::rename(&held, &path).unwrap();
        drop(
            Pending::open_existing(
                &coordinator_path,
                &response_path,
                &lane,
                incarnation,
                CAPACITY,
            )
            .unwrap(),
        );
    }
}

#[derive(Clone)]
struct CurrentProbe {
    calls: Rc<Cell<usize>>,
    action: Rc<RefCell<Option<ProbeAction>>>,
}

enum ProbeAction {
    Reject,
    Mutate(PathBuf),
}

impl KagemushaGuardBundleVerifierV1 for CurrentProbe {
    fn verify_bootstrap(
        &self,
        statement: &BootstrapStatementV1,
        normalized: &KagemushaNormalizedGuardStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        AcceptSnapshotGuardVerifierV1.verify_bootstrap(statement, normalized, bytes)
    }
    fn verify_transition(
        &self,
        statement: &HardwareTransitionStatementV1,
        proof: &TransitionProofStatementV1,
        normalized: &KagemushaNormalizedGuardStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        AcceptSnapshotGuardVerifierV1.verify_transition(statement, proof, normalized, bytes)
    }
    fn verify_credit_stage(
        &self,
        statement: &CreditStageStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        AcceptSnapshotGuardVerifierV1.verify_credit_stage(statement, bytes)
    }
    fn verify_durability_anchor(
        &self,
        statement: &DurabilityAnchorStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        AcceptSnapshotGuardVerifierV1.verify_durability_anchor(statement, bytes)
    }
    fn verify_recovery_checkpoint_cas(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        AcceptSnapshotGuardVerifierV1.verify_recovery_checkpoint_cas(statement, bytes)
    }
    fn verify_current_recovery_checkpoint(
        &self,
        _: &DurabilityAnchorStatementV1,
        _: &KagemushaRecoveryJournalsV1,
    ) -> Result<(), String> {
        self.calls.set(self.calls.get() + 1);
        match self.action.borrow_mut().take() {
            Some(ProbeAction::Reject) => return Err("fresh hardware selection refused".into()),
            Some(ProbeAction::Mutate(path)) => {
                let mut bytes = fs::read(&path).map_err(|error| error.to_string())?;
                *bytes.last_mut().unwrap() ^= 1;
                fs::write(path, bytes).map_err(|error| error.to_string())?;
            }
            None => {}
        }
        Ok(())
    }
}

fn with_probe(
    machine: Machine,
    probe: CurrentProbe,
) -> KagemushaStateMachineV1<AcceptSnapshotRecursiveVerifierV1, CurrentProbe> {
    KagemushaStateMachineV1 {
        recovery_metadata: machine.recovery_metadata,
        published_checkpoint: machine.published_checkpoint,
        state: machine.state,
        journal_revision: machine.journal_revision,
        inbox_revision: machine.inbox_revision,
        pending_credits: machine.pending_credits,
        accepted_recipient_bindings: machine.accepted_recipient_bindings,
        accepted_payment_receipts: machine.accepted_payment_receipts,
        mint_inbox: machine.mint_inbox,
        consumed_credits: machine.consumed_credits,
        authenticated_history: machine.authenticated_history,
        receiver_inbox_capacity: machine.receiver_inbox_capacity,
        sender_outbox_capacity: machine.sender_outbox_capacity,
        outgoing_candidate_journal: machine.outgoing_candidate_journal,
        proof_release: machine.proof_release,
        recursive_verifier: machine.recursive_verifier,
        guard_verifier: probe,
    }
}

#[test]
fn advanced_pair_rechecks_each_live_journal_after_fresh_selection() {
    for mutate_response in [false, true] {
        let (_root, coordinator_path, response_path, original, coordinator, responses) =
            advanced(36);
        let restored = restore_selected(&original);
        let lane = restored.state.lane.clone();
        let incarnation = restored.state.asset_incarnation;
        drop((original, coordinator, responses));
        let pending = open(&restored, &coordinator_path, &response_path);
        let file = if mutate_response {
            response_path.join(RESPONSE_FILE)
        } else {
            coordinator_path.join(FILE)
        };
        let bytes = fs::read(&file).unwrap();
        let calls = Rc::new(Cell::new(0));
        let machine = with_probe(
            restored,
            CurrentProbe {
                calls: calls.clone(),
                action: Rc::new(RefCell::new(Some(ProbeAction::Mutate(file.clone())))),
            },
        );
        assert!(matches!(
            pending.bind(machine),
            Err(KagemushaStateErrorV1::RecoveryMaterial(_))
        ));
        assert_eq!(
            calls.get(),
            1,
            "the mutation occurs only after the first pair validation"
        );
        fs::write(&file, bytes).unwrap();
        drop(
            Pending::open_existing(
                &coordinator_path,
                &response_path,
                &lane,
                incarnation,
                CAPACITY,
            )
            .unwrap(),
        );
    }
}

#[test]
fn advanced_pair_requires_fresh_hardware_selection_without_retirement_writes() {
    let (_root, coordinator_path, response_path, original, coordinator, responses) = advanced(37);
    let restored = restore_selected(&original);
    let lane = restored.state.lane.clone();
    let incarnation = restored.state.asset_incarnation;
    let before_coordinator = fs::read(coordinator_path.join(FILE)).unwrap();
    let before_responses = fs::read(response_path.join(RESPONSE_FILE)).unwrap();
    drop((original, coordinator, responses));
    let pending = open(&restored, &coordinator_path, &response_path);
    let calls = Rc::new(Cell::new(0));
    let machine = with_probe(
        restored,
        CurrentProbe {
            calls: calls.clone(),
            action: Rc::new(RefCell::new(Some(ProbeAction::Reject))),
        },
    );
    assert!(matches!(
        pending.bind(machine),
        Err(KagemushaStateErrorV1::GuardRejected(_))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(
        fs::read(coordinator_path.join(FILE)).unwrap(),
        before_coordinator
    );
    assert_eq!(
        fs::read(response_path.join(RESPONSE_FILE)).unwrap(),
        before_responses
    );
    drop(
        Pending::open_existing(
            &coordinator_path,
            &response_path,
            &lane,
            incarnation,
            CAPACITY,
        )
        .unwrap(),
    );
}

#[test]
fn advanced_pair_rejects_uncheckpointed_core_mutation_before_fresh_selection() {
    let (_root, coordinator_path, response_path, original, coordinator, responses) = advanced(38);
    let mut restored = restore_selected(&original);
    drop((original, coordinator, responses));
    let pending = open(&restored, &coordinator_path, &response_path);
    restored.journal_revision += 1;
    let calls = Rc::new(Cell::new(0));
    let machine = with_probe(
        restored,
        CurrentProbe {
            calls: calls.clone(),
            action: Rc::new(RefCell::new(None)),
        },
    );
    assert!(matches!(
        pending.bind(machine),
        Err(KagemushaStateErrorV1::SnapshotRollback)
    ));
    assert_eq!(calls.get(), 0);
}
