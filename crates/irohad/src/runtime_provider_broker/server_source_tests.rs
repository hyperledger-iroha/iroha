#[test]
fn pop_broker_sources_never_export_recipient_private_keys() {
    let broker = include_str!("../runtime_provider_broker.rs");
    let protocol = include_str!("protocol_primitives.rs");
    let recipient_client = include_str!("pop_recipient_client.rs");
    let joined = [broker, protocol, recipient_client].join("\n");
    for forbidden in [
        "PopCredentialRuntimeSecretsV1",
        "PopRuntimeResolveResultWireV1",
        "OPERATION_POP_RUNTIME_RESOLVE_V1",
        "enrollment_x25519_secret",
        "enrollment_mlkem_secret",
        "wallet_x25519_secret",
        "wallet_mlkem_secret",
        "HybridSecretKey::from_bytes",
    ] {
        assert!(
            !joined.contains(forbidden),
            "PoP broker sources must not contain retired private-key wire marker `{forbidden}`"
        );
    }
    for required in [
        "OPERATION_POP_RUNTIME_OPEN_V1",
        "OPERATION_POP_ENROLLMENT_RECIPIENT_OPEN_V1",
        "OPERATION_POP_WALLET_RECIPIENT_OPEN_V1",
        "PopBrokerEnrollmentRecipient",
        "PopBrokerWalletRecipient",
        "enrollment_recipient_public_key_digest",
        "wallet_recipient_public_key_digest",
    ] {
        assert!(
            joined.contains(required),
            "PoP broker sources must contain hard-cut capability marker `{required}`"
        );
    }
}
#[test]
fn pop_broker_preserves_caller_signed_mutation_authority() {
    let mut request = PopAuthenticateRequestWireV1 {
        opaque_credential: b"caller assertion".to_vec(),
        action: pop_action_to_wire(
            sorafs_node::pop_credentials::PopCredentialApiActionV1::SubmitEnrollment,
        ),
        request_binding: [0x31; 32],
        now_epoch: 100,
    };
    let authenticated_only = PopAuthenticatedPrincipalWireV1 {
        principal_digest: [0x32; 32],
        expires_at_epoch: 101,
        caller_signed_transaction: false,
    };
    assert_eq!(
        validate_pop_principal(authenticated_only, &request),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        validate_pop_principal(
            PopAuthenticatedPrincipalWireV1 {
                caller_signed_transaction: true,
                ..authenticated_only
            },
            &request,
        ),
        Ok(())
    );
    request.action = pop_action_to_wire(
        sorafs_node::pop_credentials::PopCredentialApiActionV1::ReadEnrollmentStatus,
    );
    assert_eq!(validate_pop_principal(authenticated_only, &request), Ok(()));
    request.action = pop_action_to_wire(
        sorafs_node::pop_credentials::PopCredentialApiActionV1::ReconcileRegistry,
    );
    assert_eq!(validate_pop_principal(authenticated_only, &request), Ok(()));
}
#[test]
fn production_unary_binding_caps_accept_defaults_and_reject_cap_plus_one() {
    let checkpoint_public_key =
        iroha_crypto::PublicKey::from_bytes(iroha_crypto::Algorithm::Ed25519, &TEST_SIGNER_KEY)
            .expect("construct checkpoint Ed25519 public key");
    let mut appeal = plain_runtime_binding(
        IrohaRuntimeProviderSlotV1::AppealFinanceCheckpoint,
        "sealed://sorafs/appeal-finance/checkpoint-primary",
    );
    appeal.appeal_finance_checkpoint_binding = Some(AppealFinanceCheckpointBindingWireV1 {
        public_key: checkpoint_public_key,
    });
    appeal.appeal_finance_checkpoint_max_bytes =
        Some(MAX_BROKER_APPEAL_FINANCE_CHECKPOINT_BYTES_V1 as u64);
    assert_eq!(validate_wire_binding(&appeal), Ok(()));
    let mut appeal_too_large = appeal;
    appeal_too_large.appeal_finance_checkpoint_max_bytes =
        Some(MAX_BROKER_APPEAL_FINANCE_CHECKPOINT_BYTES_V1 as u64 + 1);
    assert_eq!(
        validate_wire_binding(&appeal_too_large),
        Err(BrokerError::BindingMismatch)
    );
    let mut provider_checkpoint = plain_runtime_binding(
        IrohaRuntimeProviderSlotV1::ProviderIngestCheckpointStore,
        "sealed://sorafs/provider-ingest/checkpoint-primary",
    );
    provider_checkpoint.provider_ingest_checkpoint_max_bytes =
        Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES.0);
    assert_eq!(validate_wire_binding(&provider_checkpoint), Ok(()));
    provider_checkpoint.provider_ingest_checkpoint_max_bytes =
        Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT);
    assert_eq!(validate_wire_binding(&provider_checkpoint), Ok(()));
    provider_checkpoint.provider_ingest_checkpoint_max_bytes =
        Some(provider_ingest_outbox_defaults::CHECKPOINT_MAX_BYTES_LIMIT + 1);
    assert_eq!(
        validate_wire_binding(&provider_checkpoint),
        Err(BrokerError::BindingMismatch)
    );
    let signer_details = ProviderIngestSignerBindingWireV1 {
        runtime_handle: "software://sorafs/provider-ingest/signer-primary".to_owned(),
        adapter_revision: 3,
        signer_policy_id: [0xA1; 32],
        signer_policy_revision: 1,
        signer_policy_predecessor_digest: None,
        signer_policy_digest: [0xA2; 32],
        algorithm: 1,
        public_key: TEST_SIGNER_KEY.to_vec(),
    };
    let mut provider_signer = plain_runtime_binding(
        IrohaRuntimeProviderSlotV1::ProviderIngestCompletionSigner,
        &signer_details.runtime_handle,
    );
    provider_signer.revision = Some(signer_details.adapter_revision);
    provider_signer.policy_digest = Some(signer_details.signer_policy_digest);
    provider_signer.provider_ingest_signer_binding = Some(signer_details);
    provider_signer.provider_ingest_max_signed_transaction_bytes =
        Some(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT);
    assert_eq!(validate_wire_binding(&provider_signer), Ok(()));
    provider_signer.provider_ingest_max_signed_transaction_bytes =
        Some(provider_ingest_outbox_defaults::MAX_SIGNED_TRANSACTION_BYTES_LIMIT + 1);
    assert_eq!(
        validate_wire_binding(&provider_signer),
        Err(BrokerError::BindingMismatch)
    );
    let mut evidence_checkpoint =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::EvidenceViewerCheckpointStore);
    assert_eq!(validate_wire_binding(&evidence_checkpoint), Ok(()));
    evidence_checkpoint.evidence_viewer_checkpoint_max_bytes =
        Some(MAX_EVIDENCE_VIEWER_CHECKPOINT_BYTES_V1 as u64 + 1);
    assert_eq!(
        validate_wire_binding(&evidence_checkpoint),
        Err(BrokerError::BindingMismatch)
    );
    let mut evidence_archive =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::EvidenceViewerCompactionArchive);
    assert_eq!(validate_wire_binding(&evidence_archive), Ok(()));
    evidence_archive.evidence_viewer_archive_max_bytes =
        Some(MAX_BROKER_EVIDENCE_VIEWER_BULK_BYTES_V1 as u64 + 1);
    assert_eq!(
        validate_wire_binding(&evidence_archive),
        Err(BrokerError::BindingMismatch)
    );
    let mut evidence_publisher =
        evidence_viewer_binding(IrohaRuntimeProviderSlotV1::EvidenceViewerTransparencyPublisher);
    assert_eq!(validate_wire_binding(&evidence_publisher), Ok(()));
    evidence_publisher.evidence_viewer_transparency_publisher_public_key = None;
    assert_eq!(
        validate_wire_binding(&evidence_publisher),
        Err(BrokerError::BindingMismatch)
    );
    assert_eq!(
        validate_provider_ingest_account_canonical_bytes(&vec![
            0xA5;
            MAX_PROVIDER_INGEST_ACCOUNT_BYTES_V1
        ]),
        Ok(())
    );
    assert_eq!(
        validate_provider_ingest_account_canonical_bytes(&vec![
            0xA5;
            MAX_PROVIDER_INGEST_ACCOUNT_BYTES_V1
                + 1
        ]),
        Err(BrokerError::Rejected)
    );
}
#[test]
fn broker_instance_lock_records_atomic_marker_provenance() {
    let directory = tempfile::tempdir().expect("create lock provenance directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden lock provenance directory");
    let parent = fs::File::open(directory.path()).expect("open lock provenance parent");
    let uid = rustix::process::geteuid().as_raw();
    let first = endpoint_recovery::InstanceLockGuard::acquire(&parent, uid)
        .expect("exclusively create first lock marker");
    assert!(!first.marker_preexisted());
    drop(first);
    let second = endpoint_recovery::InstanceLockGuard::acquire(&parent, uid)
        .expect("open persisted lock marker");
    assert!(second.marker_preexisted());
}
#[test]
fn broker_server_preserves_active_listener_without_lock_or_readiness() {
    let (_directory, path, policy, listener) = bind_fake_broker();
    let original_identity = endpoint_identity(&policy).expect("capture unmarked listener identity");
    let marker = path
        .parent()
        .expect("broker parent")
        .join(".runtime-provider-broker-v1.lock");
    for attempt in 0..2 {
        let ready = AtomicBool::new(false);
        assert_eq!(
            serve_with_policy_and_lifecycle(
                &IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain"),
                RuntimeProviderBrokerBackendsV1::new(),
                &policy,
                Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
                || ready.store(true, Ordering::Release),
            ),
            Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
        );
        assert!(
            !ready.load(Ordering::Acquire),
            "attempt {attempt} stays unready"
        );
        assert_eq!(
            endpoint_identity(&policy).expect("unmarked listener remains"),
            original_identity
        );
        assert!(
            !marker.exists(),
            "failed attempt {attempt} must not turn an unmarked listener into recoverable state"
        );
    }
    drop(listener);
}
#[test]
fn broker_server_rejects_active_locked_socket_without_unlinking_it() {
    let (_directory, path, policy, listener) = bind_fake_broker();
    let _active_lock = hold_instance_lock(&policy);
    let before = endpoint_identity(&policy).expect("capture existing socket identity");
    let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    assert_eq!(
        serve_with_policy(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            Arc::clone(&lifecycle),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    assert!(
        lifecycle.shutdown_requested(),
        "every startup failure moves the lifecycle to stopping"
    );
    assert_eq!(
        endpoint_identity(&policy).expect("existing socket remains"),
        before
    );
    assert!(path.exists());
    drop(listener);
}
#[test]
fn broker_server_recovers_exact_stale_socket_after_unclean_exit() {
    let directory = tempfile::tempdir().expect("create stale broker directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden stale broker directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    seed_instance_lock_marker(&policy);
    let stale = UnixListener::bind(&path).expect("bind stale broker socket");
    set_socket_mode(&path).expect("harden stale broker socket");
    let stale_identity = endpoint_identity(&policy).expect("capture stale socket identity");
    drop(stale);
    let server_policy = policy.clone();
    let ready_policy = policy.clone();
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        serve_with_policy_and_lifecycle(
            &IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain"),
            RuntimeProviderBrokerBackendsV1::new(),
            &server_policy,
            server_lifecycle,
            move || {
                ready_sender
                    .send(endpoint_identity(&ready_policy))
                    .expect("publish recovered broker identity");
            },
        )
    });
    let recovered_identity = ready_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("recovered broker becomes ready")
        .expect("recovered endpoint is exact");
    assert_ne!(recovered_identity, stale_identity);
    lifecycle.request_shutdown();
    server
        .join()
        .expect("join recovered broker")
        .expect("recovered broker exits cleanly");
    assert!(!path.exists());
}
#[test]
fn broker_server_preserves_non_socket_symlink_and_wrong_mode_entries() {
    let directory = tempfile::tempdir().expect("create rejected endpoint directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden rejected endpoint directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
    fs::write(&path, b"not-a-socket").expect("write regular endpoint substitution");
    assert_eq!(
        serve_with_policy(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    assert_eq!(
        fs::read(&path).expect("regular substitution remains"),
        b"not-a-socket"
    );
    fs::remove_file(&path).expect("remove regular substitution");
    let target = path.with_extension("target");
    fs::write(&target, b"target").expect("write symlink target");
    symlink(&target, &path).expect("create endpoint symlink");
    assert_eq!(
        serve_with_policy(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    assert!(
        fs::symlink_metadata(&path)
            .expect("symlink remains")
            .file_type()
            .is_symlink()
    );
    fs::remove_file(&path).expect("remove endpoint symlink");
    let wrong_mode = UnixListener::bind(&path).expect("bind wrong-mode socket");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("set wrong socket mode");
    drop(wrong_mode);
    assert_eq!(
        serve_with_policy(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    assert!(path.exists(), "wrong-mode socket remains for inspection");
    fs::remove_file(&path).expect("remove wrong-mode socket");
}
#[test]
fn stale_socket_recovery_detects_identity_substitution_before_unlink() {
    use std::cell::RefCell;
    let directory = tempfile::tempdir().expect("create recovery race directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden recovery race directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    seed_instance_lock_marker(&policy);
    let stale = UnixListener::bind(&path).expect("bind stale race socket");
    set_socket_mode(&path).expect("harden stale race socket");
    drop(stale);
    let parent = fs::File::open(directory.path()).expect("open recovery race parent");
    let guard = endpoint_recovery::InstanceLockGuard::acquire(&parent, policy.expected_service_uid)
        .expect("acquire recovery race lock");
    let replacement = RefCell::new(None);
    assert_eq!(
        endpoint_recovery::recover_stale_endpoint_with_probe(
            &parent,
            path.file_name().expect("socket name"),
            policy.expected_service_uid,
            policy.socket_mode,
            &guard,
            || {
                fs::remove_file(&path).expect("remove observed stale socket");
                let listener = UnixListener::bind(&path).expect("bind replacement socket");
                set_socket_mode(&path).expect("harden replacement socket");
                *replacement.borrow_mut() = Some(listener);
            },
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    assert!(replacement.borrow().is_some());
    assert!(
        endpoint_identity(&policy).is_ok(),
        "replacement remains pinned"
    );
    drop(replacement);
    fs::remove_file(&path).expect("remove preserved replacement");
}
#[test]
fn orderly_cleanup_quarantines_before_detecting_identity_substitution() {
    use std::cell::RefCell;
    let directory = tempfile::tempdir().expect("create cleanup race directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden cleanup race directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    seed_instance_lock_marker(&policy);
    let original = UnixListener::bind(&path).expect("bind cleanup race socket");
    set_socket_mode(&path).expect("harden cleanup race socket");
    let original_identity = endpoint_identity(&policy).expect("capture cleanup identity");
    let parent = fs::File::open(directory.path()).expect("open cleanup race parent");
    let guard = endpoint_recovery::InstanceLockGuard::acquire(&parent, policy.expected_service_uid)
        .expect("acquire cleanup race lock");
    let replacement = RefCell::new(None);
    assert_eq!(
        endpoint_recovery::cleanup_socket_entry_with_probe(
            &parent,
            path.file_name().expect("socket name"),
            original_identity,
            policy.expected_service_uid,
            policy.socket_mode,
            &guard,
            || {
                fs::remove_file(&path).expect("remove observed cleanup socket");
                let listener = UnixListener::bind(&path).expect("bind cleanup replacement");
                set_socket_mode(&path).expect("harden cleanup replacement");
                *replacement.borrow_mut() = Some(listener);
            },
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)
    );
    let replacement_identity = endpoint_identity(&policy).expect("replacement is restored");
    assert_ne!(replacement_identity, original_identity);
    assert!(replacement.borrow().is_some());
    drop(original);
    drop(replacement);
    fs::remove_file(&path).expect("remove restored cleanup replacement");
}
#[test]
fn broker_endpoint_rejects_socket_hardlink_alias_without_removal() {
    let directory = tempfile::tempdir().expect("create hardlink regression directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden hardlink regression directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let alias = directory.path().join("runtime-provider-broker-v1.alias");
    let policy = EndpointPolicy::for_test(path.clone());
    seed_instance_lock_marker(&policy);
    let listener = UnixListener::bind(&path).expect("bind hardlinked broker socket");
    set_socket_mode(&path).expect("harden hardlinked broker socket");
    fs::hard_link(&path, &alias).expect("create broker socket hardlink alias");
    let original = fs::symlink_metadata(&path).expect("inspect hardlinked endpoint");
    assert_eq!(original.nlink(), 2);
    assert_eq!(endpoint_identity(&policy), Err(BrokerError::Unavailable));
    assert_eq!(
        serve_with_policy_and_lifecycle(
            &IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain"),
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
            || panic!("hardlinked endpoint must never become ready"),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    let canonical_after = fs::symlink_metadata(&path).expect("canonical hardlink remains");
    let alias_after = fs::symlink_metadata(&alias).expect("socket alias remains");
    assert_eq!(canonical_after.ino(), original.ino());
    assert_eq!(alias_after.ino(), original.ino());
    assert_eq!(canonical_after.nlink(), 2);
    drop(listener);
    fs::remove_file(&alias).expect("remove socket alias");
    fs::remove_file(&path).expect("remove canonical socket");
}
#[test]
fn broker_server_readiness_follows_qualification_and_secure_bind() {
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let server_policy = policy.clone();
    let ready_policy = policy.clone();
    let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        serve_with_policy_and_lifecycle(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &server_policy,
            server_lifecycle,
            move || {
                ready_sender
                    .send(endpoint_identity(&ready_policy))
                    .expect("publish broker readiness");
            },
        )
    });
    let ready_identity = ready_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("broker publishes readiness after secure bind")
        .expect("ready callback observes hardened endpoint");
    assert_eq!(
        endpoint_identity(&policy).expect("inspect ready endpoint"),
        ready_identity
    );
    lifecycle.request_shutdown();
    server
        .join()
        .expect("join ready broker server")
        .expect("ready broker server exits cleanly");
    assert!(!path.exists(), "orderly shutdown removes the bound socket");
    let rejected_path = directory.path().join("rejected-runtime-provider.sock");
    let rejected_policy = EndpointPolicy::for_test(rejected_path.clone());
    let rejected_lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let rejected_ready = AtomicBool::new(false);
    assert_eq!(
        serve_with_policy_and_lifecycle(
            &server_test_catalog(),
            RuntimeProviderBrokerBackendsV1::new(),
            &rejected_policy,
            rejected_lifecycle,
            || rejected_ready.store(true, Ordering::Release),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    );
    assert!(
        !rejected_ready.load(Ordering::Acquire),
        "incomplete backend qualification must fail before readiness"
    );
    assert!(
        !rejected_path.exists(),
        "qualification failure must precede endpoint creation"
    );
}
#[test]
fn broker_server_readiness_failure_stops_before_accept_and_cleans_endpoint() {
    let directory = tempfile::tempdir().expect("create failed-readiness server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden failed-readiness server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let callback_lifecycle = Arc::clone(&lifecycle);
    let callback_invoked = AtomicBool::new(false);
    assert_eq!(
        serve_with_policy_and_fallible_readiness(
            &IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain"),
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            Arc::clone(&lifecycle),
            || {
                callback_invoked.store(true, Ordering::Release);
                endpoint_identity(&policy).expect("callback observes the secured endpoint");
                assert!(
                    callback_lifecycle.try_begin_operation().is_none(),
                    "the lifecycle must remain starting while readiness publication runs"
                );
                Err(RuntimeProviderBrokerReadinessErrorV1)
            },
        ),
        Err(RuntimeProviderBrokerServerErrorV1::ReadinessUnavailable)
    );
    assert!(callback_invoked.load(Ordering::Acquire));
    assert!(
        lifecycle.shutdown_requested(),
        "failed readiness publication must move the lifecycle to stopping"
    );
    assert_eq!(lifecycle.active_provider_call_count(), 0);
    assert!(
        !path.exists(),
        "failed readiness publication must remove the bound endpoint"
    );
    assert!(
        UnixStream::connect(&path).is_err(),
        "no client can enter an accept loop after readiness publication fails"
    );
}
#[test]
fn unauthorized_peer_rejection_is_connection_local() {
    let directory = tempfile::tempdir().expect("create peer-authorization server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden peer-authorization server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let bindings = server_test_catalog();
    let server_bindings = bindings.clone();
    let server_policy = policy.clone();
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let authorization_attempts = Arc::new(AtomicUsize::new(0));
    let server_authorization_attempts = Arc::clone(&authorization_attempts);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let (rejected_sender, rejected_receiver) = mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        serve_with_policy_and_fallible_readiness_and_peer_authorizer(
            &server_bindings,
            server_test_backends(),
            &server_policy,
            server_lifecycle,
            move || {
                ready_sender
                    .send(())
                    .expect("publish peer-authorization readiness");
                Ok(())
            },
            move |observed_uid, expected_uid| {
                if server_authorization_attempts.fetch_add(1, Ordering::AcqRel) == 0 {
                    rejected_sender
                        .send(())
                        .expect("publish injected peer rejection");
                    Err(BrokerError::Unavailable)
                } else {
                    verify_peer_uid(observed_uid, expected_uid)
                }
            },
        )
    });
    ready_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("peer-authorization broker becomes ready");

    let rejected = UnixStream::connect(&path).expect("connect injected unauthorized peer");
    rejected_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("broker rejects the first peer before the authorized connection");
    drop(rejected);
    let (authorized, observations) = BrokerSession::connect(
        &policy,
        bindings.chain_id(),
        *bindings.network_id(),
        vec![signer_binding_for_server()],
    )
    .expect("authorized peer connects after rejected peer");
    assert_eq!(observations.len(), 1);
    assert_eq!(authorization_attempts.load(Ordering::Acquire), 2);

    drop(authorized);
    lifecycle.request_shutdown();
    server
        .join()
        .expect("join peer-authorization broker")
        .expect("peer-authorization broker exits cleanly");
    assert!(!path.exists());
}
#[test]
fn broker_server_graceful_cleanup_allows_exact_endpoint_rebind() {
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    for attempt in 0..2 {
        let server_policy = policy.clone();
        let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
        let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
        let server_lifecycle = Arc::clone(&lifecycle);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            serve_with_policy_and_lifecycle(
                &bindings,
                RuntimeProviderBrokerBackendsV1::new(),
                &server_policy,
                server_lifecycle,
                move || ready_sender.send(()).expect("publish broker readiness"),
            )
        });
        ready_receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|error| {
                panic!("broker rebind attempt {attempt} did not become ready: {error}")
            });
        assert!(
            endpoint_identity(&policy).is_ok(),
            "rebind attempt {attempt} owns the hardened endpoint"
        );
        lifecycle.request_shutdown();
        server
            .join()
            .expect("join rebound broker server")
            .unwrap_or_else(|error| panic!("broker rebind attempt {attempt} failed: {error}"));
        assert!(
            !path.exists(),
            "rebind attempt {attempt} cleans its pinned socket"
        );
    }
}
#[test]
fn broker_server_never_signals_ready_for_existing_endpoint() {
    let (_directory, path, policy, listener) = bind_fake_broker();
    let _active_lock = hold_instance_lock(&policy);
    let original_identity = endpoint_identity(&policy).expect("capture existing socket identity");
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let existing_ready = AtomicBool::new(false);
    let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
    assert_eq!(
        serve_with_policy_and_lifecycle(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            lifecycle,
            || existing_ready.store(true, Ordering::Release),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointUnavailable)
    );
    assert!(
        !existing_ready.load(Ordering::Acquire),
        "an existing endpoint must fail before readiness"
    );
    assert_eq!(
        endpoint_identity(&policy).expect("existing endpoint remains pinned"),
        original_identity
    );
    assert!(path.exists());
    drop(listener);
}
#[test]
fn broker_server_never_signals_ready_for_endpoint_substituted_during_requalification() {
    #[derive(Debug)]
    struct BlockingReadySigner {
        qualification_calls: AtomicU64,
        second_probe_entered: Arc<std::sync::Barrier>,
        release_second_probe: Arc<std::sync::Barrier>,
    }
    impl sorafs_node::GovernanceDagRuntimeSigner for BlockingReadySigner {
        fn handle(&self) -> &str {
            SERVER_TEST_SIGNER_HANDLE
        }
        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            // Preparing the server observation probes the signer twice before
            // the endpoint is bound; block the subsequent readiness probe.
            if self.qualification_calls.fetch_add(1, Ordering::SeqCst) == 2 {
                self.second_probe_entered.wait();
                self.release_second_probe.wait();
            }
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    7,
                    TEST_POLICY_DIGEST,
                ),
            )
        }
        fn publisher_peer_id(&self) -> &[u8] {
            b"12D3KooWRuntimeBrokerServerPrimary"
        }
        fn public_key(&self) -> [u8; 32] {
            TEST_SIGNER_KEY
        }
        fn sign(
            &self,
            _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
            _payload: &[u8],
        ) -> Result<[u8; 64], String> {
            Ok([0xA5; 64])
        }
    }
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let server_policy = policy.clone();
    let second_probe_entered = Arc::new(std::sync::Barrier::new(2));
    let release_second_probe = Arc::new(std::sync::Barrier::new(2));
    let ready = Arc::new(AtomicBool::new(false));
    let server_ready = Arc::clone(&ready);
    let (result_sender, result_receiver) = mpsc::sync_channel(1);
    let server = thread::spawn({
        let second_probe_entered = Arc::clone(&second_probe_entered);
        let release_second_probe = Arc::clone(&release_second_probe);
        move || {
            let result = serve_with_policy_and_lifecycle(
                &server_test_catalog(),
                RuntimeProviderBrokerBackendsV1::new().with_governance_dag_signer(Arc::new(
                    BlockingReadySigner {
                        qualification_calls: AtomicU64::new(0),
                        second_probe_entered,
                        release_second_probe,
                    },
                )),
                &server_policy,
                Arc::new(RuntimeProviderBrokerLifecycleV1::new()),
                move || server_ready.store(true, Ordering::Release),
            );
            result_sender
                .send(result)
                .expect("publish startup substitution result");
        }
    });
    second_probe_entered.wait();
    let original_identity = endpoint_identity(&policy).expect("inspect bound pre-ready endpoint");
    fs::remove_file(&path).expect("unlink pre-ready endpoint");
    let replacement = UnixListener::bind(&path).expect("bind pre-ready endpoint substitution");
    set_socket_mode(&path).expect("harden pre-ready endpoint substitution");
    let replacement_identity = endpoint_identity(&policy).expect("inspect pre-ready replacement");
    assert_ne!(replacement_identity, original_identity);
    release_second_probe.wait();
    assert_eq!(
        result_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("startup substitution is detected"),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)
    );
    server.join().expect("join pre-ready substituted broker");
    assert!(
        !ready.load(Ordering::Acquire),
        "endpoint substitution before the ready transition suppresses the callback"
    );
    assert_eq!(
        endpoint_identity(&policy).expect("pre-ready replacement remains"),
        replacement_identity
    );
    drop(replacement);
    fs::remove_file(&path).expect("remove pre-ready test replacement");
}
#[test]
fn broker_server_idle_loop_detects_endpoint_substitution_and_preserves_replacement() {
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let server_policy = policy.clone();
    let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    let (result_sender, result_receiver) = mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        let result = serve_with_policy_and_lifecycle(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &server_policy,
            server_lifecycle,
            move || ready_sender.send(()).expect("publish broker readiness"),
        );
        result_sender
            .send(result)
            .expect("publish substituted endpoint result");
    });
    ready_receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("broker becomes ready before substitution");
    let original_identity = endpoint_identity(&policy).expect("inspect original endpoint");
    fs::remove_file(&path).expect("unlink original broker endpoint");
    let replacement = UnixListener::bind(&path).expect("bind substituted endpoint inode");
    set_socket_mode(&path).expect("harden substituted endpoint");
    let replacement_identity = endpoint_identity(&policy).expect("inspect substituted endpoint");
    assert_ne!(replacement_identity, original_identity);
    assert_eq!(
        result_receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("idle broker detects endpoint substitution"),
        Err(RuntimeProviderBrokerServerErrorV1::EndpointCleanupFailed)
    );
    server.join().expect("join substituted endpoint broker");
    assert!(lifecycle.shutdown_requested());
    assert_eq!(
        endpoint_identity(&policy).expect("replacement endpoint remains"),
        replacement_identity,
        "cleanup must not knowingly unlink a substituted inode"
    );
    drop(replacement);
    fs::remove_file(&path).expect("remove test replacement endpoint");
}
#[test]
fn broker_server_callback_panic_still_cleans_bound_endpoint() {
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let bindings = IrohaRuntimeProviderBindingsV1::empty_for_test("server-test-chain");
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = serve_with_policy_and_lifecycle(
            &bindings,
            RuntimeProviderBrokerBackendsV1::new(),
            &policy,
            server_lifecycle,
            || panic!("ready callback panic probe"),
        );
    }));
    assert!(panic.is_err());
    assert!(lifecycle.shutdown_requested());
    assert!(
        !path.exists(),
        "bound endpoint guard must run while callback panic unwinds"
    );
}
#[test]
fn broker_server_requalifies_complete_catalog_immediately_before_ready() {
    #[derive(Debug)]
    struct DriftingReadySigner {
        qualification_calls: AtomicU64,
    }
    impl sorafs_node::GovernanceDagRuntimeSigner for DriftingReadySigner {
        fn handle(&self) -> &str {
            SERVER_TEST_SIGNER_HANDLE
        }
        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            let call = self.qualification_calls.fetch_add(1, Ordering::SeqCst);
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    if call == 0 { 7 } else { 8 },
                    TEST_POLICY_DIGEST,
                ),
            )
        }
        fn publisher_peer_id(&self) -> &[u8] {
            b"12D3KooWRuntimeBrokerServerPrimary"
        }
        fn public_key(&self) -> [u8; 32] {
            TEST_SIGNER_KEY
        }
        fn sign(
            &self,
            _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
            _payload: &[u8],
        ) -> Result<[u8; 64], String> {
            Ok([0xA5; 64])
        }
    }
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let ready = AtomicBool::new(false);
    assert_eq!(
        serve_with_policy_and_lifecycle(
            &server_test_catalog(),
            RuntimeProviderBrokerBackendsV1::new().with_governance_dag_signer(Arc::new(
                DriftingReadySigner {
                    qualification_calls: AtomicU64::new(0),
                }
            ),),
            &policy,
            lifecycle,
            || ready.store(true, Ordering::Release),
        ),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch)
    );
    assert!(!ready.load(Ordering::Acquire));
    assert!(
        !path.exists(),
        "failed second qualification cleans the endpoint before returning"
    );
}
#[test]
fn broker_server_preserves_requalification_failure_during_shutdown() {
    #[derive(Debug)]
    struct FailingReadySigner {
        qualification_calls: AtomicU64,
        second_probe_entered: Arc<std::sync::Barrier>,
        release_second_probe: Arc<std::sync::Barrier>,
    }
    impl sorafs_node::GovernanceDagRuntimeSigner for FailingReadySigner {
        fn handle(&self) -> &str {
            SERVER_TEST_SIGNER_HANDLE
        }
        fn qualification(
            &self,
        ) -> Result<sorafs_node::GovernanceDagRuntimeProviderQualificationV1, String> {
            if self.qualification_calls.fetch_add(1, Ordering::SeqCst) == 1 {
                self.second_probe_entered.wait();
                self.release_second_probe.wait();
                return Err("requalification failed after admission".to_owned());
            }
            Ok(
                sorafs_node::GovernanceDagRuntimeProviderQualificationV1::new(
                    7,
                    TEST_POLICY_DIGEST,
                ),
            )
        }
        fn publisher_peer_id(&self) -> &[u8] {
            b"12D3KooWRuntimeBrokerServerPrimary"
        }
        fn public_key(&self) -> [u8; 32] {
            TEST_SIGNER_KEY
        }
        fn sign(
            &self,
            _purpose: sorafs_node::GovernanceDagSigningPurposeV1,
            _payload: &[u8],
        ) -> Result<[u8; 64], String> {
            Ok([0xA5; 64])
        }
    }
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let server_policy = policy.clone();
    let second_probe_entered = Arc::new(std::sync::Barrier::new(2));
    let release_second_probe = Arc::new(std::sync::Barrier::new(2));
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let server_lifecycle = Arc::clone(&lifecycle);
    let ready = Arc::new(AtomicBool::new(false));
    let server_ready = Arc::clone(&ready);
    let server = thread::spawn({
        let second_probe_entered = Arc::clone(&second_probe_entered);
        let release_second_probe = Arc::clone(&release_second_probe);
        move || {
            serve_with_policy_and_lifecycle(
                &server_test_catalog(),
                RuntimeProviderBrokerBackendsV1::new().with_governance_dag_signer(Arc::new(
                    FailingReadySigner {
                        qualification_calls: AtomicU64::new(0),
                        second_probe_entered,
                        release_second_probe,
                    },
                )),
                &server_policy,
                server_lifecycle,
                move || server_ready.store(true, Ordering::Release),
            )
        }
    });
    second_probe_entered.wait();
    lifecycle.request_shutdown();
    release_second_probe.wait();
    assert_eq!(
        server.join().expect("join failed requalification server"),
        Err(RuntimeProviderBrokerServerErrorV1::BindingMismatch),
        "an admitted backend failure remains an error when shutdown races it"
    );
    assert!(!ready.load(Ordering::Acquire));
    assert!(lifecycle.shutdown_requested());
    assert!(
        !path.exists(),
        "failed requalification cleans the endpoint during shutdown"
    );
}
#[test]
fn stock_registry_projects_exact_streamed_provider_source_limits() {
    let limits = ProviderIngestSourceLimitsV1 {
        operation_timeout_ms: 30_000,
        max_content_bytes: 64 * 1024 * 1024,
        max_source_providers: 8,
        max_concurrent_streams: 2,
    };
    let bindings = IrohaRuntimeProviderBindingsV1::qualified_provider_ingest_source_for_test(
        "server-test-chain",
        "network://sorafs/provider-ingest/source-primary",
        5,
        [0xB1; 32],
        limits,
    );
    let projected =
        ProviderBindingWireV1::try_from_binding(bindings.iter().next().expect("source binding"))
            .expect("project source binding");
    assert_eq!(projected.provider_ingest_source_limits, Some(limits.into()));
    assert_eq!(validate_wire_binding(&projected), Ok(()));
    assert!(matches!(
        prepare_server_state(&bindings, RuntimeProviderBrokerBackendsV1::new()),
        Err(RuntimeProviderBrokerServerErrorV1::BackendSetMismatch)
    ));
}
#[test]
fn musubi_source_fetch_v1_reconstructs_exact_private_binding() {
    let payload = vec![0xD7; 4 * 1024 + 19];
    let (generic_authorization, manifest, plan) = test_source_material(&payload);
    let (authorization, musubi) = test_source_musubi_fetch_binding(
        &generic_authorization,
        &manifest,
        &plan,
        server_test_network_id(),
    );
    let expected_musubi = musubi.clone();
    let observed_request = Arc::new(Mutex::new(None));
    let source_backend = ServerTestProviderSource {
        payload: payload.clone(),
        manifest,
        plan,
        revision: Arc::new(AtomicU64::new(5)),
        fetch_delay: Duration::ZERO,
        drift_on_eof: false,
        observed_request: Some(Arc::clone(&observed_request)),
    };
    let bindings = source_test_catalog(Duration::from_secs(5), 64 * 1024, 1);
    let (_directory, policy, shutdown, server) =
        start_source_test_server(source_backend, bindings.clone());
    let source = connect_test_source(&policy, &bindings);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build Musubi source client runtime");
    let request = sorafs_node::ProviderIngestSourceRequestV1::new(
        authorization.clone(),
        SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
        Some(musubi),
    )
    .expect("construct Musubi source request");
    let mut fetched = runtime
        .block_on(
            sorafs_node::ProviderIngestAuthenticatedSourceFetchV1::fetch(source.as_ref(), request),
        )
        .expect("open Musubi source stream");
    let mut observed_payload = Vec::new();
    std::io::Read::read_to_end(&mut fetched.reader, &mut observed_payload)
        .expect("read authenticated Musubi source stream");
    assert_eq!(observed_payload, payload);
    let observed = observed_request
        .lock()
        .expect("lock captured source request")
        .take()
        .expect("server backend received source request");
    assert_eq!(observed.authorization(), &authorization);
    assert_eq!(
        observed.source_provider_ids(),
        SERVER_TEST_SOURCE_PROVIDER_IDS.as_slice()
    );
    assert_eq!(observed.musubi_archive(), Some(&expected_musubi));
    drop(fetched);
    drop(source);
    drop(runtime);
    shutdown.request_shutdown();
    server
        .join()
        .expect("join Musubi source broker")
        .expect("Musubi source broker exits cleanly");
}
#[test]
fn stalled_source_stream_releases_unary_session_capacity() {
    let payload = vec![0xA7; 8 * 1024 * 1024];
    let (authorization, manifest, plan) = test_source_material(&payload);
    let source_backend = ServerTestProviderSource {
        payload,
        manifest,
        plan,
        revision: Arc::new(AtomicU64::new(5)),
        fetch_delay: Duration::ZERO,
        drift_on_eof: false,
        observed_request: None,
    };
    let bindings = source_test_catalog(Duration::from_secs(10), 16 * 1024 * 1024, 1);
    let (_directory, policy, shutdown, server) =
        start_source_test_server(source_backend, bindings.clone());
    let source = connect_test_source(&policy, &bindings);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build source client runtime");
    let fetched = runtime
        .block_on(
            sorafs_node::ProviderIngestAuthenticatedSourceFetchV1::fetch(
                source.as_ref(),
                sorafs_node::ProviderIngestSourceRequestV1::new(
                    authorization.clone(),
                    SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
                    None,
                )
                .expect("construct generic source request"),
            ),
        )
        .expect("open stalled source stream");
    assert!(matches!(
        runtime.block_on(
            sorafs_node::ProviderIngestAuthenticatedSourceFetchV1::fetch(
                source.as_ref(),
                sorafs_node::ProviderIngestSourceRequestV1::new(
                    authorization,
                    SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
                    None,
                )
                .expect("construct generic source request"),
            )
        ),
        Err(sorafs_node::ProviderIngestSourceFetchErrorV1::ContentRejected)
    ));
    let started = std::time::Instant::now();
    crate::sorafs_provider_ingest_runtime::
                    ProviderIngestAuthenticatedSourceRuntimeV1::check_readiness(source.as_ref())
                    .expect("unary readiness remains responsive");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "a stalled stream must not retain unary broker capacity"
    );
    drop(fetched);
    drop(source);
    drop(runtime);
    shutdown.request_shutdown();
    server
        .join()
        .expect("join source broker")
        .expect("source broker exits cleanly");
}
#[test]
fn source_stream_post_qualification_runs_after_exact_backend_eof() {
    let payload = vec![0xB8; 512 * 1024 + 7];
    let (authorization, manifest, plan) = test_source_material(&payload);
    let revision = Arc::new(AtomicU64::new(5));
    let source_backend = ServerTestProviderSource {
        payload,
        manifest,
        plan,
        revision: Arc::clone(&revision),
        fetch_delay: Duration::ZERO,
        drift_on_eof: true,
        observed_request: None,
    };
    let bindings = source_test_catalog(Duration::from_secs(5), 2 * 1024 * 1024, 1);
    let (_directory, policy, shutdown, server) =
        start_source_test_server(source_backend, bindings.clone());
    let source = connect_test_source(&policy, &bindings);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build source client runtime");
    let mut fetched = runtime
        .block_on(
            sorafs_node::ProviderIngestAuthenticatedSourceFetchV1::fetch(
                source.as_ref(),
                sorafs_node::ProviderIngestSourceRequestV1::new(
                    authorization,
                    SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
                    None,
                )
                .expect("construct generic source request"),
            ),
        )
        .expect("open source stream");
    let mut observed = Vec::new();
    assert!(
        std::io::Read::read_to_end(&mut fetched.reader, &mut observed).is_err(),
        "qualification drift at exact backend EOF must invalidate the trailer"
    );
    assert_eq!(revision.load(Ordering::Acquire), 6);
    drop(fetched);
    drop(source);
    drop(runtime);
    shutdown.request_shutdown();
    server
        .join()
        .expect("join drift source broker")
        .expect("drift source broker exits cleanly");
}
#[test]
fn source_fetch_future_obeys_configured_absolute_timeout() {
    let payload = vec![0xC9; 17];
    let (authorization, manifest, plan) = test_source_material(&payload);
    let source_backend = ServerTestProviderSource {
        payload,
        manifest,
        plan,
        revision: Arc::new(AtomicU64::new(5)),
        fetch_delay: Duration::from_millis(1_500),
        drift_on_eof: false,
        observed_request: None,
    };
    let bindings = source_test_catalog(Duration::from_millis(200), 1024, 1);
    let (_directory, policy, shutdown, server) =
        start_source_test_server(source_backend, bindings.clone());
    let source = connect_test_source(&policy, &bindings);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build timeout source client runtime");
    let started = std::time::Instant::now();
    assert!(matches!(
        runtime.block_on(
            sorafs_node::ProviderIngestAuthenticatedSourceFetchV1::fetch(
                source.as_ref(),
                sorafs_node::ProviderIngestSourceRequestV1::new(
                    authorization,
                    SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
                    None,
                )
                .expect("construct generic source request"),
            )
        ),
        Err(sorafs_node::ProviderIngestSourceFetchErrorV1::Unavailable)
    ));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "the configured source deadline must preempt the slower backend future"
    );
    drop(source);
    drop(runtime);
    shutdown.request_shutdown();
    server
        .join()
        .expect("join timeout source broker")
        .expect("timeout source broker exits cleanly");
}
#[test]
fn broker_server_pre_requested_shutdown_skips_qualification_and_bind() {
    let directory = tempfile::tempdir().expect("create broker server directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden broker server directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let policy = EndpointPolicy::for_test(path.clone());
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    lifecycle.request_shutdown();
    serve_with_policy(
        &server_test_catalog(),
        RuntimeProviderBrokerBackendsV1::new(),
        &policy,
        lifecycle,
    )
    .expect("pre-requested shutdown wins before missing-backend qualification");
    assert!(
        !path.exists(),
        "pre-requested shutdown must not create an endpoint"
    );
}
#[test]
fn lifecycle_linearizes_readiness_shutdown_and_operation_admission() {
    let stopped_before_ready = RuntimeProviderBrokerLifecycleV1::new();
    stopped_before_ready.request_shutdown();
    let stopped_callback = AtomicBool::new(false);
    assert!(!stopped_before_ready.publish_ready(|| {
        stopped_callback.store(true, Ordering::Release);
    }));
    assert!(!stopped_callback.load(Ordering::Acquire));
    let failed_readiness = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    assert_eq!(
        failed_readiness.publish_ready_fallible(|| Err(RuntimeProviderBrokerReadinessErrorV1)),
        Err(RuntimeProviderBrokerReadinessErrorV1)
    );
    assert!(
        failed_readiness.try_begin_operation().is_none(),
        "a failed callback must not publish the ready state"
    );
    assert!(
        failed_readiness.shutdown_requested(),
        "a failed callback must atomically move the lifecycle to stopping"
    );
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    assert!(
        lifecycle.publish_ready(|| {}),
        "readiness wins the initial lifecycle transition"
    );
    let admitted = lifecycle
        .try_begin_operation()
        .expect("operation is admitted while ready");
    lifecycle.request_shutdown();
    assert!(
        lifecycle.try_begin_operation().is_none(),
        "shutdown prevents every later operation admission"
    );
    assert_eq!(
        lifecycle.active_provider_call_count(),
        1,
        "the already-admitted synchronous call remains explicitly in flight"
    );
    drop(admitted);
    assert_eq!(lifecycle.active_provider_call_count(), 0);
}
#[test]
fn lifecycle_shutdown_waits_for_competing_readiness_callback() {
    let lifecycle = Arc::new(RuntimeProviderBrokerLifecycleV1::new());
    let callback_finished = Arc::new(AtomicBool::new(false));
    let shutdown_started = Arc::new(AtomicBool::new(false));
    let shutdown_observed_callback = Arc::new(AtomicBool::new(false));
    let (callback_entered_sender, callback_entered_receiver) = mpsc::sync_channel(0);
    let (release_callback_sender, release_callback_receiver) = mpsc::sync_channel(0);
    let publisher_lifecycle = Arc::clone(&lifecycle);
    let publisher_callback_finished = Arc::clone(&callback_finished);
    let publisher = thread::spawn(move || {
        publisher_lifecycle.publish_ready(|| {
            callback_entered_sender
                .send(())
                .expect("publish callback gate acquisition");
            release_callback_receiver
                .recv()
                .expect("release blocked readiness callback");
            publisher_callback_finished.store(true, Ordering::Release);
        })
    });
    callback_entered_receiver
        .recv()
        .expect("readiness callback owns the publication gate");
    let shutdown_lifecycle = Arc::clone(&lifecycle);
    let shutdown_started_probe = Arc::clone(&shutdown_started);
    let shutdown_callback_probe = Arc::clone(&callback_finished);
    let shutdown_observation = Arc::clone(&shutdown_observed_callback);
    let shutdown = thread::spawn(move || {
        shutdown_started_probe.store(true, Ordering::Release);
        shutdown_lifecycle.request_shutdown();
        shutdown_observation.store(
            shutdown_callback_probe.load(Ordering::Acquire),
            Ordering::Release,
        );
    });
    while !shutdown_started.load(Ordering::Acquire) {
        thread::yield_now();
    }
    assert!(
        !callback_finished.load(Ordering::Acquire),
        "the callback remains blocked while shutdown competes for its gate"
    );
    release_callback_sender
        .send(())
        .expect("allow readiness callback to finish");
    assert!(
        publisher.join().expect("join readiness publisher"),
        "the callback won the readiness publication race"
    );
    shutdown.join().expect("join competing shutdown request");
    assert!(
        shutdown_observed_callback.load(Ordering::Acquire),
        "shutdown cannot return before the competing callback finishes"
    );
    assert!(lifecycle.shutdown_requested());
}
#[test]
fn accepted_session_controls_close_peer_during_unexpected_unwind() {
    let (mut peer, accepted) = UnixStream::pair().expect("create accepted-session socket pair");
    peer.set_read_timeout(Some(Duration::from_secs(1)))
        .expect("bound peer read");
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut controls = AcceptedSessionControlsV1::default();
        controls.insert(1, accepted);
        panic!("panic after accepted-session registration");
    }));
    assert!(unwind.is_err());
    let mut byte = [0_u8; 1];
    assert_eq!(
        std::io::Read::read(&mut peer, &mut byte).expect("observe accepted-session shutdown"),
        0,
        "RAII shutdown wakes a peer when serving unexpectedly unwinds"
    );
}
#[test]
fn broker_server_drains_two_live_persistent_sessions_on_shutdown() {
    let (_directory, _path, policy, shutdown, server) = start_test_server();
    let first = connect_test_server_session(&policy);
    let second = connect_test_server_session(&policy);
    assert!(
        !Arc::ptr_eq(&first, &second),
        "both persistent sessions complete independent handshakes"
    );
    shutdown.request_shutdown();
    server
        .join()
        .expect("join broker server")
        .expect("broker server closes and joins both live sessions");
    drop((first, second));
}
#[test]
fn broker_server_rejects_excess_persistent_session_without_queueing() {
    let (_directory, _path, policy, shutdown, server) = start_test_server();
    let mut sessions = (0..MAX_BROKER_SESSIONS_V1)
        .map(|_| connect_test_server_session(&policy))
        .collect::<Vec<_>>();
    let mut excess = connect_verified(&policy).expect("connect excess local peer");
    // `connect_verified` already applies the fixed broker I/O
    // timeout. Reapplying `SO_RCVTIMEO` after the server has won
    // the immediate-close race returns `EINVAL` on macOS, which is
    // itself compatible with the expected rejection.
    let request = make_handshake_request(
        "server-test-chain",
        server_test_network_id(),
        vec![signer_binding_for_server()],
        [0xC7; 32],
    )
    .expect("build excess handshake");
    let frame = encode_frame(
        FRAME_KIND_HANDSHAKE_REQUEST_V1,
        &request,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )
    .expect("encode excess handshake");
    let outcome = write_length_prefixed(&mut excess, &frame, MAX_HANDSHAKE_FRAME_BYTES_V1)
        .and_then(|()| read_length_prefixed(&mut excess, MAX_HANDSHAKE_FRAME_BYTES_V1).map(drop));
    assert!(
        outcome.is_err(),
        "the excess session must be closed rather than queued"
    );
    drop(excess);
    drop(sessions.pop());
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let replacement = loop {
        match BrokerSession::connect(
            &policy,
            "server-test-chain",
            server_test_network_id(),
            vec![signer_binding_for_server()],
        ) {
            Ok((session, _)) => break session,
            Err(_) if std::time::Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => {
                panic!("a released admission permit must accept a replacement: {error:?}")
            }
        }
    };
    drop(replacement);
    drop(sessions);
    shutdown.request_shutdown();
    server
        .join()
        .expect("join broker server")
        .expect("broker server exits cleanly");
}
fn read_handshake(stream: &mut UnixStream) -> HandshakeRequestV1 {
    let frame = read_length_prefixed(stream, MAX_HANDSHAKE_FRAME_BYTES_V1)
        .expect("read fake broker handshake");
    let request = decode_frame::<HandshakeRequestV1>(
        &frame,
        FRAME_KIND_HANDSHAKE_REQUEST_V1,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )
    .expect("decode fake broker handshake");
    assert_valid_handshake_request(&request);
    request
}
fn send_handshake(stream: &mut UnixStream, response: &HandshakeResponseV1) {
    let frame = encode_frame(
        FRAME_KIND_HANDSHAKE_RESPONSE_V1,
        response,
        MAX_HANDSHAKE_FRAME_BYTES_V1,
    )
    .expect("encode fake broker handshake response");
    write_length_prefixed(stream, &frame, MAX_HANDSHAKE_FRAME_BYTES_V1)
        .expect("write fake broker handshake response");
}
fn read_operation(stream: &mut UnixStream) -> OperationRequestV1 {
    // The fake broker represents a separate process, so its decode admission
    // must not compete with the in-process client for one process-local pool.
    let decode_pool = Arc::new(DecodeResourcePoolV1::new(MAX_BROKER_SHARED_DECODE_BYTES_V1));
    let (announced_slot, announced_operation, frame, admission) =
        read_operation_request_frame_inner(stream, None, Some(decode_pool))
            .expect("read fake broker operation");
    let _scope = admission.enter();
    let request = decode_operation_frame::<OperationRequestV1>(
        &frame,
        FRAME_KIND_OPERATION_REQUEST_V1,
        announced_operation,
    )
    .expect("decode fake broker operation");
    validate_operation_request(&request).expect("validate fake broker operation");
    assert_eq!(request.binding.slot, announced_slot);
    assert_eq!(request.operation, announced_operation);
    request
}
fn send_operation(stream: &mut UnixStream, response: &OperationResponseV1) {
    let frame = encode_frame(
        FRAME_KIND_OPERATION_RESPONSE_V1,
        response,
        MAX_OPERATION_FRAME_BYTES_V1,
    )
    .expect("encode fake broker operation response");
    write_length_prefixed(stream, &frame, MAX_OPERATION_FRAME_BYTES_V1)
        .expect("write fake broker operation response");
}
fn source_reader_for_test(
    payload: &[u8],
    timeout: Duration,
) -> (ProviderIngestBrokerSourceReader, UnixStream, blake3::Hasher) {
    let (reader_stream, writer_stream) = UnixStream::pair().expect("create source stream pair");
    let mut transcript = blake3::Hasher::new();
    transcript.update(PROVIDER_INGEST_SOURCE_STREAM_DOMAIN_V1);
    transcript.update(b"test-source-reader");
    (
        ProviderIngestBrokerSourceReader {
            stream: reader_stream,
            deadline: std::time::Instant::now() + timeout,
            content_length: u64::try_from(payload.len()).expect("test payload length fits u64"),
            remaining: u64::try_from(payload.len()).expect("test payload length fits u64"),
            frame_count: source_stream_frame_count(
                u64::try_from(payload.len()).expect("test payload length fits u64"),
            )
            .expect("nonempty test payload"),
            next_sequence: 0,
            pending: Vec::new(),
            pending_offset: 0,
            expected_payload_digest: blake3::hash(payload).into(),
            expected_provider_metadata_digest: [0xD4; 32],
            payload_hasher: blake3::Hasher::new(),
            transcript: transcript.clone(),
            finished: false,
            poisoned: false,
            _retained_memory: None,
        },
        writer_stream,
        transcript,
    )
}
fn write_source_chunk_for_test(
    writer: &mut UnixStream,
    transcript: &mut blake3::Hasher,
    sequence: u64,
    offset: u64,
    bytes: Vec<u8>,
) {
    let chunk = ProviderIngestSourceChunkWireV1 {
        sequence,
        offset,
        bytes,
    };
    update_source_stream_transcript(transcript, &chunk);
    let frame = encode_frame(
        FRAME_KIND_PROVIDER_INGEST_SOURCE_CHUNK_V1,
        &chunk,
        MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1,
    )
    .expect("encode source chunk");
    write_length_prefixed(
        writer,
        &frame,
        MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1,
    )
    .expect("write source chunk");
}
fn write_source_payload_for_test(
    writer: &mut UnixStream,
    transcript: &mut blake3::Hasher,
    payload: &[u8],
) {
    for (sequence, bytes) in payload
        .chunks(MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1)
        .enumerate()
    {
        let offset = sequence
            .checked_mul(MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1)
            .and_then(|offset| u64::try_from(offset).ok())
            .expect("test source offset fits u64");
        write_source_chunk_for_test(
            writer,
            transcript,
            u64::try_from(sequence).expect("test sequence fits u64"),
            offset,
            bytes.to_vec(),
        );
    }
}
fn write_source_trailer_for_test(
    writer: &mut UnixStream,
    transcript: &blake3::Hasher,
    payload: &[u8],
    payload_digest: [u8; 32],
) {
    let trailer = ProviderIngestSourceTrailerWireV1 {
        status: STATUS_OK_V1,
        content_length: u64::try_from(payload.len()).expect("test payload length fits u64"),
        frame_count: source_stream_frame_count(
            u64::try_from(payload.len()).expect("test payload length fits u64"),
        )
        .expect("nonempty test payload"),
        payload_digest,
        transcript_digest: *transcript.clone().finalize().as_bytes(),
        provider_metadata_digest: [0xD4; 32],
    };
    let frame = encode_frame(
        FRAME_KIND_PROVIDER_INGEST_SOURCE_TRAILER_V1,
        &trailer,
        MAX_PROVIDER_INGEST_SOURCE_TRAILER_FRAME_BYTES_V1,
    )
    .expect("encode source trailer");
    write_length_prefixed(
        writer,
        &frame,
        MAX_PROVIDER_INGEST_SOURCE_TRAILER_FRAME_BYTES_V1,
    )
    .expect("write source trailer");
}
fn test_source_authorization(
    content_length: u64,
) -> sorafs_node::FinalizedProviderIngestAuthorizationV1 {
    sorafs_node::FinalizedProviderIngestAuthorizationV1::from_finalized_state(
        7,
        [0x77; 32],
        [0x99; 32],
        [0x88; 32],
        [0x66; 32],
        vec![0x55; 36],
        "sorafs.sf1@1.0.0".to_owned(),
        [0x44; 32],
        [0x33; 32],
        content_length,
    )
    .expect("construct test finalized authorization")
}
#[test]
fn source_reader_preserves_backpressure_and_authenticates_exact_eof() {
    let payload = vec![0xA5; MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1 * 2 + 17];
    let (mut reader, mut writer, mut transcript) =
        source_reader_for_test(&payload, Duration::from_secs(2));
    let writer_payload = payload.clone();
    let writer_thread = thread::spawn(move || {
        for (sequence, bytes) in writer_payload
            .chunks(MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1)
            .enumerate()
        {
            thread::sleep(Duration::from_millis(5));
            let offset = sequence
                .checked_mul(MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1)
                .and_then(|offset| u64::try_from(offset).ok())
                .expect("test source offset fits u64");
            write_source_chunk_for_test(
                &mut writer,
                &mut transcript,
                u64::try_from(sequence).expect("test sequence fits u64"),
                offset,
                bytes.to_vec(),
            );
        }
        write_source_trailer_for_test(
            &mut writer,
            &transcript,
            &writer_payload,
            blake3::hash(&writer_payload).into(),
        );
        writer
            .shutdown(std::net::Shutdown::Write)
            .expect("close source writer");
    });
    let mut observed = Vec::new();
    let mut scratch = [0_u8; 17];
    loop {
        let read =
            std::io::Read::read(&mut reader, &mut scratch).expect("read authenticated source");
        if read == 0 {
            break;
        }
        observed.extend_from_slice(&scratch[..read]);
    }
    assert_eq!(observed, payload);
    assert!(reader.finished);
    writer_thread.join().expect("join source writer");
}
#[test]
fn source_reader_rejects_truncation_reordering_duplicates_and_digest_mismatch() {
    let payload = vec![0x5A; MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1 + 7];
    let (mut truncated, writer, _) = source_reader_for_test(&payload, Duration::from_secs(1));
    writer
        .shutdown(std::net::Shutdown::Both)
        .expect("truncate source stream");
    assert!(
        std::io::Read::read_to_end(&mut truncated, &mut Vec::new()).is_err(),
        "truncated stream must not produce EOF"
    );
    for first_sequence in [1, 0] {
        let (mut reader, mut writer, mut transcript) =
            source_reader_for_test(&payload, Duration::from_secs(1));
        let first_chunk = payload[..MAX_PROVIDER_INGEST_SOURCE_CHUNK_PAYLOAD_BYTES_V1].to_vec();
        let writer_thread = thread::spawn(move || {
            write_source_chunk_for_test(
                &mut writer,
                &mut transcript,
                first_sequence,
                0,
                first_chunk.clone(),
            );
            if first_sequence == 0 {
                write_source_chunk_for_test(&mut writer, &mut transcript, 0, 0, first_chunk);
            }
            let _ = writer.shutdown(std::net::Shutdown::Write);
        });
        assert!(
            std::io::Read::read_to_end(&mut reader, &mut Vec::new()).is_err(),
            "reordered or duplicate frames must fail"
        );
        writer_thread.join().expect("join malformed source writer");
    }
    let (mut reader, mut writer, mut transcript) =
        source_reader_for_test(&payload, Duration::from_secs(1));
    let writer_payload = payload.clone();
    let writer_thread = thread::spawn(move || {
        write_source_payload_for_test(&mut writer, &mut transcript, &writer_payload);
        write_source_trailer_for_test(&mut writer, &transcript, &writer_payload, [0xEE; 32]);
        let _ = writer.shutdown(std::net::Shutdown::Write);
    });
    assert!(
        std::io::Read::read_to_end(&mut reader, &mut Vec::new()).is_err(),
        "a mismatched digest trailer must fail before EOF"
    );
    writer_thread
        .join()
        .expect("join digest-mismatched source writer");
}
#[test]
fn source_reader_rejects_extra_frames_wire_trailing_bytes_and_timeout() {
    let payload = vec![0x6B; 17];
    let (mut extra_frame_reader, mut writer, mut transcript) =
        source_reader_for_test(&payload, Duration::from_secs(1));
    write_source_payload_for_test(&mut writer, &mut transcript, &payload);
    write_source_chunk_for_test(&mut writer, &mut transcript, 1, 17, vec![1]);
    writer
        .shutdown(std::net::Shutdown::Write)
        .expect("close extra-frame stream");
    assert!(std::io::Read::read_to_end(&mut extra_frame_reader, &mut Vec::new()).is_err());
    let (mut trailing_reader, mut writer, mut transcript) =
        source_reader_for_test(&payload, Duration::from_secs(1));
    write_source_payload_for_test(&mut writer, &mut transcript, &payload);
    write_source_trailer_for_test(
        &mut writer,
        &transcript,
        &payload,
        blake3::hash(&payload).into(),
    );
    std::io::Write::write_all(&mut writer, &[0xFF]).expect("append forbidden wire byte");
    writer
        .shutdown(std::net::Shutdown::Write)
        .expect("close trailing-byte stream");
    assert!(std::io::Read::read_to_end(&mut trailing_reader, &mut Vec::new()).is_err());
    let (mut timed_out, writer, _) = source_reader_for_test(&payload, Duration::from_millis(20));
    let error = std::io::Read::read(&mut timed_out, &mut [0_u8; 1])
        .expect_err("silent source must time out");
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    drop(writer);
}
#[test]
fn source_reader_drop_closes_unverified_connection() {
    let payload = vec![0x7C; 17];
    let (reader, mut writer, _) = source_reader_for_test(&payload, Duration::from_secs(1));
    writer
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("bound peer close observation");
    drop(reader);
    let mut byte = [0_u8; 1];
    match std::io::Read::read(&mut writer, &mut byte) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::NotConnected
            ) => {}
        Ok(read) => {
            panic!("unverified source reader left {read} peer byte(s) readable")
        }
        Err(error) => panic!("failed to observe source reader shutdown: {error}"),
    }
}
#[test]
fn source_fetch_v1_accepts_generic_and_rejects_musubi_substitution() {
    let payload = vec![0xD1; 4096];
    let (authorization, manifest, plan) = test_source_material(&payload);
    let bindings = source_test_catalog(Duration::from_secs(5), 64 * 1024, 1);
    let binding =
        ProviderBindingWireV1::try_from_binding(bindings.iter().next().expect("source binding"))
            .expect("project source binding");
    let generic = source_request_to_wire(
        sorafs_node::ProviderIngestSourceRequestV1::new(
            authorization.clone(),
            SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
            None,
        )
        .expect("construct generic source request"),
    )
    .expect("project generic V1 source wire");
    assert_eq!(
        validate_source_fetch_request(
            &generic,
            &binding,
            Some(&SERVER_TEST_SOURCE_PROVIDER_IDS),
            &server_test_network_id(),
        ),
        Ok(())
    );
    let (musubi_authorization, musubi) = test_source_musubi_fetch_binding(
        &authorization,
        &manifest,
        &plan,
        server_test_network_id(),
    );
    let request = sorafs_node::ProviderIngestSourceRequestV1::new(
        musubi_authorization.clone(),
        SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
        Some(musubi.clone()),
    )
    .expect("construct Musubi source request");
    let exact = source_request_to_wire(request).expect("project exact Musubi V1 source wire");
    assert_eq!(
        validate_source_fetch_request(
            &exact,
            &binding,
            Some(&SERVER_TEST_SOURCE_PROVIDER_IDS),
            &server_test_network_id(),
        ),
        Ok(())
    );
    let mut later_cursor = exact.clone();
    later_cursor
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .observed_finalized_cursor = sorafs_node::ProviderIngestFinalizedCursorV1 {
        height: authorization.finalized_height().saturating_add(1),
        block_hash: [0x78; 32],
    };
    assert_eq!(
        validate_source_fetch_request(
            &later_cursor,
            &binding,
            Some(&SERVER_TEST_SOURCE_PROVIDER_IDS),
            &server_test_network_id(),
        ),
        Ok(()),
        "a current informational claim may be newer than its retained admission"
    );
    let rejects = |candidate: &ProviderIngestSourceFetchRequestWireV1| {
        assert_eq!(
            validate_source_fetch_request(
                candidate,
                &binding,
                Some(&SERVER_TEST_SOURCE_PROVIDER_IDS),
                &server_test_network_id(),
            ),
            Err(BrokerError::Rejected)
        );
    };
    let mut inconsistent_network = exact.clone();
    inconsistent_network
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .network_id = test_network_id(0x17);
    rejects(&inconsistent_network);
    let mut zero_cursor = exact.clone();
    zero_cursor
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .observed_finalized_cursor
        .height = 0;
    rejects(&zero_cursor);
    let mut forked_cursor = exact.clone();
    forked_cursor
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .observed_finalized_cursor
        .block_hash = [0xE0; 32];
    rejects(&forked_cursor);
    let mut wrong_order = exact.clone();
    wrong_order
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding
        .replication_order =
        iroha_data_model::sorafs::pin_registry::ReplicationOrderId::new([0x61; 32]);
    rejects(&wrong_order);
    let mut wrong_archive = exact.clone();
    wrong_archive
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding
        .archive_id = iroha_data_model::musubi::ArchiveId::new([0xE2; 32]);
    rejects(&wrong_archive);
    let mut wrong_root = exact.clone();
    let wrong_root_binding = &mut wrong_root
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding;
    wrong_root_binding.commitment.root_cid =
        iroha_data_model::sorafs::pin_registry::ManifestRootCid::from_blake3_digest([0xE3; 32])
            .expect("alternate canonical root CID");
    wrong_root_binding.archive_id = wrong_root_binding.commitment.archive_id();
    rejects(&wrong_root);
    let mut wrong_chunker = exact.clone();
    let wrong_chunker_binding = &mut wrong_chunker
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding;
    wrong_chunker_binding.commitment.chunker.name = "other".to_owned();
    wrong_chunker_binding.archive_id = wrong_chunker_binding.commitment.archive_id();
    rejects(&wrong_chunker);
    let mut wrong_plan = exact.clone();
    let wrong_plan_binding = &mut wrong_plan
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding;
    wrong_plan_binding.commitment.chunk_plan_digest =
        iroha_data_model::musubi::MusubiContentDigestV1::new([0xE4; 32]);
    wrong_plan_binding.archive_id = wrong_plan_binding.commitment.archive_id();
    rejects(&wrong_plan);
    let mut wrong_por = exact.clone();
    let wrong_por_binding = &mut wrong_por
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding;
    wrong_por_binding.commitment.por_root =
        iroha_data_model::musubi::MusubiContentDigestV1::new([0xE5; 32]);
    wrong_por_binding.archive_id = wrong_por_binding.commitment.archive_id();
    rejects(&wrong_por);
    let mut wrong_length = exact;
    let wrong_length_binding = &mut wrong_length
        .musubi_archive
        .as_mut()
        .expect("Musubi wire")
        .binding;
    wrong_length_binding.commitment.content_length = wrong_length_binding
        .commitment
        .content_length
        .saturating_add(1);
    wrong_length_binding.archive_id = wrong_length_binding.commitment.archive_id();
    rejects(&wrong_length);
    let (foreign_authorization, foreign_musubi) =
        test_source_musubi_fetch_binding(&authorization, &manifest, &plan, test_network_id(0x17));
    let foreign_network_request = sorafs_node::ProviderIngestSourceRequestV1::new(
        foreign_authorization,
        SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
        Some(foreign_musubi),
    )
    .expect("construct internally consistent foreign-network Musubi request");
    let foreign_network = source_request_to_wire(foreign_network_request)
        .expect("project foreign-network Musubi source wire");
    assert_eq!(
        validate_source_fetch_request(
            &foreign_network,
            &binding,
            Some(&SERVER_TEST_SOURCE_PROVIDER_IDS),
            &server_test_network_id(),
        ),
        Err(BrokerError::BindingMismatch)
    );
}
#[test]
fn source_fetch_v1_rejects_an_incomplete_two_field_wire() {
    #[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
    struct IncompleteProviderIngestSourceFetchRequestWire {
        authorization: sorafs_node::FinalizedProviderIngestAuthorizationV1,
        source_provider_ids: Vec<[u8; 32]>,
    }
    let bindings = source_test_catalog(Duration::from_secs(5), 64 * 1024, 1);
    let binding =
        ProviderBindingWireV1::try_from_binding(bindings.iter().next().expect("source binding"))
            .expect("project source binding");
    let incomplete = IncompleteProviderIngestSourceFetchRequestWire {
        authorization: test_source_authorization(16),
        source_provider_ids: SERVER_TEST_SOURCE_PROVIDER_IDS.to_vec(),
    };
    let payload = encode_canonical(&incomplete, MAX_PROVIDER_INGEST_SOURCE_REQUEST_BYTES_V1)
        .expect("encode incomplete source request");
    assert!(
        decode_canonical::<ProviderIngestSourceFetchRequestWireV1>(
            &payload,
            MAX_PROVIDER_INGEST_SOURCE_REQUEST_BYTES_V1,
        )
        .is_err(),
        "the exact V1 source request requires its explicit Musubi archive field"
    );
    assert_eq!(OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V1, 28);
    assert!(operation_is_known(
        OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V1
    ));
    let request = make_operation_request(
        [0x91; 32],
        1,
        binding,
        [0x92; 32],
        OPERATION_PROVIDER_INGEST_SOURCE_FETCH_V1,
        payload,
    )
    .expect("construct exact V1 operation request with malformed payload");
    assert_eq!(
        validate_operation_request_for_session(
            &request,
            "server-test-chain",
            &server_test_network_id()
        ),
        Err(BrokerError::Protocol)
    );
}
#[test]
fn source_protocol_rejects_oversize_metadata_frame_count_and_total_without_allocating() {
    assert_eq!(
        validate_source_metadata_lengths(sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES + 1, 1),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        validate_source_metadata_lengths(1, MAX_PROVIDER_INGEST_SOURCE_PLAN_BYTES_V1 + 1),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        validate_source_plan_counts(sorafs_car::CAR_PLAN_MAX_CHUNKS + 1, 1),
        Err(BrokerError::Rejected)
    );
    assert_eq!(
        validate_source_plan_counts(1, MAX_PROVIDER_INGEST_SOURCE_PLAN_FILES_V1 + 1),
        Err(BrokerError::Rejected)
    );
    let oversized_frame = u32::try_from(MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1 + 1)
        .expect("source frame ceiling fits u32")
        .to_be_bytes();
    assert_eq!(
        read_length_prefixed(
            &mut Cursor::new(oversized_frame),
            MAX_PROVIDER_INGEST_SOURCE_CHUNK_FRAME_BYTES_V1
        ),
        Err(BrokerError::Protocol)
    );
    let binding = ProviderBindingWireV1 {
        slot: IrohaRuntimeProviderSlotV1::ProviderIngestAuthenticatedSource.wire_id(),
        handle: "network://sorafs/provider-ingest/source-primary".to_owned(),
        revision: Some(5),
        policy_digest: Some([0xB1; 32]),
        bootle_lantern_issuance_bindings: None,
        stream_token_signer_public_key: None,
        stream_token_gateway_admission_qualification: None,
        stream_token_gateway_admission_max_pending: None,
        stream_token_gateway_admission_max_tracked_tokens: None,
        stream_token_gateway_admission_reconcile_max_items: None,
        appeal_finance_signer_binding: None,
        appeal_finance_checkpoint_binding: None,
        appeal_finance_checkpoint_max_bytes: None,
        pop_credential_runtime_binding: None,
        por_replay_archive_binding: None,
        por_replay_archive_proof_limits: None,
        potr_runtime_binding: None,
        native_signer_binding: None,
        governance_dag_publisher_peer_id: None,
        governance_dag_publisher_public_key: None,
        governance_request_ingress_binding: None,
        provider_ingest_signer_binding: None,
        provider_ingest_source_limits: Some(ProviderIngestSourceLimitsWireV1 {
            operation_timeout_ms: 1_000,
            max_content_bytes: 16,
            max_source_providers: 2,
            max_concurrent_streams: 1,
        }),
        provider_ingest_checkpoint_max_bytes: None,
        provider_ingest_max_signed_transaction_bytes: None,
        evidence_viewer_webauthn_binding: None,
        evidence_viewer_grant_ttl_ms: None,
        evidence_viewer_receipt_signer_public_key: None,
        evidence_viewer_transparency_publisher_public_key: None,
        evidence_viewer_checkpoint_max_bytes: None,
        moderation_checkpoint_max_bytes: None,
        moderation_checkpoint_attestation_public_key: None,
        evidence_viewer_archive_id: None,
        evidence_viewer_archive_public_key: None,
        evidence_viewer_archive_max_bytes: None,
        moderation_panel_notification_archive_binding: None,
    };
    let fetch = ProviderIngestSourceFetchRequestWireV1 {
        authorization: test_source_authorization(17),
        source_provider_ids: vec![[1; 32], [2; 32]],
        musubi_archive: None,
    };
    assert_eq!(
        validate_source_fetch_request(&fetch, &binding, None, &server_test_network_id()),
        Err(BrokerError::Rejected)
    );
    let mut too_many_sources = fetch;
    too_many_sources.authorization = test_source_authorization(16);
    too_many_sources.source_provider_ids.push([3; 32]);
    assert_eq!(
        validate_source_fetch_request(&too_many_sources, &binding, None, &server_test_network_id(),),
        Err(BrokerError::Rejected)
    );
}
#[test]
fn source_plan_metadata_roundtrips_canonically_and_rejects_trailing_bytes() {
    let payload = vec![0xAB; 512 * 1024 + 3];
    let plan = sorafs_car::CarBuildPlan::single_file(&payload).expect("build test source plan");
    let bytes = encode_source_plan(&plan).expect("encode bounded source plan");
    assert_eq!(
        decode_source_plan(&bytes).expect("decode exact source plan"),
        plan
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(decode_source_plan(&trailing), Err(BrokerError::Rejected));
}
#[test]
fn source_streams_transfer_to_actual_retained_plan_reservations() {
    let payload = vec![0xAB; 512 * 1024 + 3];
    let plan =
        sorafs_car::CarBuildPlan::single_file(&payload).expect("build retained-memory test plan");
    let retained = source_retained_memory_bytes(&plan).expect("derive retained plan reservation");
    assert!(
        retained < SOURCE_PLAN_DECODE_POLICY_V1.max_composed_bytes,
        "a validated plan must not retain its full initial decode ceiling"
    );
    let pool_bytes = retained
        .checked_mul(2)
        .and_then(|bytes| {
            bytes.checked_add(SOURCE_STREAM_FRAME_DECODE_POLICY_V1.max_composed_bytes)
        })
        .expect("test pool arithmetic");
    let pool = Arc::new(DecodeResourcePoolV1::new(pool_bytes));
    let first = pool
        .try_acquire(retained)
        .expect("retain first validated source plan");
    let second = pool
        .try_acquire(retained)
        .expect("retain second validated source plan");
    let chunk = DecodeResourceAdmissionV1::acquire_from(
        Arc::clone(&pool),
        None,
        SOURCE_STREAM_FRAME_DECODE_POLICY_V1,
    )
    .expect("admit transient chunk beside retained plans");
    assert_eq!(pool.used_bytes.load(Ordering::Acquire), pool_bytes);
    drop(chunk);
    drop(second);
    drop(first);
    assert_eq!(pool.used_bytes.load(Ordering::Acquire), 0);
}
#[test]
fn stream_token_gateway_admission_qualification_roundtrips_through_dispatch() {
    const HANDLE: &str = "sealed-cas:prod/stream-token/gateway-admission/v1";
    #[derive(Debug)]
    struct QualificationOnlyProvider {
        qualification: iroha_torii::sorafs::StreamTokenGatewayAdmissionQualificationV1,
    }
    impl iroha_torii::sorafs::StreamTokenGatewayAdmissionProviderV1 for QualificationOnlyProvider {
        fn handle(&self) -> &str {
            HANDLE
        }
        fn qualification(
            &self,
        ) -> Result<
            iroha_torii::sorafs::StreamTokenGatewayAdmissionQualificationV1,
            iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
        > {
            Ok(self.qualification)
        }
        fn admit(
            &self,
            _request: &iroha_torii::sorafs::StreamTokenGatewayAdmissionRequestV1,
        ) -> Result<
            iroha_torii::sorafs::StreamTokenGatewayAdmissionResultV1,
            iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
        > {
            Err(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::Unavailable)
        }
        fn pending(
            &self,
            _max_items: u32,
        ) -> Result<
            iroha_torii::sorafs::StreamTokenGatewayAdmissionReadbackV1,
            iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
        > {
            Err(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::Unavailable)
        }
        fn acknowledge(
            &self,
            _record: iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1,
        ) -> Result<
            iroha_torii::sorafs::StreamTokenGatewayAdmissionAckV1,
            iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
        > {
            Err(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::Unavailable)
        }
        fn release_lease(
            &self,
            _record: iroha_torii::sorafs::StreamTokenGatewayAdmissionRecordV1,
        ) -> Result<
            iroha_torii::sorafs::StreamTokenGatewayAdmissionAckV1,
            iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1,
        > {
            Err(iroha_torii::sorafs::StreamTokenGatewayAdmissionErrorV1::Unavailable)
        }
    }
    let qualification = iroha_torii::sorafs::StreamTokenGatewayAdmissionQualificationV1 {
        gateway_id: [0x54; 32],
        revision: 7,
        policy_digest: TEST_POLICY_DIGEST,
        max_pending: 64,
        max_tracked_tokens: 32,
        lease_ttl_ms: 120_000,
    };
    let mut binding = plain_runtime_binding(
        IrohaRuntimeProviderSlotV1::StreamTokenGatewayAdmission,
        HANDLE,
    );
    binding.stream_token_gateway_admission_qualification = Some(qualification);
    binding.stream_token_gateway_admission_max_pending = Some(qualification.max_pending);
    binding.stream_token_gateway_admission_max_tracked_tokens =
        Some(qualification.max_tracked_tokens);
    binding.stream_token_gateway_admission_reconcile_max_items = Some(16);
    validate_wire_binding(&binding).expect("valid stream-token gateway binding");
    let backends = RuntimeProviderBrokerBackendsV1::new()
        .with_stream_token_gateway_admission(Arc::new(QualificationOnlyProvider { qualification }));
    validate_exact_backend_set(std::slice::from_ref(&binding), &backends)
        .expect("exact stream-token gateway backend set");
    let observation = make_server_observation(&binding, &backends)
        .expect("observe exact stream-token gateway backend");
    let state = BrokerServerStateV1 {
        chain_id: "server-test-chain".to_owned(),
        network_id: server_test_network_id(),
        catalog: vec![binding.clone()],
        observations: vec![observation.clone()],
        backends,
    };
    let request = make_operation_request(
        TEST_SESSION_ID,
        1,
        binding,
        observation.metadata_digest,
        OPERATION_QUALIFY_V1,
        encode_canonical(&(), MAX_STREAM_TOKEN_FRAME_BYTES_V1)
            .expect("encode qualification request"),
    )
    .expect("build qualification request");
    validate_operation_request(&request).expect("admit stream-token gateway qualification request");
    let encoded = dispatch_server_operation(&state, &request)
        .expect("dispatch stream-token gateway qualification");
    let observed =
        decode_canonical::<QualificationResultWireV1>(&encoded, MAX_OPERATION_FRAME_BYTES_V1)
            .expect("decode qualification response");
    assert_eq!(observed.revision, qualification.revision);
    assert_eq!(observed.policy_digest, qualification.policy_digest);
    validate_operation_result(&request, STATUS_OK_V1, &encoded, &state.network_id)
        .expect("validate stream-token gateway qualification response");
}
#[cfg(target_os = "macos")]
#[test]
fn macos_socket_device_identity_preserves_signed_dev_t_bits() {
    assert_eq!(
        socket_device_identity_from_raw(-1),
        u64::MAX,
        "the stat identity must use MetadataExt::dev()'s signed-to-u64 conversion"
    );
    assert_eq!(
        socket_device_identity_from_raw(i32::MIN),
        u64::MAX - u64::try_from(i32::MAX).expect("i32::MAX fits u64"),
        "valid high-bit macOS device identities must not be rejected"
    );
}
fn bind_fake_broker() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    EndpointPolicy,
    UnixListener,
) {
    let directory = tempfile::tempdir().expect("create fake broker directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("harden fake broker directory");
    let path = directory.path().join("runtime-provider-broker-v1.sock");
    let listener = UnixListener::bind(&path).expect("bind fake broker socket");
    set_socket_mode(&path).expect("set fake broker socket mode");
    let policy = EndpointPolicy::for_test(path.clone());
    (directory, path, policy, listener)
}
fn hold_instance_lock(policy: &EndpointPolicy) -> endpoint_recovery::InstanceLockGuard {
    let parent = fs::File::open(policy.path.parent().expect("broker endpoint parent"))
        .expect("open broker endpoint parent");
    endpoint_recovery::InstanceLockGuard::acquire(&parent, policy.expected_service_uid)
        .expect("hold active broker instance lock")
}
fn seed_instance_lock_marker(policy: &EndpointPolicy) {
    let marker = hold_instance_lock(policy);
    drop(marker);
}
