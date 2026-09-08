// The issuer stores canonical frames; these fixtures do not qualify an external runtime provider.

#[test]
fn issuer_enrollment_and_checkpoint_frames_survive_every_caller_layout() {
    type Snapshot = (Vec<u8>, Vec<u8>, [u8; 32], [u8; 32], Vec<u8>, Vec<u8>);
    let (_initial_temp, initial, policy, _wallet, approvers, enrollment) = service_fixture();
    let signer = Arc::clone(&initial.signer);
    let enrollment_recipient = Arc::clone(&initial.enrollment_recipient);
    drop(initial);
    let mut expected: Option<Snapshot> = None;
    let mut checked_layouts = 0;
    let mut alternate_frames = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        checked_layouts += 1;
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        // Reuse one genuinely encrypted enrollment: hybrid ML-KEM hedges caller RNG seeds with
        // live OS entropy. Check identical input through each real service/checkpoint lifecycle.
        let temp = TempDir::new().unwrap();
        let mut service = PopCredentialService::open(
            canonical_temp_root(&temp),
            policy.clone(),
            Arc::clone(&enrollment_recipient),
            Arc::clone(&signer),
        )
        .unwrap();
        let enrollment_bytes = norito::encode_canonical(&enrollment).unwrap();
        assert_eq!(encode_canonical(&enrollment).unwrap(), enrollment_bytes);
        service.submit_enrollment(&enrollment_bytes, 20).unwrap();
        let signed_approval = approval(
            "approver-0",
            &approvers[0],
            &enrollment,
            &policy,
            PopApprovalDecisionV1::Approve,
        );
        let mut signable = signed_approval.clone();
        signable.signature.clear();
        let expected_digest = digest_domain(
            APPROVAL_SIGNATURE_DOMAIN_V1,
            &norito::encode_canonical(&signable).unwrap(),
        );
        assert_eq!(signed_approval.signature_digest().unwrap(), expected_digest);
        service
            .record_approval(signed_approval.clone(), 20)
            .unwrap();
        let nullifier = scalar(77);
        service.consume_verified_nullifier(nullifier).unwrap();
        let checkpoint_path = service.checkpoint_path.clone();
        let retained = service.state.clone();
        let retained_bytes = fs::read(&checkpoint_path).unwrap();
        assert_eq!(retained_bytes, norito::encode_canonical(&retained).unwrap());
        let snapshot = (
            enrollment_bytes.clone(),
            enrollment.aad().unwrap(),
            enrollment.digest().unwrap(),
            expected_digest,
            signed_approval.signature.clone(),
            retained_bytes.clone(),
        );
        if let Some(expected) = &expected {
            for (component, actual, previous) in [
                (
                    "enrollment frame",
                    snapshot.0.as_slice(),
                    expected.0.as_slice(),
                ),
                (
                    "enrollment AAD",
                    snapshot.1.as_slice(),
                    expected.1.as_slice(),
                ),
                (
                    "enrollment digest",
                    snapshot.2.as_slice(),
                    expected.2.as_slice(),
                ),
                (
                    "approval digest",
                    snapshot.3.as_slice(),
                    expected.3.as_slice(),
                ),
                (
                    "approval signature",
                    snapshot.4.as_slice(),
                    expected.4.as_slice(),
                ),
                (
                    "checkpoint frame",
                    snapshot.5.as_slice(),
                    expected.5.as_slice(),
                ),
            ] {
                assert!(
                    actual == previous,
                    "{component}, caller {flags:#04x}: lengths {} vs {}, BLAKE3 {} vs {}, first difference {:?}",
                    actual.len(),
                    previous.len(),
                    blake3::hash(actual),
                    blake3::hash(previous),
                    actual
                        .iter()
                        .zip(previous)
                        .position(|(left, right)| left != right),
                );
            }
        } else {
            expected = Some(snapshot);
        }
        let mut conflicting = enrollment.clone();
        conflicting.encrypted_payload.ciphertext[0] ^= 1;
        assert_eq!(
            service.submit_enrollment(&encode_canonical(&conflicting).unwrap(), 21),
            Err(PopCredentialServiceError::EnrollmentReplay),
        );
        assert_eq!(
            service.record_approval(signed_approval, 21),
            Err(PopCredentialServiceError::DuplicateApproval),
        );
        assert_eq!(
            service.consume_verified_nullifier(nullifier),
            Err(PopCredentialServiceError::ReplayedProof),
        );
        assert_eq!(service.state, retained);
        assert_eq!(fs::read(&checkpoint_path).unwrap(), retained_bytes);
        let enrollment_recipient = Arc::clone(&service.enrollment_recipient);
        let signer = Arc::clone(&service.signer);
        drop(service);
        let open = || {
            PopCredentialService::open(
                canonical_temp_root(&temp),
                policy.clone(),
                Arc::clone(&enrollment_recipient),
                Arc::clone(&signer),
            )
        };
        let mut restored = {
            let _reader =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            open().expect("canonical checkpoint opens independently of the writer's layout")
        };
        assert_eq!(restored.state, retained);
        assert_eq!(
            restored.consume_verified_nullifier(nullifier),
            Err(PopCredentialServiceError::ReplayedProof),
        );
        assert_eq!(fs::read(&checkpoint_path).unwrap(), retained_bytes);
        drop(restored);

        let alternate = norito::to_bytes(&retained).unwrap();
        if alternate != retained_bytes {
            alternate_frames += 1;
            assert_eq!(
                norito::decode_from_bytes::<PopIssuerCheckpointV1>(&alternate).unwrap(),
                retained,
            );
            write_local_private_checkpoint_atomic(&checkpoint_path, &alternate).unwrap();
            assert_eq!(
                open().unwrap_err(),
                PopCredentialServiceError::PoisonedCheckpoint
            );
            assert_eq!(fs::read(&checkpoint_path).unwrap(), alternate);
        }
        let mut substituted = retained.clone();
        substituted.issuer_policy_digest[0] ^= 1;
        let substituted_bytes = norito::encode_canonical(&substituted).unwrap();
        write_local_private_checkpoint_atomic(&checkpoint_path, &substituted_bytes).unwrap();
        assert_eq!(
            open().unwrap_err(),
            PopCredentialServiceError::PoisonedCheckpoint
        );
        assert_eq!(fs::read(&checkpoint_path).unwrap(), substituted_bytes);
        write_local_private_checkpoint_atomic(&checkpoint_path, &retained_bytes).unwrap();
        assert_eq!(open().unwrap().state, retained);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(checked_layouts, 10);
    assert!(alternate_frames > 0);
}

