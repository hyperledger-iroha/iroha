"""Terminal publication cases executed by the parent release-receipt suite."""

def test_receipt_hashes_every_formal_matrix_chaos_and_soak_artifact(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    evidence = make_evidence(tmp_path)
    writer = fixture_writer(tmp_path)
    output = terminal_output_path(evidence)
    result = run_writer(evidence, output, writer)

    assert result.returncode == 0, result.stderr
    receipt = json.loads(output.read_text(encoding="utf-8"))
    assert receipt["result"] == "release-complete"
    assert receipt["identity"] == {
        "head_commit": evidence["head"],
        "head_tree": evidence["tree"],
        "index_tree": evidence["tree"],
        "cargo_lock_sha256": evidence["lock"],
        "candidate_source_manifest_sha256": evidence["candidate_manifest"],
        "sealed_source_manifest_sha256": evidence["sealed_manifest"],
    }
    assert receipt["authentication"]["schema_version"] == 2
    release_authentication = receipt["authentication"]["release_identity"]
    bootstrap_authentication = receipt["authentication"]["bootstrap"]
    assert release_authentication["signature_format"] == "ssh"
    assert release_authentication["verification_status"] == "G"
    assert release_authentication["candidate_commit_oid"] == evidence["head"]
    assert release_authentication["signer_fingerprint"] == evidence[
        "expected_signer_fingerprint"
    ]
    assert release_authentication["allowed_signers_principal"] == evidence[
        "signer_principal"
    ]
    assert release_authentication["replay"]["performed"] is True
    assert release_authentication["trust_policy"] == {
        "git_sha256": evidence["expected_git_sha256"],
        "ssh_keygen_sha256": evidence["expected_ssh_keygen_sha256"],
        "allowed_signers_sha256": evidence["expected_allowed_signers_sha256"],
        "revocation_sha256": evidence["expected_revocation_sha256"],
        "signer_fingerprint": evidence["expected_signer_fingerprint"],
    }
    assert bootstrap_authentication["completion_sha256"] == evidence[
        "expected_bootstrap_completion_sha256"
    ]
    assert bootstrap_authentication["frozen_bootstrap_sha256"] == (
        "38e5ce3632e1d7dc0471b49d90350a165fb8326c34ea3d70187a309c3d96358f"
    )
    assert bootstrap_authentication["candidate_commit_oid"] == evidence["head"]
    bootstrap_completion = evidence["bootstrap_completion"]
    assert isinstance(bootstrap_completion, Path)
    assert receipt["evidence"]["bootstrap"]["completion"] == {
        "archive_id": "release-bootstrap.completion.v2",
        "mode": "0400",
        "sha256": sha256(bootstrap_completion),
        "size_bytes": bootstrap_completion.stat().st_size,
    }
    sdk_dependencies = receipt["evidence"]["sdk_dependencies"]
    sdk_manifest = (
        Path(evidence["bootstrap_evidence_dir"])
        / "sdk-dependency-bundle-manifest.json"
    )
    assert sdk_dependencies == {
        "schema_version": 1,
        "source_disclosure": "withheld",
        "source_manifest_sha256": sha256(sdk_manifest),
        "source_state_sha256": "e" * 64,
        "archive": {
            "archive_id": "release-sdk-dependencies.bundle.v1",
            "archive_name": "sdk-dependency-bundle.tar",
            "mode": "0400",
            "sha256": sha256(Path(evidence["sdk_dependency_archive"])),
            "size_bytes": Path(evidence["sdk_dependency_archive"]).stat().st_size,
        },
        "input_inventory": {
            "archive_id": "release-sdk-dependencies.input-inventory.v1",
            "archive_name": "sdk-dependency-input.json",
            "mode": "0400",
            "sha256": sha256(Path(evidence["sdk_dependency_input_inventory"])),
            "size_bytes": Path(
                evidence["sdk_dependency_input_inventory"]
            ).stat().st_size,
        },
        "final_work_inventory": {
            "archive_id": "release-sdk-dependencies.work-final.v1",
            "archive_name": "sdk-dependency-work-final.json",
            "mode": "0400",
            "sha256": sha256(
                Path(evidence["sdk_dependency_final_work_inventory"])
            ),
            "size_bytes": Path(
                evidence["sdk_dependency_final_work_inventory"]
            ).stat().st_size,
        },
    }
    assert "path" not in json.dumps(sdk_dependencies)
    assert str(Path(evidence["sdk_dependency_archive"]).parent) not in json.dumps(
        sdk_dependencies
    )
    runtime_probe_result = Path(evidence["runtime_tool_probe_result"])
    runtime_tool_probes = receipt["evidence"]["runtime_tool_probes"]
    expected_runtime_probe_value = json.loads(
        runtime_probe_result.read_text(encoding="ascii")
    )
    assert runtime_tool_probes == {
        "format": "iroha-sumeragi-v2-release-tool-functional-probes",
        "schema_version": 1,
        "host_family": "darwin" if sys.platform == "darwin" else "linux",
        "probe_contract_sha256": expected_runtime_probe_value[
            "probe_contract_sha256"
        ],
        "tool_count": 41,
        "result": {
            "archive_id": "release-runtime.tool-probes.v1",
            "archive_name": "runtime-tool-probe-result.json",
            "mode": "0400",
            "sha256": sha256(runtime_probe_result),
            "size_bytes": runtime_probe_result.stat().st_size,
        },
    }
    assert "path" not in json.dumps(runtime_tool_probes)
    assert str(runtime_probe_result.parent) not in json.dumps(runtime_tool_probes)
    assert "/operator/" not in output.read_text(encoding="utf-8")
    approval_evidence = receipt["evidence"]["bootstrap"]["release_approvals"]
    approval_authentication = bootstrap_authentication["release_approvals"]
    assert set(approval_evidence["class_attestations"]) == set(
        APPROVAL_CLASS_IDS
    )
    assert approval_authentication == {
        "archive_id": "release-approval.set-attestation.v1",
        "sha256": approval_evidence["set_attestation"]["sha256"],
        "operation_plan_sha256": approval_evidence["operation_plan_sha256"],
    }
    approval_public_bytes = canonical_json(approval_evidence)
    assert b'"arguments"' not in approval_public_bytes
    assert str(Path(evidence["bootstrap_evidence_dir"])).encode() not in (
        approval_public_bytes
    )
    raw_approval = (
        Path(evidence["bootstrap_evidence_dir"])
        / "offline-toolchain-sdk.approval.v1.json"
    )
    raw_approval.chmod(0o600)
    rejected_approval = run_writer(
        evidence, output, writer, verify_existing=True
    )
    assert rejected_approval.returncode == 1
    assert "approval_offline_toolchain_sdk" in rejected_approval.stderr
    raw_approval.chmod(0o400)

    module = load_writer_module()
    sdk_inventory_document = json.loads(
        Path(evidence["sdk_dependency_input_inventory"]).read_text(
            encoding="utf-8"
        )
    )
    sdk_records, sdk_by_path, _ = module._sdk_inventory_records(
        sdk_inventory_document["records"],
        "fixture SDK inventory",
        root_mode="0500",
    )
    sdk_bindings = module._sdk_binding_contract(
        sdk_inventory_document["bindings"], sdk_by_path
    )
    private_attack_root = tmp_path / "sdk-private-manifest-attacks"
    private_attack_root.mkdir(mode=0o700)

    def reject_private_manifest(
        document: dict[str, object], expected: str,
    ) -> None:
        path = private_attack_root / f"manifest-{len(tuple(private_attack_root.iterdir()))}.json"
        path.write_bytes(canonical_json(document))
        path.chmod(0o400)
        with pytest.raises(module.ReceiptError, match=expected):
            module._sdk_validate_private_source_manifest(
                path=path,
                expected_sha256=sha256(path),
                archive_records=sdk_records,
                bindings=sdk_bindings,
                expected_git_sha256=evidence["expected_git_sha256"],
            )

    missing_node_member = json.loads(sdk_manifest.read_text(encoding="utf-8"))
    node_inventory = missing_node_member["node"]["node_modules_inventory"]
    node_inventory["records"] = [
        record for record in node_inventory["records"]
        if record["path"] != "fixture/index.js"
    ]
    node_inventory["record_count"] = len(node_inventory["records"])
    node_inventory["file_bytes"] = sum(
        record.get("size", 0) for record in node_inventory["records"]
    )
    node_inventory["records_sha256"] = hashlib.sha256(
        json.dumps(
            node_inventory["records"],
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()
    reject_private_manifest(missing_node_member, "retained archive subtree")

    missing_openapi_member = json.loads(sdk_manifest.read_text(encoding="utf-8"))
    openapi_inventory = missing_openapi_member["openapi_node"][
        "node_modules_inventory"
    ]
    openapi_inventory["records"] = [
        record for record in openapi_inventory["records"]
        if record["path"] != "fixture/index.js"
    ]
    openapi_inventory["record_count"] = len(openapi_inventory["records"])
    openapi_inventory["file_bytes"] = sum(
        record.get("size", 0) for record in openapi_inventory["records"]
    )
    openapi_inventory["records_sha256"] = hashlib.sha256(
        json.dumps(
            openapi_inventory["records"], ensure_ascii=True,
            sort_keys=True, separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()
    reject_private_manifest(missing_openapi_member, "retained archive subtree")

    dirty_swift_member = json.loads(sdk_manifest.read_text(encoding="utf-8"))
    swift_inventory = dirty_swift_member["swiftpm"]["cache_inventory"]
    for record in swift_inventory["records"]:
        if record["path"] == "checkouts/fixture/Sources/Fixture.swift":
            record["sha256"] = "f" * 64
    swift_inventory["records_sha256"] = hashlib.sha256(
        json.dumps(
            swift_inventory["records"],
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
    ).hexdigest()
    reject_private_manifest(dirty_swift_member, "retained archive subtree")

    wrong_gradle_key = json.loads(sdk_manifest.read_text(encoding="utf-8"))
    wrong_gradle_key["gradle"]["wrapper_cache_key"] = "0" * 24
    reject_private_manifest(wrong_gradle_key, "private source bindings")
    signature_artifacts = {
        "release_signature_attestation": "signature_attestation",
        "release_signature_transcript": "signature_transcript",
        "release_signature_raw_commit": "signature_raw_commit",
        "release_signature_cargo_lock": "signature_cargo_lock",
        "release_signature_allowed_signers": "signature_allowed_signers",
        "release_signature_revocation": "signature_revocation",
        "release_signature_git": "signature_git",
        "release_signature_ssh_keygen": "signature_ssh_keygen",
    }
    for receipt_name, fixture_name in signature_artifacts.items():
        fixture_path = evidence[fixture_name]
        assert isinstance(fixture_path, Path)
        expected_mode = "0500" if fixture_name in {
            "signature_git",
            "signature_ssh_keygen",
        } else "0400"
        assert receipt["evidence"][receipt_name] == {
            "path": str(fixture_path.resolve()),
            "sha256": sha256(fixture_path),
            "size_bytes": fixture_path.stat().st_size,
            "mode": expected_mode,
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    expected_artifacts = {
        "corridor_completion": "corridor_completion",
        "corridor_summary": "corridor_summary",
        "corridor_production_inventory": "corridor_required",
        "g_unit_focused_test_inventory": "corridor_g_unit",
        "formal_completion": "formal_completion",
        "formal_gate_log": "formal_log",
        "formal_proof_coverage": "formal_ledger",
        "formal_proof_evidence": "formal_evidence",
        "formal_verus_evidence": "formal_verus_evidence",
        "formal_verus_log": "formal_verus_log",
        "formal_multilane_apalache_evidence": (
            "formal_multilane_apalache_evidence"
        ),
        "formal_cross_tool_evidence": "formal_cross_tool_evidence",
        "formal_production_trace_extraction_evidence": (
            "formal_production_trace_extraction_evidence"
        ),
        "formal_harness_lock": "formal_harness_lock",
        "formal_toolchain": "formal_toolchain",
        "formal_tlaps_resource_jsonl": "formal_tlaps_resource_jsonl",
        "formal_tlaps_resource_summary": "formal_tlaps_resource_summary",
        "seed_matrix_completion": "seed_completion",
        "seed_matrix_summary": "seed_summary",
        "seed_matrix_localnet_manifest_index": "seed_localnet_manifest_index",
        "chaos_completion": "chaos_completion",
        "chaos_log": "chaos_log",
    }
    for receipt_name, fixture_name in expected_artifacts.items():
        fixture_path = evidence[fixture_name]
        assert isinstance(fixture_path, Path)
        assert receipt["evidence"][receipt_name] == {
            "path": str(fixture_path.resolve()),
            "sha256": sha256(fixture_path),
        }
    seed_logs = evidence["seed_logs"]
    assert isinstance(seed_logs, list)
    assert receipt["evidence"]["seed_matrix_run_logs"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)} for path in seed_logs
    ]
    seed_localnet_manifests = evidence["seed_localnet_manifests"]
    assert isinstance(seed_localnet_manifests, list)
    assert receipt["evidence"]["seed_matrix_localnet_manifests"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)}
        for path in seed_localnet_manifests
    ]
    corridor_logs = evidence["corridor_logs"]
    assert isinstance(corridor_logs, list)
    assert receipt["evidence"]["corridor_logs"] == [
        {"path": str(path.resolve()), "sha256": sha256(path)}
        for path in corridor_logs
    ]
    diagnostics_legs = {
        Path(artifact["path"]).stem.split("-", 1)[1]
        for artifact in receipt["evidence"]["corridor_logs"]
        if "-sumeragi-diagnostics-" in Path(artifact["path"]).name
    }
    assert diagnostics_legs == {
        "sumeragi-diagnostics-rust",
        "sumeragi-diagnostics-python",
        "sumeragi-diagnostics-javascript",
        "sumeragi-diagnostics-swift",
        "sumeragi-diagnostics-kotlin",
        "sumeragi-diagnostics-java",
    }
    proof_fidelity_logs = [
        artifact
        for artifact in receipt["evidence"]["corridor_logs"]
        if artifact["path"].endswith("-preflight-proof-fidelity.log")
    ]
    assert len(proof_fidelity_logs) == 1
    scaling_preflight_logs = [
        artifact
        for artifact in receipt["evidence"]["corridor_logs"]
        if artifact["path"].endswith("-preflight-multilane-scaling.log")
    ]
    assert len(scaling_preflight_logs) == 1

    prebuilt = receipt["evidence"]["prebuilt_binary_bundle"]
    prebuilt_manifest = evidence["prebuilt_manifest"]
    prebuilt_binaries = evidence["prebuilt_binaries"]
    prebuilt_bundle = evidence["prebuilt_bundle"]
    assert isinstance(prebuilt_manifest, Path)
    assert isinstance(prebuilt_binaries, list)
    assert isinstance(prebuilt_bundle, Path)
    assert prebuilt["schema_version"] == 3
    assert prebuilt["manifest"] == {
        "archive_id": "release-prebuilt.manifest.v2",
        "sha256": sha256(prebuilt_manifest),
        "size_bytes": prebuilt_manifest.stat().st_size,
        "mode": "0400",
    }
    assert prebuilt["source_manifest_sha256"] == evidence["sealed_manifest"]
    assert prebuilt["cargo_lock_sha256"] == evidence["lock"]
    assert prebuilt["archive_id"] == (
        f"release-prebuilt.bundle.v1:{prebuilt_bundle.name}"
    )
    assert prebuilt["host_triple"] == PREBUILT_HOST_TRIPLE
    assert prebuilt["target_triple"] == PREBUILT_HOST_TRIPLE
    assert prebuilt["profile"] == "release"
    assert prebuilt["version_transcripts"] == {
        "cargo": {
            "operation_id": "cargo.version.v1",
            "tool_archive_id": "release-runner-tool.cargo.v1",
            "sha256": hashlib.sha256(CARGO_VERSION_OUTPUT).hexdigest(),
            "size_bytes": len(CARGO_VERSION_OUTPUT),
        },
        "rustc": {
            "operation_id": "rustc.version.v1",
            "tool_archive_id": "release-runner-tool.rustc.v1",
            "sha256": hashlib.sha256(RUSTC_VERSION_OUTPUT).hexdigest(),
            "size_bytes": len(RUSTC_VERSION_OUTPUT),
        },
    }
    assert prebuilt["binaries"] == [
        {
            "role": role,
            "relative_path": relative,
            "archive_id": f"release-prebuilt.binary.{role}.v1",
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": "0500",
        }
        for (role, relative), path in zip(
            (
                ("irohad", "release/iroha3d"),
                (
                    "irohad_message_control",
                    "message-control/release/iroha3d",
                ),
                ("iroha", "release/iroha"),
                ("kagami", "release/kagami"),
            ),
            prebuilt_binaries,
        )
    ]

    scaling_root = evidence["scaling_root"]
    assert isinstance(scaling_root, Path)
    scaling_bundle = receipt["evidence"]["multilane_scaling_bundle"]
    scaling_paths = sorted(
        path for path in scaling_root.rglob("*") if path.is_file()
    )
    assert scaling_bundle["archive_id"] == "release-scaling.bundle.v1"
    assert scaling_bundle["file_count"] == len(scaling_paths)
    assert scaling_bundle["total_size_bytes"] == sum(
        path.stat().st_size for path in scaling_paths
    )
    assert [record["relative_path"] for record in scaling_bundle["files"]] == [
        path.relative_to(scaling_root).as_posix() for path in scaling_paths
    ]
    for record, path in zip(scaling_bundle["files"], scaling_paths):
        assert record == {
            "archive_id": (
                "release-scaling.file.v1:"
                + path.relative_to(scaling_root).as_posix()
            ),
            "relative_path": path.relative_to(scaling_root).as_posix(),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": f"{path.stat().st_mode & 0o7777:04o}",
        }
    release_root = evidence["release_root"]
    assert isinstance(release_root, Path)
    expected_retained_tooling = []
    for role, source_path in (
        ("localnet", "scripts/deploy_localnet.sh"),
        ("load_generator", "scripts/tx_load.py"),
        ("nexus_load_bundle", "scripts/nexus_lane_load_test.py"),
    ):
        retained_path = release_root / source_path
        expected_retained_tooling.append(
            {
                "role": role,
                "archive_id": f"release-scaling.retained-tool.{role}.v1",
                "sha256": sha256(retained_path),
                "size_bytes": retained_path.stat().st_size,
                "mode": f"{retained_path.stat().st_mode & 0o7777:04o}",
            }
        )
    assert receipt["evidence"]["multilane_scaling_trust_anchors"] == {
        "trial_harness_sha256": evidence[
            "expected_scaling_trial_harness_sha256"
        ],
        "configuration_sha256": evidence[
            "expected_scaling_configuration_sha256"
        ],
        "irohad_sha256": evidence["expected_scaling_irohad_sha256"],
        "iroha_cli_sha256": evidence["expected_scaling_iroha_cli_sha256"],
        "retained_tooling": expected_retained_tooling,
    }

    g4p = receipt["evidence"]["g4p_multilane"]
    assert g4p["schema_version"] == 1
    g4p_logs = evidence["g4p_logs"]
    assert isinstance(g4p_logs, list)
    g4p_expected = {
        "completion": evidence["g4p_completion"],
        "run_summary": evidence["g4p_summary"],
    }
    for receipt_name, path in g4p_expected.items():
        assert isinstance(path, Path)
        assert g4p[receipt_name] == {
            "path": str(path.resolve()),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": f"{path.stat().st_mode & 0o7777:04o}",
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    assert [record["path"] for record in g4p["run_logs"]] == [
        str(path.resolve()) for path in g4p_logs
    ]

    g12 = receipt["evidence"]["g12_cross_dataspace"]
    g12_seed_logs = evidence["g12_seed_logs"]
    assert isinstance(g12_seed_logs, list)
    g12_expected = {
        "seed_completion": evidence["g12_seed_completion"],
        "seed_summary": evidence["g12_seed_summary"],
        "fault_soak_completion": evidence["g12_soak_completion"],
        "fault_soak_log": evidence["g12_soak_log"],
    }
    for receipt_name, path in g12_expected.items():
        assert isinstance(path, Path)
        assert g12[receipt_name] == {
            "path": str(path.resolve()),
            "sha256": sha256(path),
            "size_bytes": path.stat().st_size,
            "mode": f"{path.stat().st_mode & 0o7777:04o}",
            "owner_uid": os.geteuid(),
            "nlink": 1,
        }
    assert [record["path"] for record in g12["seed_run_logs"]] == [
        str(path.resolve()) for path in g12_seed_logs
    ]

    private_sdk_manifest = Path(evidence["bootstrap_evidence_dir"]) / (
        "sdk-dependency-bundle-manifest.json"
    )
    runtime_probe_manifest = Path(evidence["runtime_tool_probe_manifest"])
    invocation_root = release_root.parent
    artifact_root = invocation_root / "output"
    pruned_build_roots = {
        *(invocation_root / name for name in (
            "runtime", "sdk-inputs", "sdk-work", "target",
        )),
        *(artifact_root / name for name in (
            "home", "tmp", "cache", "cargo-home",
        )),
    }
    for path in pruned_build_roots:
        if path.exists():
            shutil.rmtree(path)
    runtime_probe_manifest_bytes = runtime_probe_manifest.read_bytes()
    runtime_probe_manifest.unlink()
    retained_private_source = run_writer(
        evidence, output, writer, replay_existing=True
    )
    assert retained_private_source.returncode == 1
    assert "survived acknowledgment pruning" in retained_private_source.stderr
    private_sdk_manifest_bytes = private_sdk_manifest.read_bytes()
    private_sdk_manifest.unlink()
    evidence_directory = Path(evidence["bootstrap_evidence_dir"])
    runner_logs = tuple(
        evidence_directory / name
        for name in ("runner-stdout.log", "runner-stderr.log")
    )
    for path in runner_logs:
        path.chmod(0o400)
    replayed = run_writer(evidence, output, writer, replay_existing=True)
    assert replayed.returncode == 0, replayed.stderr
    recreated_target = invocation_root / "target"
    recreated_target.mkdir(mode=0o700)
    recreated = run_writer(evidence, output, writer, replay_existing=True)
    assert recreated.returncode == 1
    assert "survived retained-release pruning" in recreated.stderr
    recreated_target.rmdir()
    private_sdk_manifest.write_bytes(private_sdk_manifest_bytes)
    private_sdk_manifest.chmod(0o400)
    runtime_probe_manifest.write_bytes(runtime_probe_manifest_bytes)
    runtime_probe_manifest.chmod(0o400)
    for path in runner_logs:
        path.chmod(0o600)

    module = load_writer_module()
    candidate = evidence["candidate"]
    sealed = evidence["sealed"]
    assert isinstance(candidate, Path)
    assert isinstance(sealed, Path)
    candidate_contract = module._capture_path_contract(
        candidate,
        "fixture candidate identity",
        expected_sha256=sha256(candidate),
    )
    sealed_contract = module._capture_path_contract(
        sealed,
        "fixture sealed identity",
        expected_sha256=sha256(sealed),
    )
    contracts = module._snapshot_receipt_inputs(
        receipt,
        candidate_identity=candidate_contract,
        sealed_identity=sealed_contract,
        scaling_root=scaling_root,
        bootstrap_evidence_root=evidence["bootstrap_evidence_dir"],
        candidate_root=evidence["bootstrap_candidate_root"],
        release_root=release_root,
        bootstrap_runtime_contracts=[],
        private_build_roots_available=False,
    )
    directory_paths = {
        contract.path
        for contract in contracts
        if isinstance(contract, module.DirectoryContract)
    }
    assert {invocation_root, artifact_root} <= directory_paths
    assert pruned_build_roots.isdisjoint(directory_paths)

    family_specs = (
        (
            "corridor_completion",
            (
                "corridor_completion",
                "corridor_summary",
                "corridor_production_inventory",
                "g_unit_focused_test_inventory",
                "corridor_logs",
            ),
        ),
        (
            "formal_completion",
            (
                "formal_completion",
                "formal_gate_log",
                "formal_proof_coverage",
                "formal_proof_evidence",
                "formal_verus_evidence",
                "formal_verus_log",
                "formal_multilane_apalache_evidence",
                "formal_cross_tool_evidence",
                "formal_production_trace_extraction_evidence",
                "formal_harness_lock",
                "formal_toolchain",
                "formal_tlaps_resource_jsonl",
                "formal_tlaps_resource_summary",
            ),
        ),
        (
            "seed_matrix_completion",
            (
                "seed_matrix_completion",
                "seed_matrix_summary",
                "seed_matrix_run_logs",
                "seed_matrix_localnet_manifest_index",
                "seed_matrix_localnet_manifests",
            ),
        ),
        ("chaos_completion", ("chaos_completion", "chaos_log")),
    )

    def artifact_paths(value: object) -> list[Path]:
        paths: list[Path] = []
        if isinstance(value, dict):
            if isinstance(value.get("path"), str) and isinstance(
                value.get("sha256"), str
            ):
                paths.append(Path(value["path"]))
            for child in value.values():
                paths.extend(artifact_paths(child))
        elif isinstance(value, list):
            for child in value:
                paths.extend(artifact_paths(child))
        return paths

    family_roots: set[Path] = set()
    expected_family_directories: set[Path] = set()
    for completion_key, member_keys in family_specs:
        root = Path(receipt["evidence"][completion_key]["path"]).parent
        family_roots.add(root)
        expected_family_directories.add(root)
        for member_key in member_keys:
            for path in artifact_paths(receipt["evidence"][member_key]):
                parent = path.parent
                while True:
                    expected_family_directories.add(parent)
                    if parent == root:
                        break
                    parent = parent.parent
    actual_family_directories = {
        path
        for path in directory_paths
        if any(path == root or root in path.parents for root in family_roots)
    }
    assert actual_family_directories == expected_family_directories

    corridor_root = Path(
        receipt["evidence"]["corridor_completion"]["path"]
    ).parent
    corridor_metadata = corridor_root.stat()
    real_fsync = module.os.fsync

    def fail_corridor_root_fsync(descriptor: int) -> None:
        metadata = os.fstat(descriptor)
        if (metadata.st_dev, metadata.st_ino) == (
            corridor_metadata.st_dev,
            corridor_metadata.st_ino,
        ):
            raise OSError("fixture corridor root fsync failure")
        real_fsync(descriptor)

    unpublished = output.with_name("UNPUBLISHED.json")
    monkeypatch.setattr(module.os, "fsync", fail_corridor_root_fsync)
    with pytest.raises(module.ReceiptError, match="fsync failed"):
        module._fsync_receipt_inputs(contracts)
    assert not unpublished.exists()

    replacement_identity = candidate.with_name("candidate-replacement.json")
    replacement_identity.write_bytes(candidate.read_bytes())
    os.replace(replacement_identity, candidate)
    with pytest.raises(
        module.ReceiptError, match="changed after semantic validation"
    ):
        module._snapshot_receipt_inputs(
            receipt,
            candidate_identity=candidate_contract,
            sealed_identity=sealed_contract,
            scaling_root=scaling_root,
            bootstrap_evidence_root=evidence["bootstrap_evidence_dir"],
            candidate_root=evidence["bootstrap_candidate_root"],
            release_root=release_root,
            bootstrap_runtime_contracts=[],
            private_build_roots_available=False,
        )

def private_output(tmp_path: Path) -> tuple[Path, Path]:
    directory = tmp_path / "private-output"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    return directory, directory / "RELEASE_COMPLETED.json"


def test_terminal_publication_is_no_clobber_and_durable(tmp_path: Path) -> None:
    _case_release_approval_file_protection_and_bounds_fail_closed()
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    data = canonical_json({"result": "release-complete"})
    revalidations = 0

    def revalidate() -> None:
        nonlocal revalidations
        revalidations += 1

    module._publish_terminal_receipt(output, data, revalidate=revalidate)

    metadata = output.lstat()
    assert output.read_bytes() == data
    assert metadata.st_mode & 0o7777 == 0o400
    assert metadata.st_nlink == 1
    assert revalidations == 2
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_completes_short_writes(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    data = b"terminal receipt with deliberately short writes\n"
    real_write = module.os.write
    real_close = module.os.close
    directory_metadata = directory.stat()
    injected_close_failure = False

    def short_write(descriptor: int, pending: object) -> int:
        return real_write(descriptor, pending[:3])

    def close_with_final_directory_failure(descriptor: int) -> None:
        nonlocal injected_close_failure
        metadata = os.fstat(descriptor)
        fail = (
            not injected_close_failure
            and stat.S_ISDIR(metadata.st_mode)
            and (metadata.st_dev, metadata.st_ino)
            == (directory_metadata.st_dev, directory_metadata.st_ino)
        )
        real_close(descriptor)
        if fail:
            injected_close_failure = True
            raise OSError("fixture close reported failure after durable publication")

    monkeypatch.setattr(module.os, "write", short_write)
    monkeypatch.setattr(module.os, "close", close_with_final_directory_failure)

    module._publish_terminal_receipt(output, data, revalidate=lambda: None)

    assert injected_close_failure
    assert output.read_bytes() == data
    assert output.stat().st_nlink == 1
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_rejects_zero_length_write_progress(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    monkeypatch.setattr(module.os, "write", lambda _descriptor, _pending: 0)

    with pytest.raises(module.ReceiptError, match="write made no progress"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


class _FixtureRenameFunction:
    def __init__(self, callback: object) -> None:
        self.callback = callback
        self.argtypes: object = None
        self.restype: object = None

    def __call__(self, *args: object) -> int:
        assert callable(self.callback)
        return self.callback(*args)


class _FixtureRenameLibrary:
    def __init__(self, module: object, callback: object) -> None:
        function = _FixtureRenameFunction(callback)
        if module.sys.platform == "darwin":
            self.renameatx_np = function
        else:
            self.renameat2 = function


def _install_fixture_rename(
    module: object, monkeypatch: pytest.MonkeyPatch, callback: object
) -> None:
    real_cdll = module.ctypes.CDLL
    calls = 0

    def fixture_cdll(*args: object, **kwargs: object) -> object:
        nonlocal calls
        calls += 1
        if calls == 1:
            return _FixtureRenameLibrary(module, callback)
        return real_cdll(*args, **kwargs)

    monkeypatch.setattr(
        module.ctypes,
        "CDLL",
        fixture_cdll,
    )


@pytest.mark.parametrize("failure_point", ["stage-fsync", "rename", "directory-fsync"])
def test_terminal_publication_failure_cleans_every_owned_name(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure_point: str,
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    real_fsync = module.os.fsync
    fsync_calls = 0

    def failing_fsync(descriptor: int) -> None:
        nonlocal fsync_calls
        fsync_calls += 1
        if (
            failure_point == "stage-fsync" and fsync_calls == 1
        ) or (
            failure_point == "directory-fsync" and fsync_calls == 2
        ):
            raise OSError("fixture fsync failure")
        real_fsync(descriptor)

    monkeypatch.setattr(module.os, "fsync", failing_fsync)
    if failure_point == "rename":
        def fail_rename(*_args: object) -> int:
            module.ctypes.set_errno(module.errno.EIO)
            return -1

        _install_fixture_rename(module, monkeypatch, fail_rename)

    with pytest.raises(module.ReceiptError, match="publication failed closed"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_rename_failure_cleans_stage(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)

    def fail_rename(*_args: object) -> int:
        module.ctypes.set_errno(module.errno.EIO)
        return -1

    _install_fixture_rename(module, monkeypatch, fail_rename)

    with pytest.raises(module.ReceiptError, match="publication failed closed"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))


def test_terminal_publication_cleans_a_rename_completed_before_reported_failure(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)

    def rename_then_fail(
        source_fd: int,
        source: bytes,
        destination_fd: int,
        destination: bytes,
        _flags: int,
    ) -> int:
        module.os.rename(
            module.os.fsdecode(source),
            module.os.fsdecode(destination),
            src_dir_fd=source_fd,
            dst_dir_fd=destination_fd,
        )
        module.ctypes.set_errno(module.errno.EIO)
        return -1

    _install_fixture_rename(module, monkeypatch, rename_then_fail)

    with pytest.raises(module.ReceiptError, match="publication failed closed"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))
    assert not list(directory.glob(".unlink-quarantine.*"))


@pytest.mark.parametrize("failure_kind", ["file", "directory"])
def test_evidence_durability_fsync_failure_fails_closed(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    failure_kind: str,
) -> None:
    module = load_writer_module()
    directory = tmp_path / "evidence"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    artifact = directory / "artifact"
    artifact.write_bytes(b"progress evidence\n")
    artifact.chmod(0o400)
    path_contract = module._capture_path_contract(
        artifact,
        "fixture evidence",
        expected_sha256=sha256(artifact),
        expected_mode=0o400,
        expected_owner=os.geteuid(),
        expected_nlink=1,
        expected_size=artifact.stat().st_size,
    )
    directory_contract = module._capture_directory_contract(
        directory, "fixture evidence directory"
    )
    real_fsync = module.os.fsync

    def fail_selected(descriptor: int) -> None:
        is_directory = stat.S_ISDIR(os.fstat(descriptor).st_mode)
        if is_directory == (failure_kind == "directory"):
            raise OSError("fixture evidence durability failure")
        real_fsync(descriptor)

    monkeypatch.setattr(module.os, "fsync", fail_selected)

    with pytest.raises(module.ReceiptError, match="fsync failed"):
        module._fsync_receipt_inputs([directory_contract, path_contract])


def test_evidence_durability_rejects_directory_inventory_drift(
    tmp_path: Path,
) -> None:
    module = load_writer_module()
    directory = tmp_path / "evidence"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    contract = module._capture_directory_contract(
        directory, "fixture evidence directory"
    )
    (directory / "late-artifact").write_bytes(b"late\n")

    with pytest.raises(module.ReceiptError, match="changed before fsync"):
        module._fsync_receipt_inputs([contract])


def test_evidence_durability_orders_files_before_bottom_up_directories(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_writer_module()
    parent = tmp_path / "evidence"
    child = parent / "nested"
    child.mkdir(parents=True, mode=0o700)
    parent.chmod(0o700)
    child.chmod(0o700)
    artifact = child / "artifact"
    artifact.write_bytes(b"progress evidence\n")
    artifact.chmod(0o400)
    contracts = [
        module._capture_directory_contract(parent, "fixture parent"),
        module._capture_path_contract(
            artifact,
            "fixture evidence",
            expected_sha256=sha256(artifact),
            expected_mode=0o400,
            expected_owner=os.geteuid(),
            expected_nlink=1,
            expected_size=artifact.stat().st_size,
        ),
        module._capture_directory_contract(child, "fixture child"),
    ]
    inode_names = {
        (contract.device, contract.inode): contract.path.name
        for contract in contracts
    }
    observed: list[str] = []
    real_fsync = module.os.fsync

    def record_fsync(descriptor: int) -> None:
        metadata = os.fstat(descriptor)
        observed.append(inode_names[(metadata.st_dev, metadata.st_ino)])
        real_fsync(descriptor)

    monkeypatch.setattr(module.os, "fsync", record_fsync)

    module._fsync_receipt_inputs(contracts)

    assert observed == ["artifact", "nested", "evidence"]


@pytest.mark.parametrize("mutation_revalidation", [1, 2])
def test_terminal_publication_revalidation_failure_cleans_terminal_names(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    mutation_revalidation: int,
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    evidence = tmp_path / "evidence.tsv"
    evidence.write_bytes(b"schema_version\t1\nresult\tpassed\n")
    receipt_data = b"terminal receipt\n"
    if mutation_revalidation == 1:
        replacement = tmp_path / "replacement.tsv"
        replacement.write_bytes(b"schema_version\t1\nresult\tforged\n")
        original_digest = sha256(evidence)
        parse_snapshot = module._tsv_fields_from_snapshot

        def replace_after_semantic_validation(
            evidence_snapshot: object, name: str
        ) -> dict[str, str]:
            fields = parse_snapshot(evidence_snapshot, name)
            os.replace(replacement, evidence)
            return fields

        monkeypatch.setattr(
            module,
            "_tsv_fields_from_snapshot",
            replace_after_semantic_validation,
        )
        evidence_snapshot, fields = module._load_tsv(
            evidence, "fixture completion"
        )
        artifact = module._artifact(evidence_snapshot)
        assert fields == {"schema_version": "1", "result": "passed"}
        assert artifact == {"path": str(evidence), "sha256": original_digest}
        assert sha256(evidence) != artifact["sha256"]
        snapshot = module._snapshot_contract(evidence_snapshot)
        receipt_data = canonical_json({"evidence": artifact})
    else:
        snapshot = module._capture_path_contract(
            evidence,
            "fixture evidence",
            expected_sha256=sha256(evidence),
        )
    calls = 0

    def revalidate() -> None:
        nonlocal calls
        calls += 1
        if mutation_revalidation == 2 and calls == mutation_revalidation:
            evidence.write_bytes(b"forged evidence\n")
        module._revalidate_receipt_inputs([snapshot])

    with pytest.raises(module.ReceiptError, match="aggregate evidence"):
        module._publish_terminal_receipt(
            output, receipt_data, revalidate=revalidate
        )

    assert not output.exists()
    assert not list(directory.glob(f".{output.name}.stage.*"))
    if mutation_revalidation == 1:
        cases = (
            (b"schema_version\t1\r\n", 1024, "canonical LF-only text"),
            (b"schema_version\t1", 1024, "canonical LF-only text"),
            (b"schema_version\t" + b"1" * 32 + b"\n", 16, "size limit"),
        )
        for index, (data, maximum_bytes, expected) in enumerate(cases):
            malformed = tmp_path / f"malformed-{index}.tsv"
            malformed.write_bytes(data)
            with pytest.raises(module.ReceiptError, match=expected):
                module._load_tsv(
                    malformed,
                    f"malformed fixture {index}",
                    maximum_bytes=maximum_bytes,
                )


@pytest.mark.parametrize("existing_kind", ["regular", "symlink", "hardlink"])
def test_terminal_publication_never_overwrites_existing_terminal_name(
    tmp_path: Path, existing_kind: str
) -> None:
    module = load_writer_module()
    directory, output = private_output(tmp_path)
    protected = directory / "protected"
    protected.write_bytes(b"protected bytes\n")
    if existing_kind == "regular":
        output.write_bytes(b"previous terminal receipt\n")
    elif existing_kind == "symlink":
        output.symlink_to(protected.name)
    elif existing_kind == "hardlink":
        os.link(protected, output)
    else:
        raise AssertionError(existing_kind)
    before = output.lstat()
    protected_bytes = protected.read_bytes()

    with pytest.raises(module.ReceiptError, match="overwrite is forbidden"):
        module._publish_terminal_receipt(
            output, b"replacement receipt\n", revalidate=lambda: None
        )

    after = output.lstat()
    assert (after.st_dev, after.st_ino) == (before.st_dev, before.st_ino)
    assert protected.read_bytes() == protected_bytes
    assert not list(directory.glob(f".{output.name}.stage.*"))


@pytest.mark.parametrize("parent_state", ["missing", "mode-0755"])
def test_terminal_publication_rejects_unsafe_output_directory(
    tmp_path: Path, parent_state: str
) -> None:
    module = load_writer_module()
    directory = tmp_path / "unsafe-output"
    if parent_state == "mode-0755":
        directory.mkdir(mode=0o700)
        directory.chmod(0o755)
    output = directory / "RELEASE_COMPLETED.json"

    with pytest.raises(module.ReceiptError, match="terminal receipt output directory"):
        module._publish_terminal_receipt(
            output, b"terminal receipt\n", revalidate=lambda: None
        )

    assert not output.exists()


@pytest.mark.parametrize("mutation", ["content", "mode", "inode", "hardlink"])
def test_aggregate_snapshot_revalidation_rejects_late_mutation(
    tmp_path: Path, mutation: str
) -> None:
    module = load_writer_module()
    evidence = tmp_path / "aggregate-evidence"
    evidence.write_bytes(b"original evidence\n")
    snapshot = module._capture_path_contract(
        evidence,
        "fixture evidence",
        expected_sha256=sha256(evidence),
    )
    if mutation == "content":
        evidence.write_bytes(b"mutated evidence!\n")
    elif mutation == "mode":
        evidence.chmod(0o400)
    elif mutation == "inode":
        evidence.unlink()
        evidence.write_bytes(b"original evidence\n")
    elif mutation == "hardlink":
        os.link(evidence, tmp_path / "aggregate-evidence-alias")
    else:
        raise AssertionError(mutation)

    with pytest.raises(module.ReceiptError, match="aggregate evidence"):
        module._revalidate_receipt_inputs([snapshot])


@pytest.mark.parametrize("mutation", ["mode", "entry", "inode"])
def test_aggregate_directory_revalidation_rejects_late_mutation(
    tmp_path: Path, mutation: str
) -> None:
    module = load_writer_module()
    directory = tmp_path / "aggregate-directory"
    directory.mkdir(mode=0o700)
    directory.chmod(0o700)
    snapshot = module._capture_directory_contract(
        directory, "fixture evidence directory"
    )
    if mutation == "mode":
        directory.chmod(0o755)
    elif mutation == "entry":
        (directory / "late-entry").write_bytes(b"late evidence\n")
    elif mutation == "inode":
        directory.rmdir()
        directory.mkdir(mode=0o700)
        directory.chmod(0o700)
    else:
        raise AssertionError(mutation)

    with pytest.raises(module.ReceiptError, match="aggregate evidence directory"):
        module._revalidate_receipt_inputs([snapshot])
