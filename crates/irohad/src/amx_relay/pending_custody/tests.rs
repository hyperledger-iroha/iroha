//! Native-certified local custody fixtures; no World execution or monetary qualification.

use super::*;
use iroha_crypto::Algorithm;
use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
fn open(path: &Path, native: &NativeFinalityFixture, slots: usize) -> PendingCustody {
    PendingCustody::open(
        path,
        key(71),
        AccountId::new(key(71).public_key().clone()),
        native.verifier(),
        slots,
    )
    .unwrap()
}
fn carrier(seed: u8) -> PendingKey {
    PendingKey {
        source_plan: [seed; 32],
        instruction: Hash::new([seed]),
        kind: PendingKind::Carrier,
    }
}
fn identity() -> TransactionIdentityV1 {
    TransactionIdentityV1 {
        signed_transaction_hash: [82; 32],
        entrypoint_hash: [83; 32],
    }
}

fn handoff_group(seed: u8, effect: Hash) -> [PendingKey; 2] {
    let mut carrier = carrier(seed);
    carrier.kind = PendingKind::HandoffCarrier(effect);
    let mut handoff = carrier.clone();
    handoff.kind = PendingKind::Handoff(effect);
    [carrier, handoff]
}

#[test]
fn cold_full_ordinary_custody_preserves_capacity_for_one_exact_handoff_group() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let native = NativeFinalityFixture::new();
    let owner = open(&path, &native, 1);
    let ordinary = carrier(100);
    let reserved = owner.reserve(&ordinary).unwrap();
    let original = owner
        .bind_signed_identity(&ordinary, &reserved, identity())
        .unwrap();
    drop(owner);
    let reopened = open(&path, &native, 1);
    let effect = Hash::new(b"cold authority handoff");
    let [handoff_carrier, handoff] = handoff_group(101, effect);
    let lease = reopened
        .acquire_observation(handoff.source_plan, handoff.instruction, Some(effect))
        .unwrap();
    reopened
        .reserve_group(&[handoff_carrier.clone(), handoff.clone()])
        .expect("a full ordinary pool must not strand its prerequisite handoff");
    assert_eq!(
        reopened
            .load(&Slot::Pending(ordinary.clone()))
            .unwrap()
            .unwrap()
            .digest,
        original,
        "admitting the handoff cannot evict an unresolved ordinary identity"
    );
    assert!(
        reopened
            .load(&Slot::Pending(handoff_carrier.clone()))
            .unwrap()
            .is_some()
    );
    assert!(
        reopened
            .load(&Slot::Pending(handoff.clone()))
            .unwrap()
            .is_some()
    );
    assert_eq!(
        reopened.maximum_payload_bytes(),
        5 * MAX_PENDING_RECORD_BYTES
    );
    assert!(matches!(
        reopened.reserve_group(&handoff_group(102, Hash::new(b"another handoff"))),
        Err(CustodyError::Capacity)
    ));
    drop(lease);
    drop(reopened);
    let reopened = open(&path, &native, 1);
    for key in [ordinary, handoff_carrier, handoff] {
        assert!(reopened.contains(&Slot::Pending(key)).unwrap());
    }
}
fn publish(
    owner: &PendingCustody,
    slot: &Slot,
    expected: Option<&Digest32>,
    progress: Progress,
    native: &NativeFinalityFixture,
) -> Result<Digest32> {
    let admitted;
    let expected = if expected.is_none()
        && let Slot::Pending(key) = slot
    {
        let reserved = if let Some((source_plan, instruction, effect)) = key.handoff_group() {
            owner.reserve_group(&[
                PendingKey {
                    source_plan,
                    instruction,
                    kind: PendingKind::HandoffCarrier(effect),
                },
                PendingKey {
                    source_plan,
                    instruction,
                    kind: PendingKind::Handoff(effect),
                },
            ])?;
            owner.load(slot)?.ok_or(CustodyError::Binding)?.digest
        } else {
            owner.reserve(key)?
        };
        admitted = if let Progress::Carrier { identity, .. } = &progress {
            owner.bind_signed_identity(key, &reserved, *identity)?
        } else {
            reserved
        };
        Some(&admitted)
    } else {
        expected
    };
    owner.compare_and_publish(
        slot,
        expected,
        progress,
        &native.verifier(),
        native.latest(),
    )
}

