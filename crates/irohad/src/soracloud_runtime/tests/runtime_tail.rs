const INROU_HEALTH_SERVER_PY: &str = include_str!("fixtures/inrou_health_server.py");
const INROU_SHARED_VOLUME_SERVER_PY: &str = include_str!("fixtures/inrou_shared_volume_server.py");

#[test]
#[ignore = "requires an unprivileged guest plus a complete canonical IROHA_INROU_PORTABLE_SMOKE_BUNDLE_FILE"]
fn inrou_portable_smoke_boots_external_bundle_and_serves_healthcheck() -> Result<()> {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1")
        || std::env::var("IROHA_INROU_PORTABLE").ok().as_deref() != Some("1")
    {
        println!(
            "Skipping: set IROHA_RUN_IGNORED=1 IROHA_INROU_PORTABLE=1 to run the external bundle PortableVm smoke test."
        );
        return Ok(());
    }
    require_portable_smoke_prerequisites()?;
    let external_bundle =
        portable_smoke_required_env_path("IROHA_INROU_PORTABLE_SMOKE_BUNDLE_FILE")?;
    let external_entrypoint = std::env::var("IROHA_INROU_PORTABLE_SMOKE_ENTRYPOINT")
        .unwrap_or_else(|_| "/app/launch.sh".to_owned());
    let external_healthcheck = std::env::var("IROHA_INROU_PORTABLE_SMOKE_HEALTHCHECK")
        .unwrap_or_else(|_| "/health".to_owned());
    let temp_dir = tempfile::tempdir()?;
    let selected_guest_isa = current_host_inrou_guest_isa();
    let local_peer_id = "12D3KooWPortableVmExternalBundlePeer";
    let mut config = test_runtime_manager_config(temp_dir.path().to_path_buf())
        .with_local_host_identity(ALICE_ID.clone(), local_peer_id);
    config.inrou.start_grace = Duration::from_secs(240);
    let archive_limits =
        inrou_bundle_archive_limits(&config.inrou, config.cache_budgets.bundle_bytes.get());
    let (external_bundle_file, external_bundle_fingerprint) =
        open_soracloud_artifact_for_validation(&external_bundle).map_err(|error| {
            eyre::eyre!(
                "open external Inrou smoke bundle {} securely: {}",
                external_bundle.display(),
                error.message
            )
        })?;
    let bundle_bytes = read_opened_soracloud_artifact_bounded(
        external_bundle_file,
        &external_bundle,
        &external_bundle_fingerprint,
        archive_limits.max_compressed_bytes,
    )
    .map_err(|error| {
        eyre::eyre!(
            "read external Inrou smoke bundle {} securely: {}",
            external_bundle.display(),
            error.message
        )
    })?;
    visit_gzip_ustar(
        io::Cursor::new(&bundle_bytes),
        archive_limits,
        |_entry, payload| {
            io::copy(payload, &mut io::sink())?;
            Ok(())
        },
    )
    .wrap_err_with(|| {
        format!(
            "validate external Inrou smoke bundle {}",
            external_bundle.display()
        )
    })?;
    let mut bundle = sample_inrou_test_bundle()?;
    bundle.container.entrypoint = external_entrypoint;
    bundle.container.args.clear();
    bundle.container.bundle_path = "/bundles/external-inrou-smoke.tgz".to_owned();
    bundle.container.bundle_hash = Hash::new(&bundle_bytes);
    bundle.container.lifecycle.healthcheck_path = Some(external_healthcheck);
    bundle
        .container
        .inrou
        .as_mut()
        .expect("inrou manifest")
        .bootstrap_user_data_path = None;
    bundle
        .container
        .env
        .insert("APP_ENV".to_owned(), "production".to_owned());
    bundle.container.env.insert(
        "RUST_LOG".to_owned(),
        "hayahi_ingress=debug,tower_http=debug".to_owned(),
    );
    bundle.container.env.insert(
        "SORACLOUD_TEMPLATE".to_owned(),
        "hayahi-live-smoke".to_owned(),
    );
    if let Some(route) = bundle.service.route.as_ref() {
        bundle.container.env.insert(
            "SORACLOUD_HTTP_PORT".to_owned(),
            route.service_port.get().to_string(),
        );
    }
    bundle.service.lease_volumes = vec![
        iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
            volume_name: "root_disk".parse().expect("volume"),
            kind: SoraLeaseVolumeKindV1::PersistentRootLeaseVolume,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
            mount_path: "/".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(16 * 1024 * 1024 * 1024).expect("bytes"),
        },
        iroha_data_model::soracloud::SoraLeaseVolumeBindingV1 {
            volume_name: "shared_cache".parse().expect("volume"),
            kind: SoraLeaseVolumeKindV1::ServiceLeaseVolume,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
            mount_path: "/lease/shared-cache".to_owned(),
            max_total_bytes: std::num::NonZeroU64::new(512 * 1024 * 1024).expect("bytes"),
        },
    ];
    let mut state = test_state()?;
    let deployment_state = sample_deployment_state(&bundle);
    {
        let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
        world.soracloud_service_revisions_mut_for_testing().insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            bundle.clone(),
        );
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(bundle.service.service_name.clone(), deployment_state);
        world
            .soracloud_inrou_service_placements_mut_for_testing()
            .insert(
                (
                    bundle.service.service_name.to_string(),
                    bundle.service.service_version.clone(),
                ),
                iroha_data_model::soracloud::SoraInrouServicePlacementRecordV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_INROU_SERVICE_PLACEMENT_RECORD_VERSION_V1,
                    service_name: bundle.service.service_name.clone(),
                    service_version: bundle.service.service_version.clone(),
                    desired_replica_count: bundle.service.replicas.get(),
                    eligible_validator_count: 1,
                    placements: vec![SoraInrouReplicaPlacementV1 {
                        replica_slot: 1,
                        validator_account_id: ALICE_ID.clone(),
                        peer_id: local_peer_id.to_owned(),
                        selected_backend: SoraInrouRuntimeBackendV1::PortableVm,
                        selected_guest_isa,
                        selected_geography_tag: None,
                        selection_latency_ms: None,
                    }],
                    reconciled_at_ms: 1,
                    last_error: None,
                },
            );
    }
    let artifacts_root = temp_dir.path().join("artifacts");
    fs::create_dir_all(&artifacts_root)?;
    fs::write(
        artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
        &bundle_bytes,
    )?;
    let manager = SoracloudRuntimeManager::new(config, Arc::clone(&state));
    manager.reconcile_once()?;
    let service_dir = temp_dir
        .path()
        .join("services")
        .join(storage_path_component(bundle.service.service_name.as_ref()))
        .join(storage_path_component(&bundle.service.service_version));
    let runtime_state = wait_for_hosted_http_runtime_state_to_be_healthy(
        &manager,
        &service_dir,
        bundle.container.lifecycle.healthcheck_path.as_deref(),
        Duration::from_secs(30),
    )?;
    let replica = runtime_state
        .replicas
        .first()
        .expect("replica runtime state present");
    probe_hosted_http_health(
        replica
            .listen_base_url
            .as_deref()
            .expect("replica listen base url"),
        bundle.container.lifecycle.healthcheck_path.as_deref(),
    )?;
    Ok(())
}
#[test]
fn probe_hosted_http_health_accepts_paths_without_a_leading_slash() -> Result<()> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 1024];
            let read = std::io::Read::read(&mut stream, &mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..read]);
            let status_line = if request.starts_with("GET /healthz HTTP/1.1") {
                "HTTP/1.1 200 OK\r\n"
            } else {
                "HTTP/1.1 404 Not Found\r\n"
            };
            let body = if request.starts_with("GET /healthz HTTP/1.1") {
                b"ok\n".as_slice()
            } else {
                b"".as_slice()
            };
            let response = format!(
                "{status_line}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            let _ = std::io::Write::write_all(&mut stream, body);
        }
    });
    probe_hosted_http_health(&format!("http://{address}"), Some("healthz"))?;
    handle.join().expect("fixture thread should complete");
    Ok(())
}
#[test]
fn fetch_hosted_http_text_accepts_paths_without_a_leading_slash() -> Result<()> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 1024];
            let read = std::io::Read::read(&mut stream, &mut request).unwrap_or(0);
            let request = String::from_utf8_lossy(&request[..read]);
            let (status_line, body) = if request.starts_with("GET /root-slot HTTP/1.1") {
                ("HTTP/1.1 200 OK\r\n", b"replica-1\n".as_slice())
            } else {
                ("HTTP/1.1 404 Not Found\r\n", b"".as_slice())
            };
            let response = format!(
                "{status_line}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
            let _ = std::io::Write::write_all(&mut stream, body);
        }
    });
    let body = fetch_hosted_http_text(&format!("http://{address}"), "root-slot")?;
    handle.join().expect("fixture thread should complete");
    assert_eq!(body, "replica-1\n");
    Ok(())
}
#[test]
fn ivm_host_egress_fetch_enforces_allowlist_rate_and_byte_limits() -> Result<()> {
    let mut bundle = load_deployment_bundle_fixture()?;
    let body = b"hello-egress".to_vec();
    let expected_hash = Hash::new(&body);
    let (url, server) = spawn_http_fixture(body.clone())?;
    let (allowed_host, allowed_port) =
        url_host_port(&url).expect("fixture URL should include a host and port");
    bundle.container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            allowed_host,
            [allowed_port],
        )]);
    let temp_dir = tempfile::tempdir()?;
    let private_request = sample_ordered_mailbox_request(
        &bundle,
        "private_update",
        sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
    );
    let mut host = SoracloudIvmHost::new(
        private_request,
        temp_dir.path().to_path_buf(),
        iroha_config::parameters::actual::SoracloudRuntimeEgress {
            default_allow: false,
            allowed_hosts: vec!["127.0.0.1".to_owned()],
            rate_per_minute: std::num::NonZeroU32::new(1),
            max_bytes_per_minute: std::num::NonZeroU64::new(32),
        },
        BTreeMap::new(),
    );
    let response = host.egress_fetch(SoracloudEgressFetchRequestV1 {
        url: url.clone(),
        expected_hash: Some(expected_hash),
        max_bytes: 32,
    })?;
    server.join().expect("fixture server should complete");
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body, body);
    assert_eq!(response.body_hash, expected_hash);
    let rate_limited = host
        .egress_fetch(SoracloudEgressFetchRequestV1 {
            url,
            expected_hash: Some(expected_hash),
            max_bytes: 32,
        })
        .expect_err("second request must exceed the per-minute rate limit");
    assert_eq!(rate_limited, VMError::PermissionDenied);
    let disallowed = host
        .egress_fetch(SoracloudEgressFetchRequestV1 {
            url: "http://example.com/blocked".to_owned(),
            expected_hash: Some(Hash::new(b"blocked")),
            max_bytes: 32,
        })
        .expect_err("disallowed hosts must be rejected before fetch");
    assert_eq!(disallowed, VMError::PermissionDenied);
    let (url, server) = spawn_http_fixture(b"too-large".to_vec())?;
    let (allowed_host, allowed_port) =
        url_host_port(&url).expect("fixture URL should include a host and port");
    bundle.container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            allowed_host,
            [allowed_port],
        )]);
    let private_request = sample_ordered_mailbox_request(
        &bundle,
        "private_update",
        sample_mailbox_message(&bundle, "private_update", b"private-2".to_vec()),
    );
    let mut byte_limited_host = SoracloudIvmHost::new(
        private_request,
        temp_dir.path().to_path_buf(),
        iroha_config::parameters::actual::SoracloudRuntimeEgress {
            default_allow: false,
            allowed_hosts: vec!["127.0.0.1".to_owned()],
            rate_per_minute: std::num::NonZeroU32::new(5),
            max_bytes_per_minute: std::num::NonZeroU64::new(4),
        },
        BTreeMap::new(),
    );
    let byte_limited = byte_limited_host
        .egress_fetch(SoracloudEgressFetchRequestV1 {
            url,
            expected_hash: Some(Hash::new(b"too-large")),
            max_bytes: 16,
        })
        .expect_err("responses above the byte budget must be rejected");
    server.join().expect("fixture server should complete");
    assert_eq!(byte_limited, VMError::PermissionDenied);
    Ok(())
}
#[test]
fn ivm_host_egress_fetch_rejects_oversized_content_type() -> Result<()> {
    let mut bundle = load_deployment_bundle_fixture()?;
    let body = b"bounded-body".to_vec();
    let expected_hash = Hash::new(&body);
    let (url, server) = spawn_http_fixture_with_content_type(
        body,
        "x".repeat(SORACLOUD_EGRESS_CONTENT_TYPE_MAX_BYTES + 1),
    )?;
    let (allowed_host, allowed_port) =
        url_host_port(&url).expect("fixture URL should include a host and port");
    bundle.container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new(
            allowed_host,
            [allowed_port],
        )]);
    let temp_dir = tempfile::tempdir()?;
    let private_request = sample_ordered_mailbox_request(
        &bundle,
        "private_update",
        sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
    );
    let mut host = SoracloudIvmHost::new(
        private_request,
        temp_dir.path().to_path_buf(),
        iroha_config::parameters::actual::SoracloudRuntimeEgress {
            default_allow: false,
            allowed_hosts: vec!["127.0.0.1".to_owned()],
            rate_per_minute: std::num::NonZeroU32::new(1),
            max_bytes_per_minute: std::num::NonZeroU64::new(32),
        },
        BTreeMap::new(),
    );
    let error = host
        .egress_fetch(SoracloudEgressFetchRequestV1 {
            url,
            expected_hash: Some(expected_hash),
            max_bytes: 32,
        })
        .expect_err("oversized content type must fail before response materialization");
    server.join().expect("fixture server should complete");
    assert_eq!(error, VMError::PermissionDenied);
    assert_eq!(host.egress_requests, 0);
    assert_eq!(host.egress_bytes, 0);
    Ok(())
}
#[test]
fn ivm_host_egress_fetch_rejects_allowlisted_host_on_unlisted_port() -> Result<()> {
    let mut bundle = load_deployment_bundle_fixture()?;
    bundle.container.capabilities.network =
        SoraNetworkPolicyV1::Allowlist(vec![SoraNetworkAllowlistEntryV1::new("127.0.0.1", [443])]);
    let temp_dir = tempfile::tempdir()?;
    let private_request = sample_ordered_mailbox_request(
        &bundle,
        "private_update",
        sample_mailbox_message(&bundle, "private_update", b"private".to_vec()),
    );
    let mut host = SoracloudIvmHost::new(
        private_request,
        temp_dir.path().to_path_buf(),
        iroha_config::parameters::actual::SoracloudRuntimeEgress {
            default_allow: false,
            allowed_hosts: vec!["127.0.0.1".to_owned()],
            rate_per_minute: std::num::NonZeroU32::new(5),
            max_bytes_per_minute: std::num::NonZeroU64::new(32),
        },
        BTreeMap::new(),
    );
    let error = host
        .egress_fetch(SoracloudEgressFetchRequestV1 {
            url: "http://127.0.0.1:9/disallowed-port".to_owned(),
            expected_hash: Some(Hash::new(b"blocked")),
            max_bytes: 32,
        })
        .expect_err("requests on unlisted ports must be rejected before fetch");
    assert_eq!(error, VMError::PermissionDenied);
    Ok(())
}
#[test]
fn execute_ordered_mailbox_returns_deterministic_failure_for_missing_bundle_cache() -> Result<()> {
    let state = test_state()?;
    let mut bundle = load_deployment_bundle_fixture()?;
    let artifact_bytes = simple_soracloud_contract_artifact(&["apply_update"]);
    bundle.container.bundle_hash = Hash::new(&artifact_bytes);
    let temp_dir = tempfile::tempdir()?;
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf()),
        Arc::clone(&state),
    );
    let handle = test_runtime_handle(&manager, Arc::clone(&state));
    let request = sample_ordered_mailbox_request(
        &bundle,
        "update",
        sample_mailbox_message(&bundle, "update", b"missing-bundle".to_vec()),
    );
    let result = handle
        .execute_ordered_mailbox(request)
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    assert!(result.state_mutations.is_empty());
    assert!(result.outbound_mailbox_messages.is_empty());
    assert_eq!(
        result.runtime_state.expect("runtime state").health_status,
        SoraServiceHealthStatusV1::Degraded
    );
    assert_eq!(result.runtime_receipt.journal_artifact_hash, None);
    assert_eq!(result.runtime_receipt.checkpoint_artifact_hash, None);
    Ok(())
}
#[test]
fn warmed_ordered_mailbox_invalidates_a_changed_bundle_file() -> Result<()> {
    let state = test_state()?;
    let mut bundle = load_deployment_bundle_fixture()?;
    let artifact_bytes = simple_soracloud_contract_artifact(&["apply_update"]);
    bundle.container.bundle_hash = Hash::new(&artifact_bytes);
    let temp_dir = tempfile::tempdir()?;
    let artifacts_root = temp_dir.path().join("artifacts");
    fs::create_dir_all(&artifacts_root)?;
    let bundle_path = artifacts_root.join(hash_cache_name(bundle.container.bundle_hash));
    fs::write(&bundle_path, &artifact_bytes)?;
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf()),
        Arc::clone(&state),
    );
    let handle = test_runtime_handle(&manager, Arc::clone(&state));
    let request = sample_ordered_mailbox_request(
        &bundle,
        "update",
        sample_mailbox_message(&bundle, "update", b"validated".to_vec()),
    );
    let first = handle
        .execute_ordered_mailbox(request.clone())
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    assert_eq!(
        first.runtime_state.expect("runtime state").health_status,
        SoraServiceHealthStatusV1::Healthy
    );
    fs::write(&bundle_path, b"tampered Soracloud bundle")?;
    let changed = handle
        .execute_ordered_mailbox(request)
        .map_err(|error| eyre::eyre!("{error:?}"))?;
    assert_eq!(
        changed
            .runtime_state
            .expect("changed runtime state")
            .health_status,
        SoraServiceHealthStatusV1::Degraded
    );
    let stats = handle.ivm_runtime_cache_stats();
    assert_eq!(stats.artifact_reads, 2);
    assert_eq!(stats.artifact_hashes, 2);
    assert_eq!(stats.contract_preparations, 1);
    assert_eq!(stats.runtime_allocations, 1);
    assert_eq!(stats.invalidations, 1);
    assert_eq!(stats.prepared_entries, 0);
    assert_eq!(stats.idle_runtimes, 0);
    Ok(())
}
#[test]
fn execute_local_read_fails_closed_when_runtime_snapshot_is_behind() -> Result<()> {
    let mut state = test_state()?;
    let mut bundle = load_deployment_bundle_fixture()?;
    let bundle_bytes = b"ivm bundle bytes".to_vec();
    bundle.container.bundle_hash = Hash::new(&bundle_bytes);
    let temp_dir = tempfile::tempdir()?;
    let artifacts_root = temp_dir.path().join("artifacts");
    fs::create_dir_all(&artifacts_root)?;
    fs::write(
        artifacts_root.join(hash_cache_name(bundle.container.bundle_hash)),
        &bundle_bytes,
    )?;
    {
        let world = &mut Arc::get_mut(&mut state).expect("unique test state").world;
        world.soracloud_service_revisions_mut_for_testing().insert(
            (
                bundle.service.service_name.to_string(),
                bundle.service.service_version.clone(),
            ),
            bundle.clone(),
        );
        world
            .soracloud_service_deployments_mut_for_testing()
            .insert(
                bundle.service.service_name.clone(),
                sample_deployment_state(&bundle),
            );
        world.soracloud_service_runtime_mut_for_testing().insert(
            bundle.service.service_name.clone(),
            sample_runtime_state(&bundle),
        );
    }
    let manager = SoracloudRuntimeManager::new(
        test_runtime_manager_config(temp_dir.path().to_path_buf()),
        Arc::clone(&state),
    );
    manager.reconcile_once()?;
    manager.snapshot.write().observed_height = 99;
    let handle = test_runtime_handle(&manager, Arc::clone(&state));
    let error = handle
        .execute_local_read(SoracloudLocalReadRequest {
            observed_height: 0,
            observed_block_hash: None,
            service_name: bundle.service.service_name.to_string(),
            service_version: bundle.service.service_version.clone(),
            handler_name: "query".to_owned(),
            handler_class: iroha_core::soracloud_runtime::SoracloudLocalReadKind::Query,
            request_method: "GET".to_owned(),
            request_path: "/app/query".to_owned(),
            handler_path: "/".to_owned(),
            request_query: None,
            request_headers: BTreeMap::new(),
            request_body: Vec::new(),
            request_commitment: Hash::new(b"stale-query"),
        })
        .expect_err("stale runtime snapshots must fail closed");
    assert_eq!(error.kind, SoracloudRuntimeExecutionErrorKind::Unavailable);
    Ok(())
}
