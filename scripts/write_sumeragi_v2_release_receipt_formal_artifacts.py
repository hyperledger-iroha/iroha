# Executed lexically in write_sumeragi_v2_release_receipt.py; do not import directly.

def _validate_multilane_apalache_evidence(
    snapshot: EvidenceSnapshot,
    workspace_source_manifest: str,
    multilane_source_manifest: str,
) -> None:
    data = snapshot.data
    if (
        len(data) > _MAX_SCALING_JSON_BYTES
        or not data.endswith(b"\n")
        or b"\r" in data
        or b"\0" in data
    ):
        raise ReceiptError(
            "formal multilane Apalache evidence is not bounded LF-only TSV"
        )
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError as error:
        raise ReceiptError(
            "formal multilane Apalache evidence is not UTF-8"
        ) from error
    expected_header = [
        "schema_version\t2",
        "backend\tapalache",
        f"version\t{_APALACHE_VERSION}",
        f"launcher_sha256\t{_APALACHE_LAUNCHER_SHA256}",
        f"jar_sha256\t{_APALACHE_JAR_SHA256}",
        f"workspace_source_manifest_sha256\t{workspace_source_manifest}",
        f"multilane_source_manifest_sha256\t{multilane_source_manifest}",
        f"result_count\t{len(_APALACHE_RESULTS)}",
    ]
    if len(lines) != len(expected_header) + len(_APALACHE_RESULTS):
        raise ReceiptError(
            "formal multilane Apalache evidence has the wrong result inventory"
        )
    if lines[: len(expected_header)] != expected_header:
        raise ReceiptError(
            "formal multilane Apalache evidence header is not the exact pinned profile"
        )
    for index, expected in enumerate(_APALACHE_RESULTS):
        fields = lines[len(expected_header) + index].split("\t")
        expected_prefix = ("result", *expected, "NoError")
        if (
            len(fields) != 9
            or tuple(fields[:6]) != expected_prefix
            or any(_DIGEST_RE.fullmatch(value) is None for value in fields[6:])
        ):
            raise ReceiptError(
                "formal multilane Apalache evidence result "
                f"{index} is not exact source-bound NoError evidence"
            )