#[test]
fn signed_pending_slots_reserve_capacity_and_resume_originals_when_full() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let mut native = NativeFinalityFixture::start("amx-signed-pending-custody");
    let owner = open(&path, &native, 1);
    assert_eq!(owner.maximum_payload_bytes(), 5 * MAX_PENDING_RECORD_BYTES);
    let first = Slot::Pending(carrier(72));
    assert!(owner.load(&first).unwrap().is_none());
    assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    let original = publish(
        &owner,
        &first,
        None,
        Progress::Carrier {
            identity: identity(),
            target_height: 70,
        },
        &native,
    )
    .unwrap();
    assert!(matches!(
        publish(
            &owner,
            &Slot::Pending(carrier(73)),
            None,
            Progress::Carrier {
                identity: identity(),
                target_height: 50
            },
            &native
        ),
        Err(CustodyError::Capacity)
    ));
    let live = publish(
        &owner,
        &Slot::LiveApproval,
        None,
        Progress::LiveApproval,
        &native,
    )
    .unwrap();
    let block = native.block_with_submitted_work(native.next_header());
    native.certify(block);
    let advanced = publish(
        &owner,
        &first,
        Some(&original),
        Progress::Carrier {
            identity: identity(),
            target_height: 70,
        },
        &native,
    )
    .unwrap();
    assert_ne!(original, advanced);
    publish(
        &owner,
        &Slot::LiveApproval,
        Some(&live),
        Progress::LiveApproval,
        &native,
    )
    .unwrap();
    drop(owner);
    let reopened = open(&path, &native, 1);
    let saved = reopened.load(&first).unwrap().unwrap();
    assert_eq!(saved.digest, advanced);
    assert_eq!(saved.tip.as_ref().unwrap().height(), 2);
    assert_eq!(
        saved.verifier.as_ref().unwrap().authenticated_height(),
        Some(2)
    );
    assert_eq!(
        saved.progress,
        Progress::Carrier {
            identity: identity(),
            target_height: 70
        }
    );
    assert!(matches!(
        publish(
            &reopened,
            &first,
            Some(&original),
            Progress::Carrier {
                identity: identity(),
                target_height: 70
            },
            &native
        ),
        Err(CustodyError::Conflict)
    ));
}

#[test]
fn signed_pending_custody_refuses_key_source_root_purpose_and_byte_substitution() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let native = NativeFinalityFixture::new();
    let owner = open(&path, &native, 4);
    let first = Slot::Pending(carrier(74));
    let digest = publish(
        &owner,
        &first,
        None,
        Progress::Carrier {
            identity: identity(),
            target_height: 50,
        },
        &native,
    )
    .unwrap();
    let original = owner
        .directory
        .read_optional(owner.name(&first).unwrap(), MAX_PENDING_RECORD_BYTES)
        .unwrap()
        .unwrap();
    let other = Slot::Pending(carrier(75));
    owner
        .directory
        .write_atomic(
            owner.name(&other).unwrap(),
            &original,
            PublishMode::CreateNew,
        )
        .unwrap();
    assert!(matches!(owner.load(&other), Err(CustodyError::Binding)));
    let wrong = PendingCustody::open(
        &path,
        key(76),
        owner.route.authority.clone(),
        native.verifier(),
        4,
    )
    .unwrap();
    owner
        .directory
        .write_atomic(
            wrong.name(&first).unwrap(),
            &original,
            PublishMode::CreateNew,
        )
        .unwrap();
    assert!(matches!(wrong.load(&first), Err(CustodyError::Binding)));
    let foreign = NativeFinalityFixture::start("different-original-root");
    assert!(matches!(
        publish(
            &owner,
            &first,
            Some(&digest),
            Progress::Carrier {
                identity: identity(),
                target_height: 50
            },
            &foreign
        ),
        Err(CustodyError::Binding)
    ));
    let mut envelope: Envelope = norito::decode_canonical(&original).unwrap();
    envelope.signature = Signature::try_new(
        owner.key.private_key(),
        &domain_digest(b"different-local-purpose", &[&envelope.body]),
    )
    .unwrap();
    owner
        .directory
        .write_atomic(
            owner.name(&first).unwrap(),
            &norito::encode_canonical(&envelope).unwrap(),
            PublishMode::Replace,
        )
        .unwrap();
    assert!(matches!(owner.load(&first), Err(CustodyError::Binding)));
    assert!(matches!(
        publish(
            &owner,
            &first,
            Some(&digest),
            Progress::Carrier {
                identity: identity(),
                target_height: 50
            },
            &native
        ),
        Err(CustodyError::Binding)
    ));
    let mut corrupt = original.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    owner
        .directory
        .write_atomic(owner.name(&first).unwrap(), &corrupt, PublishMode::Replace)
        .unwrap();
    assert!(owner.load(&first).is_err());
    assert_eq!(
        owner
            .directory
            .read_optional(owner.name(&first).unwrap(), MAX_PENDING_RECORD_BYTES)
            .unwrap()
            .unwrap(),
        corrupt
    );
}

