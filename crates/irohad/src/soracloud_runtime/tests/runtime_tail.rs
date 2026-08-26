const INROU_HEALTH_SERVER_PY: &str = include_str!("fixtures/inrou_health_server.py");

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
