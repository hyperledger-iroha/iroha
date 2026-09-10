// Issuer enrollment, approval, durable outbox and checkpoint security regressions.

#[test]
fn enrollment_is_encrypted_and_debug_is_payload_free() {
    let (_temp, mut service, _policy, _wallet, _approvers, enrollment) = service_fixture();
    let bytes = encode_canonical(&enrollment).expect("encode");
    let rendered = format!("{enrollment:?}");
    assert!(!rendered.contains("private-applicant"));
    assert!(
        !bytes
            .windows(b"private-applicant".len())
            .any(|window| window == b"private-applicant")
    );
    let status = service.submit_enrollment(&bytes, 20).expect("submit");
    assert_eq!(status.state, PopEnrollmentStateV1::AwaitingApproval);
    let checkpoint = fs::read(service.checkpoint_path).expect("checkpoint");
    assert!(
        !checkpoint
            .windows(b"biometric".len())
            .any(|window| window == b"biometric")
    );
    assert!(
        !checkpoint
            .windows(b"private-applicant".len())
            .any(|window| window == b"private-applicant")
    );
}
#[test]
fn enrollment_replay_is_idempotent_only_for_identical_ciphertext() {
    let (_temp, mut service, _policy, _wallet, _approvers, mut enrollment) = service_fixture();
    let bytes = encode_canonical(&enrollment).expect("encode");
    service.submit_enrollment(&bytes, 20).expect("first");
    service.submit_enrollment(&bytes, 21).expect("idempotent");
    enrollment.encrypted_payload.ciphertext[0] ^= 1;
    assert_eq!(
        service.submit_enrollment(&encode_canonical(&enrollment).unwrap(), 21),
        Err(PopCredentialServiceError::EnrollmentReplay)
    );
}
fn fail_before_checkpoint_rename(
    _path: &Path,
    _bytes: &[u8],
) -> Result<(), PopCheckpointPersistFailure> {
    Err(PopCheckpointPersistFailure {
        error: PopCredentialServiceError::CheckpointIo,
        committed: false,
    })
}
fn fail_checkpoint_parent_sync(_: &Path) -> io::Result<()> {
    Err(io::Error::other("injected parent sync failure"))
}
fn fail_after_checkpoint_rename(
    path: &Path,
    bytes: &[u8],
) -> Result<(), PopCheckpointPersistFailure> {
    crate::write_local_checkpoint_atomic_with_mode_and_parent_sync(
        path,
        bytes,
        true,
        fail_checkpoint_parent_sync,
    )
    .map_err(|error| PopCheckpointPersistFailure {
        error: if error.committed {
            PopCredentialServiceError::CheckpointDurabilityUncertain
        } else {
            PopCredentialServiceError::CheckpointIo
        },
        committed: error.committed,
    })
}
#[test]
fn crash_before_and_after_rename_preserve_transaction_boundaries() {
    let (_temp, mut service, policy, _wallet, approvers, enrollment) = service_fixture();
    let canonical = encode_canonical(&enrollment).unwrap();
    let original_checkpoint = fs::read(&service.checkpoint_path).unwrap();
    service.checkpoint_writer = fail_before_checkpoint_rename;
    assert_eq!(
        service.submit_enrollment(&canonical, 20),
        Err(PopCredentialServiceError::CheckpointIo)
    );
    assert!(service.state.enrollments.is_empty());
    assert_eq!(
        fs::read(&service.checkpoint_path).unwrap(),
        original_checkpoint
    );
    service.checkpoint_writer = fail_after_checkpoint_rename;
    assert_eq!(
        service.submit_enrollment(&canonical, 20),
        Err(PopCredentialServiceError::CheckpointDurabilityUncertain)
    );
    assert_eq!(service.state.enrollments.len(), 1);
    let visible = fs::read(&service.checkpoint_path).unwrap();
    let restored: PopIssuerCheckpointV1 = decode_canonical(
        &visible,
        POP_ISSUER_CHECKPOINT_MAX_BYTES_V1,
        POP_SERVICE_COLLECTION_MAX_V1,
    )
    .unwrap();
    assert_eq!(restored.enrollments.len(), 1);
    let approval = approval(
        "approver-0",
        &approvers[0],
        &enrollment,
        &policy,
        PopApprovalDecisionV1::Approve,
    );
    assert_eq!(
        service.record_approval(approval, 21),
        Err(PopCredentialServiceError::CheckpointDurabilityUncertain)
    );
}
#[test]
fn dual_control_rejects_duplicates_wrong_policy_and_revoked_signer() {
    let (_temp, mut service, policy, _wallet, approvers, enrollment) = service_fixture();
    service
        .submit_enrollment(&encode_canonical(&enrollment).unwrap(), 20)
        .unwrap();
    let first = approval(
        "approver-0",
        &approvers[0],
        &enrollment,
        &policy,
        PopApprovalDecisionV1::Approve,
    );
    service.record_approval(first.clone(), 20).unwrap();
    assert_eq!(
        service.record_approval(first, 20),
        Err(PopCredentialServiceError::DuplicateApproval)
    );
    let mut wrong_policy = approval(
        "approver-1",
        &approvers[1],
        &enrollment,
        &policy,
        PopApprovalDecisionV1::Approve,
    );
    wrong_policy.issuer_policy_digest = [9; 32];
    assert_eq!(
        service.record_approval(wrong_policy, 20),
        Err(PopCredentialServiceError::ApprovalBinding)
    );
    service.policy.approval_signers[1].revoked_at_epoch = Some(20);
    let revoked = approval(
        "approver-1",
        &approvers[1],
        &enrollment,
        &policy,
        PopApprovalDecisionV1::Approve,
    );
    assert_eq!(
        service.record_approval(revoked, 20),
        Err(PopCredentialServiceError::SignerRevoked)
    );
}
#[test]
fn approval_policy_rejects_duplicate_keys_under_distinct_ids() {
    let signer = TestSigner {
        key_id: "software://sorafs/pop-credentials/primary".to_owned(),
        keypair: ed25519(1),
    };
    let approvers = vec![ed25519(2), ed25519(3)];
    let mut policy = policy(&signer, &approvers);
    policy.approval_signers[1].public_key = policy.approval_signers[0].public_key;
    assert_eq!(
        policy.validate(),
        Err(PopCredentialServiceError::InvalidInput {
            field: "approval_signer_public_key"
        })
    );
}
#[derive(Debug)]
struct FailingSubmitter;
impl PopRegistrySubmitter for FailingSubmitter {
    fn submit(
        &self,
        _idempotency_key: [u8; 32],
        _operation: &PopRegistryOperationV1,
    ) -> Result<(), String> {
        Err("private upstream details".to_owned())
    }
}
#[test]
fn retry_exhaustion_is_durable_and_payload_free() {
    let (_temp, mut service, policy, _wallet, _approvers, _enrollment) = service_fixture();
    let signer = TestSigner {
        key_id: policy.issuer_signer_handle.clone(),
        keypair: ed25519(1),
    };
    let revocations = sign_revocation_with_signer(
        unsigned_revocations(signer.public_key(), scalar(101), 1),
        &signer,
    )
    .expect("signed revocations");
    let operation =
        PopRegistryOperationV1::new(PopRegistryOperationKindV1::PublishRevocationList {
            canonical_revocation_list: encode_canonical(&revocations).unwrap(),
            issuer_policy_digest: policy.issuer_policy_digest,
        })
        .expect("operation envelope");
    assert_pop_frame(
        &operation,
        "sorafs_node::pop_credentials::PopRegistryOperationV1",
    );
    let digest = operation.operation_digest;
    service
        .transact(|state| {
            state.next_outbox_sequence = 2;
            state.outbox.push(PopRegistryOutboxEntryV1 {
                sequence: 1,
                idempotency_key: registry_idempotency_key(1, digest),
                operation,
                accepted_once: false,
                attempt_count: 0,
                last_attempt_epoch: None,
            });
            Ok(())
        })
        .unwrap();
    assert_eq!(
        service.submit_next(&FailingSubmitter, 30),
        Ok(PopOutboxSubmitOutcomeV1::RetryScheduled {
            operation_digest: digest
        })
    );
    assert_eq!(
        service.submit_next(&FailingSubmitter, 31),
        Ok(PopOutboxSubmitOutcomeV1::DeadLettered {
            operation_digest: digest
        })
    );
    assert!(service.state.outbox.is_empty());
    assert_eq!(service.state.dead_letters.len(), 1);
    let checkpoint = fs::read(&service.checkpoint_path).unwrap();
    assert!(
        !checkpoint
            .windows(b"private upstream details".len())
            .any(|window| window == b"private upstream details")
    );
}
#[test]
fn semantically_poisoned_checkpoint_policy_binding_fails_closed() {
    let (temp, mut service, policy, _wallet, _approvers, enrollment) = service_fixture();
    service
        .submit_enrollment(&encode_canonical(&enrollment).unwrap(), 20)
        .unwrap();
    let enrollment_recipient = Arc::clone(&service.enrollment_recipient);
    let signer = Arc::clone(&service.signer);
    let checkpoint_path = service.checkpoint_path.clone();
    let mut poisoned = service.state.clone();
    let record = poisoned.enrollments.first_mut().unwrap();
    let mut envelope: PopEncryptedEnrollmentV1 = decode_canonical(
        &record.canonical_encrypted_enrollment,
        POP_ENCRYPTED_ENROLLMENT_MAX_BYTES_V1 as u64,
        POP_SERVICE_COLLECTION_MAX_V1,
    )
    .unwrap();
    envelope.issuer_id = "substituted-issuer".to_owned();
    record.canonical_encrypted_enrollment = encode_canonical(&envelope).unwrap();
    record.envelope_digest = envelope.digest().unwrap();
    let poisoned_bytes = encode_canonical(&poisoned).unwrap();
    drop(service);
    write_local_private_checkpoint_atomic(&checkpoint_path, &poisoned_bytes).unwrap();
    assert_eq!(
        PopCredentialService::open(
            canonical_temp_root(&temp),
            policy,
            enrollment_recipient,
            signer,
        )
        .expect_err("semantic poison"),
        PopCredentialServiceError::PoisonedCheckpoint
    );
}
#[test]
fn poisoned_checkpoint_and_symlink_target_fail_closed() {
    let (temp, service, policy, _wallet, _approvers, _enrollment) = service_fixture();
    let checkpoint = service.checkpoint_path.clone();
    drop(service);
    fs::write(&checkpoint, b"not norito").expect("poison");
    let mut rng = ChaCha20Rng::from_seed([0x22; 32]);
    let issuer_encryption = HybridKeyPair::generate(&mut rng).expect("key");
    let signer = Arc::new(TestSigner {
        key_id: policy.issuer_signer_handle.clone(),
        keypair: ed25519(1),
    });
    assert_eq!(
        PopCredentialService::open(
            canonical_temp_root(&temp),
            policy.clone(),
            test_recipient(&policy.enrollment_recipient_key_id, &issuer_encryption),
            signer.clone(),
        )
        .expect_err("poisoned"),
        PopCredentialServiceError::PoisonedCheckpoint
    );
    fs::remove_file(&checkpoint).expect("remove poison");
    let outside = temp.path().join("outside");
    fs::write(&outside, b"sentinel").expect("outside");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &checkpoint).expect("symlink");
    #[cfg(unix)]
    assert_eq!(
        PopCredentialService::open(
            canonical_temp_root(&temp),
            policy.clone(),
            test_recipient(&policy.enrollment_recipient_key_id, &issuer_encryption),
            signer,
        )
        .expect_err("symlink"),
        PopCredentialServiceError::CheckpointIo
    );
    assert_eq!(fs::read(outside).unwrap(), b"sentinel");
}