#[test]
fn live_floor_and_contiguous_handoff_frontier_never_rewind_with_carrier_replay() {
    let root = tempfile::tempdir().unwrap();
    let mut native = NativeFinalityFixture::start("pending-frontiers");
    let genesis = native.clone();
    let owner = open(&root.path().join("pending"), &native, 2);
    let block = native.block_with_submitted_work(native.next_header());
    native.certify(block);
    let live = publish(
        &owner,
        &Slot::LiveApproval,
        None,
        Progress::LiveApproval,
        &native,
    )
    .unwrap();
    assert!(matches!(
        publish(
            &owner,
            &Slot::LiveApproval,
            Some(&live),
            Progress::LiveApproval,
            &genesis
        ),
        Err(CustodyError::Binding)
    ));
    let mut handoff = carrier(77);
    handoff.kind = PendingKind::Handoff(Hash::new(b"exact original handoff effect"));
    let handoff = Slot::Pending(handoff);
    assert!(matches!(
        publish(
            &owner,
            &handoff,
            None,
            Progress::Handoff {
                inspected_height: 1,
                captured_tip: 70
            },
            &native
        ),
        Err(CustodyError::Binding)
    ));
    let inspected = publish(
        &owner,
        &handoff,
        None,
        Progress::Handoff {
            inspected_height: 2,
            captured_tip: 70,
        },
        &native,
    )
    .unwrap();
    assert!(matches!(
        publish(
            &owner,
            &handoff,
            Some(&inspected),
            Progress::Handoff {
                inspected_height: 1,
                captured_tip: 70
            },
            &genesis
        ),
        Err(CustodyError::Binding)
    ));
    assert!(matches!(
        publish(
            &owner,
            &handoff,
            Some(&inspected),
            Progress::Handoff {
                inspected_height: 2,
                captured_tip: 50
            },
            &native
        ),
        Err(CustodyError::Binding)
    ));
    let carrier = Slot::Pending(carrier(78));
    assert!(matches!(
        publish(
            &owner,
            &handoff,
            Some(&inspected),
            Progress::Handoff {
                inspected_height: 2,
                captured_tip: 80
            },
            &native,
        ),
        Err(CustodyError::Binding)
    ));
    let high = publish(
        &owner,
        &carrier,
        None,
        Progress::Carrier {
            identity: identity(),
            target_height: 70,
        },
        &native,
    )
    .unwrap();
    assert!(matches!(
        publish(
            &owner,
            &carrier,
            Some(&high),
            Progress::Carrier {
                identity: identity(),
                target_height: 70
            },
            &genesis
        ),
        Err(CustodyError::Binding)
    ));
    publish(
        &owner,
        &carrier,
        Some(&high),
        Progress::Carrier {
            identity: identity(),
            target_height: 50,
        },
        &genesis,
    )
    .unwrap();
    assert_eq!(
        owner
            .load(&Slot::LiveApproval)
            .unwrap()
            .unwrap()
            .tip
            .as_ref()
            .unwrap()
            .height(),
        2
    );
    assert_eq!(
        owner
            .load(&handoff)
            .unwrap()
            .unwrap()
            .tip
            .as_ref()
            .unwrap()
            .height(),
        2
    );
}

