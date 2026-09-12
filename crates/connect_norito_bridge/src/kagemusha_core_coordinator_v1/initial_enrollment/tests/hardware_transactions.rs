//! Actual signature and release admission tests for independent journal evidence.
//! Fixed test keys model the applet; these tests do not certify physical device behavior.

use super::*;
use iroha_core::zk::{kagemusha_v1_recursion::*, kagemusha_v1_state::*};
use std::{result::Result, sync::Mutex};

fn lane(fixture: &Fixture) -> KagemushaLaneIdV1 {
    KagemushaLaneIdV1 {
        network_id: fixture.owner.runtime.network_id,
        device_lane_id: fixture.owner.lane_id,
        asset: fixture.owner.runtime.asset.clone(),
        scale: fixture.owner.runtime.scale,
    }
}

fn anchor(fixture: &Fixture) -> DurabilityAnchorStatementV1 {
    let credential = fixture.qualification.credential;
    DurabilityAnchorStatementV1 {
        metadata_revision: 1,
        version: 1,
        lane: lane(fixture),
        state_commitment: [11; 32],
        hardware_epoch: HardwareEpochV1 {
            generation: u128::from(credential.hardware_epoch_generation),
            epoch_id: credential.hardware_epoch_id,
        },
        device_policy_binding: DevicePolicyBindingV1 {
            device_key_reference: credential.device_key_reference,
            hardware_policy_id: fixture.release.provider_policy_root(),
        },
        state_nonce_commitment: [12; 32],
        logical_sequence: 3,
        journal_revision: 4,
        inbox_revision: 5,
        snapshot_commitment: [13; 32],
    }
}

fn subject(
    fixture: &Fixture,
    transaction: KagemushaHardwareTransactionV1,
) -> KagemushaHardwareTransactionSubjectV1 {
    KagemushaHardwareTransactionSubjectV1::new(
        [78; 32],
        fixture.release.release_id(),
        fixture.release.hardware_policy_digest(),
        fixture.qualification.credential,
        500,
        transaction,
    )
}

fn signed(subject: KagemushaHardwareTransactionSubjectV1, signer: &SigningKey) -> Vec<u8> {
    let signature: p256::ecdsa::Signature = signer.sign(&subject.signing_bytes().unwrap());
    norito::encode_canonical(&KagemushaHardwareTransactionCertificateV1 {
        subject,
        signature: KagemushaDeviceSignatureV1::from_raw_bytes(
            &signature.normalize_s().unwrap_or(signature).to_bytes(),
        )
        .unwrap(),
    })
    .unwrap()
}

struct NoTransport;
impl KagemushaHardwareCheckpointTransportV1 for NoTransport {
    fn read_current_checkpoint(&self, _: [u8; 32]) -> Result<Vec<u8>, String> {
        Err("test transport unavailable".to_owned())
    }
}

fn verifier(fixture: &Fixture) -> KagemushaHardwareTransactionVerifierV1 {
    KagemushaHardwareTransactionVerifierV1::new(
        fixture.release.clone(),
        lane(fixture),
        fixture.qualification.profile.hardware_profile_id,
        Arc::new(NoTransport),
    )
    .unwrap()
}

fn auxiliary_transactions(fixture: &Fixture) -> Vec<KagemushaHardwareTransactionV1> {
    let anchor = anchor(fixture);
    vec![
        KagemushaHardwareTransactionV1::MintReservation(MintReservationStatementV1 {
            version: 1,
            lane: lane(fixture),
            hardware_epoch: anchor.hardware_epoch,
            state_commitment: anchor.state_commitment,
            inbox_revision_before: 0,
            inbox_revision_after: 1,
            reservation_digest: [41; 32],
            predecessor_journal_commitment: [42; 32],
            successor_journal_commitment: [43; 32],
            successor_capacity_commitment: [44; 32],
        }),
        KagemushaHardwareTransactionV1::MintStage(MintStageStatementV1 {
            version: 1,
            lane: lane(fixture),
            hardware_epoch: anchor.hardware_epoch,
            state_commitment: anchor.state_commitment,
            inbox_revision_before: 1,
            inbox_revision_after: 2,
            reservation_digest: [41; 32],
            credit_id: CreditIdV1([45; 32]),
            envelope_digest: [46; 32],
            staged_at_ms: 500,
            predecessor_journal_commitment: [43; 32],
            successor_journal_commitment: [47; 32],
            successor_capacity_commitment: [48; 32],
        }),
        KagemushaHardwareTransactionV1::CreditStage(CreditStageStatementV1 {
            version: 1,
            recipient_lane: lane(fixture),
            receiver_state_commitment: anchor.state_commitment,
            receiver_hardware_epoch: anchor.hardware_epoch,
            receiver_device_policy_binding: anchor.device_policy_binding,
            receiver_state_nonce_commitment: anchor.state_nonce_commitment,
            credit_id: CreditIdV1([49; 32]),
            envelope_digest: [50; 32],
            staged_at_ms: 500,
            journal_revision_before: 8,
            journal_revision_after: 9,
        }),
        KagemushaHardwareTransactionV1::RecoveryCheckpoint(
            KagemushaRecoveryCheckpointStatementV1 {
                operation_id: [78; 32],
                previous: KagemushaRecoveryCheckpointIdentityV1 {
                    revision: 0,
                    snapshot_commitment: [0; 32],
                },
                successor: anchor,
            },
        ),
    ]
}

