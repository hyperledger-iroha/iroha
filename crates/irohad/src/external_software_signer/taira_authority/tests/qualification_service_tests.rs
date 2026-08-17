use super::*;

pub(super) fn qualification_artifacts() -> Vec<(String, Vec<u8>)> {
    [
        (
            "source/capability/exact12-capability-manifest-v1.norito",
            b"NRT0qualification-capability-fixture-v1\0".as_slice(),
        ),
        (
            "source/sdk/iroha_python_privacy_v1.whl",
            b"qualification-wheel-fixture-v1\0".as_slice(),
        ),
        (
            "source/worker/iroha_privacy_wallet_worker",
            b"qualification-worker-fixture-v1\0".as_slice(),
        ),
        (
            "source/abi22/libconnect_norito_bridge.so",
            b"qualification-abi22-fixture-v1\0".as_slice(),
        ),
    ]
    .into_iter()
    .map(|(name, bytes)| (name.to_owned(), bytes.to_vec()))
    .collect()
}

fn qualification_manifest(
    artifacts: &[(String, Vec<u8>)],
) -> Vec<super::super::protocol::TairaAuthorityArtifactManifestEntryV1> {
    artifacts
        .iter()
        .enumerate()
        .map(|(ordinal, (name, bytes))| {
            super::super::protocol::TairaAuthorityArtifactManifestEntryV1 {
                ordinal: ordinal as u16,
                name: name.clone(),
                size: bytes.len() as u64,
                sha256: Sha256::digest(bytes).into(),
            }
        })
        .collect()
}

fn stage_qualification_artifacts(parent: &Path, artifacts: &[(String, Vec<u8>)]) -> Vec<PathBuf> {
    artifacts
        .iter()
        .enumerate()
        .map(|(ordinal, (_, bytes))| {
            create_artifact(parent, &format!("qualification-{ordinal}.bin"), bytes)
        })
        .collect()
}