#[test]
fn authenticated_cas_metadata_cannot_restore_or_replace_a_malformed_checkpoint() {
    let root = tempfile::tempdir().unwrap();
    let native = NativeFinalityFixture::new();
    let owner = open(&root.path().join("pending"), &native, 1);
    let slot = Slot::Pending(carrier(103));
    let progress = Progress::Carrier {
        identity: identity(),
        target_height: 70,
    };
    publish(&owner, &slot, None, progress.clone(), &native).unwrap();
    // Even a locally signed outer binding is not a native proof. This negative
    // fixture bypasses the internal authenticated-verifier publisher deliberately.
    let body = norito::encode_canonical(&Body {
        route: owner.route.clone(),
        slot: slot.clone(),
        progress: progress.clone(),
        checkpoint: Some(vec![0, 1, 2, 3]),
    })
    .unwrap();
    let signature = Signature::try_new(
        owner.key.private_key(),
        &domain_digest(SIGN_DOMAIN, &[&body]),
    )
    .unwrap();
    let bytes = norito::encode_canonical(&Envelope { body, signature }).unwrap();
    owner
        .directory
        .write_atomic(owner.name(&slot).unwrap(), &bytes, PublishMode::Replace)
        .unwrap();
    let digest = domain_digest(DIGEST_DOMAIN, &[&bytes]);
    assert!(
        owner.load(&slot).is_err(),
        "restoration still requires the complete native checkpoint"
    );
    assert!(
        owner.recorded(&slot).is_err(),
        "CAS metadata is still bounded canonical material"
    );
    assert!(
        owner
            .compare_and_publish(
                &slot,
                Some(&digest),
                progress,
                &native.verifier(),
                native.latest()
            )
            .is_err()
    );
    assert_eq!(
        owner
            .directory
            .read_optional(owner.name(&slot).unwrap(), MAX_PENDING_RECORD_BYTES)
            .unwrap()
            .unwrap()
            .as_slice(),
        bytes.as_slice()
    );
}

#[test]
fn pending_reclamation_requires_exact_original_and_preserves_live_floor() {
    let root = tempfile::tempdir().unwrap();
    let native = NativeFinalityFixture::new();
    let owner = open(&root.path().join("pending"), &native, 1);
    let key = carrier(79);
    let slot = Slot::Pending(key.clone());
    let digest = publish(
        &owner,
        &slot,
        None,
        Progress::Carrier {
            identity: identity(),
            target_height: 70,
        },
        &native,
    )
    .unwrap();
    let live = publish(
        &owner,
        &Slot::LiveApproval,
        None,
        Progress::LiveApproval,
        &native,
    )
    .unwrap();
    let lock = owner.directory.open_lock(LOCK).unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(
        owner.retire_after_durable_route_result(&key, &digest),
        Err(CustodyError::Busy)
    ));
    drop(lock);
    assert!(matches!(
        owner.retire_after_durable_route_result(&key, &[0; 32]),
        Err(CustodyError::Conflict)
    ));
    assert_eq!(owner.load(&slot).unwrap().unwrap().digest, digest);
    // This exercises only the custody CAS. Production may call it solely at the
    // reviewed durable engine/route completion boundary, never from outcome().
    owner
        .retire_after_durable_route_result(&key, &digest)
        .unwrap();
    assert!(owner.load(&slot).unwrap().is_none());
    assert_eq!(
        owner.load(&Slot::LiveApproval).unwrap().unwrap().digest,
        live
    );
    publish(
        &owner,
        &Slot::Pending(carrier(80)),
        None,
        Progress::Carrier {
            identity: identity(),
            target_height: 50,
        },
        &native,
    )
    .unwrap();
}