def _validate_formal_snapshot_replays(
    *,
    snapshots: dict[str, EvidenceSnapshot],
    checker: Path,
    checker_environment: dict[str, str],
    repo_root: Path,
) -> None:
    """Run retained formal validators over private copies of captured bytes."""

    replay_keys = (
        "ledger",
        "evidence",
        "verus_evidence",
        "verus_log",
        "cross_tool_evidence",
        "production_trace_extraction_evidence",
    )
    with tempfile.TemporaryDirectory(
        prefix="sumeragi-v2-formal-snapshot-replay-"
    ) as temporary:
        replay_root = Path(temporary).resolve(strict=True)
        replay_paths: dict[str, Path] = {}
        for key in replay_keys:
            snapshot = snapshots[key]
            destination = replay_root / snapshot.path.name
            try:
                destination.write_bytes(snapshot.data)
                destination.chmod(0o400)
            except OSError as error:
                raise ReceiptError(
                    "formal snapshot replay could not materialize captured evidence"
                ) from error
            replay_paths[key] = destination

        cross_tool_status, cross_tool_stdout, _ = _run_bounded_python_validator(
            checker,
            [
                "--ledger",
                str(replay_paths["ledger"]),
                "--print-cross-tool-obligations",
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived formal cross-tool validator",
        )
        if cross_tool_status != 0:
            raise ReceiptError(
                "archived formal ledger has an invalid cross-tool evidence requirement"
            )
        if not cross_tool_stdout.strip():
            raise ReceiptError(
                "archived formal release ledger does not require cross-tool evidence"
            )

        verus_checker = (
            repo_root / "scripts" / "formal" / "sumeragi_v2_verus_evidence.py"
        )
        verus_status, _, _ = _run_bounded_python_validator(
            verus_checker,
            [
                "validate",
                "--root",
                str(repo_root),
                "--evidence",
                str(replay_paths["verus_evidence"]),
                "--log",
                str(replay_paths["verus_log"]),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived formal Verus validator",
        )
        if verus_status != 0:
            raise ReceiptError("archived formal Verus evidence failed validation")

        status, _, _ = _run_bounded_python_validator(
            checker,
            [
                "--ledger",
                str(replay_paths["ledger"]),
                "--release",
                "--evidence",
                str(replay_paths["evidence"]),
                "--verus-evidence",
                str(replay_paths["verus_evidence"]),
                "--verus-log",
                str(replay_paths["verus_log"]),
                "--cross-tool-evidence",
                str(replay_paths["cross_tool_evidence"]),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="archived formal release validator",
        )
        if status != 0:
            raise ReceiptError(
                "archived formal ledger/evidence failed release validation"
            )

        # Recompute the canonical theorem certificate from the sealed checkout
        # and the private artifact snapshots. A stored boolean or certificate
        # digest alone is never release authority.
        trace_status, _, _ = _run_bounded_python_validator(
            checker,
            [
                "--ledger",
                str(replay_paths["ledger"]),
                "--release",
                "--evidence",
                str(replay_paths["evidence"]),
                "--verus-evidence",
                str(replay_paths["verus_evidence"]),
                "--verus-log",
                str(replay_paths["verus_log"]),
                "--cross-tool-evidence",
                str(replay_paths["cross_tool_evidence"]),
                "--production-trace-extraction-evidence",
                str(replay_paths["production_trace_extraction_evidence"]),
            ],
            cwd=repo_root,
            environment=checker_environment,
            name="authenticated production trace-extraction validator",
        )
        if trace_status != 0:
            raise ReceiptError(
                "archived formal evidence does not authenticate production "
                "trace extraction"
            )


def _formal_artifacts(
    completion: EvidenceSnapshot,
    fields: dict[str, str],
    sealed: dict[str, Any],
    checker_environment: dict[str, str],
    repo_root: Path,
    bootstrap_runner_tools: dict[str, Any],
    corridor_fields: dict[str, str],
    private_build_roots_available: bool,
) -> tuple[
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
    PathContract,
]:
    completion_path = completion.path
    checker = repo_root / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
    expected_completion_fields = {
        "schema_version",
        "head_commit",
        "head_tree",
        "source_manifest_sha256",
        "cargo_lock_sha256",
        "formal_gate_log_sha256",
        "proof_coverage_sha256",
        "proof_evidence_sha256",
        "verus_evidence_sha256",
        "verus_log_sha256",
        "multilane_apalache_evidence_sha256",
        "cross_tool_evidence_sha256",
        "production_trace_extraction_evidence_sha256",
        "harness_cargo_lock_sha256",
        "formal_toolchain_sha256",
        "tlaps_resource_jsonl_sha256",
        "tlaps_resource_summary_sha256",
    }
    if fields.get("schema_version") != "2":
        raise ReceiptError(
            "formal completion is release-ineligible without authenticated "
            "production trace-extraction evidence"
        )
    _require_fields(
        fields,
        expected_completion_fields,
        "formal completion",
    )
    expected = {
        "schema_version": "2",
        "head_commit": sealed["head_commit"],
        "head_tree": sealed["head_tree"],
        "source_manifest_sha256": sealed["workspace_source_manifest_sha256"],
        "cargo_lock_sha256": sealed["cargo_lock_sha256"],
    }
    if any(fields.get(name) != value for name, value in expected.items()):
        raise ReceiptError("formal completion is not bound to the release identity")

    artifact_specs = (
        (
            "gate_log",
            completion_path.with_name("formal-gate.log"),
            "formal_gate_log_sha256",
            "formal gate log",
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "ledger",
            completion_path.with_name("proof_coverage.json"),
            "proof_coverage_sha256",
            "formal proof ledger",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "evidence",
            completion_path.with_name("proof_evidence.json"),
            "proof_evidence_sha256",
            "formal proof evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "verus_evidence",
            completion_path.with_name("verus_evidence.json"),
            "verus_evidence_sha256",
            "formal Verus evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "verus_log",
            completion_path.with_name("verus.log"),
            "verus_log_sha256",
            "formal Verus log",
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "multilane_apalache_evidence",
            completion_path.with_name("multilane_apalache_evidence.tsv"),
            "multilane_apalache_evidence_sha256",
            "formal multilane Apalache evidence",
            _MAX_RELEASE_TSV_BYTES,
        ),
        (
            "cross_tool_evidence",
            completion_path.with_name("cross_tool_evidence.json"),
            "cross_tool_evidence_sha256",
            "formal cross-tool evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "production_trace_extraction_evidence",
            completion_path.with_name(
                "production_trace_extraction_evidence.json"
            ),
            "production_trace_extraction_evidence_sha256",
            "formal production trace-extraction evidence",
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "harness_lock",
            completion_path.with_name("harness-Cargo.lock"),
            "harness_cargo_lock_sha256",
            "formal harness lock",
            _MAX_LOCK_BYTES,
        ),
        (
            "toolchain",
            completion_path.with_name("formal-toolchain.tsv"),
            "formal_toolchain_sha256",
            "formal toolchain",
            _MAX_RELEASE_TSV_BYTES,
        ),
        (
            "tlaps_resource_jsonl",
            completion_path.with_name("tlaps_resource.jsonl"),
            "tlaps_resource_jsonl_sha256",
            "TLAPS resource samples",
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "tlaps_resource_summary",
            completion_path.with_name("tlaps_resource_summary.json"),
            "tlaps_resource_summary_sha256",
            "TLAPS resource summary",
            _MAX_RELEASE_JSON_BYTES,
        ),
    )
    snapshots: dict[str, EvidenceSnapshot] = {}
    for key, path, digest_field, name, maximum_bytes in artifact_specs:
        snapshot = _bounded_evidence_snapshot(
            path,
            name,
            maximum_bytes=maximum_bytes,
        )
        if snapshot.sha256 != fields[digest_field]:
            raise ReceiptError(f"{name} digest mismatch")
        snapshots[key] = snapshot
    gate_log = snapshots["gate_log"]
    ledger = snapshots["ledger"]
    evidence = snapshots["evidence"]
    verus_evidence = snapshots["verus_evidence"]
    verus_log = snapshots["verus_log"]
    multilane_apalache_evidence = snapshots["multilane_apalache_evidence"]
    cross_tool_evidence = snapshots["cross_tool_evidence"]
    production_trace_extraction_evidence = snapshots[
        "production_trace_extraction_evidence"
    ]
    harness_lock = snapshots["harness_lock"]
    toolchain_snapshot = snapshots["toolchain"]
    tlaps_resource_jsonl = snapshots["tlaps_resource_jsonl"]
    tlaps_resource_summary = snapshots["tlaps_resource_summary"]
    _validate_formal_snapshot_replays(
        snapshots=snapshots,
        checker=checker,
        checker_environment=checker_environment,
        repo_root=repo_root,
    )
    trace = _decode_canonical_json(
        production_trace_extraction_evidence.data,
        "production trace-extraction evidence",
    )
    if trace.get("workspace_source_manifest_sha256") != sealed[
        "workspace_source_manifest_sha256"
    ]:
        raise ReceiptError(
            "production trace extraction is not bound to the sealed workspace"
        )
    multilane_source_manifest = trace.get("multilane_source_manifest_sha256")
    if (
        not isinstance(multilane_source_manifest, str)
        or _DIGEST_RE.fullmatch(multilane_source_manifest) is None
    ):
        raise ReceiptError(
            "production trace extraction lacks its multilane source manifest"
        )
    _validate_multilane_apalache_evidence(
        multilane_apalache_evidence,
        sealed["workspace_source_manifest_sha256"],
        multilane_source_manifest,
    )
    _validate_tlaps_resource_evidence(
        tlaps_resource_jsonl, tlaps_resource_summary
    )
    if fields["harness_cargo_lock_sha256"] != _HARNESS_LOCK_SHA256:
        raise ReceiptError("formal harness lock is not the pinned dependency graph")
    toolchain = _tsv_fields_from_snapshot(
        toolchain_snapshot, "formal toolchain"
    )
    _require_fields(
        toolchain,
        {
            "schema_version",
            "java_path",
            "java_sha256",
            "tlapm_path",
            "tlapm_sha256",
            "tla2tools_path",
            "tla2tools_sha256",
            "verus_path",
            "verus_sha256",
            "cargo_verus_path",
            "cargo_verus_sha256",
            "tlc_profile",
            "tlaps_threads",
        },
        "formal toolchain",
    )
    if (
        toolchain["schema_version"] != "1"
        or toolchain["tlc_profile"] != "ci"
        or toolchain["tlaps_threads"] != "1"
    ):
        raise ReceiptError("formal toolchain does not describe the pinned release profile")
    runtime_root = repo_root.parent / "runtime"
    expected_tool_paths = {
        "java": runtime_root / "java-runtime" / "bin" / "java",
        "tlapm": runtime_root / "tlapm-distribution" / "bin" / "tlapm",
        "tla2tools": runtime_root / "tla2tools.jar",
        "verus": runtime_root / "verus-distribution" / "verus",
        "cargo_verus": runtime_root / "verus-distribution" / "cargo-verus",
    }
    runtime_inventory = _bounded_evidence_snapshot(
        Path(corridor_fields["runtime_inventory_path"]),
        "formal private runtime inventory",
        maximum_bytes=_MAX_CARGO_CACHE_INPUT_INVENTORY_BYTES,
        expected_mode=0o400,
        allowed_owners={os.geteuid()},
        require_single_link=True,
    )
    if runtime_inventory.sha256 != _require_digest(
        corridor_fields["runtime_inventory_sha256"],
        "formal private runtime inventory digest",
    ):
        raise ReceiptError("formal private runtime inventory binding is not exact")
    runtime_document = _decode_canonical_json(
        runtime_inventory.data, "formal private runtime inventory"
    )
    runtime_records = (
        runtime_document.get("records")
        if isinstance(runtime_document, dict)
        else None
    )
    tla2tools_records = (
        [
            record
            for record in runtime_records
            if isinstance(record, dict)
            and record.get("path") == "tla2tools.jar"
            and record.get("kind") == "file"
        ]
        if isinstance(runtime_records, list)
        else []
    )
    if len(tla2tools_records) != 1:
        raise ReceiptError("formal TLA2Tools runtime record is not exact")
    expected_tool_digests = {
        "tla2tools": _require_digest(
            tla2tools_records[0].get("sha256"),
            "formal TLA2Tools runtime digest",
        ),
    }
    for tool, runner_name in (
        ("java", "java"),
        ("tlapm", "tlapm"),
        ("verus", "verus"),
        ("cargo_verus", "cargo-verus"),
    ):
        runner_record = bootstrap_runner_tools.get(runner_name)
        if (
            not isinstance(runner_record, dict)
            or runner_record.get("archive_name")
            != f"runner-tools/{runner_name}"
            or runner_record.get("alias_name") != runner_name
        ):
            raise ReceiptError(
                f"formal {tool} is not an authenticated bootstrap runner tool"
            )
        expected_tool_digests[tool] = runner_record.get("sha256")
    for tool in ("java", "tlapm", "tla2tools", "verus", "cargo_verus"):
        raw_path = Path(toolchain[f"{tool}_path"])
        digest = toolchain[f"{tool}_sha256"]
        if (
            not raw_path.is_absolute()
            or raw_path != expected_tool_paths[tool]
            or not _DIGEST_RE.fullmatch(digest)
            or digest != expected_tool_digests[tool]
        ):
            raise ReceiptError(f"formal {tool} tool binding is not exact")
        if not private_build_roots_available:
            continue
        tool_snapshot = _bounded_evidence_snapshot(
            raw_path,
            f"formal {tool} tool",
            maximum_bytes=_MAX_TOOL_BYTES,
            require_single_link=False,
        )
        if tool_snapshot.sha256 != digest:
            raise ReceiptError(f"formal {tool} tool digest mismatch")
    log_lines = _decode_lf_text(gate_log, "formal gate log").splitlines()
    if (
        not log_lines
        or log_lines[-1] != _FORMAL_FINAL_MARKER
        or log_lines.count(_FORMAL_FINAL_MARKER) != 1
    ):
        raise ReceiptError("formal gate log lacks its one exact final success marker")
    return (
        _snapshot_contract(gate_log),
        _snapshot_contract(ledger),
        _snapshot_contract(evidence),
        _snapshot_contract(verus_evidence),
        _snapshot_contract(verus_log),
        _snapshot_contract(multilane_apalache_evidence),
        _snapshot_contract(cross_tool_evidence),
        _snapshot_contract(production_trace_extraction_evidence),
        _snapshot_contract(harness_lock),
        _snapshot_contract(toolchain_snapshot),
        _snapshot_contract(tlaps_resource_jsonl),
        _snapshot_contract(tlaps_resource_summary),
    )


def _formal_replay_release(
    *,
    source_receipt_path: Path,
    release_root_path: Path,
    expected_signature_sha256: str,
    expected_ssh_keygen_sha256: str,
    expected_allowed_signers_sha256: str,
    expected_revocation_sha256: str,
    principal: str,
    expected_signer_fingerprint: str,
    checker_environment: dict[str, str],
    repo_root: Path,
) -> dict[str, Any]:
    """Verify and inventory the detached-SSH formal replay release bundle."""

    expected_signature_sha256 = _require_digest(
        expected_signature_sha256,
        "expected formal replay signature digest",
    )
    verifier = (
        repo_root
        / "scripts"
        / "formal"
        / "verify_sumeragi_v2_replay_release.py"
    )
    try:
        if release_root_path.resolve(strict=True) != release_root_path:
            raise ReceiptError("formal replay release root is not canonical")
        root_metadata = release_root_path.lstat()
        names = {entry.name for entry in os.scandir(release_root_path)}
    except OSError as error:
        raise ReceiptError("formal replay release root is unavailable") from error
    expected_names = {
        "receipt.json",
        "receipt.json.sig",
        "ssh-keygen.release-tool",
        "allowed_signers",
        "revocation.krl",
        "release-attestation.json",
        "tlapm-projection",
    }
    if (
        not stat.S_ISDIR(root_metadata.st_mode)
        or stat.S_IMODE(root_metadata.st_mode) != 0o700
        or root_metadata.st_uid != os.geteuid()
        or names != expected_names
    ):
        raise ReceiptError("formal replay release inventory is not exact")
    release_root_contract = _capture_directory_contract(
        release_root_path,
        "formal replay release root",
    )
    projection_root = release_root_path / "tlapm-projection"
    try:
        projection_metadata = projection_root.lstat()
        projection_names = {
            entry.name for entry in os.scandir(projection_root)
        }
    except OSError as error:
        raise ReceiptError(
            "formal replay TLAPM projection is unavailable"
        ) from error
    if (
        not stat.S_ISDIR(projection_metadata.st_mode)
        or stat.S_IMODE(projection_metadata.st_mode) != 0o555
        or projection_metadata.st_uid != os.geteuid()
        or projection_names != {"Folds.tla", "Functions.tla"}
    ):
        raise ReceiptError(
            "formal replay TLAPM projection inventory is not exact"
        )
    projection_root_contract = _capture_directory_contract(
        projection_root,
        "formal replay TLAPM projection",
    )

    specs = (
        (
            "source_receipt",
            source_receipt_path,
            0o600,
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "receipt",
            release_root_path / "receipt.json",
            0o400,
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "signature",
            release_root_path / "receipt.json.sig",
            0o400,
            _MAX_POLICY_BYTES,
        ),
        (
            "ssh_keygen",
            release_root_path / "ssh-keygen.release-tool",
            0o500,
            _MAX_TOOL_BYTES,
        ),
        (
            "allowed_signers",
            release_root_path / "allowed_signers",
            0o400,
            _MAX_POLICY_BYTES,
        ),
        (
            "revocation",
            release_root_path / "revocation.krl",
            0o400,
            _MAX_POLICY_BYTES,
        ),
        (
            "attestation",
            release_root_path / "release-attestation.json",
            0o400,
            _MAX_RELEASE_JSON_BYTES,
        ),
        (
            "tlapm_folds",
            projection_root / "Folds.tla",
            0o444,
            _MAX_RELEASE_TEXT_BYTES,
        ),
        (
            "tlapm_functions",
            projection_root / "Functions.tla",
            0o444,
            _MAX_RELEASE_TEXT_BYTES,
        ),
    )
    snapshots: dict[str, EvidenceSnapshot] = {}
    for label, path, mode, maximum in specs:
        snapshot = _bounded_evidence_snapshot(
            path,
            f"formal replay release {label}",
            maximum_bytes=maximum,
        )
        if (
            snapshot.mode != mode
            or snapshot.owner != os.geteuid()
            or snapshot.nlink != 1
        ):
            raise ReceiptError(
                f"formal replay release {label} metadata is not exact"
            )
        snapshots[label] = snapshot
    if snapshots["signature"].sha256 != expected_signature_sha256:
        raise ReceiptError("formal replay signature digest changed after verification")
    for label, expected in (
        ("ssh_keygen", expected_ssh_keygen_sha256),
        ("allowed_signers", expected_allowed_signers_sha256),
        ("revocation", expected_revocation_sha256),
    ):
        if snapshots[label].sha256 != expected:
            raise ReceiptError(
                f"formal replay release {label} differs from protected policy"
            )

    if snapshots["source_receipt"].data != snapshots["receipt"].data:
        raise ReceiptError(
            "formal replay source and finalized receipt bytes differ"
        )
    try:
        source_value = json.loads(snapshots["source_receipt"].data.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ReceiptError("formal replay source receipt is malformed") from error
    if not isinstance(source_value, dict):
        raise ReceiptError("formal replay source receipt is not an object")
    source_inventory = source_value.get("artifact_inventory")
    if not isinstance(source_inventory, list):
        raise ReceiptError("formal replay source artifact inventory is absent")
    source_root = source_receipt_path.parent
    source_root_contract = _capture_directory_contract(
        source_root,
        "formal replay source root",
    )
    source_artifacts: list[EvidenceSnapshot] = []
    source_names: list[str] = []
    for index, record in enumerate(source_inventory):
        if not isinstance(record, dict):
            raise ReceiptError(
                f"formal replay source artifact {index} is malformed"
            )
        relative = record.get("path")
        relative_path = PurePosixPath(relative) if isinstance(relative, str) else None
        if (
            relative_path is None
            or relative_path.is_absolute()
            or not relative_path.parts
            or any(part in {"", ".", ".."} for part in relative_path.parts)
        ):
            raise ReceiptError(
                f"formal replay source artifact {index} path is unsafe"
            )
        source_names.append(relative)
        artifact = _bounded_evidence_snapshot(
            source_root.joinpath(*relative_path.parts),
            f"formal replay source artifact {index}",
            maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
        )
        if (
            record.get("sha256") != artifact.sha256
            or record.get("size_bytes") != artifact.size
            or record.get("mode") != artifact.mode
            or record.get("nlink") != artifact.nlink
            or artifact.owner != os.geteuid()
            or artifact.nlink != 1
        ):
            raise ReceiptError(
                f"formal replay source artifact {index} metadata differs"
            )
        source_artifacts.append(artifact)
    if source_names != sorted(set(source_names)):
        raise ReceiptError(
            "formal replay source artifact paths are not sorted and unique"
        )

    verifier_dependencies = tuple(
        _bounded_evidence_snapshot(
            repo_root / "scripts" / "formal" / name,
            f"signed formal replay release verifier dependency {name}",
            maximum_bytes=_MAX_HELPER_BYTES,
            allowed_owners={os.geteuid()},
        )
        for name in (
            "check_sumeragi_v2_replay_receipt.py",
            "sumeragi_v2_replay_signing.py",
        )
    )
    status, stdout, stderr = _run_bounded_python_validator(
        verifier,
        [
            str(source_receipt_path),
            "--release-root",
            str(release_root_path),
            "--expected-signature-sha256",
            expected_signature_sha256,
            "--expected-ssh-keygen-sha256",
            expected_ssh_keygen_sha256,
            "--expected-allowed-signers-sha256",
            expected_allowed_signers_sha256,
            "--expected-revocation-sha256",
            expected_revocation_sha256,
            "--principal",
            principal,
            "--expected-signer-fingerprint",
            expected_signer_fingerprint,
        ],
        cwd=repo_root,
        environment=checker_environment,
        name="signed formal replay release verifier",
        watched_contracts=(
            *snapshots.values(),
            *source_artifacts,
            *verifier_dependencies,
        ),
    )
    expected_stdout = (
        "verified finalized Sumeragi V2 replay release for "
        f"{expected_signer_fingerprint}\n"
    ).encode("utf-8")
    if status != 0 or stdout != expected_stdout or stderr:
        raise ReceiptError(
            "signed formal replay release failed independent verification"
        )
    if (
        _capture_directory_contract(
            release_root_path,
            "formal replay release root after verification",
        )
        != release_root_contract
        or _capture_directory_contract(
            source_root,
            "formal replay source root after verification",
        )
        != source_root_contract
        or _capture_directory_contract(
            projection_root,
            "formal replay TLAPM projection after verification",
        )
        != projection_root_contract
    ):
        raise ReceiptError(
            "formal replay release directories changed during verification"
        )
    for label, path, _mode, maximum in specs:
        current = _bounded_evidence_snapshot(
            path,
            f"formal replay release {label} after verification",
            maximum_bytes=maximum,
        )
        if current != snapshots[label]:
            raise ReceiptError(
                f"formal replay release {label} changed during verification"
            )
    for index, expected in enumerate(source_artifacts):
        current = _bounded_evidence_snapshot(
            expected.path,
            f"formal replay source artifact {index} after verification",
            maximum_bytes=_MAX_RELEASE_TEXT_BYTES,
        )
        if current != expected:
            raise ReceiptError(
                f"formal replay source artifact {index} changed during verification"
            )

    def full_record(snapshot: EvidenceSnapshot) -> dict[str, Any]:
        return {
            "path": str(snapshot.path),
            "sha256": snapshot.sha256,
            "size_bytes": snapshot.size,
            "mode": f"{snapshot.mode:04o}",
            "owner_uid": snapshot.owner,
            "nlink": snapshot.nlink,
        }

    return {
        "schema_version": 1,
        "scheme": "detached-ssh",
        "provider": "openssh-sshsig",
        "namespace": "iroha-sumeragi-v2-replay-receipt-v1",
        "principal": principal,
        "signer_fingerprint": expected_signer_fingerprint,
        "source_artifacts": [
            full_record(snapshot) for snapshot in source_artifacts
        ],
        **{
            label: full_record(snapshots[label])
            for label, _path, _mode, _maximum in specs
        },
    }