#[test]
fn each_independent_transaction_requires_exact_canonical_statement_and_request_id() {
    let fixture = Fixture::new();
    let verifier = verifier(&fixture);
    for transaction in auxiliary_transactions(&fixture) {
        let original = subject(&fixture, transaction.clone());
        let bytes = signed(original.clone(), &fixture.device);
        verifier
            .verify_for_request([78; 32], &transaction, &bytes)
            .unwrap();
        assert!(
            verifier
                .verify_for_request([79; 32], &transaction, &bytes)
                .is_err()
        );
        let mut changed = original.clone();
        changed.request_id = [0; 32];
        assert!(
            verifier
                .verify(&transaction, &signed(changed, &fixture.device))
                .is_err()
        );
        let mut changed = original;
        changed.transaction = KagemushaHardwareTransactionV1::DurabilityAnchor(anchor(&fixture));
        assert!(
            verifier
                .verify(&transaction, &signed(changed, &fixture.device))
                .is_err()
        );
    }
}

#[test]
fn independent_transactions_reject_counter_skip_overflow_time_and_cas_substitution() {
    let fixture = Fixture::new();
    let verifier = verifier(&fixture);
    for transaction in auxiliary_transactions(&fixture) {
        for mutation in 0..4 {
            let mut changed = transaction.clone();
            match &mut changed {
                KagemushaHardwareTransactionV1::MintReservation(value) => match mutation {
                    0 => value.version = 2,
                    1 => value.inbox_revision_after += 1,
                    2 => {
                        value.inbox_revision_before = u128::MAX;
                        value.inbox_revision_after = 0;
                    }
                    _ => value.successor_journal_commitment = value.predecessor_journal_commitment,
                },
                KagemushaHardwareTransactionV1::MintStage(value) => match mutation {
                    0 => value.version = 2,
                    1 => value.inbox_revision_after += 1,
                    2 => {
                        value.inbox_revision_before = u128::MAX;
                        value.inbox_revision_after = 0;
                    }
                    _ => value.staged_at_ms += 1,
                },
                KagemushaHardwareTransactionV1::CreditStage(value) => match mutation {
                    0 => value.version = 2,
                    1 => value.journal_revision_after += 1,
                    2 => {
                        value.journal_revision_before = u128::MAX;
                        value.journal_revision_after = 0;
                    }
                    _ => value.staged_at_ms += 1,
                },
                KagemushaHardwareTransactionV1::RecoveryCheckpoint(value) => match mutation {
                    0 => value.operation_id = [79; 32],
                    1 => value.successor.metadata_revision += 1,
                    2 => {
                        value.previous.revision = u128::MAX;
                        value.successor.metadata_revision = 0;
                    }
                    _ => value.previous.snapshot_commitment = [80; 32],
                },
                _ => unreachable!("helper only contains four independent operations"),
            }
            let bytes = signed(subject(&fixture, changed.clone()), &fixture.device);
            assert!(
                verifier.verify(&changed, &bytes).is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn journal_certificate_accepts_exact_governed_commit_and_rejects_substitution() {
    let fixture = Fixture::new();
    let verifier = verifier(&fixture);
    let expected = KagemushaHardwareTransactionV1::DurabilityAnchor(anchor(&fixture));
    let original = subject(&fixture, expected.clone());
    let bytes = signed(original.clone(), &fixture.device);
    verifier.verify(&expected, &bytes).unwrap();
    // Historical trusted time remains usable after the credential's wall-clock expiration.
    verifier.verify(&expected, &bytes).unwrap();
    for change in 0..8 {
        let mut changed = original.clone();
        match change {
            0 => changed.version = 2,
            1 => changed.domain.push('x'),
            2 => changed.release_id[0] ^= 1,
            3 => changed.hardware_policy_digest[0] ^= 1,
            4 => changed.committed_at_ms = changed.credential.issued_at_ms - 1,
            5 => changed.committed_at_ms = changed.credential.expires_at_ms,
            6 => changed.credential.hardware_epoch_id[0] ^= 1,
            _ => {
                if let KagemushaHardwareTransactionV1::DurabilityAnchor(ref mut value) =
                    changed.transaction
                {
                    value.snapshot_commitment[0] ^= 1;
                }
            }
        }
        assert!(
            verifier
                .verify(&expected, &signed(changed, &fixture.device))
                .is_err(),
            "mutation {change}"
        );
    }
    let wrong_key = SigningKey::from_bytes((&[62; 32]).into()).unwrap();
    assert!(
        verifier
            .verify(&expected, &signed(original, &wrong_key))
            .is_err()
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(verifier.verify(&expected, &trailing).is_err());
    assert!(
        verifier
            .verify(&expected, &bytes[..bytes.len() - 1])
            .is_err()
    );
}

#[test]
fn journal_certificate_cannot_change_checkpoint_key_or_wallet() {
    let fixture = Fixture::new();
    let verifier = verifier(&fixture);
    for change in 0..5 {
        let mut value = anchor(&fixture);
        match change {
            0 => value.device_policy_binding.device_key_reference[0] ^= 1,
            1 => value.device_policy_binding.hardware_policy_id[0] ^= 1,
            2 => value.lane.device_lane_id[0] ^= 1,
            3 => value.hardware_epoch.generation += 1,
            _ => value.lane.scale += 1,
        }
        let expected = KagemushaHardwareTransactionV1::DurabilityAnchor(value);
        let bytes = signed(subject(&fixture, expected.clone()), &fixture.device);
        assert!(
            verifier.verify(&expected, &bytes).is_err(),
            "mutation {change}"
        );
    }
    assert!(
        KagemushaHardwareTransactionVerifierV1::new(
            fixture.release.clone(),
            lane(&fixture),
            [0; 32],
            Arc::new(NoTransport)
        )
        .is_err()
    );
}

struct ReplayTransport {
    subject: KagemushaHardwareTransactionSubjectV1,
    key: SigningKey,
    first_response: Mutex<Option<Vec<u8>>>,
}

impl KagemushaHardwareCheckpointTransportV1 for ReplayTransport {
    fn read_current_checkpoint(&self, challenge: [u8; 32]) -> Result<Vec<u8>, String> {
        let mut retained = self.first_response.lock().unwrap();
        if let Some(response) = &*retained {
            return Ok(response.clone());
        }
        let mut subject = self.subject.clone();
        subject.request_id = challenge;
        if let KagemushaHardwareTransactionV1::CurrentCheckpoint {
            challenge: ref mut actual,
            ..
        } = subject.transaction
        {
            *actual = challenge;
        }
        let response = signed(subject, &self.key);
        *retained = Some(response.clone());
        Ok(response)
    }
}

#[test]
fn checkpoint_read_rejects_cached_response_after_recreation() {
    let fixture = Fixture::new();
    let anchor = anchor(&fixture);
    let prefix = KagemushaRecoveryJournalPrefixV1 {
        byte_len: 128,
        sequence: 1,
        head: [25; 32],
    };
    let journals = KagemushaRecoveryJournalsV1 {
        coordinator: prefix,
        responses: prefix,
        response_history_root: [26; 32],
        retirement_transition_id: [27; 32],
    };
    let transport = Arc::new(ReplayTransport {
        subject: subject(
            &fixture,
            KagemushaHardwareTransactionV1::CurrentCheckpoint {
                statement: anchor.clone(),
                journals: journals.clone(),
                challenge: [0; 32],
            },
        ),
        key: fixture.device.clone(),
        first_response: Mutex::new(None),
    });
    let open = || {
        KagemushaHardwareTransactionVerifierV1::new(
            fixture.release.clone(),
            lane(&fixture),
            fixture.qualification.profile.hardware_profile_id,
            transport.clone(),
        )
        .unwrap()
    };
    open().verify_current(&anchor, &journals).unwrap();
    assert!(open().verify_current(&anchor, &journals).is_err());
}

#[cfg(unix)]
#[test]
fn admitted_history_factory_reopens_exact_wal_and_rejects_conflicts() {
    let fixture = Fixture::new();
    let credential = fixture.qualification.credential;
    let authenticate = |credentials| {
        KagemushaHistoryDeviceCredentialsV1::authenticate(
            &fixture.release,
            &lane(&fixture),
            fixture.qualification.profile.hardware_profile_id,
            credentials,
        )
    };
    assert!(authenticate(vec![credential, credential]).is_err());
    let mut changed = credential;
    changed.lane_commitment[0] ^= 1;
    assert!(authenticate(vec![changed]).is_err());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("history");
    let store = KagemushaDiskAuthenticatedHistoryStoreV1::create_new(
        &path,
        [44; 32],
        authenticate(vec![credential]).unwrap(),
        1024 * 1024,
    )
    .unwrap();
    assert!(
        KagemushaDiskAuthenticatedHistoryStoreV1::open_existing(
            &path,
            [44; 32],
            authenticate(vec![credential]).unwrap(),
            1024 * 1024
        )
        .is_err()
    );
    drop(store);
    let reopened = KagemushaDiskAuthenticatedHistoryStoreV1::open_existing(
        &path,
        [44; 32],
        authenticate(vec![credential]).unwrap(),
        1024 * 1024,
    )
    .unwrap();
    drop(reopened);
    assert!(
        KagemushaDiskAuthenticatedHistoryStoreV1::open_existing(
            &path,
            [45; 32],
            authenticate(vec![credential]).unwrap(),
            1024 * 1024
        )
        .is_err()
    );
    use std::io::{Seek as _, Write as _};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(path.join("history.norito.wal"))
        .unwrap();
    file.seek(std::io::SeekFrom::End(-1)).unwrap();
    file.write_all(&[0xff]).unwrap();
    file.sync_all().unwrap();
    drop(file);
    assert!(
        KagemushaDiskAuthenticatedHistoryStoreV1::open_existing(
            &path,
            [44; 32],
            authenticate(vec![credential]).unwrap(),
            1024 * 1024
        )
        .is_err()
    );
}

#[cfg(unix)]
struct InterruptedApplet {
    template: KagemushaHardwareTransactionSubjectV1,
    key: SigningKey,
    committed: Mutex<Option<([u8; 32], KagemushaHardwareTransactionV1, Vec<u8>)>>,
}

#[cfg(unix)]
impl KagemushaHardwareTransactionTransportV1 for InterruptedApplet {
    fn commit_or_recover(
        &self,
        id: [u8; 32],
        transaction: &KagemushaHardwareTransactionV1,
    ) -> Result<Vec<u8>, String> {
        let mut committed = self.committed.lock().unwrap();
        if let Some((original_id, original, bytes)) = &*committed {
            if *original_id != id || original != transaction {
                return Err("device conflict".to_owned());
            }
            return Ok(bytes.clone());
        }
        let mut subject = self.template.clone();
        subject.request_id = id;
        subject.transaction = transaction.clone();
        *committed = Some((id, transaction.clone(), signed(subject, &self.key)));
        Err("injected loss after irreversible device commit".to_owned())
    }
}

#[cfg(unix)]
#[test]
fn hardware_journal_recovers_commit_loss_and_exact_exposed_bytes() {
    let fixture = Fixture::new();
    let transaction = KagemushaHardwareTransactionV1::DurabilityAnchor(anchor(&fixture));
    let applet = Arc::new(InterruptedApplet {
        template: subject(&fixture, transaction.clone()),
        key: fixture.device.clone(),
        committed: Mutex::new(None),
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("transactions");
    let id = [74; 32];
    let mut journal = KagemushaHardwareTransactionJournalV1::create_new(
        &path,
        verifier(&fixture),
        applet.clone(),
    )
    .unwrap();
    let initial = journal.recovery_prefix().unwrap();
    assert!(journal.commit_or_recover(id, transaction.clone()).is_err());
    let prepared = journal.recovery_prefix().unwrap();
    assert!(prepared.byte_len > initial.byte_len);
    assert_eq!(prepared.sequence, initial.sequence + 1);
    let expected = applet.committed.lock().unwrap().as_ref().unwrap().2.clone();
    drop(journal);
    let mut journal = KagemushaHardwareTransactionJournalV1::open_existing(
        &path,
        verifier(&fixture),
        applet.clone(),
    )
    .unwrap();
    assert_eq!(
        journal.commit_or_recover(id, transaction.clone()).unwrap(),
        expected
    );
    let completed = journal.recovery_prefix().unwrap();
    assert_eq!(completed.sequence, prepared.sequence + 1);
    assert_eq!(
        journal.commit_or_recover(id, transaction.clone()).unwrap(),
        expected
    );
    assert_eq!(journal.recovery_prefix().unwrap(), completed);
    let mut conflict = anchor(&fixture);
    conflict.snapshot_commitment[0] ^= 1;
    assert!(
        journal
            .commit_or_recover(
                id,
                KagemushaHardwareTransactionV1::DurabilityAnchor(conflict)
            )
            .is_err()
    );
    assert_eq!(journal.recovery_prefix().unwrap(), completed);
    drop(journal);
    let mut journal =
        KagemushaHardwareTransactionJournalV1::open_existing(&path, verifier(&fixture), applet)
            .unwrap();
    assert_eq!(
        journal.commit_or_recover(id, transaction).unwrap(),
        expected
    );
    assert_eq!(journal.recovery_prefix().unwrap(), completed);
}