#[test]
fn pending_staging_recovery_is_bounded_and_preserves_original_records() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let native = NativeFinalityFixture::new();
    let owner = open(&path, &native, 1);
    let slot = Slot::Pending(carrier(81));
    let digest = publish(
        &owner,
        &slot,
        None,
        Progress::Carrier {
            identity: identity(),
            target_height: 70,
        },
        &native,
    )
    .unwrap();
    owner
        .directory
        .write_atomic(
            ".iroha-fs-1-0.tmp",
            b"unpublished partial record",
            PublishMode::CreateNew,
        )
        .unwrap();
    drop(owner);
    let reopened = open(&path, &native, 1);
    let replacement = publish(
        &reopened,
        &slot,
        Some(&digest),
        Progress::Carrier {
            identity: identity(),
            target_height: 70,
        },
        &native,
    )
    .unwrap();
    assert!(!path.join(".iroha-fs-1-0.tmp").exists());
    assert_eq!(reopened.load(&slot).unwrap().unwrap().digest, replacement);
    for name in [".iroha-fs-1-1.tmp", ".iroha-fs-1-2.tmp"] {
        reopened
            .directory
            .write_atomic(name, b"excess staging", PublishMode::CreateNew)
            .unwrap();
    }
    assert!(
        publish(
            &reopened,
            &slot,
            Some(&replacement),
            Progress::Carrier {
                identity: identity(),
                target_height: 70
            },
            &native
        )
        .is_err()
    );
    assert_eq!(reopened.load(&slot).unwrap().unwrap().digest, replacement);
    assert!(path.join(".iroha-fs-1-1.tmp").exists());
    assert!(path.join(".iroha-fs-1-2.tmp").exists());
}

#[test]
fn reservation_precedes_identity_and_full_capacity_preserves_the_original_slot() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let native = NativeFinalityFixture::start("pre-signing-admission");
    let owner = open(&path, &native, 1);
    let key = carrier(84);
    let reserved = owner.reserve(&key).unwrap();
    let saved = owner.load(&Slot::Pending(key.clone())).unwrap().unwrap();
    assert_eq!(saved.progress, Progress::Reserved);
    assert!(saved.verifier.is_none());
    assert!(saved.tip.is_none());
    assert!(matches!(
        owner.reserve(&carrier(85)),
        Err(CustodyError::Capacity)
    ));
    let signed = owner
        .bind_signed_identity(&key, &reserved, identity())
        .unwrap();
    let saved = owner.load(&Slot::Pending(key.clone())).unwrap().unwrap();
    assert_eq!(
        saved.progress,
        Progress::SignedCarrier {
            identity: identity()
        }
    );
    assert!(saved.verifier.is_none());
    assert!(saved.tip.is_none());
    let mut replacement = identity();
    replacement.signed_transaction_hash[0] ^= 1;
    assert!(matches!(
        owner.bind_signed_identity(&key, &signed, replacement),
        Err(CustodyError::Binding)
    ));
    drop(owner);
    let reopened = open(&path, &native, 1);
    assert_eq!(reopened.reserve(&key).unwrap(), signed);
    assert_eq!(
        reopened
            .bind_signed_identity(&key, &signed, identity())
            .unwrap(),
        signed
    );
    assert_eq!(
        std::fs::read_dir(&path).unwrap().count(),
        2,
        "one reservation survives restart, without a fabricated checkpoint"
    );
}