#[test]
fn enrollment_encryption_uses_canonical_private_payload_and_aad_under_every_caller_layout() {
    let (_temp, _service, policy, wallet, _approvers, enrollment) = service_fixture();
    let private = private_enrollment(&wallet);
    let mut expected_plaintext = norito::encode_canonical(&private).unwrap();
    let expected_plaintext = SensitiveBytesGuard::new(&mut expected_plaintext);
    let metadata = PopEnrollmentAadV1 {
        version: POP_ENCRYPTED_ENROLLMENT_VERSION_V1,
        request_id: private.request.request_id,
        issuer_policy_digest: policy.issuer_policy_digest,
        issuer_id: policy.issuer_id.clone(),
        recipient_key_id: policy.enrollment_recipient_key_id.clone(),
    };
    let canonical_metadata = norito::encode_canonical(&metadata).unwrap();
    let expected_aad = [ENROLLMENT_AAD_DOMAIN_V1, canonical_metadata.as_slice()].concat();
    assert_eq!(enrollment.aad().unwrap(), expected_aad);
    let mut recipient_rng = ChaCha20Rng::from_seed([0x45; 32]);
    let recipient = HybridKeyPair::generate(&mut recipient_rng).unwrap();
    let mut checked_layouts = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        checked_layouts += 1;
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let mut rng = ChaCha20Rng::from_seed([0x46; 32]);
        let encrypted =
            encrypt_pop_enrollment_v1(&private, &policy, recipient.public(), &mut rng).unwrap();
        assert_eq!(encrypted.aad().unwrap(), expected_aad);
        // Fresh ML-KEM entropy changes ciphertext, while opening it with independently framed
        // AAD must recover these exact private bytes, not a caller-layout encoding of the value.
        let mut opened = decrypt_payload(
            &encrypted.encrypted_payload,
            &expected_aad,
            recipient.secret(),
        )
        .unwrap();
        let opened = SensitiveBytesGuard::new(&mut opened);
        assert!(
            opened.as_slice() == expected_plaintext.as_slice(),
            "private enrollment frame differs under caller {flags:#04x}",
        );
        let mut wrong_aad = expected_aad.clone();
        wrong_aad[0] ^= 1;
        assert!(matches!(
            decrypt_payload(&encrypted.encrypted_payload, &wrong_aad, recipient.secret()),
            Err(sorafs_manifest::hybrid_envelope::HybridEnvelopeError::AeadFailure),
        ));
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(checked_layouts, 10);
}

#[test]
fn nullifier_replay_cache_is_atomic_and_survives_restart() {
    let (temp, mut service, policy, _wallet, _approvers, _enrollment) = service_fixture();
    let enrollment_recipient = Arc::clone(&service.enrollment_recipient);
    let signer = Arc::clone(&service.signer);
    let nullifier = scalar(77);
    service
        .consume_verified_nullifier(nullifier)
        .expect("first consumption");
    assert_eq!(
        service.consume_verified_nullifier(nullifier),
        Err(PopCredentialServiceError::ReplayedProof)
    );
    drop(service);
    let mut restored = PopCredentialService::open(
        canonical_temp_root(&temp),
        policy,
        enrollment_recipient,
        signer,
    )
    .unwrap();
    assert_eq!(
        restored.consume_verified_nullifier(nullifier),
        Err(PopCredentialServiceError::ReplayedProof)
    );
}