fn qualification_descriptors(paths: &[PathBuf]) -> Vec<OwnedFd> {
    read_only_descriptors(&paths.iter().map(PathBuf::as_path).collect::<Vec<_>>())
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn ordinary_qualification_artifacts_cannot_select_the_test_probe() {
    if rustix::process::geteuid().as_raw() != 0 {
        eprintln!("qualification production-selection test requires Linux root");
        return;
    }

    let artifacts = qualification_artifacts();
    let manifest = qualification_manifest(&artifacts);
    let parent = temporary_parent();
    let paths = stage_qualification_artifacts(parent.path(), &artifacts);
    let mut files = paths
        .iter()
        .map(|path| {
            OpenOptions::new()
                .read(true)
                .open(path)
                .expect("open qualification artifact")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        super::super::sandbox::qualification_test_sandbox_run_count(),
        None,
    );
    let started = std::time::Instant::now();
    let result = super::super::sandbox::run_qualification_probes(&mut files, &manifest);
    assert!(
        result.is_err(),
        "ordinary artifacts selected the test probe"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "invalid production artifacts were not refused promptly",
    );
    assert_eq!(
        super::super::sandbox::qualification_test_sandbox_run_count(),
        None,
        "artifact bytes must not create the test-only selection capability",
    );
}

#[cfg(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn qualification_full_service_sandbox_authorize_replay_recover_and_verify() {
    if rustix::process::geteuid().as_raw() != 0 {
        eprintln!("qualification full-service sandbox test requires Linux root");
        return;
    }

    let artifacts = qualification_artifacts();
    let manifest = qualification_manifest(&artifacts);
    let expected_probe_results = super::super::sandbox::qualification_test_probe_result(&manifest);
    let artifact_refs = artifacts
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect::<Vec<_>>();
    let parent = temporary_parent();
    let paths = stage_qualification_artifacts(parent.path(), &artifacts);
    let fixture = ClientRequestFixtureV1::new(
        TairaAuthorityRoleV1::Qualification,
        "authority-owned-sandbox",
        &artifact_refs,
    );
    let service = provision(parent.path(), fixture.role);
    assign_active_run(&service, &fixture);

    let ((), sandbox_runs) = super::super::sandbox::with_qualification_test_sandbox(|| {
        let authorized = authorize(
            &service,
            &fixture.request_json(),
            qualification_descriptors(&paths),
            TEST_NOW_MILLIS_V1,
        )
        .expect("authorize qualification through authority-owned sandbox");
        assert_eq!(authorized.status, OperationStatusV1::Ok);
        assert_eq!(result_status(&authorized), "authorized");
        let authority_envelope = sidecar_bytes(&authorized, "authority_envelope");
        let durable_receipt = sidecar_bytes(&authorized, "durable_receipt");
        let envelope = parse_json(&authority_envelope);
        let actual_probe_results = envelope
            .get("claims")
            .and_then(|claims| claims.get("role_result"))
            .and_then(|role_result| role_result.get("probe_results"))
            .expect("signed qualification probe result claims");
        assert_eq!(actual_probe_results, &expected_probe_results);

        let persisted: BTreeMap<[u8; 32], StoredAuthorizationV1> =
            load_canonical_records(&state_directory(parent.path()).join("authority-receipts-v1"))
                .expect("load persisted qualification sidecars");
        let stored = persisted
            .get(&fixture.operation_id)
            .expect("persisted qualification authorization");
        assert_eq!(stored.authority_envelope_json, authority_envelope);
        assert_eq!(stored.durable_receipt_json, durable_receipt);

        let after_authorize = service
            .provenance()
            .expect("qualification authorization provenance");
        let replayed = authorize(
            &service,
            &fixture.request_json(),
            qualification_descriptors(&paths),
            TEST_NOW_MILLIS_V1 + 1,
        )
        .expect("replay exact qualification authorization");
        assert_eq!(replayed.status, OperationStatusV1::Replayed);
        assert_eq!(
            sidecar_bytes(&replayed, "authority_envelope"),
            authority_envelope
        );
        assert_eq!(sidecar_bytes(&replayed, "durable_receipt"), durable_receipt);
        assert_eq!(
            service
                .provenance()
                .expect("qualification replay provenance"),
            after_authorize,
        );

        let verification = verification_json(&fixture, &authorized);
        let client_uid = service
            .public_binding()
            .expect("qualification binding")
            .signer
            .client_uid;
        drop(service);
        let recovered =
            TairaAuthorityServiceV1::open(state_directory(parent.path()), wrapping_key())
                .expect("reopen qualification authority");
        assert_eq!(
            recovered
                .provenance()
                .expect("recovered qualification provenance"),
            after_authorize,
        );
        let recovered_replay = authorize(
            &recovered,
            &fixture.request_json(),
            qualification_descriptors(&paths),
            TEST_NOW_MILLIS_V1 + 2,
        )
        .expect("replay qualification after reopen");
        assert_eq!(recovered_replay.status, OperationStatusV1::Replayed);
        assert_eq!(
            sidecar_bytes(&recovered_replay, "authority_envelope"),
            authority_envelope
        );
        assert_eq!(
            sidecar_bytes(&recovered_replay, "durable_receipt"),
            durable_receipt
        );
        let verified = recovered
            .verify_json(&verification, qualification_descriptors(&paths), client_uid)
            .expect("historically verify qualification sidecars");
        assert_eq!(verified.status, OperationStatusV1::Ok);
        assert_eq!(result_status(&verified), "valid");
        assert_eq!(
            recovered
                .provenance()
                .expect("qualification historical verification provenance"),
            after_authorize,
            "replay, reopen, and history must not rerun or re-sign",
        );
    });
    assert_eq!(sandbox_runs, 1, "only fresh authorization runs probes");
}

#[cfg(not(all(
    target_os = "linux",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
#[test]
fn qualification_service_probe_remains_fail_closed_off_linux() {
    let artifacts = qualification_artifacts();
    let manifest = qualification_manifest(&artifacts);
    let parent = temporary_parent();
    let paths = stage_qualification_artifacts(parent.path(), &artifacts);
    let mut files = paths
        .iter()
        .map(|path| {
            OpenOptions::new()
                .read(true)
                .open(path)
                .expect("open artifact")
        })
        .collect::<Vec<_>>();
    let (result, sandbox_runs) = super::super::sandbox::with_qualification_test_sandbox(|| {
        super::super::sandbox::run_qualification_probes(&mut files, &manifest)
    });
    assert_eq!(result, Err(TairaAuthorityErrorV1::Rejected));
    assert_eq!(
        sandbox_runs, 0,
        "the hermetic Linux fixture must not bypass the off-Linux refusal",
    );
}