#[test]
fn verified_rejected_retry_reuses_reserved_slot_without_relying_on_journal_labels() {
    use iroha_crypto::HashOf;
    use iroha_data_model::{
        executor::ValidationFail,
        query::CommittedTransaction,
        transaction::{Executable, error::TransactionRejectionReason},
    };
    for rejected in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("pending");
        let mut native = NativeFinalityFixture::start("explicit-rejected-retry");
        let mut block = native.block_with_submitted_work(native.next_header());
        let transaction = block.external_transactions().next().unwrap();
        let original = TransactionIdentityV1 {
            signed_transaction_hash: *Hash::from(transaction.hash()).as_ref(),
            entrypoint_hash: *Hash::from(transaction.hash_as_entrypoint()).as_ref(),
        };
        let authority = transaction.authority().clone();
        let Executable::Instructions(instructions) = transaction.instructions() else {
            panic!("native input");
        };
        let pending = PendingKey {
            source_plan: [86; 32],
            instruction: Hash::new(norito::encode_canonical(&instructions[0]).unwrap()),
            kind: PendingKind::Carrier,
        };
        if rejected {
            NativeFinalityFixture::install_network_results(
                &mut block,
                vec![Err(TransactionRejectionReason::Validation(
                    ValidationFail::NotPermitted("synthetic exact rejection".into()),
                ))],
            );
        }
        let entrypoint = block.network_entrypoint_at(0).unwrap().clone();
        let (output_index, _) = block.network_output_at(0).unwrap();
        let output = block.execution_outputs()[output_index as usize].clone();
        let evidence = CommittedTransaction {
            block_hash: block.hash(),
            entrypoint_hash: entrypoint.hash(),
            entrypoint_proof: block.network_input_proof(0).unwrap(),
            entrypoint,
            output_hash: HashOf::new(&output),
            output_proof: block.output_proof(output_index).unwrap(),
            output,
        };
        native.certify(block);
        let owner = PendingCustody::open(&path, key(71), authority, native.verifier(), 1).unwrap();
        let reserved = owner.reserve(&pending).unwrap();
        let signed = owner
            .bind_signed_identity(&pending, &reserved, original)
            .unwrap();
        let completed = owner
            .compare_and_publish(
                &Slot::Pending(pending.clone()),
                Some(&signed),
                Progress::Carrier {
                    identity: original,
                    target_height: 2,
                },
                &native.verifier(),
                native.latest(),
            )
            .unwrap();
        let new_identity = identity();
        let changed = owner.rebind_after_durable_rejected_attempt(
            &pending,
            &completed,
            new_identity,
            &evidence,
        );
        if rejected {
            let rebound = changed.unwrap();
            let reopened = PendingCustody::open(
                &path,
                key(71),
                owner.route.authority.clone(),
                native.verifier(),
                1,
            )
            .unwrap();
            let saved = reopened
                .load(&Slot::Pending(pending.clone()))
                .unwrap()
                .unwrap();
            assert_eq!(saved.digest, rebound);
            assert_eq!(
                saved.progress,
                Progress::SignedCarrier {
                    identity: new_identity
                }
            );
            assert!(saved.tip.is_none());
            assert!(matches!(
                reopened.reserve(&carrier(87)),
                Err(CustodyError::Capacity)
            ));
            assert_eq!(
                std::fs::read_dir(&path).unwrap().count(),
                2,
                "retry replaces identity inside the same admitted slot"
            );
        } else {
            assert!(matches!(changed, Err(CustodyError::Binding)));
            assert_eq!(
                owner.load(&Slot::Pending(pending)).unwrap().unwrap().digest,
                completed
            );
        }
    }
}

#[test]
fn group_admission_repairs_only_its_original_partial_group_and_rejects_relabeling() {
    let root = tempfile::tempdir().unwrap();
    let native = NativeFinalityFixture::new();
    let owner = open(&root.path().join("pending"), &native, 1);
    let effect = Hash::new(b"original handoff");
    let group = handoff_group(90, effect);
    let _lease = owner
        .acquire_observation(group[0].source_plan, group[0].instruction, Some(effect))
        .unwrap();
    let mut mismatched = group.clone();
    mismatched[1].instruction = Hash::new(b"different instruction");
    assert!(matches!(
        owner.reserve_group(&mismatched),
        Err(CustodyError::Binding)
    ));
    let mut relabeled = group.clone();
    relabeled[0].kind = PendingKind::Carrier;
    assert!(matches!(
        owner.reserve_group(&relabeled),
        Err(CustodyError::Binding)
    ));
    assert!(matches!(
        owner.reserve(&group[0]),
        Err(CustodyError::Binding)
    ));
    // Simulate the first durable member of an interrupted two-record publication.
    let reserved = owner.reserve_member(&group[0]).unwrap();
    let signed = owner
        .bind_signed_identity(&group[0], &reserved, identity())
        .unwrap();
    let ordinary = carrier(91);
    owner.reserve(&ordinary).unwrap();
    owner.reserve_group(&group).unwrap();
    assert_eq!(
        owner
            .load(&Slot::Pending(group[0].clone()))
            .unwrap()
            .unwrap()
            .digest,
        signed
    );
    assert!(owner.contains(&Slot::Pending(group[1].clone())).unwrap());
    assert!(owner.contains(&Slot::Pending(ordinary)).unwrap());
    // A valid signed record copied under a different quota-class name is not admission.
    let foreign_name = owner.name(&Slot::Pending(carrier(92))).unwrap();
    let bytes = owner
        .directory
        .read_optional(
            owner.name(&Slot::Pending(group[0].clone())).unwrap(),
            MAX_PENDING_RECORD_BYTES,
        )
        .unwrap()
        .unwrap();
    owner
        .directory
        .write_atomic(foreign_name, &bytes, PublishMode::CreateNew)
        .unwrap();
    assert!(matches!(owner.inventory(), Err(CustodyError::Binding)));
}

#[test]
fn observation_lease_survives_reopen_and_cannot_select_a_foreign_native_owner() {
    let root = tempfile::tempdir().unwrap();
    let native = NativeFinalityFixture::new();
    let path = root.path().join("pending");
    let owner = open(&path, &native, 2);
    let key = carrier(91);
    let lease = owner
        .acquire_observation(key.source_plan, key.instruction, None)
        .unwrap();
    let reopened = open(&path, &native, 2);
    assert!(matches!(
        reopened.acquire_observation(key.source_plan, key.instruction, None),
        Err(CustodyError::Busy)
    ));
    reopened.check_observation(&lease).unwrap();
    let foreign = open(&root.path().join("foreign"), &native, 2);
    assert!(foreign.check_observation(&lease).is_err());
    drop(lease);
    reopened
        .acquire_observation(key.source_plan, key.instruction, None)
        .unwrap();
}

#[tokio::test]
async fn canceled_panicking_native_job_retains_original_observation_until_tls_exit() {
    use std::{cell::RefCell, sync::mpsc, time::Duration};
    struct NativeExit {
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        release: mpsc::Receiver<()>,
    }
    impl Drop for NativeExit {
        fn drop(&mut self) {
            if let Some(entered) = self.entered.take() {
                let _ = entered.send(());
            }
            self.release.recv().unwrap();
        }
    }
    thread_local! {
        static NATIVE_EXIT: RefCell<Option<NativeExit>> = const { RefCell::new(None) };
    }
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("pending");
    let native = NativeFinalityFixture::new();
    let owner = open(&path, &native, 1);
    let key = carrier(104);
    assert!(matches!(owner.native_custody(), Err(CustodyError::Binding)));
    let lease = owner
        .acquire_observation(key.source_plan, key.instruction, None)
        .unwrap();
    let (permit, original) = owner.native_custody().unwrap();
    let (entered, exiting) = tokio::sync::oneshot::channel();
    let (release, finish) = mpsc::channel();
    let task = tokio::spawn(iroha::client::run_native_worker(
        permit,
        original,
        move || -> () {
            NATIVE_EXIT.with(|slot| {
                *slot.borrow_mut() = Some(NativeExit {
                    entered: Some(entered),
                    release: finish,
                })
            });
            panic!("intentional original native-job panic");
        },
    ));
    tokio::time::timeout(Duration::from_secs(5), exiting)
        .await
        .unwrap()
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    drop(lease);
    let reopened = open(&path, &native, 1);
    assert!(matches!(
        reopened.acquire_observation(key.source_plan, key.instruction, None),
        Err(CustodyError::Busy)
    ));
    assert!(matches!(
        owner.native_custody(),
        Err(CustodyError::Capacity)
    ));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while owner.native_jobs.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    reopened
        .acquire_observation(key.source_plan, key.instruction, None)
        .unwrap();
}
